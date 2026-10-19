//! Free edges move one window; own-side first-level groups move together without creating another region.
use super::*;
use gpui_base::{Placement, dock::InsertTarget};

/// Construct center-local bands through Base's public edits; the tested move still uses real titles.
pub(super) fn seed_center(
    app: &Entity<EditorApp>,
    moves: &[(PanelId, Placement)],
    visual: &mut VisualTestContext,
) {
    let editor = panels(app, visual)[2];
    visual.update(|window, cx| {
        let area = app.read(cx).dock_area.clone();
        area.update(cx, |area, cx| {
            for (id, placement) in moves {
                let node = area
                    .layout(DockPlacement::Center)
                    .unwrap()
                    .find_panel_node(editor)
                    .unwrap();
                area.move_panel(
                    *id,
                    InsertTarget::Split {
                        node,
                        placement: *placement,
                        size: Some(px(280.)),
                    },
                    window,
                    cx,
                );
            }
        });
    });
}

/// Close the ordinary bottom pane through its actual UI so it no longer occupies an editor side.
pub(super) fn close_bottom(visual: &mut VisualTestContext) {
    visual.update(|window, cx| window.draw(cx).clear(cx));
    let close = visual.debug_bounds("problem-close").unwrap();
    visual.simulate_click(close.center(), Default::default());
}

/// A free right side accepts only the grabbed child; its left siblings keep their original region.
#[gpui::test]
fn child_drag_to_free_right_keeps_its_siblings(cx: &mut TestAppContext) {
    let (_directory, app, visual) = setup(cx, true);
    let [explorer, outline, editor] = panels(&app, visual);
    let before = [explorer, outline, editor].map(|id| outer(&app, id, visual));
    begin(&app, outline, visual);
    let pointer = edge(before[2], gpui_base::Placement::Right);
    hover(pointer, visual);
    assert_eq!(marked(visual, "local-dock-source-0"), before[1]);
    assert!(visual.debug_bounds("local-dock-source-1").is_none());
    assert!(visual.debug_bounds("local-dock-target-1").is_none());
    assert!(visual.debug_bounds("local-dock-drag-card-1").is_none());
    visual.simulate_mouse_up(pointer, MouseButton::Left, Default::default());
    let kept = outer(&app, explorer, visual);
    let moved = outer(&app, outline, visual);
    let editor = outer(&app, editor, visual);
    assert!((kept.left() - before[0].left()).abs() < px(2.));
    assert!((kept.size.width - before[0].size.width).abs() < px(2.));
    assert!(editor.right() <= moved.left() + px(2.));
    visual.update(|_, cx| {
        let area = app.read(cx).dock_area.read(cx);
        assert!(
            area.layout(DockPlacement::Left)
                .unwrap()
                .find_panel_node(explorer)
                .is_some()
        );
        assert!(
            area.layout(DockPlacement::Left)
                .unwrap()
                .find_panel_node(outline)
                .is_none()
        );
    });
}

/// A first-level member carries its remaining left sibling when relocating that occupied editor side.
#[gpui::test]
fn first_level_drag_to_own_left_moves_its_whole_group(cx: &mut TestAppContext) {
    let (_directory, app, visual) = setup(cx, true);
    let [explorer, outline, editor] = panels(&app, visual);
    let ids = [explorer, outline, editor];
    let before = ids.map(|id| outer(&app, id, visual));
    begin(&app, outline, visual);
    let pointer = edge(before[2], gpui_base::Placement::Left);
    hover(pointer, visual);
    assert!(visual.debug_bounds("local-dock-drop-indicator").is_some());
    assert_eq!(marked(visual, "local-dock-source-0"), before[0]);
    assert_eq!(marked(visual, "local-dock-source-1"), before[1]);
    assert!(visual.debug_bounds("local-dock-target-1").is_some());
    assert!(visual.debug_bounds("local-dock-drag-card-1").is_some());
    assert_eq!(ids.map(|id| outer(&app, id, visual)), before);
    visual.simulate_mouse_up(pointer, MouseButton::Left, Default::default());
    let after = super::nested::allocations(&app, ids, visual);
    assert_eq!(after[0].left(), after[1].left());
    assert!((after[0].size.width - before[0].size.width).abs() < px(2.));
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
}

