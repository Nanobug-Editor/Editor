//! Real host-panel drags must commit the split shown by the production drop preview.
use super::*;
use gpui_kit::{MouseButton, Point, VisualTestContext, point};

mod live;

/// Open the real app with the given workspace and isolated session, returning its native test window.
fn open(
    cx: &mut TestAppContext,
    workspace: Workspace,
    session: SessionState,
) -> (Entity<EditorApp>, &mut VisualTestContext) {
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
    (app, visual)
}

/// Resolve Explorer, Outline and Editor again after reopening creates new host entities.
fn panels(app: &Entity<EditorApp>, visual: &mut VisualTestContext) -> [PanelId; 3] {
    visual.update(|_, cx| {
        let app = app.read(cx);
        [
            PanelId::from(app.explorer_panel.entity_id()),
            PanelId::from(app.outline_panel.entity_id()),
            PanelId::from(app.editor_panel.entity_id()),
        ]
    })
}

/// Drag the requested panel to destination through its current title, optionally painting the final preview.
fn drag(
    app: &Entity<EditorApp>,
    panel: PanelId,
    destination: Point<Pixels>,
    preview: bool,
    visual: &mut VisualTestContext,
) -> Option<Bounds<Pixels>> {
    native_panel_bounds(app, panel, visual);
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
        .unwrap()
    });
    let title = visual
        .debug_bounds(Box::leak(
            format!("local-dock-title-{}", node.as_u64()).into_boxed_str(),
        ))
        .unwrap();
    visual.simulate_mouse_down(title.center(), MouseButton::Left, Default::default());
    visual.simulate_mouse_move(
        title.center() + point(px(12.), px(0.)),
        MouseButton::Left,
        Default::default(),
    );
    visual.simulate_mouse_move(destination, MouseButton::Left, Default::default());
    let advertised = preview.then(|| {
        visual.update(|window, cx| window.draw(cx).clear(cx));
        visual
            .debug_bounds("local-dock-drop-indicator")
            .expect("the blue marker must preview the proposed allocation")
    });
    visual.simulate_mouse_up(destination, MouseButton::Left, Default::default());
    advertised
}

/// Right-footer clicks preserve the requested left panels after the sidebar gained another column.
fn check_right(app: &Entity<EditorApp>, ids: &[PanelId], visual: &mut VisualTestContext) {
    let before: Vec<_> = ids
        .iter()
        .map(|id| native_panel_bounds(app, *id, visual))
        .collect();
    for open in [true, false] {
        let button = visual.debug_bounds("host-messages-toggle").unwrap();
        visual.simulate_click(button.center(), Default::default());
        visual.update(|_, cx| assert_eq!(app.read(cx).messages_visible(cx), open));
        for (id, before) in ids.iter().zip(&before) {
            let after = native_panel_bounds(app, *id, visual);
            assert!((after.left() - before.left()).abs() < px(2.));
            assert!(
                (after.size.width - before.size.width).abs() < px(2.),
                "right open={open} resized left: {before:?} -> {after:?}"
            );
        }
    }
}

/// Returning Explorer above Outline must free its custom column instead of widening the old column.
fn rejoin(
    app: &Entity<EditorApp>,
    ids: [PanelId; 3],
    width: Pixels,
    visual: &mut VisualTestContext,
) {
    let outline = native_panel_bounds(app, ids[1], visual);
    drag(
        app,
        ids[0],
        point(outline.center().x, outline.top() + px(8.)),
        false,
        visual,
    );
    let after = ids.map(|id| native_panel_bounds(app, id, visual));
    assert!((after[0].size.width - width).abs() < px(2.));
    assert!((after[1].size.width - width).abs() < px(2.));
    assert!(after[0].bottom() <= after[1].top() + px(2.));
    assert!((after[2].left() - width).abs() < px(2.));
}

