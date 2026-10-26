//! Independent timeline components exercise retained admission through the public Package and Manager.
use plugin_runtime::{
    Manager, Package,
    plugin_protocol::{Environment, api, ui},
};
use std::time::{Duration, Instant};
#[path = "canvas_retained/lifecycle.rs"]
mod lifecycle;
#[path = "support/packages.rs"]
mod test_packages;

/// Keep native owners alive until Manager has retired its component and private profile.
struct Harness {
    manager: Manager,
    _profile: tempfile::TempDir,
    _workspace: tempfile::TempDir,
}
impl Harness {
    /// Install and activate the real independent guest through its ordinary panel demand.
    fn open() -> Self {
        let package = std::env::var_os("NANOBUG_CANVAS_TIMELINE_PACKAGE").map_or_else(
            || test_packages::package("canvas-timeline-1.0.0"),
            |path| Package::read(std::path::Path::new(&path)).unwrap(),
        );
        let profile = tempfile::tempdir().unwrap();
        let workspace = tempfile::tempdir().unwrap();
        let mut manager = Manager::open(
            profile.path().into(),
            Environment {
                workspace: workspace.path().display().to_string(),
                ..Default::default()
            },
        )
        .unwrap();
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
        let available = manager.availability("canvas-timeline").unwrap();
        let demand = manager
            .begin_demand(
                &available,
                plugin_runtime::DemandReason::View("timeline".into()),
                60000,
            )
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(90);
        loop {
            manager.poll();
            if let api::RequestUpdate::Completed { result } = demand.status() {
                result.unwrap();
                break;
            }
            assert!(Instant::now() < deadline, "timeline activation timed out");
            std::thread::sleep(Duration::from_millis(10));
        }
        Self {
            manager,
            _profile: profile,
            _workspace: workspace,
        }
    }
}

/// The real guest changes one drawing item without returning a full View.
#[test]
#[ignore = "build the independent canvas-timeline Package through --plugin-cargo; see fixture ledger"]
fn retained_timeline_admits_one_public_delta_and_preserves_the_accepted_base() {
    let mut harness = Harness::open();
    let manager = &mut harness.manager;
    let before = manager.live["canvas-timeline"].views["timeline"].clone();
    let ui::Kind::Canvas(original) = &before.root.kind else {
        panic!("expected ordinary Canvas")
    };
    assert_eq!(original.scene.as_ref().unwrap().revision, 1);
    let observations = manager.runtime_observations();
    observations.start_capture();
    manager
        .invoke_command("canvas-timeline", "mutate", serde_json::json!({"index":0}))
        .unwrap();
    let after = &manager.live["canvas-timeline"].views["timeline"];
    let ui::Kind::Canvas(changed) = &after.root.kind else {
        panic!("expected ordinary Canvas")
    };
    assert_eq!(
        changed.scene.as_ref().unwrap().revision,
        2,
        "scene delta must be admitted, not silently ignored"
    );
    assert!(after.revision > before.revision);
    assert_eq!(
        original.scene.as_ref().unwrap().revision,
        1,
        "published bases stay immutable"
    );
    assert_ne!(original.scene, changed.scene);
    let records = observations.snapshot(None).records;
    let encoded = records
        .iter()
        .filter(|record| record.stage == plugin_runtime::observability::Stage::Publication)
        .filter_map(|record| record.bytes)
        .collect::<Vec<_>>();
    assert!(
        !encoded.is_empty(),
        "publication must expose actual encoded byte observations"
    );
    assert!(
        encoded.iter().all(|bytes| *bytes < 4096),
        "one event cannot serialize the 100000-item dataset: {encoded:?}"
    );
    rejected_keeps_view(manager, "stale-patch", api::ErrorCode::StaleRevision);
    manager
        .invoke_command("canvas-timeline", "resync", serde_json::Value::Null)
        .unwrap();
    rejected_keeps_view(manager, "invalid-patch", api::ErrorCode::InvalidRequest);
    manager
        .invoke_command("canvas-timeline", "resync", serde_json::Value::Null)
        .unwrap();
    manager
        .invoke_command("canvas-timeline", "new-scene", serde_json::Value::Null)
        .unwrap();
    rejected_keeps_view(manager, "aba", api::ErrorCode::StaleRevision);
    manager
        .invoke_command("canvas-timeline", "resync", serde_json::Value::Null)
        .unwrap();
    let ui::Kind::Canvas(recovered) = &manager.live["canvas-timeline"].views["timeline"].root.kind
    else {
        panic!("missing recovered Canvas")
    };
    assert_eq!(recovered.scene.as_ref().unwrap().identity, 402);
    assert!(recovered.scene.as_ref().unwrap().revision > 1);
}