/// An existing bottom window excludes a second bottom region even when the source comes from a side.
#[gpui::test]
fn child_drag_to_occupied_bottom_is_rejected(cx: &mut TestAppContext) {
    let (_directory, app, visual) = setup(cx, true);
    let [explorer, outline, editor] = panels(&app, visual);
    let bottom = visual.update(|_, cx| PanelId::from(app.read(cx).problems.panel.entity_id()));
    let ids = [explorer, outline, editor, bottom];
    let before = ids.map(|id| outer(&app, id, visual));
    begin(&app, outline, visual);
    let pointer = edge(before[2], gpui_base::Placement::Bottom);
    hover(pointer, visual);
    assert!(visual.debug_bounds("local-dock-drop-indicator").is_none());
    visual.simulate_mouse_up(pointer, MouseButton::Left, Default::default());
    assert_eq!(ids.map(|id| outer(&app, id, visual)), before);
}

/// An occupied native right dock blocks another editor-right region, regardless of source side.
#[gpui::test]
fn child_drag_to_occupied_right_is_rejected(cx: &mut TestAppContext) {
    let (_directory, app, visual) = setup(cx, true);
    visual.update(|window, cx| app.update(cx, |app, cx| app.toggle_messages(window, cx)));
    let [explorer, outline, editor] = panels(&app, visual);
    let right = visual.update(|_, cx| PanelId::from(app.read(cx).messages.entity_id()));
    let ids = [explorer, outline, editor, right];
    let before = super::nested::allocations(&app, ids, visual);
    begin(&app, outline, visual);
    let pointer = edge(before[2], Placement::Right);
    hover(pointer, visual);
    assert!(visual.debug_bounds("local-dock-drop-indicator").is_none());
    visual.simulate_mouse_up(pointer, MouseButton::Left, Default::default());
    assert_eq!(super::nested::allocations(&app, ids, visual), before);
}

/// A child can occupy an empty bottom side without moving its remaining left sibling.
#[gpui::test]
fn child_drag_to_free_bottom_uses_only_its_actual_height(cx: &mut TestAppContext) {
    let (_directory, app, visual) = setup(cx, true);
    close_bottom(visual);
    let [explorer, outline, editor] = panels(&app, visual);
    let before = super::nested::allocations(&app, [explorer, outline, editor], visual);
    begin(&app, outline, visual);
    let pointer = edge(before[2], Placement::Bottom);
    hover(pointer, visual);
    assert_eq!(marked(visual, "local-dock-source-0"), before[1]);
    assert_eq!(
        marked(visual, "local-dock-drop-indicator").size.height,
        before[1].size.height
    );
    assert!(visual.debug_bounds("local-dock-source-1").is_none());
    visual.simulate_mouse_up(pointer, MouseButton::Left, Default::default());
    let after = super::nested::allocations(&app, [explorer, outline, editor], visual);
    assert!((after[0].size.width - before[0].size.width).abs() < px(2.));
    assert!((after[0].bottom() - before[1].bottom()).abs() < px(2.));
    assert!((after[1].size.height - before[1].size.height).abs() < px(2.));
    assert!(after[2].bottom() <= after[1].top() + px(2.));
}

