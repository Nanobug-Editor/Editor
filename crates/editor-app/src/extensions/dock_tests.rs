//! Native panel geometry and resize regressions through EditorApp publication and the production dock renderer.
use super::*;
use gpui_base::dock::{DockPlacement, PanelId};
use gpui_kit::{TestAppContext, gpui};

#[cfg(test)]
mod drag;
#[cfg(test)]
mod right_width;

/// A manifest's left hint joins the same visible sidebar as host peers instead of claiming another editor column.
#[gpui::test]
fn left_native_panels_share_explorer_outline_sidebar_without_extra_column(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        typography::init(cx);
        apply_theme(builtin_theme(false), cx);
        init(cx);
        cx.set_reduce_motion(true);
    });
    let directory = tempfile::tempdir().unwrap();
    let workspace_path = directory.path().join("workspace");
    std::fs::create_dir_all(&workspace_path).unwrap();
    let workspace = Workspace::open(&workspace_path).unwrap();
    let mut session =
        SessionState::load_in_profile(&workspace_path, &directory.path().join("profile"));
    session.explorer_visible = true;
    session.outline_visible = true;
    session.messages_visible = false;
    let slot = Rc::new(RefCell::new(None));
    let capture = slot.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let app =
            cx.new(|cx| EditorApp::new_with_session(workspace, None, Some(session), window, cx));
        *capture.borrow_mut() = Some(app.clone());
        Root::new(app, window, cx)
    });
    let app = slot.borrow_mut().take().unwrap();
    visual.simulate_resize(size(px(1200.), px(820.)));
    let (explorer, outline, editor) = visual.update(|_, cx| {
        let app = app.read(cx);
        (
            PanelId::from(app.explorer_panel.entity_id()),
            PanelId::from(app.outline_panel.entity_id()),
            PanelId::from(app.editor_panel.entity_id()),
        )
    });
    let editor_before = native_panel_bounds(&app, editor, visual);
    let explorer_before = native_panel_bounds(&app, explorer, visual);
    let outline_before = native_panel_bounds(&app, outline, visual);
    assert!(explorer_before.size.height > px(0.) && outline_before.size.height > px(0.));
    assert!((explorer_before.left() - outline_before.left()).abs() < px(2.));
    assert!((explorer_before.size.width - outline_before.size.width).abs() < px(2.));

    let mut manifest: protocol::Manifest =
        crate::extensions::test_manifest(include_str!("fixtures/panel-contract.json"));
    // An arbitrary contribution proves that layout is a generic native-panel contract.
    manifest.id = "sidebar-fixture".into();
    manifest.panels.truncate(1);
    manifest.panels[0].id = "review".into();
    manifest.panels[0].position = "left".into();
    manifest.panels[0].default_visible = true;
    let scene = protocol::ui::Document::new(
        protocol::ui::Node::new(
            "sidebar-fixture-content",
            protocol::ui::Kind::Text {
                text: "Fixture side content".into(),
            },
        )
        .grow(),
    )
    .revision(1);
    let plugin = visual.update(|window, cx| {
        let owner = app.read(cx).extensions.clone();
        owner.update(cx, |owner, cx| {
            let mut state = owner.worker.state.lock().unwrap();
            state.entries = vec![Installed {
                replaced_by: None,
                manifest: manifest.clone(),
                digest: "sidebar-fixture".into(),
                grants: manifest.permissions.clone(),
                enabled: true,
                project_enabled: Default::default(),
                global_enabled: None,
                retired_ui_contract: false,
                error: None,
            }];
            state
                .views
                .insert("sidebar-fixture/review".into(), Arc::new(scene));
            drop(state);
            owner.poll(cx);
        });
        app.update(cx, |app, cx| app.sync_plugin_panels(window, cx));
        let panel = app.read(cx).plugin_panels["sidebar-fixture/review"].clone();
        panel.update(cx, |panel, cx| panel.poll(cx));
        PanelId::from(panel.entity_id())
    });

    let explorer = native_panel_bounds(&app, explorer, visual);
    let outline = native_panel_bounds(&app, outline, visual);
    let plugin = native_panel_bounds(&app, plugin, visual);
    let editor_after = native_panel_bounds(&app, editor, visual);
    let mut sidebar = [
        ("Explorer", explorer),
        ("Outline", outline),
        ("native plugin", plugin),
    ];
    for (name, bounds) in &sidebar {
        assert!(
            bounds.size.width > px(0.) && bounds.size.height > px(0.),
            "{name} must remain visible: {bounds:?}"
        );
        assert!(
            (bounds.left() - explorer.left()).abs() < px(2.),
            "{name} must share the Explorer/Outline sidebar x coordinate: {bounds:?}, Explorer={explorer:?}"
        );
        assert!(
            (bounds.size.width - explorer.size.width).abs() < px(2.),
            "{name} must share the Explorer/Outline sidebar width: {bounds:?}, Explorer={explorer:?}"
        );
        assert!(
            bounds.right() <= editor_after.left() + px(2.),
            "{name} must remain beside the editor"
        );
    }
    sidebar.sort_by(|left, right| left.1.top().partial_cmp(&right.1.top()).unwrap());
    for pair in sidebar.windows(2) {
        assert!(
            pair[0].1.bottom() <= pair[1].1.top() + px(2.),
            "sidebar peers must have separate visible split slots: {pair:?}"
        );
    }
    // Sharing vertical space may change heights, but it must not reserve a second horizontal band.
    assert!(
        (editor_after.left() - editor_before.left()).abs() < px(2.),
        "a left plugin must not shift the editor into a second sidebar column: {editor_before:?} -> {editor_after:?}"
    );
    assert!(
        (editor_after.size.width - editor_before.size.width).abs() < px(2.),
        "a left plugin must not consume another sidebar width: {editor_before:?} -> {editor_after:?}"
    );
    visual.update(|_, cx| {
        assert_eq!(
            cx.windows().len(),
            1,
            "ordinary sidebar contributions must stay in the existing GPUI window"
        )
    });
}

