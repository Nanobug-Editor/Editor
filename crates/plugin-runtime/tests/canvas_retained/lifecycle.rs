//! Actual timeline owners preserve logical snapshots while native frame leases expire at retirement.
use super::*;
use plugin_runtime::{CanvasFrameResource, InstallControl, InstallStage};
use serde_json::{Value, json};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

/// Read the same explicit candidate as the parent suite; frozen fixture archives remain immutable.
fn package() -> Package {
    std::env::var_os("NANOBUG_CANVAS_TIMELINE_PACKAGE").map_or_else(
        || test_packages::package("canvas-timeline-1.0.0"),
        |path| Package::read(std::path::Path::new(&path)).unwrap(),
    )
}

/// Change only ordinary manifest version metadata, retaining the independently built WASM component.
fn update_package() -> Package {
    let mut files = package().files;
    let mut manifest: Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    manifest["version"] = json!("1.0.1");
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    Package::from_files(files).unwrap()
}

/// Re-enable stays cold until an ordinary public view demand activates the replacement instance.
fn activate(manager: &mut Manager, plugin: &str, panel: &str) {
    let availability = manager.availability(plugin).unwrap();
    let demand = manager
        .begin_demand(
            &availability,
            plugin_runtime::DemandReason::View(panel.into()),
            60_000,
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(90);
    loop {
        manager.poll();
        if let api::RequestUpdate::Completed { result } = demand.status() {
            result.unwrap();
            return;
        }
        assert!(Instant::now() < deadline, "{plugin} activation timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Seed actual sparse edits and a nonzero viewport before any native request or lifecycle operation.
fn seed(manager: &mut Manager) {
    manager
        .invoke_command("canvas-timeline", "mutate", json!({"index":0}))
        .unwrap();
    manager
        .invoke_command("canvas-timeline", "set-offset", json!({"x":1200,"y":2400}))
        .unwrap();
}

/// Obtain a runtime-issued one-shot lease through its ordinary public native-adapter handoff.
fn pending(manager: &mut Manager) -> Arc<CanvasFrameResource> {
    manager
        .invoke_command("canvas-timeline", "animate", Value::Null)
        .unwrap();
    let mut requests = manager.take_canvas_frame_requests();
    assert_eq!(requests.len(), 1);
    requests.pop().unwrap().1
}

/// A retired native reference cannot complete again even when its adapter still holds the Arc.
fn revoked(lease: &CanvasFrameResource) {
    assert!(!lease.is_pending());
    lease.set_visible(true);
    let failure = lease
        .present(
            plugin_runtime::plugin_protocol::Rect {
                x: 0.,
                y: 0.,
                w: 640.,
                h: 360.,
            },
            16.7,
        )
        .unwrap_err();
    assert_eq!(failure.code, api::ErrorCode::InvalidState);
}

/// Sparse data, viewport and retained scene generation are logical state; native playback is not.
fn same_logical_state(before: &Value, after: &Value) {
    for field in [
        "edits",
        "seed",
        "offset",
        "revision",
        "scene_revision",
        "identity",
    ] {
        assert_eq!(
            before[field], after[field],
            "logical snapshot field {field} changed"
        );
    }
    assert_eq!(
        after["animating"], false,
        "native playback must await a fresh request"
    );
}

/// Disable saves logical content, and both disabled and finally dropped owners seal old frame Arcs.
#[test]
#[ignore = "executes actual timeline Package through public Manager disable, demand and owner drop"]
fn retained_disable_restores_logical_scene_without_reusing_native_frame_lease() {
    let mut harness = Harness::open();
    seed(&mut harness.manager);
    let before = timeline_snapshot(&mut harness.manager);
    let original = harness
        .manager
        .instance_id("canvas-timeline")
        .unwrap()
        .to_owned();
    let lease = pending(&mut harness.manager);
    harness.manager.disable("canvas-timeline").unwrap();
    revoked(&lease);
    assert!(harness.manager.take_canvas_frame_requests().is_empty());
    harness.manager.enable("canvas-timeline").unwrap();
    activate(&mut harness.manager, "canvas-timeline", "timeline");
    assert_ne!(
        harness.manager.instance_id("canvas-timeline"),
        Some(original.as_str())
    );
    same_logical_state(&before, &timeline_snapshot(&mut harness.manager));
    assert!(harness.manager.take_canvas_frame_requests().is_empty());
    let replacement = pending(&mut harness.manager);
    assert!(!Arc::ptr_eq(&lease, &replacement));
    drop(harness);
    revoked(&replacement);
}

/// Failed real candidate preparation preserves the accepted view and old native owner; a later
/// cutover snapshots edits made after preparation and revokes every old frame reference.
#[test]
#[ignore = "executes actual timeline Package preparation cancellation and ordinary update cutover"]
fn retained_update_keeps_failed_preparation_owner_and_retires_it_on_success() {
    let mut harness = Harness::open();
    let manager = &mut harness.manager;
    seed(manager);
    let lease = pending(manager);
    let owner = manager.instance_id("canvas-timeline").unwrap().to_owned();
    let view = manager.live["canvas-timeline"].views["timeline"].clone();
    let snapshot = timeline_snapshot(manager);
    let candidate = update_package();
    let slot = Arc::new(Mutex::new(None::<InstallControl>));
    let cancelling = slot.clone();
    let reached = Arc::new(AtomicBool::new(false));
    let progress = reached.clone();
    let cancelled = InstallControl::new(move |stage| {
        if stage == InstallStage::Prepared {
            progress.store(true, Ordering::Release);
            cancelling.lock().unwrap().as_ref().unwrap().cancel();
        }
    });
    *slot.lock().unwrap() = Some(cancelled.clone());
    let error = manager
        .prepare_installation(
            &candidate,
            candidate.manifest.permissions.clone(),
            &cancelled,
        )
        .err()
        .expect("cancelled preparation must fail");
    slot.lock().unwrap().take();
    assert!(
        reached.load(Ordering::Acquire),
        "failure must occur inside candidate preparation"
    );
    assert!(format!("{error:#}").contains("cancelled"));
    assert_eq!(manager.instance_id("canvas-timeline"), Some(owner.as_str()));
    assert!(Arc::ptr_eq(
        &view,
        &manager.live["canvas-timeline"].views["timeline"]
    ));
    assert_eq!(snapshot, timeline_snapshot(manager));
    assert!(
        lease.is_pending(),
        "failed preparation cannot withdraw the working native lease"
    );

    let control = InstallControl::default();
    let prepared = manager
        .prepare_installation(&candidate, candidate.manifest.permissions.clone(), &control)
        .unwrap();
    assert!(lease.is_pending());
    // Return to an actually visible event and edit after candidate preparation, before snapshot cutover.
    manager
        .invoke_command("canvas-timeline", "set-offset", json!({"x":0,"y":0}))
        .unwrap();
    manager
        .invoke_command("canvas-timeline", "mutate", json!({"index":1}))
        .unwrap();
    let latest = timeline_snapshot(manager);
    manager.commit_installation(prepared, &control).unwrap();
    revoked(&lease);
    assert_ne!(manager.instance_id("canvas-timeline"), Some(owner.as_str()));
    same_logical_state(&latest, &timeline_snapshot(manager));
    assert!(manager.take_canvas_frame_requests().is_empty());
    let replacement = pending(manager);
    assert!(!Arc::ptr_eq(&lease, &replacement));
    assert!(replacement.request().request > lease.request().request);
}

/// Independent D guests share one ordinary Manager and reuse node names without sharing owners.
#[test]
#[ignore = "set Flow/Table/Timeline D candidate overrides; executes all three actual independent Packages"]
fn retained_three_consumers_keep_frame_patch_and_clipboard_owners_isolated() {
    use plugin_runtime::plugin_protocol::transfer::ClipboardValue;
    let mut harness = Harness::open();
    let manager = &mut harness.manager;
    for (variable, plugin, panel) in [
        ("NANOBUG_CANVAS_EDITABLE_PACKAGE", "canvas-flow", "diagram"),
        ("NANOBUG_CANVAS_TABLE_PACKAGE", "canvas-table", "table"),
    ] {
        let path =
            std::env::var_os(variable).expect("set the explicit independent D Package override");
        let package = Package::read(std::path::Path::new(&path)).unwrap();
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
        activate(manager, plugin, panel);
    }
    let timeline = pending(manager);
    let flow = manager.live["canvas-flow"].views["diagram"].clone();
    let ui::Kind::Canvas(canvas) = &flow.active_node("drawing").unwrap().kind else {
        panic!("missing Flow Canvas")
    };
    let request = api::Output {
        canvas_frames: vec![ui::CanvasFrameRequest {
            panel: "diagram".into(),
            canvas: "drawing".into(),
            identity: canvas.scene.as_ref().unwrap().identity,
            request: 1,
        }],
        ..Default::default()
    };
    manager
        .invoke_command(
            "canvas-flow",
            "publication-probe",
            serde_json::to_value(request).unwrap(),
        )
        .unwrap();
    let frames = manager.take_canvas_frame_requests();
    assert_eq!(
        frames.len(),
        2,
        "same node names must not collapse independent owners"
    );
    let flow_frame = frames
        .iter()
        .find(|(plugin, _)| plugin == "canvas-flow")
        .unwrap()
        .1
        .clone();
    assert!(Arc::ptr_eq(
        &timeline,
        &frames
            .iter()
            .find(|(plugin, _)| plugin == "canvas-timeline")
            .unwrap()
            .1
    ));

    let table = manager.live["canvas-table"].views["table"].clone();
    manager
        .event(
            "canvas-table",
            Some("table".into()),
            api::Notification::Ui(ui::UiEvent {
                revision: table.revision,
                node: "copy".into(),
                action: ui::Action::Click,
            }),
        )
        .unwrap();
    let (owner, clipboard) = manager
        .take_editor_requests()
        .pop()
        .expect("Table owns its clipboard operation");
    assert_eq!(owner, "canvas-table");
    assert!(clipboard.begin());
    clipboard.finish(Ok(api::EditorValue::ClipboardFormats(
        ClipboardValue::Written,
    )));
    manager.poll();
    let table_after = manager.live["canvas-table"].views["table"].clone();
    manager
        .event(
            "canvas-flow",
            Some("diagram".into()),
            api::Notification::Ui(ui::UiEvent {
                revision: flow.revision,
                node: "pan".into(),
                action: ui::Action::Click,
            }),
        )
        .unwrap();
    manager
        .invoke_command("canvas-timeline", "mutate", json!({"index":0}))
        .unwrap();
    assert!(timeline.is_pending() && flow_frame.is_pending());
    assert!(Arc::ptr_eq(
        &table_after,
        &manager.live["canvas-table"].views["table"]
    ));

    manager.disable("canvas-flow").unwrap();
    revoked(&flow_frame);
    assert!(timeline.is_pending(), "retiring Flow cannot pause Timeline");
    assert!(Arc::ptr_eq(
        &table_after,
        &manager.live["canvas-table"].views["table"]
    ));
    let remaining = manager.take_canvas_frame_requests();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].0, "canvas-timeline");
    timeline.set_visible(true);
    timeline
        .present(
            plugin_runtime::plugin_protocol::Rect {
                x: 0.,
                y: 0.,
                w: 640.,
                h: 360.,
            },
            16.7,
        )
        .unwrap();
    manager.poll();
    assert_eq!(timeline_snapshot(manager)["frames"], 1);
    assert!(!manager.live.contains_key("canvas-flow"));
    assert!(Arc::ptr_eq(
        &table_after,
        &manager.live["canvas-table"].views["table"]
    ));
}