/// A concrete bottom-window target joins the existing row instead of adding another bottom region.
#[gpui::test]
fn child_drag_joins_existing_bottom_region(cx: &mut TestAppContext) {
    let (_directory, app, visual) = setup(cx, true);
    let [explorer, outline, editor] = panels(&app, visual);
    let bottom = visual.update(|_, cx| PanelId::from(app.read(cx).problems.panel.entity_id()));
    let ids = [explorer, outline, editor, bottom];
    let before = super::nested::allocations(&app, ids, visual);
    begin(&app, outline, visual);
    let pointer = edge(before[3], Placement::Left);
    hover(pointer, visual);
    assert!(visual.debug_bounds("local-dock-drop-indicator").is_some());
    assert!(visual.debug_bounds("local-dock-source-1").is_none());
    visual.simulate_mouse_up(pointer, MouseButton::Left, Default::default());
    let after = super::nested::allocations(&app, ids, visual);
    assert!(after[1].right() <= after[3].left() + px(2.));
    assert!((after[1].top() - after[3].top()).abs() < px(2.));
    assert!((after[0].size.width - before[0].size.width).abs() < px(2.));
    assert_eq!(after[2], before[2]);
    visual.update(|_, cx| {
        let area = app.read(cx).dock_area.read(cx);
        assert!(
            area.layout(DockPlacement::Bottom)
                .unwrap()
                .find_panel_node(outline)
                .is_some()
        );
        assert!(
            area.layout(DockPlacement::Center)
                .unwrap()
                .find_panel_node(outline)
                .is_none()
        );
    });
}

/// Bands at any depth around Editor reserve their side, including ancestors outside another row.
#[gpui::test]
fn child_drag_center_bands_reserve_all_four_sides(cx: &mut TestAppContext) {
    for side in [
        Placement::Left,
        Placement::Right,
        Placement::Top,
        Placement::Bottom,
    ] {
        for outer_band in [false, true] {
            let (_directory, app, visual) = setup(cx, true);
            close_bottom(visual);
            let [explorer, outline, editor] = panels(&app, visual);
            let source_side = if side.axis() == gpui_kit::Axis::Horizontal {
                Placement::Top
            } else {
                Placement::Right
            };
            let moves = if outer_band {
                [(outline, side), (explorer, source_side)]
            } else {
                [(explorer, source_side), (outline, side)]
            };
            seed_center(&app, &moves, visual);
            let ids = [explorer, outline, editor];
            let before = super::nested::allocations(&app, ids, visual);
            begin(&app, explorer, visual);
            let pointer = edge(before[2], side);
            hover(pointer, visual);
            assert!(
                visual.debug_bounds("local-dock-drop-indicator").is_none(),
                "occupied {side:?}, outer={outer_band}"
            );
            assert!(visual.debug_bounds("local-dock-source-1").is_none());
            visual.simulate_mouse_up(pointer, MouseButton::Left, Default::default());
            assert_eq!(super::nested::allocations(&app, ids, visual), before);
        }
    }
}

/// Removing the last visible member frees its side; hidden registrations do not block relocation.
#[gpui::test]
fn child_drag_last_member_can_relocate_its_own_side(cx: &mut TestAppContext) {
    for side in [
        Placement::Left,
        Placement::Right,
        Placement::Top,
        Placement::Bottom,
    ] {
        let (_directory, app, visual) = setup(cx, false);
        let [explorer, _, editor] = panels(&app, visual);
        let source = match side {
            Placement::Right => {
                visual
                    .update(|window, cx| app.update(cx, |app, cx| app.toggle_messages(window, cx)));
                visual.update(|_, cx| PanelId::from(app.read(cx).messages.entity_id()))
            }
            Placement::Top => {
                seed_center(&app, &[(explorer, side)], visual);
                explorer
            }
            Placement::Bottom => {
                visual.update(|_, cx| PanelId::from(app.read(cx).problems.panel.entity_id()))
            }
            Placement::Left => explorer,
        };
        let before = outer(&app, source, visual);
        let target = outer(&app, editor, visual);
        begin(&app, source, visual);
        let pointer = edge(target, side);
        hover(pointer, visual);
        assert!(
            visual.debug_bounds("local-dock-drop-indicator").is_some(),
            "last member at {side:?}"
        );
        assert_eq!(marked(visual, "local-dock-source-0"), before);
        visual.simulate_mouse_up(pointer, MouseButton::Left, Default::default());
        visual.update(|_, cx| {
            assert!(
                app.read(cx)
                    .dock_area
                    .read(cx)
                    .layout(DockPlacement::Center)
                    .unwrap()
                    .find_panel_node(source)
                    .is_some()
            )
        });
    }
}

