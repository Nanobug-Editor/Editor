//! Real pending frame leases cross the panel scheduler before the test platform presents them.
use super::*;
use gpui_kit::{TestAppContext, VisualTestContext, gpui};

/// Keep the independent guest and its isolated workspace alive through every native callback.
struct Fixture {
    /// Private storage survives until the runtime owner and its native window have been released.
    _directory: tempfile::TempDir,
    /// The public runtime retains the real guest and receives completed native frame opportunities.
    manager: plugin_runtime::Manager,
    /// The ordinary host root owns the dock, ExtensionPanel and its mounted native canvas.
    app: Entity<EditorApp>,
}

impl Fixture {
    /// Install and activate the frozen timeline through public Manager admission, then mount its dock.
    fn mount(cx: &mut TestAppContext) -> (Self, &mut VisualTestContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            typography::init(cx);
            apply_theme(builtin_theme(false), cx);
            cx.set_reduce_motion(true);
        });
        let directory = tempfile::tempdir().unwrap();
        let workspace = Workspace::open(directory.path()).unwrap();
        let package = crate::test_packages::package("canvas-timeline-1.0.1");
        let mut manager = plugin_runtime::Manager::open(
            directory.path().join(".runtime-plugin-test"),
            protocol::Environment {
                workspace: workspace.root().display().to_string(),
                ..Default::default()
            },
        )
        .unwrap();
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
        super::super::demand_plugin_fixture(
            &mut manager,
            "canvas-timeline",
            plugin_runtime::DemandReason::View("timeline".into()),
        )
        .unwrap();
        let slot = Rc::new(RefCell::new(None));
        let capture = slot.clone();
        let (_, ui) = cx.add_window_view(move |window, cx| {
            let app = cx.new(|cx| EditorApp::new(workspace, None, window, cx));
            *capture.borrow_mut() = Some(app.clone());
            Root::new(app, window, cx)
        });
        let app = slot.borrow_mut().take().unwrap();
        ui.update(|window, _| window.activate_window());
        ui.simulate_resize(size(px(1200.), px(800.)));
        super::super::composable_tests::publish(
            &mut manager,
            &mut images::VectorRenderer::default(),
            &app,
            ui,
        );
        ui.run_until_parked();
        (
            Self {
                _directory: directory,
                manager,
                app,
            },
            ui,
        )
    }

    /// Hand the native adapter the same Manager-issued resources that the real actor publishes.
    fn next_request(
        &mut self,
        panel: &Entity<ExtensionPanel>,
        ui: &mut VisualTestContext,
    ) -> Arc<plugin_runtime::CanvasFrameResource> {
        let frames = self.manager.take_canvas_frame_requests();
        let request = frames
            .iter()
            .find(|(plugin, _)| plugin == "canvas-timeline")
            .expect("the real guest requested its next visible frame")
            .1
            .clone();
        assert!(request.is_pending());
        ui.update(|_, cx| {
            panel.read(cx).worker.state.lock().unwrap().canvas_frames = frames;
        });
        request
    }

    /// Read the guest's own logical counter after Manager delivers the native opportunity.
    fn frame_count(&mut self) -> u64 {
        self.manager.poll();
        let snapshot = self
            .manager
            .live
            .get_mut("canvas-timeline")
            .unwrap()
            .snapshot()
            .unwrap();
        serde_json::from_str::<serde_json::Value>(&snapshot.data).unwrap()["frames"]
            .as_u64()
            .unwrap()
    }
}

/// Pending work must survive a nested render borrow, pause while hidden, and resume exactly once.
#[gpui::test]
#[ignore = "opt-in frozen Package/WASM fixtures; see crates/plugin-runtime/tests/fixtures/packages/README.md"]
fn pending_canvas_frames_cross_nested_panel_borrows_and_hidden_surfaces(cx: &mut TestAppContext) {
    let (mut fixture, ui) = Fixture::mount(cx);
    let (panel, native) = ui.update(|_, cx| {
        let panel = fixture.app.read(cx).plugin_panels["canvas-timeline/timeline"].clone();
        let native = panel
            .read(cx)
            .native_ui
            .clone()
            .expect("visible native canvas body");
        (panel, native)
    });
    assert!(ui.debug_bounds("plugin-ui-drawing").is_some());
    fixture
        .manager
        .invoke_command("canvas-timeline", "animate", serde_json::Value::Null)
        .unwrap();
    let first = fixture.next_request(&panel, ui);
    ui.update(|_, cx| {
        // Composed source rendering holds this same native lease while it asks the panel to route.
        native.update(cx, |_, cx| {
            panel.update(cx, |panel, cx| panel.schedule_canvas_frames(cx));
        });
    });
    assert!(
        first.is_pending(),
        "scheduling alone is not a native opportunity"
    );
    ui.update(|window, cx| {
        window.simulate_next_frame(cx);
    });
    assert!(!first.is_pending());
    assert_eq!(fixture.frame_count(), 1);

    let second = fixture.next_request(&panel, ui);
    ui.update(|_, cx| {
        native.update(cx, |_, cx| {
            panel.update(cx, |panel, cx| panel.schedule_canvas_frames(cx));
        });
        // Withdraw before the queued routing callback executes, after releasing the native borrow.
        panel.update(cx, |panel, cx| panel.hide(cx));
    });
    ui.update(|window, cx| {
        window.simulate_next_frame(cx);
    });
    assert!(
        second.is_pending(),
        "a hidden surface must retain its original pending lease"
    );
    assert_eq!(fixture.frame_count(), 1);

    ui.update(|window, cx| {
        panel.update(cx, |panel, cx| {
            panel.visible.set(true);
            panel.activate_dock();
            cx.notify();
        });
        window.draw(cx).clear(cx);
    });
    ui.update(|window, cx| {
        window.simulate_next_frame(cx);
    });
    assert!(!second.is_pending());
    assert_eq!(fixture.frame_count(), 2);
}
