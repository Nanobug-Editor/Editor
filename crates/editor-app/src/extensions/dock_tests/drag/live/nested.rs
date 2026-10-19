//! Hierarchical dock regressions distinguish the regional row/column from layouts inside its cells.
use super::*;
use gpui_base::{
    Placement,
    dock::{DockLayout, InsertTarget},
};
use gpui_kit::Axis;

/// Sample every panel in one rendered frame after Base's deferred container sizing has settled.
/// Serial `outer` calls draw separate frames and can mix pre-settle and post-settle allocations.
pub(super) fn allocations<const N: usize>(
    app: &Entity<EditorApp>,
    ids: [PanelId; N],
    visual: &mut VisualTestContext,
) -> [Bounds<Pixels>; N] {
    let mut previous = None;
    for _ in 0..20 {
        visual.run_until_parked();
        visual.update(|window, cx| window.draw(cx).clear(cx));
        let nodes = visual.update(|_, cx| {
            let area = app.read(cx).dock_area.read(cx);
            ids.map(|id| {
                [
                    DockPlacement::Center,
                    DockPlacement::Left,
                    DockPlacement::Right,
                    DockPlacement::Bottom,
                ]
                .into_iter()
                .find_map(|region| area.layout(region)?.find_panel_node(id))
                .unwrap()
            })
        });
        let current = nodes.map(|node| {
            let content = visual
                .debug_bounds(Box::leak(
                    format!("local-dock-content-{}", node.as_u64()).into_boxed_str(),
                ))
                .unwrap();
            let title = visual
                .debug_bounds(Box::leak(
                    format!("local-dock-title-{}", node.as_u64()).into_boxed_str(),
                ))
                .unwrap_or(content);
            Bounds::new(
                point(content.left(), title.top()),
                size(content.size.width, content.bottom() - title.top()),
            )
        });
        if previous == Some(current) {
            return current;
        }
        previous = Some(current);
    }
    panic!("Base allocations did not settle: {previous:?}");
}

/// Prepare the illustrated bottom row through Base's public edits; actual gestures remain native input.
/// The result is `[vertical(Explorer, Outline), Problems]`, plus the unchanged Editor above it.
fn bottom_group(
    app: &Entity<EditorApp>,
    visual: &mut VisualTestContext,
    nested: bool,
) -> [PanelId; 4] {
    let [explorer, outline, editor] = panels(app, visual);
    let bottom = visual.update(|window, cx| {
        let app = app.read(cx);
        let bottom = PanelId::from(app.problems.panel.entity_id());
        let area = app.dock_area.clone();
        area.update(cx, |area, cx| {
            let target = area
                .layout(DockPlacement::Bottom)
                .unwrap()
                .find_panel_node(bottom)
                .unwrap();
            area.move_panel(
                explorer,
                InsertTarget::Split {
                    node: target,
                    placement: Placement::Left,
                    size: Some(px(280.)),
                },
                window,
                cx,
            );
            if nested {
                let target = area
                    .layout(DockPlacement::Bottom)
                    .unwrap()
                    .find_panel_node(explorer)
                    .unwrap();
                area.move_panel(
                    outline,
                    InsertTarget::Split {
                        node: target,
                        placement: Placement::Bottom,
                        size: Some(px(130.)),
                    },
                    window,
                    cx,
                );
            }
            // Give the illustrated cell enough room for two nested children without hitting Base's
            // minimum panes or overflowing title controls; retain every hidden registered peer.
            let leaf = |id| DockLayout::tabs().panel_view(area.panel(id).unwrap().clone(), cx);
            let cell = if nested {
                DockLayout::v_split()
                    .child(leaf(explorer), Some(px(130.)))
                    .child(leaf(outline), Some(px(130.)))
            } else {
                leaf(explorer)
            };
            let mut layout = DockLayout::h_split()
                .child(cell, Some(px(400.)))
                .child(leaf(bottom), Some(px(600.)));
            for id in area
                .layout(DockPlacement::Bottom)
                .unwrap()
                .panels()
                .filter(|id| ![explorer, outline, bottom].contains(id))
            {
                layout = layout.child(leaf(id), None);
            }
            area.set_dock(DockPlacement::Bottom, layout, window, cx);
        });
        bottom
    });
    let ids = [explorer, outline, editor, bottom];
    allocations(app, ids, visual);
    ids
}

