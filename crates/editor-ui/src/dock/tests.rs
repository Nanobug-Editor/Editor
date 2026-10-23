//! Native split dragging and the host's single-panel layout policy.

use super::*;
use gpui_base::Placement;
use gpui_base::dock::{DockPlacement, PaneRef, Panel, PanelEvent, PanelId};
use gpui_kit::{EventEmitter, FocusHandle, Focusable, Render, TestAppContext, gpui};

mod reveal;

/// Minimal content exercises real DockArea registration without a plugin worker.
struct TestPanel(FocusHandle);

impl EventEmitter<PanelEvent> for TestPanel {}

impl Focusable for TestPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        // Keep the panel's focus identity stable across split reconciliation.
        self.0.clone()
    }
}

impl Render for TestPanel {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        // Only layout structure matters in this regression test.
        Empty
    }
}

impl Panel for TestPanel {
    fn panel_name(&self) -> &'static str {
        // Instances deliberately share a type name; entity IDs identify panels.
        "LocalDockTestPanel"
    }
}

impl super::Panel for TestPanel {
    /// Test panels need only the local title contract to participate in presentation filtering.
    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

#[gpui::test]
fn panels_stay_in_independent_slots(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::typography::init(cx);
        crate::theme::apply_theme(crate::theme::builtin_theme(false), cx);
    });
    let slot = Rc::new(std::cell::RefCell::new(None));
    let capture = slot.clone();
    let (_, cx) = cx.add_window_view(move |window, cx| {
        let area = LocalDock::new(28.).create_area("independent-panels", None, window, cx);
        *capture.borrow_mut() = Some(area.clone());
        gpui_base::Root::new(area, window, cx)
    });
    let area = slot.borrow_mut().take().unwrap();
    cx.update(|window, cx| {
        area.update(cx, |area, cx| {
            assert!(!area.is_locked(), "panel rearrangement must remain enabled");
            for placement in [
                DockPlacement::Left,
                DockPlacement::Right,
                DockPlacement::Bottom,
                DockPlacement::Center,
            ] {
                let mut ids = Vec::new();
                for _ in 0..3 {
                    let panel = cx.new(|cx| TestPanel(cx.focus_handle()));
                    ids.push(PanelId::from(panel.entity_id()));
                    add_panel_view(
                        area,
                        Arc::new(panel.clone()),
                        placement,
                        Some(px(180.)),
                        window,
                        cx,
                    );
                    // Re-registering an existing panel must leave the layout intact.
                    add_panel_view(area, Arc::new(panel), placement, Some(px(180.)), window, cx);
                }
                let tree = area.layout(placement).unwrap();
                assert_eq!(tree.panels().collect::<Vec<_>>(), ids);
                tree.root().walk(&mut |node| {
                    if let PaneRef::Tabs { panels, .. } = node.kind() {
                        assert_eq!(panels.len(), 1, "panels must never share a tab group");
                    }
                });
                // The center has no dock extent; peripheral regions remain resizable.
                if placement != DockPlacement::Center {
                    area.set_dock_size(placement, px(220.), window, cx);
                    assert_eq!(area.dock_size(placement), Some(px(220.)));
                }
            }
        });
    });
}
/// Real mouse events must move panels across docks without allowing central merges.
fn exercise_native_split_drag(
    cx: &mut TestAppContext,
    placement: Placement,
    immediate: bool,
    target_region: DockPlacement,
) {
    use gpui_kit::{MouseButton, point, size};
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::typography::init(cx);
        crate::theme::apply_theme(crate::theme::builtin_theme(false), cx);
    });
    let slot = Rc::new(std::cell::RefCell::new(None));
    let capture = slot.clone();
    let (_, cx) = cx.add_window_view(move |window, cx| {
        let area = LocalDock::new(28.).create_area("drag-panels", None, window, cx);
        *capture.borrow_mut() = Some(area.clone());
        gpui_base::Root::new(area, window, cx)
    });
    let area = slot.borrow_mut().take().unwrap();
    cx.simulate_resize(size(px(1000.), px(700.)));
    let (source_id, target_id, source_node, target_node) = cx.update(|window, cx| {
        area.update(cx, |area, cx| {
            let source = cx.new(|cx| TestPanel(cx.focus_handle()));
            let target = cx.new(|cx| TestPanel(cx.focus_handle()));
            let source_id = PanelId::from(source.entity_id());
            let target_id = PanelId::from(target.entity_id());
            add_panel_view(
                area,
                Arc::new(source),
                DockPlacement::Left,
                Some(px(240.)),
                window,
                cx,
            );
            add_panel_view(
                area,
                Arc::new(target),
                target_region,
                Some(px(240.)),
                window,
                cx,
            );
            (
                source_id,
                target_id,
                area.layout(DockPlacement::Left)
                    .unwrap()
                    .find_panel_node(source_id)
                    .unwrap(),
                area.layout(target_region)
                    .unwrap()
                    .find_panel_node(target_id)
                    .unwrap(),
            )
        })
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let title = cx
        .debug_bounds(Box::leak(
            format!("local-dock-title-{}", source_node.as_u64()).into_boxed_str(),
        ))
        .unwrap();
    let target = cx
        .debug_bounds(Box::leak(
            format!("local-dock-content-{}", target_node.as_u64()).into_boxed_str(),
        ))
        .unwrap();
    let edge = match placement {
        Placement::Left => point(target.left() + px(10.), target.center().y),
        Placement::Right => point(target.right() - px(10.), target.center().y),
        Placement::Top => point(target.center().x, target.top() + px(10.)),
        Placement::Bottom => point(target.center().x, target.bottom() - px(10.)),
    };
    let allowed = target_region == DockPlacement::Center
        || if target_region == DockPlacement::Bottom {
            matches!(placement, Placement::Left | Placement::Right)
        } else {
            matches!(placement, Placement::Top | Placement::Bottom)
        };
    for destination in [target.center(), edge] {
        // Start a new drag for each attempt; the center attempt must leave the source intact.
        cx.simulate_mouse_down(title.center(), MouseButton::Left, Default::default());
        cx.simulate_mouse_move(
            title.center() + point(px(12.), px(0.)),
            MouseButton::Left,
            Default::default(),
        );
        if immediate {
            // Release in a new zone before another frame can refresh a renderer snapshot.
            let previous = if destination == target.center() {
                edge
            } else {
                target.center()
            };
            cx.simulate_mouse_move(previous, MouseButton::Left, Default::default());
            cx.update(|window, cx| window.draw(cx).clear(cx));
            // An immediate release still resolves the real target's invalid middle zone.
            let release = if destination == target.center() {
                let node = cx.update(|_, cx| {
                    area.read(cx)
                        .layout(target_region)
                        .unwrap()
                        .find_panel_node(target_id)
                        .unwrap()
                });
                cx.debug_bounds(Box::leak(
                    format!("local-dock-content-{}", node.as_u64()).into_boxed_str(),
                ))
                .unwrap()
                .center()
            } else {
                destination
            };
            cx.update(|window, cx| {
                window.dispatch_event(
                    gpui_kit::PlatformInput::MouseMove(gpui_kit::MouseMoveEvent {
                        position: release,
                        pressed_button: Some(MouseButton::Left),
                        modifiers: Default::default(),
                    }),
                    cx,
                );
                window.dispatch_event(
                    gpui_kit::PlatformInput::MouseUp(gpui_kit::MouseUpEvent {
                        position: release,
                        button: MouseButton::Left,
                        modifiers: Default::default(),
                        click_count: 1,
                    }),
                    cx,
                );
            });
        } else {
            cx.simulate_mouse_move(destination, MouseButton::Left, Default::default());
            cx.update(|window, cx| window.draw(cx).clear(cx));
            if destination != target.center() {
                // Preview markers do not change Base's real allocation before release.
                assert_eq!(
                    cx.debug_bounds("local-dock-drop-indicator").is_some(),
                    allowed
                );
                assert!(cx.debug_bounds("local-dock-floating-preview").is_some());
            }
            cx.simulate_mouse_up(destination, MouseButton::Left, Default::default());
        }
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.update(|_, cx| {
            let area = area.read(cx);
            let tree = area.layout(target_region).unwrap();
            if destination == target.center() || !allowed {
                assert_eq!(tree.panels().collect::<Vec<_>>(), vec![target_id]);
                assert_eq!(
                    area.layout(DockPlacement::Left)
                        .unwrap()
                        .panels()
                        .collect::<Vec<_>>(),
                    vec![source_id]
                );
            } else {
                let expected = match placement {
                    Placement::Left | Placement::Top => vec![source_id, target_id],
                    Placement::Right | Placement::Bottom => vec![target_id, source_id],
                };
                assert_eq!(tree.panels().collect::<Vec<_>>(), expected);
                // Center may retain its initial one-child framing split; inspect the actual fork.
                let mut fork = tree.root();
                while let PaneRef::Split { children, .. } = fork.kind() {
                    if children.len() != 1 {
                        break;
                    }
                    fork = &children[0];
                }
                let PaneRef::Split { axis, .. } = fork.kind() else {
                    panic!("edge drops must produce a split, not a tab group");
                };
                assert_eq!(axis, placement.axis());
                assert!(
                    area.layout(DockPlacement::Left)
                        .unwrap()
                        .panels()
                        .next()
                        .is_none()
                );
            }
            tree.root().walk(&mut |node| {
                if let PaneRef::Tabs { panels, .. } = node.kind() {
                    assert_eq!(panels.len(), 1);
                }
            });
        });
    }
}
/// Side regions reject both horizontal directions as well as the central merge zone.
#[gpui::test]
fn native_side_dock_rejects_horizontal_splits_and_tab_merges(cx: &mut TestAppContext) {
    exercise_native_split_drag(cx, Placement::Left, false, DockPlacement::Right);
    exercise_native_split_drag(cx, Placement::Right, false, DockPlacement::Right);
}

/// Both vertical drop directions use Base's split geometry and layout edits.
#[gpui::test]
fn native_vertical_splits_reject_tab_merges(cx: &mut TestAppContext) {
    exercise_native_split_drag(cx, Placement::Top, false, DockPlacement::Right);
    exercise_native_split_drag(cx, Placement::Bottom, false, DockPlacement::Right);
}
/// Drag/drop decisions must use live native hit state even before the next frame.
#[gpui::test]
fn immediate_release_preserves_splits_and_rejects_merges(cx: &mut TestAppContext) {
    exercise_native_split_drag(cx, Placement::Right, true, DockPlacement::Right);
    exercise_native_split_drag(cx, Placement::Bottom, true, DockPlacement::Right);
}
/// A reusable center with no configured editor anchor still permits all four split directions.
#[gpui::test]
fn native_center_without_anchor_accepts_all_edges(cx: &mut TestAppContext) {
    for edge in [
        Placement::Left,
        Placement::Right,
        Placement::Top,
        Placement::Bottom,
    ] {
        exercise_native_split_drag(cx, edge, false, DockPlacement::Center);
    }
}

/// Explicit horizontal and vertical split composition remains native.
#[gpui::test]
fn nested_split_layouts_keep_their_axes(cx: &mut TestAppContext) {
    use gpui_base::dock::DockLayout;
    use gpui_kit::Axis;
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::typography::init(cx);
        crate::theme::apply_theme(crate::theme::builtin_theme(false), cx);
    });
    cx.add_window_view(|window, cx| {
        let area = LocalDock::new(28.).create_area("nested-layout", None, window, cx);
        area.update(cx, |area, cx| {
            let panels = (0..3)
                .map(|_| cx.new(|cx| TestPanel(cx.focus_handle())))
                .collect::<Vec<_>>();
            area.set_center(
                DockLayout::h_split()
                    .child(DockLayout::tabs().panel(panels[0].clone()), Some(px(200.)))
                    .child(
                        DockLayout::v_split()
                            .child(DockLayout::tabs().panel(panels[1].clone()), None)
                            .child(DockLayout::tabs().panel(panels[2].clone()), None),
                        None,
                    ),
                window,
                cx,
            );
            let tree = area.layout(DockPlacement::Center).unwrap();
            let PaneRef::Split { axis, children, .. } = tree.root().kind() else {
                panic!("missing horizontal split")
            };
            assert_eq!(axis, Axis::Horizontal);
            let PaneRef::Split { axis, .. } = children[1].kind() else {
                panic!("missing vertical split")
            };
            assert_eq!(axis, Axis::Vertical);
            assert_eq!(tree.panels().count(), 3);
        });
        gpui_base::Root::new(area, window, cx)
    });
}

