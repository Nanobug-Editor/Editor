//! Whole-region fallback keeps the editor's occupied side unique without promoting nested children.
use super::*;
use gpui_base::{
    Placement,
    dock::{DockLayout, InsertTarget},
};
use gpui_kit::Axis;
use nested::allocations;

/// Seed three real host panels on `side`; `nested` puts the first two inside a local cross-axis cell.
/// Returns the three members in tree order, the elastic editor, and an unrelated perpendicular pane.
fn arrange(
    app: &Entity<EditorApp>,
    side: Placement,
    nested: bool,
    visual: &mut VisualTestContext,
) -> [PanelId; 5] {
    visual.update(|window, cx| app.update(cx, |app, cx| app.toggle_messages(window, cx)));
    let [explorer, outline, editor] = panels(app, visual);
    let (problems, messages) = visual.update(|_, cx| {
        let app = app.read(cx);
        (
            PanelId::from(app.problems.panel.entity_id()),
            PanelId::from(app.messages.entity_id()),
        )
    });
    let horizontal = matches!(side, Placement::Top | Placement::Bottom);
    let members = if horizontal {
        [explorer, outline, problems]
    } else {
        [explorer, outline, messages]
    };
    let other = if horizontal { messages } else { problems };
    visual.update(|window, cx| {
        let area = app.read(cx).dock_area.clone();
        area.update(cx, |area, cx| {
            let region = match side {
                Placement::Left => DockPlacement::Left,
                Placement::Right => DockPlacement::Right,
                Placement::Bottom => DockPlacement::Bottom,
                Placement::Top => DockPlacement::Center,
            };
            // Public moves detach members from their native trees before rebuilding the fixture.
            let seed = match side {
                Placement::Left => members[0],
                Placement::Right | Placement::Bottom => members[2],
                Placement::Top => editor,
            };
            for id in members.into_iter().filter(|id| *id != seed) {
                let node = area.layout(region).unwrap().find_panel_node(seed).unwrap();
                area.move_panel(
                    id,
                    InsertTarget::Split {
                        node,
                        placement: if side == Placement::Top {
                            side
                        } else if horizontal {
                            Placement::Left
                        } else {
                            Placement::Top
                        },
                        size: Some(px(250.)),
                    },
                    window,
                    cx,
                );
            }
            let leaf = |id| DockLayout::tabs().panel_view(area.panel(id).unwrap().clone(), cx);
            let mut group = if horizontal {
                DockLayout::h_split()
            } else {
                DockLayout::v_split()
            };
            if nested {
                let cell = if horizontal {
                    DockLayout::v_split()
                } else {
                    DockLayout::h_split()
                }
                .child(leaf(members[0]), Some(px(140.)))
                .child(leaf(members[1]), Some(px(140.)));
                group = group.child(cell, Some(px(360.)));
            } else {
                group = group
                    .child(leaf(members[0]), Some(px(260.)))
                    .child(leaf(members[1]), Some(px(180.)));
            }
            group = group.child(leaf(members[2]), Some(px(200.)));
            if side == Placement::Top {
                area.set_center(
                    DockLayout::v_split()
                        .child(group, Some(px(320.)))
                        .child(leaf(editor), None),
                    window,
                    cx,
                );
            } else {
                // Hidden registrations remain part of Base's tree but never become moving members.
                for id in area
                    .layout(region)
                    .unwrap()
                    .panels()
                    .filter(|id| !members.contains(id))
                {
                    group = group.child(leaf(id), None);
                }
                area.set_dock(region, group, window, cx);
                area.set_dock_size(
                    region,
                    if horizontal { px(320.) } else { px(400.) },
                    window,
                    cx,
                );
            }
        });
    });
    let ids = [members[0], members[1], members[2], editor, other];
    allocations(app, ids, visual);
    ids
}

/// Read the union of the original visible members without including hidden registered peers.
fn envelope(bounds: &[Bounds<Pixels>]) -> Bounds<Pixels> {
    let left = bounds.iter().map(Bounds::left).min().unwrap();
    let top = bounds.iter().map(Bounds::top).min().unwrap();
    let right = bounds.iter().map(Bounds::right).max().unwrap();
    let bottom = bounds.iter().map(Bounds::bottom).max().unwrap();
    Bounds::new(point(left, top), size(right - left, bottom - top))
}