/// A nested child moves alone to free editor edges; the still-occupied bottom edge rejects it.
#[gpui::test]
fn dock_nested_child_editor_drop_does_not_expand_to_bottom_region(cx: &mut TestAppContext) {
    for placement in [
        Placement::Left,
        Placement::Right,
        Placement::Top,
        Placement::Bottom,
    ] {
        let (_directory, app, visual) = setup(cx, true);
        let ids = bottom_group(&app, visual, true);
        let before = allocations(&app, ids, visual);
        begin(&app, ids[0], visual);
        let pointer = edge(before[2], placement);
        hover(pointer, visual);
        if placement == Placement::Bottom {
            assert!(visual.debug_bounds("local-dock-drop-indicator").is_none());
            visual.simulate_mouse_up(pointer, MouseButton::Left, Default::default());
            assert_eq!(allocations(&app, ids, visual), before);
            continue;
        }
        assert_eq!(marked(visual, "local-dock-source-0"), before[0]);
        for name in [
            "local-dock-source-1",
            "local-dock-target-1",
            "local-dock-drag-card-1",
        ] {
            assert!(
                visual.debug_bounds(name).is_none(),
                "nested child unexpectedly selected {name}"
            );
        }
        assert_eq!(allocations(&app, ids, visual), before);
        visual.simulate_mouse_up(pointer, MouseButton::Left, Default::default());
        let after = allocations(&app, ids, visual);
        assert_eq!(
            after[3], before[3],
            "unrelated bottom cell changed: {before:?} -> {after:?}"
        );
        assert!((after[1].left() - before[0].left()).abs() < px(2.));
        assert!((after[1].top() - before[0].top()).abs() < px(2.));
        assert!((after[1].bottom() - before[1].bottom()).abs() < px(2.));
        visual.update(|_, cx| {
            let area = app.read(cx).dock_area.read(cx);
            let bottom = area.layout(DockPlacement::Bottom).unwrap();
            assert_eq!(
                bottom
                    .panels()
                    .filter(|id| area.panel(*id).is_some_and(|view| view.visible(cx)))
                    .collect::<Vec<_>>(),
                vec![ids[1], ids[3]]
            );
            assert!(
                area.layout(DockPlacement::Center)
                    .unwrap()
                    .find_panel_node(ids[0])
                    .is_some()
            );
        });
    }
}

/// An existing bottom row's first cell can be split vertically without changing the row's other cell.
#[gpui::test]
fn dock_nested_layout_can_be_created_inside_first_level_cell(cx: &mut TestAppContext) {
    for placement in [Placement::Top, Placement::Bottom] {
        let (_directory, app, visual) = setup(cx, true);
        let ids = bottom_group(&app, visual, false);
        let before = allocations(&app, ids, visual);
        begin(&app, ids[1], visual);
        let pointer = edge(before[0], placement);
        hover(pointer, visual);
        assert!(
            visual.debug_bounds("local-dock-drop-indicator").is_some(),
            "a row cell must accept a nested vertical split"
        );
        assert_eq!(allocations(&app, ids, visual), before);
        visual.simulate_mouse_up(pointer, MouseButton::Left, Default::default());
        let after = allocations(&app, ids, visual);
        // The last left member departed, so the entire bottom expands into the freed outer column.
        assert_eq!(after[3].right(), before[3].right());
        assert_eq!(after[3].top(), before[3].top());
        assert_eq!(after[3].size.height, before[3].size.height);
        visual.update(|_, cx| {
            let area = app.read(cx).dock_area.read(cx);
            let gpui_base::dock::PaneRef::Split { axis, children, .. } =
                area.layout(DockPlacement::Bottom).unwrap().root().kind()
            else {
                panic!("missing bottom row")
            };
            assert_eq!(axis, Axis::Horizontal);
            let children = children
                .iter()
                .filter(|child| {
                    let mut visible = false;
                    child.walk(&mut |node| {
                        if let gpui_base::dock::PaneRef::Tabs { panels, .. } = node.kind() {
                            visible |= panels
                                .iter()
                                .any(|id| [ids[0], ids[1], ids[3]].contains(id));
                        }
                    });
                    visible
                })
                .collect::<Vec<_>>();
            assert_eq!(
                children.len(),
                2,
                "hidden registered peers are not displayed cells"
            );
            assert!(matches!(
                children[0].kind(),
                gpui_base::dock::PaneRef::Split {
                    axis: Axis::Vertical,
                    ..
                }
            ));
        });
    }
}