/// Releasing over the own occupied side replaces an earlier single-window preview with the whole group.
#[gpui::test]
fn first_level_drag_release_rechecks_own_side_group(cx: &mut TestAppContext) {
    let (_directory, app, visual) = setup(cx, true);
    let [explorer, outline, editor] = panels(&app, visual);
    let ids = [explorer, outline, editor];
    let before = super::nested::allocations(&app, ids, visual);
    begin(&app, outline, visual);
    hover(edge(before[2], Placement::Right), visual);
    assert!(visual.debug_bounds("local-dock-drop-indicator").is_some());
    visual.simulate_mouse_up(
        edge(before[2], Placement::Left),
        MouseButton::Left,
        Default::default(),
    );
    let after = super::nested::allocations(&app, ids, visual);
    assert_eq!(after[0].left(), after[1].left());
    assert!((after[0].size.width - before[0].size.width).abs() < px(2.));
    assert!(after[0].bottom() <= after[1].top() + px(2.));
    assert!((after[1].bottom() - after[2].bottom()).abs() < px(2.));
    visual.update(|_, cx| {
        let area = app.read(cx).dock_area.read(cx);
        assert!(area.is_empty(DockPlacement::Left, cx));
        assert!(!app.read(cx).dock_ui.is_dragging() && !cx.has_active_drag());
    });
}

/// Revealing a peer invalidates a same-position release even if no pointer move refreshed the marker.
#[gpui::test]
fn child_drag_same_pointer_release_rechecks_newly_visible_peer(cx: &mut TestAppContext) {
    let (_directory, app, visual) = setup(cx, false);
    let [explorer, outline, editor] = panels(&app, visual);
    let editor_before = outer(&app, editor, visual);
    begin(&app, explorer, visual);
    let pointer = edge(editor_before, Placement::Left);
    hover(pointer, visual);
    assert!(visual.debug_bounds("local-dock-drop-indicator").is_some());
    // This is the production visibility action, not a duplicate test-only layout mutation.
    visual.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.toggle_outline(&crate::outline::ToggleOutline, window, cx)
        });
    });
    let ids = [explorer, outline, editor];
    let before_release = super::nested::allocations(&app, ids, visual);
    assert_eq!(
        before_release[2], editor_before,
        "the pointer must still identify the same editor edge"
    );
    visual.simulate_mouse_up(pointer, MouseButton::Left, Default::default());
    assert_eq!(
        super::nested::allocations(&app, ids, visual),
        before_release
    );
    visual.update(|_, cx| {
        let area = app.read(cx).dock_area.read(cx);
        let left = area.layout(DockPlacement::Left).unwrap();
        assert!(
            left.find_panel_node(explorer).is_some() && left.find_panel_node(outline).is_some()
        );
        assert!(
            area.layout(DockPlacement::Center)
                .unwrap()
                .find_panel_node(explorer)
                .is_none()
        );
    });
}

/// Closing a visible right dock leaves its registered peers available without reserving that side.
#[gpui::test]
fn child_drag_closed_right_does_not_reserve_pixels(cx: &mut TestAppContext) {
    let (_directory, app, visual) = setup(cx, true);
    for _ in 0..2 {
        visual.update(|window, cx| app.update(cx, |app, cx| app.toggle_messages(window, cx)));
    }
    let [_, outline, editor] = panels(&app, visual);
    let target = outer(&app, editor, visual);
    begin(&app, outline, visual);
    let pointer = edge(target, Placement::Right);
    hover(pointer, visual);
    assert!(visual.debug_bounds("local-dock-drop-indicator").is_some());
    visual.simulate_mouse_up(pointer, MouseButton::Left, Default::default());
    assert!(outer(&app, editor, visual).right() <= outer(&app, outline, visual).left() + px(2.));
}