/// An own-side title relocates the whole first-level group, or the sole visible member, and persists it.
fn exercise(cx: &mut TestAppContext, outline_visible: bool, preview: bool) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        typography::init(cx);
        apply_theme(builtin_theme(false), cx);
        init(cx);
        cx.set_reduce_motion(true);
    });
    let directory = tempfile::tempdir().unwrap();
    let profile = directory.path().join("profile");
    let workspace = Workspace::open(directory.path()).unwrap();
    let workspace_root = workspace.root().to_owned();
    let mut session = SessionState::load_in_profile(workspace.root(), &profile);
    session.explorer_visible = true;
    session.outline_visible = outline_visible;
    session.messages_visible = false;
    let (app, visual) = open(cx, workspace, session);
    let ids = panels(&app, visual);
    let before = native_panel_bounds(&app, ids[0], visual);
    let target = native_panel_bounds(&app, ids[2], visual);
    let advertised = drag(
        &app,
        ids[0],
        point(target.left() + px(10.), target.center().y),
        preview,
        visual,
    );
    let moved = native_panel_bounds(&app, ids[0], visual);
    visual.update(|_, cx| {
        let area = app.read(cx).dock_area.read(cx);
        let center = area.layout(DockPlacement::Center).unwrap();
        assert!(center.find_panel_node(ids[0]).is_some());
        if outline_visible {
            assert!(center.find_panel_node(ids[1]).is_some());
        }
        assert!(area.is_empty(DockPlacement::Left, cx));
    });
    let expected = advertised.map_or(before.size.width, |bounds| bounds.size.width);
    assert!(
        (moved.size.width - expected).abs() < px(2.),
        "accepted drop must match the previewed width: expected={expected:?}, Explorer={moved:?}"
    );
    assert!(moved.right() <= native_panel_bounds(&app, ids[2], visual).left() + px(2.));
    let mut left = vec![ids[0]];
    if outline_visible {
        let outline = native_panel_bounds(&app, ids[1], visual);
        assert!((moved.left() - outline.left()).abs() < px(2.));
        assert!(moved.bottom() <= outline.top() + px(2.));
        assert!((outline.size.width - before.size.width).abs() < px(2.));
        left.push(ids[1]);
    }
    check_right(&app, &left, visual);
    // Exercise disk persistence through a fresh app, rather than only comparing an in-memory dump.
    visual.update(|_, cx| {
        app.update(cx, |app, cx| {
            app.capture_dock_layout(cx);
            app.session_state.try_save().unwrap();
        })
    });
    let session = SessionState::load_in_profile(&workspace_root, &profile);
    assert!(session.dock_layout.is_some());
    let (reopened, visual) = open(cx, Workspace::open(&workspace_root).unwrap(), session);
    let ids = panels(&reopened, visual);
    let restored = native_panel_bounds(&reopened, ids[0], visual);
    assert!((restored.left() - moved.left()).abs() < px(2.));
    assert!(
        (restored.size.width - moved.size.width).abs() < px(2.),
        "saved column pixels must restore before the initial viewport reflows: {moved:?} -> {restored:?}"
    );
    if outline_visible {
        let outline = native_panel_bounds(&reopened, ids[1], visual);
        assert!((outline.left() - restored.left()).abs() < px(2.));
        assert!((outline.size.width - restored.size.width).abs() < px(2.));
    } else {
        // A hidden peer remains at the former native sidebar; revealing it creates a real return target.
        let toggle = visual.debug_bounds("outline-toggle").unwrap();
        visual.simulate_click(toggle.center(), Default::default());
        rejoin(&reopened, ids, before.size.width, visual);
    }
}

/// Explorer carries its visible first-level peer rather than creating a second left region.
#[gpui::test]
fn explorer_drop_on_editor_left_moves_its_whole_group(cx: &mut TestAppContext) {
    exercise(cx, true, true);
}

/// An empty former source column must not leave reserved pixels after the move or restart.
#[gpui::test]
fn explorer_drop_on_editor_left_without_visible_peers(cx: &mut TestAppContext) {
    exercise(cx, false, true);
}

/// Immediate release computes the own-side moving group without needing a preview frame.
#[gpui::test]
fn explorer_drop_on_editor_left_before_preview_frame(cx: &mut TestAppContext) {
    exercise(cx, true, false);
}

