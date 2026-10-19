//! Real pointer gestures restrict regional first-level forks while allowing nested cell layouts.
use super::*;
use gpui_base::Placement;
use gpui_kit::{MouseMoveEvent, MouseUpEvent, PlatformInput};

/// Release in a forbidden edge without an intervening paint, so stale valid previews cannot commit.
fn release_at(pointer: Point<Pixels>, visual: &mut VisualTestContext) {
    visual.update(|window, cx| {
        window.dispatch_event(
            PlatformInput::MouseMove(MouseMoveEvent {
                position: pointer,
                pressed_button: Some(MouseButton::Left),
                modifiers: Default::default(),
            }),
            cx,
        );
        window.dispatch_event(
            PlatformInput::MouseUp(MouseUpEvent {
                position: pointer,
                button: MouseButton::Left,
                modifiers: Default::default(),
                click_count: 1,
            }),
            cx,
        );
    });
}

/// Unsplit left/right regions reject a horizontal root fork; an unsplit bottom rejects a vertical one.
#[gpui::test]
fn dock_regional_edges_reject_forbidden_release_after_valid_preview(cx: &mut TestAppContext) {
    for region in [
        DockPlacement::Left,
        DockPlacement::Right,
        DockPlacement::Bottom,
    ] {
        for second in [false, true] {
            let (_directory, app, visual) = setup(cx, false);
            visual.update(|window, cx| app.update(cx, |app, cx| app.toggle_messages(window, cx)));
            let [explorer, _, editor] = panels(&app, visual);
            let (bottom, right) = visual.update(|_, cx| {
                let app = app.read(cx);
                (
                    PanelId::from(app.problems.panel.entity_id()),
                    PanelId::from(app.messages.entity_id()),
                )
            });
            let ids = [explorer, editor, bottom, right];
            let before = ids.map(|id| outer(&app, id, visual));
            let (source, target, allowed, forbidden) = match region {
                DockPlacement::Left => (
                    bottom,
                    before[0],
                    Placement::Top,
                    if second {
                        Placement::Right
                    } else {
                        Placement::Left
                    },
                ),
                DockPlacement::Right => (
                    bottom,
                    before[3],
                    Placement::Bottom,
                    if second {
                        Placement::Right
                    } else {
                        Placement::Left
                    },
                ),
                _ => (
                    explorer,
                    before[2],
                    Placement::Left,
                    if second {
                        Placement::Bottom
                    } else {
                        Placement::Top
                    },
                ),
            };
            begin(&app, source, visual);
            hover(edge(target, allowed), visual);
            assert!(visual.debug_bounds("local-dock-drop-indicator").is_some());
            let rejected = edge(target, forbidden);
            release_at(rejected, visual);
            assert_eq!(ids.map(|id| outer(&app, id, visual)), before);
            begin(&app, source, visual);
            hover(rejected, visual);
            assert!(visual.debug_bounds("local-dock-drop-indicator").is_none());
            assert_eq!(ids.map(|id| outer(&app, id, visual)), before);
            visual.simulate_keystrokes("escape");
        }
    }
}

/// Before a regional split exists, corner hits follow the permitted first-level axis.
#[gpui::test]
fn dock_regional_corner_placement_keeps_the_permitted_axis(cx: &mut TestAppContext) {
    for region in [
        DockPlacement::Left,
        DockPlacement::Right,
        DockPlacement::Bottom,
    ] {
        let (_directory, app, visual) = setup(cx, false);
        visual.update(|window, cx| app.update(cx, |app, cx| app.toggle_messages(window, cx)));
        let [explorer, _, _] = panels(&app, visual);
        let (bottom, right) = visual.update(|_, cx| {
            let app = app.read(cx);
            (
                PanelId::from(app.problems.panel.entity_id()),
                PanelId::from(app.messages.entity_id()),
            )
        });
        let (source, target) = match region {
            DockPlacement::Left => (bottom, explorer),
            DockPlacement::Right => (bottom, right),
            _ => (explorer, bottom),
        };
        let bounds = outer(&app, target, visual);
        begin(&app, source, visual);
        let pointer = bounds.origin + point(px(8.), px(8.));
        hover(pointer, visual);
        let preview = marked(visual, "local-dock-drop-indicator");
        if region == DockPlacement::Bottom {
            assert_eq!(preview.size.height, bounds.size.height);
        } else {
            assert_eq!(preview.size.width, bounds.size.width);
        }
        visual.simulate_mouse_up(pointer, MouseButton::Left, Default::default());
        let moved = outer(&app, source, visual);
        let target = outer(&app, target, visual);
        if region == DockPlacement::Bottom {
            assert!(moved.right() <= target.left() + px(2.));
        } else {
            assert!(moved.bottom() <= target.top() + px(2.));
        }
        visual.update(|_, cx| {
            let axis = if region == DockPlacement::Bottom {
                gpui_kit::Axis::Horizontal
            } else {
                gpui_kit::Axis::Vertical
            };
            app.read(cx)
                .dock_area
                .read(cx)
                .layout(region)
                .unwrap()
                .root()
                .walk(&mut |node| {
                    if let gpui_base::dock::PaneRef::Split { axis: actual, .. } = node.kind() {
                        assert_eq!(actual, axis);
                    }
                });
        });
    }
}