/// Require one yellow/blue marker and one compact card for each member, in original tree order.
fn assert_group_markers(before: &[Bounds<Pixels>; 5], visual: &mut VisualTestContext) {
    for (index, bounds) in before[..3].iter().enumerate() {
        let name =
            |kind| Box::leak(format!("local-dock-{kind}-{index}").into_boxed_str()) as &'static str;
        assert_eq!(marked(visual, name("source")), *bounds);
        assert!(visual.debug_bounds(name("target")).is_some());
        assert!(visual.debug_bounds(name("drag-card")).is_some());
    }
    assert!(visual.debug_bounds("local-dock-source-3").is_none());
}

/// Every first-level member can relocate its own whole region on all four sides of Editor.
#[gpui::test]
fn grouped_drag_any_member_relocates_all_four_sides(cx: &mut TestAppContext) {
    for side in [
        Placement::Left,
        Placement::Right,
        Placement::Top,
        Placement::Bottom,
    ] {
        for grabbed in 0..3 {
            let (_directory, app, visual) = setup(cx, true);
            let ids = arrange(&app, side, false, visual);
            let before = allocations(&app, ids, visual);
            begin(&app, ids[grabbed], visual);
            let pointer = edge(before[3], side);
            hover(pointer, visual);
            assert_group_markers(&before, visual);
            assert_eq!(allocations(&app, ids, visual), before);
            let source = envelope(&before[..3]);
            let preview = marked(visual, "local-dock-drop-indicator");
            let dimension = |bounds: Bounds<Pixels>| {
                if side.axis() == Axis::Horizontal {
                    bounds.size.width
                } else {
                    bounds.size.height
                }
            };
            let desired = dimension(source);
            let available = dimension(before[3]);
            assert!(
                (dimension(preview)
                    - if desired >= available {
                        available / 2.
                    } else {
                        desired
                    })
                .abs()
                    < px(2.)
            );
            visual.simulate_mouse_up(pointer, MouseButton::Left, Default::default());
            let after = allocations(&app, ids, visual);
            visual.update(|_, cx| {
                let area = app.read(cx).dock_area.read(cx);
                let center = area.layout(DockPlacement::Center).unwrap();
                assert!(
                    ids[..3]
                        .iter()
                        .all(|id| center.find_panel_node(*id).is_some())
                );
            });
            for pair in after[..3].windows(2) {
                if side.axis() == Axis::Horizontal {
                    assert!((pair[0].left() - pair[1].left()).abs() < px(2.));
                    assert!(pair[0].bottom() <= pair[1].top() + px(2.));
                } else {
                    assert!((pair[0].top() - pair[1].top()).abs() < px(2.));
                    assert!(pair[0].right() <= pair[1].left() + px(2.));
                }
            }
            if matches!(side, Placement::Left | Placement::Right) {
                let group = envelope(&after[..3]);
                assert!((group.top() - after[3].top()).abs() < px(2.));
                assert!((group.bottom() - after[3].bottom()).abs() < px(2.));
                assert!((after[4].left() - group.left().min(after[3].left())).abs() < px(2.));
                assert!((after[4].right() - group.right().max(after[3].right())).abs() < px(2.));
            }
        }
    }
}

/// Target switches recompute the moving set: own side carries the group; a free side or tool carries one.
#[gpui::test]
fn grouped_drag_switches_scope_and_escape_keeps_original_layout(cx: &mut TestAppContext) {
    for side in [
        Placement::Left,
        Placement::Right,
        Placement::Top,
        Placement::Bottom,
    ] {
        let (_directory, app, visual) = setup(cx, true);
        let ids = arrange(&app, side, false, visual);
        let before = allocations(&app, ids, visual);
        begin(&app, ids[1], visual);
        hover(edge(before[3], side), visual);
        assert_group_markers(&before, visual);
        let opposite = match side {
            Placement::Left => Placement::Right,
            Placement::Right => Placement::Left,
            Placement::Top => Placement::Bottom,
            Placement::Bottom => Placement::Top,
        };
        for (target, placement) in [
            (before[3], opposite),
            (
                before[4],
                if side.axis() == Axis::Horizontal {
                    Placement::Left
                } else {
                    Placement::Top
                },
            ),
        ] {
            hover(edge(target, placement), visual);
            assert!(visual.debug_bounds("local-dock-drop-indicator").is_some());
            assert_eq!(marked(visual, "local-dock-source-0"), before[1]);
            assert!(visual.debug_bounds("local-dock-source-1").is_none());
            assert!(visual.debug_bounds("local-dock-target-1").is_none());
            assert!(visual.debug_bounds("local-dock-drag-card-1").is_none());
        }
        hover(edge(before[3], side), visual);
        assert_group_markers(&before, visual);
        visual.simulate_keystrokes("escape");
        assert_eq!(allocations(&app, ids, visual), before);
        assert!(visual.debug_bounds("local-dock-drop-indicator").is_none());
    }
}