/// Equality uses half the complete target, for actual measured sizes on both placement axes.
#[gpui::test]
fn dock_equal_actual_dimension_uses_half_target(cx: &mut TestAppContext) {
    for placement in [
        Placement::Left,
        Placement::Right,
        Placement::Top,
        Placement::Bottom,
    ] {
        let (_directory, app, visual) = setup(cx, false);
        let [explorer, _, editor] = panels(&app, visual);
        let horizontal = placement.axis() == Axis::Horizontal;
        let source = if horizontal {
            if placement == Placement::Left {
                // The equality case needs an empty left side, not a second independent sidebar.
                visual
                    .update(|window, cx| app.update(cx, |app, cx| app.toggle_explorer(window, cx)));
            }
            visual.update(|_, cx| PanelId::from(app.read(cx).problems.panel.entity_id()))
        } else {
            visual.update(|window, cx| window.draw(cx).clear(cx));
            let close = visual.debug_bounds("problem-close").unwrap();
            visual.simulate_click(close.center(), Default::default());
            explorer
        };
        let before = outer(&app, source, visual);
        let target = outer(&app, editor, visual);
        let dimension = |bounds: Bounds<Pixels>| {
            if horizontal {
                bounds.size.width
            } else {
                bounds.size.height
            }
        };
        assert_eq!(
            dimension(before),
            dimension(target),
            "fixture must exercise exact equality"
        );
        begin(&app, source, visual);
        let pointer = edge(target, placement);
        hover(pointer, visual);
        assert_eq!(
            dimension(marked(visual, "local-dock-drop-indicator")),
            dimension(target) / 2.
        );
        visual.simulate_mouse_up(pointer, MouseButton::Left, Default::default());
        assert!((dimension(outer(&app, source, visual)) - dimension(target) / 2.).abs() < px(2.));
    }
}

/// Side regions keep their first-level vertical stack while a cell accepts a horizontal child layout.
#[gpui::test]
fn dock_nested_side_cell_child_moves_alone_to_editor(cx: &mut TestAppContext) {
    for placement in [Placement::Left, Placement::Right] {
        let (_directory, app, visual) = setup(cx, true);
        let [explorer, outline, editor] = panels(&app, visual);
        let bottom = visual.update(|_, cx| PanelId::from(app.read(cx).problems.panel.entity_id()));
        let target = outer(&app, explorer, visual);
        let peer = outer(&app, outline, visual);
        begin(&app, bottom, visual);
        let pointer = edge(target, placement);
        hover(pointer, visual);
        assert!(visual.debug_bounds("local-dock-drop-indicator").is_some());
        visual.simulate_mouse_up(pointer, MouseButton::Left, Default::default());
        assert_eq!(outer(&app, outline, visual), peer);
        visual.update(|_, cx| {
            let area = app.read(cx).dock_area.read(cx);
            let gpui_base::dock::PaneRef::Split { axis, children, .. } =
                area.layout(DockPlacement::Left).unwrap().root().kind()
            else {
                panic!("missing side stack")
            };
            assert_eq!(axis, Axis::Vertical);
            assert!(matches!(
                children[0].kind(),
                gpui_base::dock::PaneRef::Split {
                    axis: Axis::Horizontal,
                    ..
                }
            ));
        });
        begin(&app, bottom, visual);
        let pointer = edge(outer(&app, editor, visual), Placement::Top);
        hover(pointer, visual);
        assert!(visual.debug_bounds("local-dock-drop-indicator").is_some());
        assert!(visual.debug_bounds("local-dock-source-1").is_none());
        visual.simulate_mouse_up(pointer, MouseButton::Left, Default::default());
        assert_eq!(outer(&app, outline, visual), peer);
        assert_eq!(outer(&app, explorer, visual), target);
    }
}

/// Deeper alternating splits remain local; removing their dragged leaf collapses only that local branch.
#[gpui::test]
fn dock_nested_third_level_child_reclaims_only_its_own_cell(cx: &mut TestAppContext) {
    let (_directory, app, visual) = setup(cx, true);
    visual.update(|window, cx| app.update(cx, |app, cx| app.toggle_messages(window, cx)));
    let ids = bottom_group(&app, visual, true);
    let message = visual.update(|_, cx| PanelId::from(app.read(cx).messages.entity_id()));
    let pointer = edge(outer(&app, ids[0], visual), Placement::Right);
    begin(&app, message, visual);
    hover(pointer, visual);
    assert!(visual.debug_bounds("local-dock-drop-indicator").is_some());
    visual.simulate_mouse_up(pointer, MouseButton::Left, Default::default());
    let all = [ids[0], ids[1], ids[2], ids[3], message];
    let sampled = allocations(&app, all, visual);
    let before = [sampled[0], sampled[1], sampled[2], sampled[3]];
    let message_before = sampled[4];
    begin(&app, message, visual);
    let pointer = edge(before[2], Placement::Right);
    hover(pointer, visual);
    assert_eq!(marked(visual, "local-dock-source-0"), message_before);
    assert!(visual.debug_bounds("local-dock-source-1").is_none());
    assert!(visual.debug_bounds("local-dock-target-1").is_none());
    // Escape must not commit any of the nested ancestor's inferred membership or split preferences.
    visual.simulate_keystrokes("escape");
    assert_eq!(allocations(&app, all, visual), sampled);
    begin(&app, message, visual);
    hover(pointer, visual);
    visual.simulate_mouse_up(pointer, MouseButton::Left, Default::default());
    let after = allocations(&app, all, visual);
    assert_eq!(after[1], before[1]);
    assert_eq!(after[3], before[3]);
    let remaining = after[0];
    assert_eq!(remaining.origin, before[0].origin);
    assert!((remaining.right() - message_before.right()).abs() < px(2.));
}

