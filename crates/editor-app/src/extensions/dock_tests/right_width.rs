//! Painted geometry verifies that right-side visibility changes consume only editor space.
use super::*;

/// Real footer clicks preserve a user-sized sidebar across opening, closing and viewport resizing.
fn exercise(cx: &mut TestAppContext, legacy: bool) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        typography::init(cx);
        apply_theme(builtin_theme(false), cx);
        init(cx);
        cx.set_reduce_motion(true);
    });
    let directory = tempfile::tempdir().unwrap();
    let workspace = Workspace::open(directory.path()).unwrap();
    let mut session =
        SessionState::load_in_profile(workspace.root(), &directory.path().join("profile"));
    session.explorer_visible = true;
    session.outline_visible = true;
    session.messages_visible = false;
    session.explorer_width = 297.;
    if legacy {
        session.plugin_sidebar_layout_version = 1;
        use gpui_base::dock::{DockAreaState, PanelInfo, PanelState};
        let leaf = |name: &str| PanelState {
            panel_name: "TabPanel".into(),
            info: PanelInfo::tabs(0),
            children: vec![PanelState::new(name)],
        };
        let sidebar = PanelState {
            panel_name: "StackPanel".into(),
            info: PanelInfo::stack(vec![px(420.), px(180.)], gpui_kit::Axis::Vertical),
            children: vec![leaf("Explorer"), leaf("Outline")],
        };
        session.dock_layout = Some(DockAreaState {
            center: PanelState {
                panel_name: "StackPanel".into(),
                info: PanelInfo::stack(vec![px(297.), px(983.)], gpui_kit::Axis::Horizontal),
                children: vec![sidebar, leaf("Editor")],
            },
            ..Default::default()
        });
    }
    let slot = Rc::new(RefCell::new(None));
    let capture = slot.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let app =
            cx.new(|cx| EditorApp::new_with_session(workspace, None, Some(session), window, cx));
        *capture.borrow_mut() = Some(app.clone());
        Root::new(app, window, cx)
    });
    let app = slot.borrow_mut().take().unwrap();
    visual.simulate_resize(size(px(1280.), px(820.)));
    let ids = visual.update(|_, cx| {
        let app = app.read(cx);
        [
            PanelId::from(app.explorer_panel.entity_id()),
            PanelId::from(app.outline_panel.entity_id()),
            PanelId::from(app.editor_panel.entity_id()),
        ]
    });
    let initial = native_panel_bounds(&app, ids[0], visual);
    // Use the actual native resize handle; saved preferences alone do not verify a user's drag.
    let handle = gpui_kit::point(initial.right() - px(1.), initial.center().y);
    visual.simulate_mouse_down(handle, gpui_kit::MouseButton::Left, Default::default());
    visual.simulate_mouse_move(
        handle + gpui_kit::point(px(12.), px(0.)),
        gpui_kit::MouseButton::Left,
        Default::default(),
    );
    visual.simulate_mouse_move(
        handle + gpui_kit::point(px(40.), px(0.)),
        gpui_kit::MouseButton::Left,
        Default::default(),
    );
    visual.simulate_mouse_up(
        handle + gpui_kit::point(px(40.), px(0.)),
        gpui_kit::MouseButton::Left,
        Default::default(),
    );
    let before = ids.map(|id| native_panel_bounds(&app, id, visual));
    assert!(
        before[0].size.width > initial.size.width + px(30.),
        "native sidebar resize must take effect"
    );
    for open in [true, false, true] {
        let button = visual.debug_bounds("host-messages-toggle").unwrap();
        visual.simulate_click(button.center(), Default::default());
        let after = ids.map(|id| native_panel_bounds(&app, id, visual));
        for index in [0, 1] {
            assert!(
                (after[index].size.width - before[index].size.width).abs() < px(2.),
                "right open={open} changed left width: {:?} -> {:?}",
                before[index],
                after[index]
            );
            assert!((after[index].left() - before[index].left()).abs() < px(2.));
            assert!((after[index].size.height - before[index].size.height).abs() < px(2.));
        }
        assert!((after[2].left() - before[2].left()).abs() < px(2.));
        if open {
            assert!(after[2].size.width < before[2].size.width - px(200.));
        } else {
            assert!((after[2].size.width - before[2].size.width).abs() < px(2.));
        }
    }
    visual.simulate_resize(size(px(1460.), px(900.)));
    let expanded = ids.map(|id| native_panel_bounds(&app, id, visual));
    for index in [0, 1] {
        assert!((expanded[index].size.width - before[index].size.width).abs() < px(2.));
    }
    assert!(expanded[2].size.width > px(600.));
    // Hiding both host peers closes the region without discarding either saved leaf.
    for selector in ["explorer-panel-toggle", "outline-toggle"] {
        let button = visual.debug_bounds(selector).unwrap();
        visual.simulate_click(button.center(), Default::default());
        visual.run_until_parked();
        visual.update(|window, cx| window.draw(cx).clear(cx));
    }
    visual.update(|_, cx| {
        let area = app.read(cx).dock_area.read(cx);
        assert!(!area.is_dock_open(DockPlacement::Left));
        for id in [ids[0], ids[1]] {
            assert!(
                area.layout(DockPlacement::Left)
                    .unwrap()
                    .find_panel_node(id)
                    .is_some()
            );
        }
    });
    for selector in ["outline-toggle", "explorer-panel-toggle"] {
        let button = visual.debug_bounds(selector).unwrap();
        visual.simulate_click(button.center(), Default::default());
        visual.run_until_parked();
        visual.update(|window, cx| window.draw(cx).clear(cx));
    }
    for id in [ids[0], ids[1]] {
        let reopened = native_panel_bounds(&app, id, visual);
        assert!((reopened.size.width - before[0].size.width).abs() < px(2.));
    }
}

/// New sessions reserve an independent left width before any panel has been resized.
#[gpui::test]
fn right_dock_preserves_left_width(cx: &mut TestAppContext) {
    exercise(cx, false);
}

/// Existing center-tree sidebars must be converted before the first right-side reveal.
#[gpui::test]
fn right_dock_preserves_left_width_in_legacy_session(cx: &mut TestAppContext) {
    exercise(cx, true);
}
