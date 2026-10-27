//! Actual packages retain original native receipts across a bounded accepted/drawn pipeline gap.
use plugin_runtime::{
    Manager, Package,
    plugin_protocol::{Environment, api, ui},
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

/// Own the disposable profile until its real independent timeline component has retired.
struct Harness {
    manager: Manager,
    _profile: tempfile::TempDir,
    _workspace: tempfile::TempDir,
}
impl Harness {
    /// Require the explicitly built 1.0.1 candidate instead of reinterpreting a frozen old package.
    fn open() -> Self {
        let path = std::env::var_os("NANOBUG_CANVAS_TIMELINE_PACKAGE")
            .expect("set the independently built timeline 1.0.1 Package");
        let package = Package::read(std::path::Path::new(&path)).unwrap();
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
        let availability = manager.availability("canvas-timeline").unwrap();
        let demand = manager
            .begin_demand(
                &availability,
                plugin_runtime::DemandReason::View("timeline".into()),
                60_000,
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
    /// Read only accepted immutable UI, never private guest memory or a fabricated host scene.
    fn document(&self) -> Arc<ui::Document> {
        self.manager.live["canvas-timeline"].views["timeline"].clone()
    }
    /// Native observations use exactly the ordinary owned Manager event boundary.
    fn send(&mut self, event: ui::UiEvent) -> anyhow::Result<()> {
        self.manager.event(
            "canvas-timeline",
            Some("timeline".into()),
            api::Notification::Ui(event),
        )
    }
    /// Change actual guest artwork without presenting or rewriting any existing native callback.
    fn advance(&mut self) {
        self.manager
            .invoke_command("canvas-timeline", "mutate", serde_json::json!({"index":0}))
            .unwrap();
    }
}

/// The receipt is copied from the exact accepted snapshot that the native adapter actually painted.
fn receipt(shown: &ui::Document) -> ui::UiEvent {
    let node = shown.active_node("drawing").unwrap();
    let ui::Kind::Canvas(canvas) = &node.kind else {
        panic!("ordinary Canvas")
    };
    let scene = canvas.scene.as_ref().unwrap();
    ui::UiEvent {
        revision: shown.revision,
        node: "drawing".into(),
        action: ui::Action::ViewportInput(ui::ViewportInput {
            content: node.viewport.as_ref().unwrap().content.unwrap(),
            event: ui::CanvasEvent::ScenePresented {
                identity: scene.identity,
                scene_revision: scene.revision,
                source: shown.source.clone(),
                file: shown.file.clone(),
                projection: node.viewport.clone(),
                error: None,
            },
        }),
    }
}

/// Original shown-target addressing remains immutable even while newer drawing-only output is accepted.
fn wheel(shown: &ui::Document, sequence: u64) -> ui::UiEvent {
    let node = shown.active_node("drawing").unwrap();
    let ui::Kind::Canvas(canvas) = &node.kind else {
        panic!("ordinary Canvas")
    };
    let scene = canvas.scene.as_ref().unwrap();
    ui::UiEvent {
        revision: shown.revision,
        node: "drawing".into(),
        action: ui::Action::Canvas(ui::CanvasEvent::Input {
            input: ui::CanvasInput {
                identity: scene.identity,
                presented_revision: scene.revision,
                target: "timeline-surface".into(),
                epoch: 1,
                sequence,
                source: shown.source.clone(),
                file: shown.file.clone(),
                projection: node.viewport.clone(),
                kind: ui::CanvasInputKind::Wheel(ui::CanvasWheelInput {
                    x: 300.,
                    y: 140.,
                    delta_x: -480.,
                    delta_y: 0.,
                    unit: ui::CanvasWheelUnit::Lines,
                    modifiers: Default::default(),
                }),
            },
        }),
    }
}

/// Rejections keep a healthy current publication and expose the actual typed boundary failure.
fn stale(harness: &mut Harness, event: ui::UiEvent) {
    let current = harness.document();
    let error = harness
        .send(event)
        .expect_err("unproven or retired native input must reject");
    let failure = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<api::Failure>())
        .unwrap_or_else(|| panic!("typed failure missing: {error:#}"));
    assert_eq!(failure.code, api::ErrorCode::StaleRevision, "{error:#}");
    assert!(
        Arc::ptr_eq(&current, &harness.document()),
        "native rejection changed current artwork"
    );
}

/// Reproduce the real paint/actor race: accepted B cannot erase a delayed truthful receipt for A.
#[test]
#[ignore = "set NANOBUG_CANVAS_TIMELINE_PACKAGE to the independent 1.0.1 candidate"]
fn late_actual_presentation_keeps_original_shown_target_after_new_artwork_is_accepted() {
    let mut harness = Harness::open();
    let shown = harness.document();
    harness.advance();
    let current = harness.document();
    assert!(current.revision > shown.revision);
    stale(&mut harness, wheel(&shown, 1));
    harness
        .send(receipt(&shown))
        .expect("a delayed real receipt must validate its original accepted snapshot");
    assert!(
        Arc::ptr_eq(&current, &harness.document()),
        "a receipt cannot publish or rewrite artwork"
    );
    harness
        .send(wheel(&shown, 1))
        .expect("the original native input must retain its shown-frame proof");
    assert!(
        Arc::ptr_eq(&current, &harness.document()),
        "boundary input cannot scroll twice or invent UI output"
    );
    // A late failed paint must not retire a newer successful native proof.
    harness.send(receipt(&current)).unwrap();
    let mut old_failure = receipt(&shown);
    if let ui::Action::ViewportInput(input) = &mut old_failure.action {
        if let ui::CanvasEvent::ScenePresented { error, .. } = &mut input.event {
            *error = Some("old renderer failure".into());
        }
    }
    stale(&mut harness, old_failure);
    harness
        .send(wheel(&current, 2))
        .expect("a stale failed receipt cannot cancel the newer shown target");
    let mut future = receipt(&current);
    future.revision += 1;
    stale(&mut harness, future);
    harness
        .manager
        .invoke_command("canvas-timeline", "new-scene", serde_json::Value::Null)
        .unwrap();
    stale(&mut harness, receipt(&shown));
    stale(&mut harness, wheel(&shown, 3));
}

/// History is an exact finite address book, not authorization for made-up projections or old revisions.
#[test]
#[ignore = "set NANOBUG_CANVAS_TIMELINE_PACKAGE to the independent 1.0.1 candidate"]
fn late_presentation_rejects_forged_projection_and_evicted_accepted_snapshot() {
    let mut harness = Harness::open();
    let shown = harness.document();
    harness.advance();
    let mut forged = receipt(&shown);
    if let ui::Action::ViewportInput(input) = &mut forged.action {
        if let ui::CanvasEvent::ScenePresented {
            projection: Some(projection),
            ..
        } = &mut input.event
        {
            projection.transform.as_mut().unwrap().x = 1.;
        }
    }
    stale(&mut harness, forged);
    for _ in 0..8 {
        harness.advance();
    }
    stale(&mut harness, receipt(&shown));
    stale(&mut harness, wheel(&shown, 1));
}