/// A direct member carries the band on its own side and moves alone on free sides; nested topology survives both.
#[gpui::test]
fn dock_first_level_child_preserves_nested_sibling_layout(cx: &mut TestAppContext) {
    for placement in [Placement::Top, Placement::Right, Placement::Bottom] {
        let (_directory, app, visual) = setup(cx, true);
        let ids = bottom_group(&app, visual, true);
        let before = allocations(&app, ids, visual);
        begin(&app, ids[3], visual);
        let pointer = edge(before[2], placement);
        hover(pointer, visual);
        if placement == Placement::Bottom {
            assert!(visual.debug_bounds("local-dock-source-2").is_some());
            assert!(visual.debug_bounds("local-dock-target-2").is_some());
            assert!(visual.debug_bounds("local-dock-drag-card-2").is_some());
        } else {
            assert_eq!(marked(visual, "local-dock-source-0"), before[3]);
            assert!(visual.debug_bounds("local-dock-source-1").is_none());
            assert!(visual.debug_bounds("local-dock-target-1").is_none());
        }
        assert_eq!(allocations(&app, ids, visual), before);
        visual.simulate_mouse_up(pointer, MouseButton::Left, Default::default());
        let after = allocations(&app, ids, visual);
        assert_eq!(after[0].left(), after[1].left());
        assert!(after[0].bottom() <= after[1].top() + px(2.));
        assert!(
            (after[0].size.height / after[1].size.height
                - before[0].size.height / before[1].size.height)
                .abs()
                < 0.03
        );
    }
}

/// A right-side cell uses the same nested policy as the left, without plugin-specific direction rules.
#[gpui::test]
fn dock_nested_right_cell_child_moves_alone_to_editor(cx: &mut TestAppContext) {
    let (_directory, app, visual) = setup(cx, true);
    visual.update(|window, cx| app.update(cx, |app, cx| app.toggle_messages(window, cx)));
    visual.update(|window, cx| {
        let area = app.read(cx).dock_area.clone();
        area.update(cx, |area, cx| {
            area.set_dock_size(DockPlacement::Right, px(480.), window, cx)
        });
    });
    let [explorer, outline, editor] = panels(&app, visual);
    let message = visual.update(|_, cx| PanelId::from(app.read(cx).messages.entity_id()));
    let pointer = edge(outer(&app, message, visual), Placement::Top);
    begin(&app, outline, visual);
    hover(pointer, visual);
    visual.simulate_mouse_up(pointer, MouseButton::Left, Default::default());
    let pointer = edge(outer(&app, message, visual), Placement::Left);
    begin(&app, explorer, visual);
    hover(pointer, visual);
    assert!(visual.debug_bounds("local-dock-drop-indicator").is_some());
    visual.simulate_mouse_up(pointer, MouseButton::Left, Default::default());
    let ids = [explorer, outline, editor, message];
    let before = allocations(&app, ids, visual);
    let peer = before[1];
    let source = before[0];
    let message_before = before[3];
    begin(&app, explorer, visual);
    let pointer = edge(outer(&app, editor, visual), Placement::Top);
    hover(pointer, visual);
    assert!(visual.debug_bounds("local-dock-source-1").is_none());
    visual.simulate_mouse_up(pointer, MouseButton::Left, Default::default());
    let after = allocations(&app, ids, visual);
    assert_eq!(after[1], peer);
    let message_after = after[3];
    assert_eq!(message_after.origin, source.origin);
    assert!((message_after.right() - message_before.right()).abs() < px(2.));
}

