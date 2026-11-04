//! Actual package defaults and native controls respect each workspace's persisted visibility.

use super::*;
use plugin_runtime::Manager;

/// Validate a real ZIP through Manager, then enter fresh and restored native workspace windows.
#[gpui::test]
#[ignore = "set NANOBUG_PANEL_DEFAULTS_PACKAGE to an independently built package with closed panel defaults"]
fn package_panel_defaults_keep_new_projects_closed_and_restore_user_choices(
    cx: &mut TestAppContext,
) {
    let archive = std::env::var_os("NANOBUG_PANEL_DEFAULTS_PACKAGE").expect("candidate ZIP");
    let package = Package::read(std::path::Path::new(&archive)).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(
        directory.path().join("runtime"),
        protocol::Environment::default(),
    )
    .unwrap();
    // Profile-only installation validates the original package without starting repository tools.
    manager.select_application(protocol::Environment::default());
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    assert!(manager.live.is_empty());
    let installed = manager.installed[&package.manifest.id].clone();
    let profile = directory.path().join("sessions");
    let first = directory.path().join("first");
    let second = directory.path().join("second");
    std::fs::create_dir_all(&first).unwrap();
    std::fs::create_dir_all(&second).unwrap();
    cx.update(|cx| {
        gpui_kit::init(cx);
        typography::init(cx);
        apply_theme(builtin_theme(false), cx);
        init(cx);
        cx.set_reduce_motion(true);
    });
    // Each phase constructs a fresh EditorApp and loads the production per-workspace session.
    workspace_visibility(cx, &first, &profile, &installed, false, true);
    workspace_visibility(cx, &second, &profile, &installed, false, false);
    workspace_visibility(cx, &first, &profile, &installed, true, true);
    workspace_visibility(cx, &first, &profile, &installed, false, false);
}

/// Load `workspace` in `profile`, check `opened`, optionally toggle both ordinary controls, and save.
/// `installed` comes unchanged from public installation; this checks native visibility without executing WASM.
fn workspace_visibility(
    cx: &mut TestAppContext,
    workspace: &std::path::Path,
    profile: &std::path::Path,
    installed: &Installed,
    opened: bool,
    toggle: bool,
) {
    let workspace = Workspace::open(workspace).unwrap();
    let mut session = SessionState::load_in_profile(workspace.root(), profile);
    // Only disposable test projects receive trust; native controls use the ordinary admission path.
    session.workspace_trusted = true;
    let capture = Rc::new(RefCell::new(None));
    let slot = capture.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let app =
            cx.new(|cx| EditorApp::new_with_session(workspace, None, Some(session), window, cx));
        *slot.borrow_mut() = Some(app.clone());
        Root::new(app, window, cx)
    });
    let app = capture.borrow_mut().take().unwrap();
    visual.simulate_resize(size(px(1200.), px(820.)));
    visual.update(|window, cx| {
        let owner = app.read(cx).extensions.clone();
        owner.update(cx, |owner, cx| {
            owner.worker.state.lock().unwrap().entries = vec![installed.clone()];
            owner.poll(cx);
        });
        app.update(cx, |app, cx| app.sync_plugin_panels(window, cx));
    });
    visual.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear(cx));
    let controls = installed
        .manifest
        .panels
        .iter()
        .filter(|panel| panel.show_in_window_controls && panel.position != "editor")
        .map(|panel| format!("{}/{}", installed.manifest.id, panel.id))
        .collect::<Vec<_>>();
    assert!(!controls.is_empty());
    visual.update(|_, cx| {
        let app = app.read(cx);
        assert!(app.plugin_windows.is_empty());
        for (key, panel) in &app.plugin_panels {
            let expected = opened && controls.contains(key);
            assert_eq!(panel.read(cx).visible.get(), expected, "{key}");
            assert_eq!(panel.read(cx).panel_visibility.displayed, expected, "{key}");
        }
    });
    for key in &controls {
        // GPUI retains debug selectors for the entire test, so its API requires static storage.
        let button: &'static str = Box::leak(format!("plugin-window-{key}").into_boxed_str());
        let bounds = visual
            .debug_bounds(button)
            .expect("accessible native window control");
        if toggle {
            visual.simulate_click(bounds.center(), Modifiers::default());
            visual.run_until_parked();
            visual.update(|window, cx| window.draw(cx).clear(cx));
            visual.update(|_, cx| {
                let panel = app.read(cx).plugin_panels[key].read(cx);
                assert_eq!(panel.visible.get(), !opened, "native toggle: {key}");
                assert_eq!(panel.panel_visibility.displayed, !opened, "{key}");
            });
        }
    }
    visual.update(|_, cx| {
        app.update(cx, |app, cx| {
            app.capture_dock_layout(cx);
            app.session_state.try_save().unwrap();
        });
    });
}
