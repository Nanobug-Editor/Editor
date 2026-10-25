//! Actual package installation and native button drags preserve window order across plugin lifecycle changes.

use super::*;
use gpui_kit::{MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point};

/// Reuse immutable guest code under an independent identity, exposing just two ordinary window controls.
/// `priority` deliberately competes across packages; only panels in the same package may use it to reorder.
fn ordered_package(id: &str, version: &str, priority: i32) -> Package {
    let source = Package::read(&crate::test_packages::archive("tools-example")).unwrap();
    let mut files = identify(source, id, id).files;
    let mut manifest: serde_json::Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    manifest["version"] = version.into();
    // Hiding the unused fixture windows requires the public 1.1 panel-control visibility contract.
    manifest["api"]["required"]["ui.panels"] = ">=1.1,<2".into();
    for panel in manifest["panels"].as_array_mut().unwrap() {
        let id = panel["id"].as_str().unwrap().to_owned();
        panel["default_visible"] = false.into();
        panel["show_in_window_controls"] = matches!(id.as_str(), "window-a" | "window-b").into();
        panel["status_order"] = (priority + if id == "window-a" { 20 } else { 10 }).into();
    }
    if let Some(path) = manifest["contributions"].as_str() {
        let mut contributions: toml::Value =
            toml::from_str(std::str::from_utf8(&files[path]).unwrap()).unwrap();
        contributions["plugin"]["version"] = toml::Value::String(version.into());
        files.insert(
            path.into(),
            toml::to_string(&contributions).unwrap().into_bytes(),
        );
    }
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    Package::from_files(files).unwrap()
}