/// Moving away the last regional peer cannot turn the remaining row into a forbidden vertical root.
#[gpui::test]
fn dock_regional_peer_removal_does_not_bypass_first_level_axis(cx: &mut TestAppContext) {
    let (_directory, app, visual) = setup(cx, true);
    let ids = bottom_group(&app, visual, false);
    let before = allocations(&app, ids, visual);
    begin(&app, ids[0], visual);
    let pointer = edge(before[3], Placement::Top);
    hover(pointer, visual);
    assert!(visual.debug_bounds("local-dock-drop-indicator").is_none());
    visual.simulate_mouse_up(pointer, MouseButton::Left, Default::default());
    assert_eq!(allocations(&app, ids, visual), before);
}

/// A same-cell reorder uses the source's actual height after its old parent collapses.
#[gpui::test]
fn dock_nested_reorder_keeps_actual_height_and_unrelated_cell(cx: &mut TestAppContext) {
    use gpui_base::dock::DockLayout;
    let (_directory, app, visual) = setup(cx, true);
    let ids = bottom_group(&app, visual, true);
    visual.update(|window, cx| {
        let area = app.read(cx).dock_area.clone();
        area.update(cx, |area, cx| {
            let leaf = |id| DockLayout::tabs().panel_view(area.panel(id).unwrap().clone(), cx);
            let mut layout = DockLayout::h_split()
                .child(
                    DockLayout::v_split()
                        .child(leaf(ids[0]), Some(px(200.)))
                        .child(leaf(ids[1]), Some(px(400.))),
                    Some(px(400.)),
                )
                .child(leaf(ids[3]), Some(px(880.)));
            for id in area
                .layout(DockPlacement::Bottom)
                .unwrap()
                .panels()
                .filter(|id| !ids.contains(id))
            {
                layout = layout.child(leaf(id), None);
            }
            area.set_dock(DockPlacement::Bottom, layout, window, cx);
            area.set_dock_size(DockPlacement::Bottom, px(600.), window, cx);
        });
    });
    let before = allocations(&app, ids, visual);
    assert!(before[0].size.height < before[1].size.height);
    begin(&app, ids[0], visual);
    let pointer = edge(before[1], Placement::Bottom);
    hover(pointer, visual);
    assert!(
        (marked(visual, "local-dock-drop-indicator").size.height - before[0].size.height).abs()
            < px(1.)
    );
    visual.simulate_mouse_up(pointer, MouseButton::Left, Default::default());
    let after = allocations(&app, ids, visual);
    assert!(
        (after[0].size.height - before[0].size.height).abs() < px(2.),
        "{before:?} -> {after:?}"
    );
    assert_eq!(after[3], before[3]);
}

/// A 90.5px half-preview commits at Base's 100px minimum without allocating 9.5px extra to the cell.
#[gpui::test]
fn dock_nested_half_below_minimum_does_not_overallocate_the_cell(cx: &mut TestAppContext) {
    let (_directory, app, visual) = setup(cx, true);
    let ids = bottom_group(&app, visual, true);
    visual.update(|window, cx| {
        let area = app.read(cx).dock_area.clone();
        area.update(cx, |area, cx| {
            let leaf = |id| DockLayout::tabs().panel_view(area.panel(id).unwrap().clone(), cx);
            let mut layout = DockLayout::h_split()
                .child(
                    DockLayout::v_split()
                        .child(leaf(ids[0]), Some(px(419.)))
                        .child(leaf(ids[1]), Some(px(181.))),
                    Some(px(400.)),
                )
                .child(leaf(ids[3]), Some(px(880.)));
            for id in area
                .layout(DockPlacement::Bottom)
                .unwrap()
                .panels()
                .filter(|id| !ids.contains(id))
            {
                layout = layout.child(leaf(id), None);
            }
            area.set_dock(DockPlacement::Bottom, layout, window, cx);
            area.set_dock_size(DockPlacement::Bottom, px(600.), window, cx);
        });
    });
    let before = allocations(&app, ids, visual);
    begin(&app, ids[0], visual);
    let pointer = edge(before[1], Placement::Bottom);
    hover(pointer, visual);
    assert_eq!(
        marked(visual, "local-dock-drop-indicator").size.height,
        px(90.5)
    );
    visual.simulate_mouse_up(pointer, MouseButton::Left, Default::default());
    let after = allocations(&app, ids, visual);
    assert_eq!(after[0].size.height, gpui_base::PANEL_MIN_SIZE);
    assert_eq!(
        after[0].size.height + after[1].size.height,
        before[3].size.height
    );
    assert_eq!(after[0].bottom(), after[3].bottom());
    assert_eq!(after[3], before[3]);
}
