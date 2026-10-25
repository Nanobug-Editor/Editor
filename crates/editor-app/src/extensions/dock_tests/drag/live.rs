//! Native input verifies marker-only dragging and one final placement with actual pre-drag dimensions.
use super::*;

mod exclusive;
mod grouped;
mod nested;
mod one_side;
mod regions;
mod resize;

/// An isolated real editor starts with the illustrated left group and an ordinary bottom window.
fn setup(
    cx: &mut TestAppContext,
    outline: bool,
) -> (tempfile::TempDir, Entity<EditorApp>, &mut VisualTestContext) {
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
    session.outline_visible = outline;
    session.messages_visible = false;
    session.problems_visible = true;
    let (app, visual) = open(cx, workspace, session);
    ordinary_bottom(&app, visual);
    ordinary_right(&app, visual);
    (directory, app, visual)
}

/// Keep the regional drag fixture's Messages in a native Right dock after the product default
/// has placed that hidden panel in the center's right column.
fn ordinary_right(app: &Entity<EditorApp>, visual: &mut VisualTestContext) {
    visual.update(|window, cx| {
        app.update(cx, |app, cx| {
            let editor = app.dock_presentation.panel_handle(app.editor_panel.clone());
            let messages = app.dock_presentation.panel_handle(app.messages.clone());
            app.dock_area.update(cx, |area, cx| {
                area.set_center(
                    gpui_base::dock::DockLayout::tabs().panel_view(editor, cx),
                    window,
                    cx,
                );
                area.set_dock(
                    DockPlacement::Right,
                    gpui_base::dock::DockLayout::tabs().panel_view(messages, cx),
                    window,
                    cx,
                );
                area.set_dock_size(DockPlacement::Right, px(360.), window, cx);
                if area.is_dock_open(DockPlacement::Right) {
                    area.toggle_dock(DockPlacement::Right, window, cx);
                }
            });
        });
    });
}