/// Verify supplied controls are visibly ordered left to right on the same native status line.
fn assert_order(visual: &mut VisualTestContext, expected: &[&'static str]) {
    let buttons: Vec<_> = expected
        .iter()
        .map(|id| {
            (
                id,
                visual
                    .debug_bounds(*id)
                    .unwrap_or_else(|| panic!("missing visible window button {id}")),
            )
        })
        .collect();
    for pair in buttons.windows(2) {
        assert!(
            pair[0].1.right() <= pair[1].1.left()
                && (pair[0].1.top() - pair[1].1.top()).abs() < px(2.),
            "window controls are out of order: {buttons:?}"
        );
    }
}

/// Drag an actual button before a visible peer after the deliberate long-press threshold.
/// Every frame still uses the ordinary Package/Manager publication path instead of synthetic entries.
fn drag_before(
    visual: &mut VisualTestContext,
    app: &Entity<EditorApp>,
    manager: &mut Manager,
    source: &'static str,
    before: &'static str,
) {
    let bounds = visual.debug_bounds(before).unwrap();
    let target = point(bounds.left() + px(1.), bounds.center().y);
    drag_to(visual, app, manager, source, target);
}

/// Start the native drag inside its source, then move to a horizontal or vertical insertion point.
/// Test windows start inactive; explicitly activate this window before the real hold admission timer.
fn drag_to(
    visual: &mut VisualTestContext,
    app: &Entity<EditorApp>,
    manager: &mut Manager,
    source: &'static str,
    target: Point<Pixels>,
) {
    visual.update(|window, _| window.activate_window());
    draw(app, manager, visual);
    let start = visual.debug_bounds(source).unwrap().center();
    visual.simulate_event(MouseDownEvent {
        position: start,
        button: MouseButton::Left,
        modifiers: Default::default(),
        click_count: 1,
        first_mouse: false,
    });
    visual.run_until_parked();
    visual.executor().advance_clock(Duration::from_millis(450));
    draw(app, manager, visual);
    visual.simulate_event(MouseMoveEvent {
        position: start + point(px(8.), px(0.)),
        pressed_button: Some(MouseButton::Left),
        modifiers: Default::default(),
    });
    draw(app, manager, visual);
    assert!(visual.update(|_, cx| cx.has_active_drag()));
    visual.simulate_event(MouseMoveEvent {
        position: target,
        pressed_button: Some(MouseButton::Left),
        modifiers: Default::default(),
    });
    draw(app, manager, visual);
    visual.simulate_event(MouseUpEvent {
        position: target,
        button: MouseButton::Left,
        modifiers: Default::default(),
        click_count: 1,
    });
    draw(app, manager, visual);
}

/// Install order wins over names/priorities, and user moves survive upgrades, unavailable slots and new packages.
#[gpui::test]
#[ignore = "requires frozen tools-example Package/WASM; see crates/plugin-runtime/tests/fixtures/packages/README.md"]
fn window_toolbar_real_packages_keep_installation_and_user_order(cx: &mut TestAppContext) {
    crate::tests::with_shortcut_editor(cx, false, Vec::new(), |visual, app, document| {
        let workspace = document.parent().unwrap();
        let mut manager = Manager::open(
            workspace.join(".runtime-plugin-test"),
            protocol::Environment {
                workspace: workspace.display().to_string(),
                ..Default::default()
            },
        )
        .unwrap();
        for package in [
            ordered_package("z-first", "1.0.0", 900),
            ordered_package("a-second", "1.0.0", -900),
        ] {
            manager
                .install(&package, package.manifest.permissions.clone())
                .unwrap();
        }
        draw(&app, &mut manager, visual);
        let initial = [
            "explorer-panel-toggle",
            "outline-toggle",
            "host-messages-toggle",
            "terminal-toggle",
            "plugin-window-z-first/window-b",
            "plugin-window-z-first/window-a",
            "plugin-window-a-second/window-b",
            "plugin-window-a-second/window-a",
        ];
        assert_order(visual, &initial);

        // Replacing the first package after the second must not turn update time into installation time.
        let upgrade = ordered_package("z-first", "2.0.0", 900);
        manager
            .install(&upgrade, upgrade.manifest.permissions.clone())
            .unwrap();
        draw(&app, &mut manager, visual);
        assert_order(visual, &initial);

        drag_before(
            visual,
            &app,
            &mut manager,
            "plugin-window-a-second/window-a",
            "plugin-window-z-first/window-b",
        );
        assert_order(
            visual,
            &[
                "explorer-panel-toggle",
                "outline-toggle",
                "host-messages-toggle",
                "terminal-toggle",
                "plugin-window-a-second/window-a",
                "plugin-window-z-first/window-b",
                "plugin-window-z-first/window-a",
                "plugin-window-a-second/window-b",
            ],
        );

        // A second user move while a package is disabled must retain that package's saved slots.
        manager.disable("z-first").unwrap();
        draw(&app, &mut manager, visual);
        for id in [
            "plugin-window-z-first/window-a",
            "plugin-window-z-first/window-b",
        ] {
            assert!(visual.debug_bounds(id).is_none());
        }
        drag_before(
            visual,
            &app,
            &mut manager,
            "plugin-window-a-second/window-b",
            "terminal-toggle",
        );
        manager.enable("z-first").unwrap();
        draw(&app, &mut manager, visual);
        let arranged = [
            "explorer-panel-toggle",
            "outline-toggle",
            "host-messages-toggle",
            "plugin-window-a-second/window-b",
            "terminal-toggle",
            "plugin-window-a-second/window-a",
            "plugin-window-z-first/window-b",
            "plugin-window-z-first/window-a",
        ];
        assert_order(visual, &arranged);

        // A package may finish installing while this popup still owns a captured membership list.
        let saved_order =
            visual.update(|_, cx| app.read(cx).session_state.window_button_order.clone());
        visual.simulate_resize(size(px(360.), px(850.)));
        draw(&app, &mut manager, visual);
        super::click("plugin-windows-overflow", &app, &mut manager, visual);
        assert!(visual.debug_bounds("native-popup-menu").is_some());
        let late = ordered_package("m-late", "1.0.0", -2000);
        manager
            .install(&late, late.manifest.permissions.clone())
            .unwrap();
        draw(&app, &mut manager, visual);
        let first = visual
            .debug_bounds("native-menu-explorer-panel-toggle")
            .unwrap();
        drag_to(
            visual,
            &app,
            &mut manager,
            "native-menu-host-messages-toggle",
            point(first.center().x, first.top() + px(1.)),
        );
        assert!(visual.update(|_, cx| app.read(cx).tool_overflow.is_none()));
        assert!(visual.debug_bounds("native-popup-menu").is_none());
        assert_eq!(
            visual.update(|_, cx| app.read(cx).session_state.window_button_order.clone()),
            saved_order,
            "a stale popup must not publish its locally rearranged order"
        );

        let mut appended = arranged.to_vec();
        appended.extend([
            "plugin-window-m-late/window-b",
            "plugin-window-m-late/window-a",
        ]);
        super::click("plugin-windows-overflow", &app, &mut manager, visual);
        let reopened = visual.update(|_, cx| {
            app.read(cx)
                .tool_overflow
                .as_ref()
                .unwrap()
                .read(cx)
                .items
                .iter()
                .map(|item| item.id.clone())
                .collect::<Vec<_>>()
        });
        assert_eq!(reopened, appended);
        assert!(
            visual
                .debug_bounds("native-menu-plugin-window-m-late/window-b")
                .is_some()
        );
        visual.simulate_keystrokes("escape");
        visual.simulate_resize(size(px(1100.), px(850.)));
        draw(&app, &mut manager, visual);
        assert_order(visual, &appended);
    });
}