/// Existing center-local bands retain their axis while concrete cells accept nested cross-axis drops.
#[gpui::test]
fn dock_center_bands_adopt_region_axis_and_allow_nested_targets(cx: &mut TestAppContext) {
    for placement in [
        Placement::Left,
        Placement::Right,
        Placement::Top,
        Placement::Bottom,
    ] {
        let (_directory, app, visual) = setup(cx, true);
        let [explorer, outline, editor] = panels(&app, visual);
        let bottom = visual.update(|_, cx| PanelId::from(app.read(cx).problems.panel.entity_id()));
        let ids = [explorer, outline, editor, bottom];
        let side = matches!(placement, Placement::Left | Placement::Right);
        let free_side = if side {
            Placement::Top
        } else {
            Placement::Right
        };
        // Fixture construction uses Base edits, not the superseded implicit whole-band gesture.
        // The final starting layout has one E/O combination and one source on another free side.
        visual.update(|window, cx| {
            let area = app.read(cx).dock_area.clone();
            area.update(cx, |area, cx| {
                for (source, target, edge) in [
                    (explorer, editor, placement),
                    (
                        outline,
                        explorer,
                        if side {
                            Placement::Bottom
                        } else {
                            Placement::Right
                        },
                    ),
                    (bottom, editor, free_side),
                ] {
                    let node = area
                        .layout(DockPlacement::Center)
                        .unwrap()
                        .find_panel_node(target)
                        .unwrap();
                    area.move_panel(
                        source,
                        gpui_base::dock::InsertTarget::Split {
                            node,
                            placement: edge,
                            size: Some(px(280.)),
                        },
                        window,
                        cx,
                    );
                }
            });
        });
        let before = super::nested::allocations(&app, ids, visual);
        if side {
            assert!(before[0].bottom() <= before[1].top() + px(2.));
        } else {
            assert!(before[0].right() <= before[1].left() + px(2.));
        }
        begin(&app, bottom, visual);
        hover(
            edge(
                before[0],
                if side {
                    Placement::Top
                } else {
                    Placement::Left
                },
            ),
            visual,
        );
        assert!(visual.debug_bounds("local-dock-drop-indicator").is_some());
        let nested = edge(
            before[0],
            if side {
                Placement::Left
            } else {
                Placement::Top
            },
        );
        hover(nested, visual);
        assert!(visual.debug_bounds("local-dock-drop-indicator").is_some());
        assert_eq!(ids.map(|id| outer(&app, id, visual)), before);
        release_at(nested, visual);
        visual.update(|_, cx| {
            let area = app.read(cx).dock_area.read(cx);
            assert!(
                area.layout(DockPlacement::Center)
                    .unwrap()
                    .find_panel_node(bottom)
                    .is_some()
            );
        });
        begin(&app, bottom, visual);
        hover(edge(outer(&app, editor, visual), free_side), visual);
        assert!(visual.debug_bounds("local-dock-drop-indicator").is_some());
        assert!(
            visual.debug_bounds("local-dock-source-1").is_none(),
            "nested Center child must move alone"
        );
        visual.simulate_keystrokes("escape");
    }
}