/// Recoverable admission errors keep the exact accepted Arc and do not retire a healthy guest.
fn rejected_keeps_view(manager: &mut Manager, command: &str, code: api::ErrorCode) {
    let before = manager.live["canvas-timeline"].views["timeline"].clone();
    let error = manager
        .invoke_command("canvas-timeline", command, serde_json::Value::Null)
        .err()
        .expect("malformed retained publication must be rejected");
    let failure = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<api::Failure>())
        .expect("typed admission failure");
    assert_eq!(failure.code, code, "{error:#}");
    assert!(
        std::sync::Arc::ptr_eq(&before, &manager.live["canvas-timeline"].views["timeline"]),
        "invalid peer must preserve the exact prior publication"
    );
}

/// Native frame leases are finite; hidden opportunities never stop independent business timers.
#[test]
#[ignore = "executes the independent timeline through public Package/Manager and native lease admission"]
fn retained_frame_requests_pause_without_stopping_business_and_revoke_on_replacement() {
    let mut harness = Harness::open();
    let manager = &mut harness.manager;
    let before = manager.live["canvas-timeline"].views["timeline"].clone();
    manager
        .invoke_command("canvas-timeline", "animate", serde_json::Value::Null)
        .unwrap();
    assert!(
        std::sync::Arc::ptr_eq(&before, &manager.live["canvas-timeline"].views["timeline"]),
        "frame demand cannot invent a UI revision"
    );
    let pending = manager.take_canvas_frame_requests();
    assert_eq!(pending.len(), 1);
    let lease = pending[0].1.clone();
    for _ in 0..300 {
        let again = manager.take_canvas_frame_requests();
        assert_eq!(
            again.len(),
            1,
            "a hidden request cannot disappear during native handoff"
        );
        assert!(std::sync::Arc::ptr_eq(&lease, &again[0].1));
    }
    manager
        .invoke_command("canvas-timeline", "request-again", serde_json::Value::Null)
        .unwrap();
    rejected_keeps_view(manager, "request-conflict", api::ErrorCode::Conflict);
    let viewport = plugin_runtime::plugin_protocol::Rect {
        x: 0.,
        y: 0.,
        w: 640.,
        h: 360.,
    };
    lease.set_visible(false);
    assert!(lease.present(viewport, 16.7).is_err());
    manager
        .invoke_command("canvas-timeline", "business-timer", serde_json::Value::Null)
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        manager.poll();
        let observed = timeline_snapshot(manager);
        assert_eq!(observed["frames"].as_u64(), Some(0));
        if observed["business_ticks"].as_u64().unwrap() >= 2 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "hidden business timer did not remain reachable"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        std::sync::Arc::ptr_eq(&before, &manager.live["canvas-timeline"].views["timeline"]),
        "business ticks cannot dirty hidden artwork"
    );
    lease.set_visible(true);
    lease.present(viewport, 16.7).unwrap();
    assert!(
        lease.present(viewport, 16.7).is_err(),
        "a native opportunity completes only once"
    );
    manager.poll();
    assert_eq!(timeline_snapshot(manager)["frames"].as_u64(), Some(1));
    let next = manager.take_canvas_frame_requests();
    assert_eq!(next.len(), 1);
    assert!(next[0].1.request().request > lease.request().request);
    manager
        .invoke_command("canvas-timeline", "stop", serde_json::Value::Null)
        .unwrap();
    next[0].1.set_visible(true);
    next[0].1.present(viewport, 16.7).unwrap();
    manager.poll();
    assert!(manager.take_canvas_frame_requests().is_empty());
    assert_eq!(timeline_snapshot(manager)["frames"].as_u64(), Some(1));
    manager
        .invoke_command("canvas-timeline", "business-stop", serde_json::Value::Null)
        .unwrap();
    manager
        .invoke_command("canvas-timeline", "animate", serde_json::Value::Null)
        .unwrap();
    let retired = manager.take_canvas_frame_requests()[0].1.clone();
    manager
        .invoke_command("canvas-timeline", "new-scene", serde_json::Value::Null)
        .unwrap();
    assert!(
        !retired.is_pending(),
        "a replacement scene cannot borrow the old frame request"
    );
    retired.set_visible(true);
    assert!(retired.present(viewport, 16.7).is_err());
}

/// Observe ordinary opaque snapshots, with no new host API reserved for this acceptance fixture.
fn timeline_snapshot(manager: &mut Manager) -> serde_json::Value {
    let snapshot = manager
        .live
        .get_mut("canvas-timeline")
        .unwrap()
        .snapshot()
        .unwrap();
    serde_json::from_str(&snapshot.data).unwrap()
}