/// Resolve the current title and start Base's actual drag payload through native input.
fn begin(app: &Entity<EditorApp>, id: PanelId, visual: &mut VisualTestContext) {
    native_panel_bounds(app, id, visual);
    let node = visual.update(|_, cx| {
        let area = app.read(cx).dock_area.read(cx);
        [
            DockPlacement::Center,
            DockPlacement::Left,
            DockPlacement::Right,
            DockPlacement::Bottom,
        ]
        .into_iter()
        .find_map(|placement| area.layout(placement)?.find_panel_node(id))
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
}

/// Read the complete original allocation, including its title rather than only content.
fn outer(app: &Entity<EditorApp>, id: PanelId, visual: &mut VisualTestContext) -> Bounds<Pixels> {
    let content = native_panel_bounds(app, id, visual);
    let node = visual.update(|_, cx| {
        let area = app.read(cx).dock_area.read(cx);
        [
            DockPlacement::Center,
            DockPlacement::Left,
            DockPlacement::Right,
            DockPlacement::Bottom,
        ]
        .into_iter()
        .find_map(|placement| area.layout(placement)?.find_panel_node(id))
        .unwrap()
    });
    let title = visual
        .debug_bounds(Box::leak(
            format!("local-dock-title-{}", node.as_u64()).into_boxed_str(),
        ))
        .unwrap_or(content);
    Bounds::new(
        point(content.left(), title.top()),
        size(content.size.width, content.bottom() - title.top()),
    )
}

/// All four directions use the target's real rectangle; its middle remains an invalid merge zone.
fn edge(bounds: Bounds<Pixels>, edge: gpui_base::Placement) -> Point<Pixels> {
    match edge {
        gpui_base::Placement::Left => point(bounds.left() + px(8.), bounds.center().y),
        gpui_base::Placement::Right => point(bounds.right() - px(8.), bounds.center().y),
        gpui_base::Placement::Top => point(bounds.center().x, bounds.top() + px(8.)),
        gpui_base::Placement::Bottom => point(bounds.center().x, bounds.bottom() - px(8.)),
    }
}

/// Paint a gesture frame without changing any native allocations.
fn hover(pointer: Point<Pixels>, visual: &mut VisualTestContext) {
    visual.simulate_mouse_move(pointer, MouseButton::Left, Default::default());
    visual.update(|window, cx| window.draw(cx).clear(cx));
}

/// Resolve a named visual marker after the current frame.
fn marked(visual: &mut VisualTestContext, name: &'static str) -> Bounds<Pixels> {
    visual
        .debug_bounds(name)
        .unwrap_or_else(|| panic!("missing {name}"))
}

/// A gesture keeps both source and neighbor allocations unchanged, even outside the dock.
#[gpui::test]
fn marked_drag_keeps_original_bounds_until_release(cx: &mut TestAppContext) {
    let (_directory, app, visual) = setup(cx, true);
    let [explorer, outline, editor] = panels(&app, visual);
    let bottom = visual.update(|_, cx| PanelId::from(app.read(cx).problems.panel.entity_id()));
    let ids = [explorer, outline, editor, bottom];
    let before = ids.map(|id| outer(&app, id, visual));
    begin(&app, explorer, visual);
    hover(point(px(-30.), px(160.)), visual);
    assert_eq!(ids.map(|id| outer(&app, id, visual)), before);
    assert_eq!(marked(visual, "local-dock-source-0"), before[0]);
    assert!(visual.debug_bounds("local-dock-drop-indicator").is_none());
    let cards = marked(visual, "local-dock-floating-preview");
    assert!(cards.size.width < px(200.) && cards.size.height <= px(PANEL_HEADER_HEIGHT));
    visual.simulate_keystrokes("escape");
    assert_eq!(ids.map(|id| outer(&app, id, visual)), before);
}

/// A free editor edge marks only the grabbed child; cancellation does not persist any gesture state.
#[gpui::test]
fn marked_drag_editor_child_keeps_geometry_and_cancel_does_not_save(cx: &mut TestAppContext) {
    let (_directory, app, visual) = setup(cx, true);
    let [explorer, outline, editor] = panels(&app, visual);
    let bottom = visual.update(|_, cx| PanelId::from(app.read(cx).problems.panel.entity_id()));
    let ids = [explorer, outline, editor, bottom];
    let before = ids.map(|id| outer(&app, id, visual));
    let saved = visual.update(|_, cx| app.read(cx).session_state.dock_layout.clone());
    begin(&app, outline, visual);
    let pointer = edge(before[2], gpui_base::Placement::Right);
    hover(pointer, visual);
    assert_eq!(ids.map(|id| outer(&app, id, visual)), before);
    assert_eq!(marked(visual, "local-dock-source-0"), before[1]);
    assert!(visual.debug_bounds("local-dock-source-1").is_none());
    let destination = marked(visual, "local-dock-drop-indicator");
    assert!((destination.size.width - before[0].size.width).abs() < px(2.));
    assert_eq!(destination.size.height, before[2].size.height);
    assert_eq!(marked(visual, "local-dock-target-0"), destination);
    assert!(visual.debug_bounds("local-dock-target-1").is_none());
    let card = marked(visual, "local-dock-drag-card-0");
    assert!(card.contains(&pointer));
    assert!(visual.debug_bounds("local-dock-drag-card-1").is_none());
    visual.update(|_, cx| {
        app.update(cx, |app, cx| {
            app.capture_dock_layout(cx);
            assert_eq!(app.session_state.dock_layout, saved);
        })
    });
    visual.simulate_keystrokes("escape");
    assert!(visual.debug_bounds("local-dock-drop-indicator").is_none());
    assert_eq!(ids.map(|id| outer(&app, id, visual)), before);
    visual.update(|_, cx| assert!(!app.read(cx).dock_ui.is_dragging() && !cx.has_active_drag()));
}

/// Invalid centers and outside release clear the destination without ever changing the original tree.
#[gpui::test]
fn marked_drag_invalid_target_and_release_leave_layout_unchanged(cx: &mut TestAppContext) {
    let (_directory, app, visual) = setup(cx, true);
    let [explorer, outline, editor] = panels(&app, visual);
    let ids = [explorer, outline, editor];
    let before = ids.map(|id| outer(&app, id, visual));
    begin(&app, explorer, visual);
    hover(edge(before[2], gpui_base::Placement::Right), visual);
    assert!(visual.debug_bounds("local-dock-target-0").is_some());
    assert!(visual.debug_bounds("local-dock-target-1").is_none());
    hover(before[2].center(), visual);
    assert!(visual.debug_bounds("local-dock-drop-indicator").is_none());
    assert!(visual.debug_bounds("local-dock-source-1").is_none());
    let outside = point(px(-40.), px(200.));
    hover(outside, visual);
    visual.simulate_mouse_up(outside, MouseButton::Left, Default::default());
    assert_eq!(ids.map(|id| outer(&app, id, visual)), before);
    visual.update(|_, cx| assert!(!app.read(cx).dock_ui.is_dragging()));
}

/// Bottom tool targets allow only horizontal placement; rejected edges preserve every allocation.
#[gpui::test]
fn marked_drag_window_edges_move_only_one_panel(cx: &mut TestAppContext) {
    for placement in [
        gpui_base::Placement::Left,
        gpui_base::Placement::Right,
        gpui_base::Placement::Top,
        gpui_base::Placement::Bottom,
    ] {
        let (_directory, app, visual) = setup(cx, true);
        let [explorer, outline, editor] = panels(&app, visual);
        let bottom = visual.update(|_, cx| PanelId::from(app.read(cx).problems.panel.entity_id()));
        let ids = [explorer, outline, editor, bottom];
        let before = ids.map(|id| outer(&app, id, visual));
        begin(&app, explorer, visual);
        let pointer = edge(before[3], placement);
        hover(pointer, visual);
        assert_eq!(ids.map(|id| outer(&app, id, visual)), before);
        assert!(visual.debug_bounds("local-dock-source-1").is_none());
        assert!(visual.debug_bounds("local-dock-target-1").is_none());
        if matches!(
            placement,
            gpui_base::Placement::Top | gpui_base::Placement::Bottom
        ) {
            assert!(visual.debug_bounds("local-dock-drop-indicator").is_none());
            visual.simulate_mouse_up(pointer, MouseButton::Left, Default::default());
            assert_eq!(ids.map(|id| outer(&app, id, visual)), before);
            continue;
        }
        visual.simulate_mouse_up(pointer, MouseButton::Left, Default::default());
        let source = outer(&app, explorer, visual);
        let destination = outer(&app, bottom, visual);
        match placement {
            gpui_base::Placement::Left => assert!(source.right() <= destination.left() + px(2.)),
            gpui_base::Placement::Right => assert!(destination.right() <= source.left() + px(2.)),
            gpui_base::Placement::Top => assert!(source.bottom() <= destination.top() + px(2.)),
            gpui_base::Placement::Bottom => assert!(destination.bottom() <= source.top() + px(2.)),
        }
        visual.update(|_, cx| {
            let area = app.read(cx).dock_area.read(cx);
            let left = area.layout(DockPlacement::Left).unwrap();
            assert!(
                left.find_panel_node(outline).is_some() && left.find_panel_node(explorer).is_none()
            );
            assert!(
                area.layout(DockPlacement::Bottom)
                    .unwrap()
                    .find_panel_node(explorer)
                    .is_some()
            );
        });
    }
}

/// A bottom panel taller than half the editor retains its actual height when it still fits the whole target.
#[gpui::test]
fn marked_drag_bottom_keeps_actual_height_when_committed_above_editor(cx: &mut TestAppContext) {
    let (_directory, app, visual) = setup(cx, true);
    let [explorer, outline, editor] = panels(&app, visual);
    let bottom = visual.update(|_, cx| PanelId::from(app.read(cx).problems.panel.entity_id()));
    let ids = [explorer, outline, editor, bottom];
    let before = ids.map(|id| outer(&app, id, visual));
    assert!(before[3].size.height > before[2].size.height / 2.);
    begin(&app, bottom, visual);
    let pointer = edge(before[2], gpui_base::Placement::Top);
    hover(pointer, visual);
    assert_eq!(ids.map(|id| outer(&app, id, visual)), before);
    assert_eq!(
        marked(visual, "local-dock-drop-indicator").size.height,
        before[3].size.height
    );
    let card = marked(visual, "local-dock-drag-card-0");
    assert!(card.size.width < px(200.) && card.contains(&pointer));
    visual.simulate_mouse_up(pointer, MouseButton::Left, Default::default());
    let placed = outer(&app, bottom, visual);
    assert!(
        (placed.size.height - before[3].size.height).abs() < px(2.),
        "{before:?} -> {placed:?}"
    );
    assert!(placed.bottom() <= outer(&app, editor, visual).top() + px(2.));
    assert_eq!(outer(&app, explorer, visual), before[0]);
    assert_eq!(outer(&app, outline, visual), before[1]);
}

/// The side preview uses actual width above half; reaching the full target switches to half.
#[gpui::test]
fn marked_drag_width_uses_pre_drag_pixels_and_only_oversize_falls_back(cx: &mut TestAppContext) {
    for requested in [200., 500., 900.] {
        let (_directory, app, visual) = setup(cx, false);
        visual.update(|window, cx| {
            let area = app.read(cx).dock_area.clone();
            area.update(cx, |area, cx| {
                area.set_dock_size(DockPlacement::Left, px(requested), window, cx)
            });
        });
        let [explorer, _, editor] = panels(&app, visual);
        let source = outer(&app, explorer, visual);
        let target = outer(&app, editor, visual);
        begin(&app, explorer, visual);
        let pointer = edge(target, gpui_base::Placement::Left);
        hover(pointer, visual);
        let expected = if source.size.width >= target.size.width {
            target.size.width / 2.
        } else {
            source.size.width
        };
        let preview = marked(visual, "local-dock-drop-indicator");
        assert!(
            (preview.size.width - expected).abs() < px(2.),
            "{source:?}, {target:?} -> {preview:?}"
        );
        assert_eq!(outer(&app, explorer, visual), source);
        assert_eq!(outer(&app, editor, visual), target);
        visual.simulate_mouse_up(pointer, MouseButton::Left, Default::default());
        assert!((outer(&app, explorer, visual).size.width - expected).abs() < px(2.));
    }
}

/// Moving the right dock retains its pre-drag width and never changes the left column.
#[gpui::test]
fn marked_drag_right_source_preserves_left_before_and_after_release(cx: &mut TestAppContext) {
    let (_directory, app, visual) = setup(cx, true);
    visual.update(|window, cx| app.update(cx, |app, cx| app.toggle_messages(window, cx)));
    let [explorer, _, editor] = panels(&app, visual);
    let right = visual.update(|_, cx| PanelId::from(app.read(cx).messages.entity_id()));
    let left_before = outer(&app, explorer, visual);
    let editor_before = outer(&app, editor, visual);
    let right_before = outer(&app, right, visual);
    begin(&app, right, visual);
    let pointer = edge(editor_before, gpui_base::Placement::Right);
    hover(pointer, visual);
    assert_eq!(outer(&app, explorer, visual), left_before);
    assert_eq!(outer(&app, editor, visual), editor_before);
    assert_eq!(outer(&app, right, visual), right_before);
    assert!(
        (marked(visual, "local-dock-drop-indicator").size.width - right_before.size.width).abs()
            < px(2.)
    );
    visual.simulate_mouse_up(pointer, MouseButton::Left, Default::default());
    assert_eq!(outer(&app, explorer, visual), left_before);
    assert!((outer(&app, right, visual).size.width - right_before.size.width).abs() < px(2.));
}

/// Free editor edges accept one child, the own left side carries the group, and unrelated Bottom blocks.
#[gpui::test]
fn marked_drag_editor_edges_respect_occupied_sides(cx: &mut TestAppContext) {
    for placement in [
        gpui_base::Placement::Left,
        gpui_base::Placement::Right,
        gpui_base::Placement::Top,
        gpui_base::Placement::Bottom,
    ] {
        let (_directory, app, visual) = setup(cx, true);
        let [explorer, outline, editor] = panels(&app, visual);
        let before = [explorer, outline, editor].map(|id| outer(&app, id, visual));
        let target = before[2];
        begin(&app, explorer, visual);
        let pointer = edge(target, placement);
        hover(pointer, visual);
        if placement == gpui_base::Placement::Bottom {
            assert!(visual.debug_bounds("local-dock-drop-indicator").is_none());
            visual.simulate_mouse_up(pointer, MouseButton::Left, Default::default());
            assert_eq!(
                [explorer, outline, editor].map(|id| outer(&app, id, visual)),
                before
            );
            continue;
        }
        if placement == gpui_base::Placement::Left {
            assert_eq!(marked(visual, "local-dock-source-0"), before[0]);
            assert_eq!(marked(visual, "local-dock-source-1"), before[1]);
            assert!(visual.debug_bounds("local-dock-target-1").is_some());
            assert!(visual.debug_bounds("local-dock-drag-card-1").is_some());
            assert_eq!(
                [explorer, outline, editor].map(|id| outer(&app, id, visual)),
                before
            );
            visual.simulate_mouse_up(pointer, MouseButton::Left, Default::default());
            let after = [explorer, outline, editor].map(|id| outer(&app, id, visual));
            assert!((after[0].size.width - before[0].size.width).abs() < px(2.));
            assert_eq!(after[0].left(), after[1].left());
            assert!(after[0].bottom() <= after[1].top() + px(2.));
            assert!((after[1].bottom() - after[2].bottom()).abs() < px(2.));
            visual.update(|_, cx| {
                assert!(
                    app.read(cx)
                        .dock_area
                        .read(cx)
                        .is_empty(DockPlacement::Left, cx)
                )
            });
            continue;
        }
        assert!(visual.debug_bounds("local-dock-target-1").is_none());
        visual.simulate_mouse_up(pointer, MouseButton::Left, Default::default());
        let source = outer(&app, explorer, visual);
        let peer = outer(&app, outline, visual);
        let editor = outer(&app, editor, visual);
        assert!((peer.left() - before[1].left()).abs() < px(2.));
        assert!((peer.size.width - before[1].size.width).abs() < px(2.));
        match placement {
            gpui_base::Placement::Left => assert!(source.right() <= editor.left() + px(2.)),
            gpui_base::Placement::Right => assert!(editor.right() <= source.left() + px(2.)),
            gpui_base::Placement::Top => assert!(source.bottom() <= editor.top() + px(2.)),
            gpui_base::Placement::Bottom => assert!(editor.bottom() <= source.top() + px(2.)),
        }
        if matches!(
            placement,
            gpui_base::Placement::Left | gpui_base::Placement::Right
        ) {
            assert!((source.size.width - before[0].size.width).abs() < px(2.));
        }
        visual.update(|_, cx| {
            assert!(
                !app.read(cx)
                    .dock_area
                    .read(cx)
                    .is_empty(DockPlacement::Left, cx)
            )
        });
    }
}

/// Switching from a free edge to the own side promotes the group; a concrete tool target restores one member.
#[gpui::test]
fn marked_drag_switches_targets_and_moving_scope(cx: &mut TestAppContext) {
    let (_directory, app, visual) = setup(cx, true);
    let [explorer, outline, editor] = panels(&app, visual);
    let bottom = visual.update(|_, cx| PanelId::from(app.read(cx).problems.panel.entity_id()));
    let ids = [explorer, outline, editor, bottom];
    let before = ids.map(|id| outer(&app, id, visual));
    begin(&app, explorer, visual);
    hover(edge(before[2], gpui_base::Placement::Right), visual);
    assert!(visual.debug_bounds("local-dock-source-1").is_none());
    assert!(visual.debug_bounds("local-dock-drag-card-1").is_none());
    hover(edge(before[2], gpui_base::Placement::Left), visual);
    assert!(visual.debug_bounds("local-dock-drop-indicator").is_some());
    assert!(visual.debug_bounds("local-dock-source-1").is_some());
    assert!(visual.debug_bounds("local-dock-target-1").is_some());
    assert!(visual.debug_bounds("local-dock-drag-card-1").is_some());
    let pointer = edge(before[3], gpui_base::Placement::Left);
    hover(pointer, visual);
    assert!(visual.debug_bounds("local-dock-source-1").is_none());
    assert!(visual.debug_bounds("local-dock-target-1").is_none());
    assert!(visual.debug_bounds("local-dock-drag-card-1").is_none());
    assert_eq!(ids.map(|id| outer(&app, id, visual)), before);
    visual.simulate_mouse_up(pointer, MouseButton::Left, Default::default());
    visual.update(|_, cx| {
        let area = app.read(cx).dock_area.read(cx);
        assert!(
            area.layout(DockPlacement::Left)
                .unwrap()
                .find_panel_node(outline)
                .is_some()
        );
        assert!(
            area.layout(DockPlacement::Bottom)
                .unwrap()
                .find_panel_node(explorer)
                .is_some()
        );
        assert!(
            area.layout(DockPlacement::Bottom)
                .unwrap()
                .find_panel_node(outline)
                .is_none()
        );
    });
}