/// MouseUp must promote the whole source region even when the previous paint previewed one member.
#[gpui::test]
fn grouped_drag_immediate_release_rechecks_moving_members(cx: &mut TestAppContext) {
    for side in [
        Placement::Left,
        Placement::Right,
        Placement::Top,
        Placement::Bottom,
    ] {
        let (_directory, app, visual) = setup(cx, true);
        let ids = arrange(&app, side, false, visual);
        let before = allocations(&app, ids, visual);
        begin(&app, ids[2], visual);
        hover(point(px(-20.), px(160.)), visual);
        visual.simulate_mouse_up(edge(before[3], side), MouseButton::Left, Default::default());
        visual.update(|_, cx| {
            let area = app.read(cx).dock_area.read(cx);
            assert!(ids[..3].iter().all(|id| {
                area.layout(DockPlacement::Center)
                    .unwrap()
                    .find_panel_node(*id)
                    .is_some()
            }));
            assert!(!cx.has_active_drag() && !app.read(cx).dock_ui.is_dragging());
        });
    }
}

/// Another region on the same editor side still blocks the group after excluding every moving member.
#[gpui::test]
fn grouped_drag_cannot_bypass_an_unrelated_same_side_region(cx: &mut TestAppContext) {
    for side in [
        Placement::Left,
        Placement::Right,
        Placement::Top,
        Placement::Bottom,
    ] {
        let (_directory, app, visual) = setup(cx, true);
        let ids = arrange(&app, side, false, visual);
        one_side::seed_center(&app, &[(ids[4], side)], visual);
        let before = allocations(&app, ids, visual);
        begin(&app, ids[0], visual);
        let pointer = edge(before[3], side);
        hover(pointer, visual);
        assert!(visual.debug_bounds("local-dock-drop-indicator").is_none());
        visual.simulate_mouse_up(pointer, MouseButton::Left, Default::default());
        assert_eq!(allocations(&app, ids, visual), before);
    }
}

/// A nested child remains independent even when its same-side target would otherwise promote the region.
#[gpui::test]
fn grouped_drag_nested_child_never_promotes_its_parent_region(cx: &mut TestAppContext) {
    for side in [
        Placement::Left,
        Placement::Right,
        Placement::Top,
        Placement::Bottom,
    ] {
        let (_directory, app, visual) = setup(cx, true);
        let ids = arrange(&app, side, true, visual);
        let before = allocations(&app, ids, visual);
        begin(&app, ids[0], visual);
        let pointer = edge(before[3], side);
        hover(pointer, visual);
        assert!(visual.debug_bounds("local-dock-drop-indicator").is_none());
        assert_eq!(marked(visual, "local-dock-source-0"), before[0]);
        assert!(visual.debug_bounds("local-dock-source-1").is_none());
        visual.simulate_mouse_up(pointer, MouseButton::Left, Default::default());
        assert_eq!(allocations(&app, ids, visual), before);
    }
}

/// Grabbing a direct member moves its sibling cell intact, including that cell's original nested axis.
#[gpui::test]
fn grouped_drag_direct_member_preserves_nested_sibling_layout(cx: &mut TestAppContext) {
    for side in [
        Placement::Left,
        Placement::Right,
        Placement::Top,
        Placement::Bottom,
    ] {
        let (_directory, app, visual) = setup(cx, true);
        let ids = arrange(&app, side, true, visual);
        let before = allocations(&app, ids, visual);
        begin(&app, ids[2], visual);
        let pointer = edge(before[3], side);
        hover(pointer, visual);
        assert_group_markers(&before, visual);
        visual.simulate_mouse_up(pointer, MouseButton::Left, Default::default());
        let after = allocations(&app, ids, visual);
        if side.axis() == Axis::Horizontal {
            assert!((after[0].top() - after[1].top()).abs() < px(2.));
            assert!(after[0].right() <= after[1].left() + px(2.));
            assert!(after[0].bottom() <= after[2].top() + px(2.));
        } else {
            assert!((after[0].left() - after[1].left()).abs() < px(2.));
            assert!(after[0].bottom() <= after[1].top() + px(2.));
            assert!(after[0].right() <= after[2].left() + px(2.));
        }
    }
}