/// An exclusive window beside the elastic editor owns its whole region, not a zero-weight alternative.
#[gpui::test]
fn exclusive_editor_edge_keeps_its_regional_allocation(cx: &mut TestAppContext) {
    use gpui_base::dock::DockLayout;
    use gpui_kit::Axis;

    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::typography::init(cx);
        crate::theme::apply_theme(crate::theme::builtin_theme(false), cx);
    });
    for edge in [
        Placement::Left,
        Placement::Right,
        Placement::Top,
        Placement::Bottom,
    ] {
        let slot = Rc::new(RefCell::new(None));
        let capture = slot.clone();
        let (_, visual) = cx.add_window_view(move |window, cx| {
            let editor = cx.new(|cx| TestPanel(cx.focus_handle()));
            let exclusive = cx.new(|cx| TestPanel(cx.focus_handle()));
            let editor_id = PanelId::from(editor.entity_id());
            let exclusive_id = PanelId::from(exclusive.entity_id());
            let presentation = PanelPresentation::default();
            presentation.set_exclusive(exclusive_id, true);
            let dock = LocalDock::new(28.).with_elastic_panel(editor_id);
            let area = dock.create_area("exclusive-editor-edge", None, window, cx);
            area.update(cx, |area, cx| {
                let editor = DockLayout::tabs().panel_view(presentation.panel_handle(editor), cx);
                let exclusive =
                    DockLayout::tabs().panel_view(presentation.panel_handle(exclusive), cx);
                let horizontal = edge.axis() == Axis::Horizontal;
                let layout = if horizontal {
                    DockLayout::h_split()
                } else {
                    DockLayout::v_split()
                };
                let editor_size = Some(px(if horizontal { 800. } else { 500. }));
                let exclusive_size = Some(px(200.));
                let layout = if matches!(edge, Placement::Left | Placement::Top) {
                    layout
                        .child(exclusive, exclusive_size)
                        .child(editor, editor_size)
                } else {
                    layout
                        .child(editor, editor_size)
                        .child(exclusive, exclusive_size)
                };
                area.set_center(layout, window, cx);
            });
            *capture.borrow_mut() = Some((area.clone(), dock, [editor_id, exclusive_id]));
            gpui_base::Root::new(area, window, cx)
        });
        let (area, dock, ids) = slot.borrow_mut().take().unwrap();
        visual.simulate_resize(size(px(1000.), px(700.)));
        visual.run_until_parked();
        visual.update(|window, cx| window.draw(cx).clear(cx));
        let selectors = ids.map(|id| {
            let node = visual.update(|_, cx| {
                area.read(cx)
                    .layout(DockPlacement::Center)
                    .unwrap()
                    .find_panel_node(id)
                    .unwrap()
            });
            &*Box::leak(format!("local-dock-content-{}", node.as_u64()).into_boxed_str())
        });
        let before = selectors.map(|selector| visual.debug_bounds(selector).unwrap());
        visual.update(|window, cx| {
            area.update(cx, |area, cx| {
                let tree = area.layout(DockPlacement::Center).unwrap();
                let root = tree.root().id();
                let PaneRef::Split { sizes, .. } = tree.root().kind() else {
                    panic!("the editor edge must retain its regional split");
                };
                let before = sizes.to_vec();
                dock.prepare_frame(area, window.viewport_size().width, window, cx);
                let tree = area.layout(DockPlacement::Center).unwrap();
                assert_eq!(
                    tree.root().id(),
                    root,
                    "preparation must keep the live tree"
                );
                let PaneRef::Split { sizes, .. } = tree.root().kind() else {
                    panic!("preparation must retain the regional split");
                };
                assert_eq!(
                    sizes,
                    before.as_slice(),
                    "{edge:?} lost its outer allocation"
                );
            });
        });
        visual.run_until_parked();
        visual.update(|window, cx| window.draw(cx).clear(cx));
        for (selector, before) in selectors.into_iter().zip(before) {
            let after = visual.debug_bounds(selector).unwrap();
            assert!(
                (after.left() - before.left()).abs() < px(2.)
                    && (after.top() - before.top()).abs() < px(2.)
                    && (after.size.width - before.size.width).abs() < px(2.)
                    && (after.size.height - before.size.height).abs() < px(2.),
                "{edge:?} preparation changed a regional allocation: {before:?} -> {after:?}"
            );
        }
    }
}