/// A drop beside Editor belongs to its upper row; the bottom panel spans both resulting columns.
#[gpui::test]
fn editor_left_drop_keeps_bottom_across_both_columns(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        typography::init(cx);
        apply_theme(builtin_theme(false), cx);
        init(cx);
        cx.set_reduce_motion(true);
    });
    let directory = tempfile::tempdir().unwrap();
    let workspace = Workspace::open(directory.path()).unwrap();
    let root = workspace.root().to_owned();
    let profile = directory.path().join("profile");
    let mut session = SessionState::load_in_profile(workspace.root(), &profile);
    session.explorer_visible = true;
    session.outline_visible = false;
    session.messages_visible = false;
    // This ordinary bottom leaf uses the same regional geometry as a plugin's Git window.
    session.problems_visible = true;
    let (app, visual) = open(cx, workspace, session);
    let [explorer, _, editor] = panels(&app, visual);
    let bottom = visual.update(|_, cx| PanelId::from(app.read(cx).problems.panel.entity_id()));
    let target = native_panel_bounds(&app, editor, visual);
    let before_bottom = native_panel_bounds(&app, bottom, visual);
    assert!(before_bottom.top() > target.top() && before_bottom.size.height > px(80.));
    let preview = drag(
        &app,
        explorer,
        point(target.left() + px(10.), target.center().y),
        true,
        visual,
    )
    .unwrap();
    let ids = [explorer, editor, bottom];
    let placed = row_geometry(&app, ids, visual);
    assert!((placed[0].size.width - preview.size.width).abs() < px(2.));
    // Resize the real row divider so restore and right-side reveals also cover user pixel choices.
    let edge = point(placed[0].right() - px(1.), placed[0].center().y);
    visual.simulate_mouse_down(edge, MouseButton::Left, Default::default());
    visual.simulate_mouse_move(
        edge + point(px(12.), px(0.)),
        MouseButton::Left,
        Default::default(),
    );
    visual.simulate_mouse_move(
        edge + point(px(40.), px(0.)),
        MouseButton::Left,
        Default::default(),
    );
    visual.simulate_mouse_up(
        edge + point(px(40.), px(0.)),
        MouseButton::Left,
        Default::default(),
    );
    let resized = row_geometry(&app, ids, visual);
    assert!(resized[0].size.width > placed[0].size.width + px(30.));
    for open in [true, false] {
        let button = visual.debug_bounds("host-messages-toggle").unwrap();
        visual.simulate_click(button.center(), Default::default());
        let after = row_geometry(&app, ids, visual);
        assert!(
            (after[0].size.width - resized[0].size.width).abs() < px(2.),
            "right open={open} changed the row's left column: {resized:?} -> {after:?}"
        );
        if open {
            assert!(after[1].size.width < resized[1].size.width - px(200.));
        }
    }
    visual.simulate_resize(size(px(1460.), px(900.)));
    let larger = row_geometry(&app, ids, visual);
    assert!((larger[0].size.width - resized[0].size.width).abs() < px(2.));
    visual.update(|_, cx| {
        app.update(cx, |app, cx| {
            app.capture_dock_layout(cx);
            app.session_state.try_save().unwrap();
        })
    });
    let session = SessionState::load_in_profile(&root, &profile);
    assert_eq!(
        session.plugin_sidebar_layout_version,
        session_state::DOCK_LAYOUT_VERSION
    );
    let (reopened, visual) = open(cx, Workspace::open(&root).unwrap(), session);
    let ids = visual.update(|_, cx| {
        let app = reopened.read(cx);
        [
            PanelId::from(app.explorer_panel.entity_id()),
            PanelId::from(app.editor_panel.entity_id()),
            PanelId::from(app.problems.panel.entity_id()),
        ]
    });
    let restored = row_geometry(&reopened, ids, visual);
    assert!(
        (restored[0].size.width - resized[0].size.width).abs() < px(2.),
        "row pixel preference must survive a different initial viewport: {resized:?} -> {restored:?}"
    );
}

/// Read the actual upper row and bottom allocation for the requested Explorer, Editor and bottom IDs.
fn row_geometry(
    app: &Entity<EditorApp>,
    ids: [PanelId; 3],
    visual: &mut VisualTestContext,
) -> [Bounds<Pixels>; 3] {
    let [moved, editor, bottom] = ids.map(|id| native_panel_bounds(app, id, visual));
    assert!(
        (moved.bottom() - editor.bottom()).abs() < px(2.),
        "the moved panel must end at Editor's row: Explorer={moved:?}, Editor={editor:?}, bottom={bottom:?}"
    );
    assert!((bottom.left() - moved.left()).abs() < px(2.));
    assert!((bottom.right() - editor.right()).abs() < px(2.));
    assert!(moved.bottom() <= bottom.top());
    [moved, editor, bottom]
}