/// Measure the current normalized Base leaf through the production LocalDock frame after painting.
pub(super) fn native_panel_bounds(
    app: &Entity<EditorApp>,
    panel: PanelId,
    visual: &mut gpui_kit::VisualTestContext,
) -> Bounds<Pixels> {
    visual.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear(cx));
    let node = visual.update(|_, cx| {
        let area = app.read(cx).dock_area.read(cx);
        [
            DockPlacement::Center,
            DockPlacement::Left,
            DockPlacement::Right,
            DockPlacement::Bottom,
        ]
        .into_iter()
        .find_map(|placement| area.layout(placement)?.find_panel_node(panel))
        .expect("a visible panel must belong to the current native DockArea")
    });
    let selector = format!("local-dock-content-{}", node.as_u64());
    visual
        .debug_bounds(Box::leak(selector.into_boxed_str()))
        .expect("the native dock panel must be painted")
}

/// Native dock dragging forwards canvas sizes and retains the user's size after hiding.
#[gpui::test]
fn native_dock_drag_preserves_canvas_viewport_and_saved_size(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        typography::init(cx);
        apply_theme(builtin_theme(false), cx);
        init(cx);
    });
    let directory = tempfile::tempdir().unwrap();
    let workspace = Workspace::open(directory.path()).unwrap();
    let slot = Rc::new(RefCell::new(None));
    let capture = slot.clone();
    let (_, cx) = cx.add_window_view(move |window, cx| {
        let app = cx.new(|cx| EditorApp::new(workspace, None, window, cx));
        *capture.borrow_mut() = Some(app.clone());
        Root::new(app, window, cx)
    });
    let app = slot.borrow_mut().take().unwrap();
    let mut manifest: protocol::Manifest =
        crate::extensions::test_manifest(include_str!("fixtures/panel-contract.json"));
    // This test explicitly opens the dock; the packaged terminal now starts hidden.
    manifest.panels[0].default_visible = true;
    let scene = protocol::ui::Document::new(
        protocol::ui::Node::new(
            "output",
            protocol::ui::Kind::Canvas(protocol::ui::Canvas {
                focusable: true,
                grid: true,
                ..Default::default()
            }),
        )
        .grow(),
    )
    .revision(1);
    cx.simulate_resize(size(px(1100.), px(800.)));
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let full_editor = cx.debug_bounds("editor-panel-content").unwrap();
    let owner = cx.update(|window, cx| {
        let owner = app.read(cx).extensions.clone();
        owner.update(cx, |owner, cx| {
            let mut state = owner.worker.state.lock().unwrap();
            state.entries = vec![Installed {
                replaced_by: None,
                manifest: manifest.clone(),
                digest: "fixture".into(),
                grants: manifest.permissions.clone(),
                enabled: true,
                project_enabled: Default::default(),
                global_enabled: None,
                retired_ui_contract: false,
                error: None,
            }];
            state
                .views
                .insert("terminal/terminal".into(), Arc::new(scene.clone()));
            drop(state);
            owner.poll(cx);
        });
        app.update(cx, |app, cx| app.sync_plugin_panels(window, cx));
        let panel = app.read(cx).plugin_panels["terminal/terminal"].clone();
        panel.update(cx, |panel, cx| {
            panel.poll(cx);
            panel.focus(window, cx);
        });
        window.draw(cx).clear(cx);
        owner
    });
    let bounds = cx
        .debug_bounds("plugin-ui-output")
        .expect("manifest panel is visible without restarting");
    assert!(bounds.top() > px(300.));
    assert!(bounds.size.height > px(100.));
    // Drag the dock's upper edge, then verify both the visible size and persisted height.
    let edge = point(bounds.center().x, bounds.top() - px(PANEL_HEADER_HEIGHT));
    let target = edge - point(px(0.), px(100.));
    cx.simulate_mouse_down(edge, MouseButton::Left, Default::default());
    cx.simulate_mouse_move(
        edge - point(px(0.), px(10.)),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_move(target, MouseButton::Left, Default::default());
    cx.simulate_mouse_up(target, MouseButton::Left, Default::default());
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let resized = cx.debug_bounds("plugin-ui-output").unwrap();
    assert!(
        resized.size.height > bounds.size.height + px(60.),
        "dragging the dock edge must enlarge the terminal: {bounds:?} -> {resized:?}"
    );
    cx.update(|_, cx| {
        let height = app.read(cx).session_state.plugin_dock_sizes["bottom"];
        assert!((height - (resized.size.height / px(1.) + PANEL_HEADER_HEIGHT)).abs() < 2.);
    });
    // The same handle must shrink the dock and forward its new viewport to the guest.
    let edge = point(resized.center().x, resized.top() - px(PANEL_HEADER_HEIGHT));
    let target = edge + point(px(0.), px(70.));
    cx.simulate_mouse_down(edge, MouseButton::Left, Default::default());
    cx.simulate_mouse_move(
        edge + point(px(0.), px(10.)),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_move(target, MouseButton::Left, Default::default());
    cx.simulate_mouse_up(target, MouseButton::Left, Default::default());
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let shrunk = cx.debug_bounds("plugin-ui-output").unwrap();
    assert!(shrunk.size.height < resized.size.height - px(40.));
    let messages = cx.update(|_, cx| {
        owner
            .read(cx)
            .worker
            .recorded
            .lock()
            .unwrap()
            .try_iter()
            .collect::<Vec<_>>()
    });
    assert!(messages.iter().any(|work| matches!(work,
        Work::Event(_, _, Some(_), PluginEvent::Ui(protocol::ui::UiEvent {
            action: protocol::ui::Action::Canvas(protocol::ui::CanvasEvent::Resize {height,..}), ..
        })) if (*height - shrunk.size.height / px(1.)).abs() < 2.)));
    // Hiding reclaims editor space; reopening restores the persisted user height.
    cx.update(|_, cx| {
        app.update(cx, |app, cx| {
            app.hide_plugin_panel("terminal", "terminal", cx)
        })
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(cx.debug_bounds("plugin-ui-output").is_none());
    assert!(
        cx.debug_bounds("editor-panel-content").unwrap().size.height >= full_editor.size.height
    );
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            // This layout fixture restores dock geometry, not a fabricated plugin command.
            // Real command admission and guest execution have separate package-driven coverage.
            app.plugin_panels["terminal/terminal"].update(cx, |panel, cx| {
                panel.show(window, cx);
                cx.notify();
            });
            app.dock_area.update(cx, |_, cx| cx.notify());
            cx.notify();
        })
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let restored = cx.debug_bounds("plugin-ui-output").unwrap();
    assert!((restored.size.height - shrunk.size.height).abs() < px(2.));
}
