//! Native splitter input must resize the visible neighbors even with hidden retained slots.

use super::*;
use gpui_base::dock::DockLayout;
use gpui_kit::{Along, Axis, MouseButton, VisualTestContext, gpui, point};

/// Exercise the same pre-render hidden-size projection used by the real host.
struct ResizeHarness {
    area: Entity<DockArea>,
    dock: LocalDock,
}

impl Render for ResizeHarness {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.area.update(cx, |area, cx| {
            self.dock
                .prepare_frame(area, window.viewport_size().width, window, cx)
        });
        self.area.clone()
    }
}

/// Hidden leaves keep their identity in the saved layout but must not own a visible divider.
struct ResizePanel {
    focus: FocusHandle,
    visible: bool,
}

impl EventEmitter<PanelEvent> for ResizePanel {}

impl Focusable for ResizePanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for ResizePanel {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().track_focus(&self.focus)
    }
}

impl Panel for ResizePanel {
    fn panel_name(&self) -> &'static str {
        "ResizePanel"
    }

    fn visible(&self, _: &App) -> bool {
        self.visible
    }
}

/// Settle the native layout without synthesizing a second pointer event.
fn draw(visual: &mut VisualTestContext) {
    visual.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear(cx));
}

/// A hidden middle slot models Outline/Open Files retained between Explorer and a plugin panel.
fn exercise_split_resize(cx: &mut TestAppContext, hidden_middle: bool, axis: Axis) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::typography::init(cx);
        crate::theme::apply_theme(crate::theme::builtin_theme(false), cx);
    });
    let slot = Rc::new(RefCell::new(None));
    let capture = slot.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let panels = [true, !hidden_middle, true].map(|visible| {
            cx.new(|cx| ResizePanel {
                focus: cx.focus_handle(),
                visible,
            })
        });
        let dock = LocalDock::new(28.);
        let area = dock.create_area("split-resize", None, window, cx);
        area.update(cx, |area, cx| {
            area.set_center(
                (if axis == Axis::Vertical {
                    DockLayout::v_split()
                } else {
                    DockLayout::h_split()
                })
                .child(DockLayout::tabs().panel(panels[0].clone()), Some(px(300.)))
                .child(DockLayout::tabs().panel(panels[1].clone()), Some(px(200.)))
                .child(
                    DockLayout::tabs().panel(panels[2].clone()),
                    Some(px(if hidden_middle { 400. } else { 200. })),
                ),
                window,
                cx,
            );
        });
        *capture.borrow_mut() = Some((area.clone(), dock.clone(), panels));
        let harness = cx.new(|_| ResizeHarness { area, dock });
        gpui_base::Root::new(harness, window, cx)
    });
    let (area, dock, panels) = slot.borrow_mut().take().unwrap();
    visual.simulate_resize(size(px(700.), px(700.)));
    draw(visual);
    let node = visual.update(|_, cx| {
        area.read(cx)
            .layout(DockPlacement::Center)
            .unwrap()
            .find_panel_node(PanelId::from(panels[2].entity_id()))
            .unwrap()
    });
    let selector: &'static str =
        Box::leak(format!("local-dock-title-{}", node.as_u64()).into_boxed_str());
    let before = visual.debug_bounds(selector).unwrap();
    let start = if axis == Axis::Vertical {
        point(before.center().x, before.top())
    } else {
        point(before.left(), before.center().y)
    };
    let offset = |delta| {
        if axis == Axis::Vertical {
            point(px(0.), px(delta))
        } else {
            point(px(delta), px(0.))
        }
    };
    visual.simulate_mouse_down(start, MouseButton::Left, Default::default());
    // Base starts the drag after crossing its threshold; subsequent moves drive the handle.
    visual.simulate_mouse_move(start + offset(8.), MouseButton::Left, Default::default());
    draw(visual);
    for delta in [24., 60., 60., 60., 20., 0.] {
        visual.simulate_mouse_move(start + offset(delta), MouseButton::Left, Default::default());
        draw(visual);
        let after = visual.debug_bounds(selector).unwrap();
        assert!(
            (after.origin.along(axis) - before.origin.along(axis) - px(delta)).abs() < px(2.),
            "hidden={hidden_middle}, axis={axis:?}, move={delta}: divider moved from {:?} to {:?}",
            before.origin,
            after.origin
        );
    }
    visual.simulate_mouse_up(start, MouseButton::Left, Default::default());
    draw(visual);
    assert!(!dock.is_dragging());
    if hidden_middle {
        // A resize never consumes the hidden preference or replaces the panel entities.
        visual.update(|_, cx| {
            let saved = dock.dump(area.read(cx), cx);
            let gpui_base::dock::PanelInfo::Stack { sizes, .. } = saved.center.info else {
                panic!("split retained")
            };
            assert_eq!(sizes[1], px(200.));
            for panel in &panels {
                assert_eq!(
                    area.read(cx)
                        .panel(PanelId::from(panel.entity_id()))
                        .unwrap()
                        .view()
                        .entity_id(),
                    panel.entity_id()
                );
            }
        });
        // Escape after an actual change restores the allocation and retires the gesture.
        visual.update(|window, cx| {
            let focus = panels[2].read(cx).focus.clone();
            window.focus(&focus, cx);
        });
        visual.simulate_mouse_down(start, MouseButton::Left, Default::default());
        visual.simulate_mouse_move(start + offset(80.), MouseButton::Left, Default::default());
        draw(visual);
        visual.simulate_keystrokes("escape");
        draw(visual);
        assert!(!dock.is_dragging());
        assert_eq!(visual.debug_bounds(selector).unwrap(), before);
        visual.simulate_mouse_up(start, MouseButton::Left, Default::default());
        // Clamp only the adjacent visible pair; overshoot must not accumulate or consume hidden space.
        visual.simulate_mouse_down(start, MouseButton::Left, Default::default());
        for (delta, boundary) in [
            (10_000., 600.),
            (10_000., 600.),
            (-10_000., 100.),
            (80., 380.),
        ] {
            visual.simulate_mouse_move(
                start + offset(delta),
                MouseButton::Left,
                Default::default(),
            );
            draw(visual);
            assert!(
                (visual.debug_bounds(selector).unwrap().origin.along(axis) - px(boundary)).abs()
                    < px(2.)
            );
        }
        visual.simulate_mouse_up(start + offset(80.), MouseButton::Left, Default::default());
        draw(visual);
        panels[1].update(visual, |panel, cx| {
            panel.visible = true;
            cx.notify();
        });
        draw(visual);
        let restored = visual.debug_bounds(selector).unwrap();
        draw(visual);
        assert_eq!(
            visual.debug_bounds(selector).unwrap(),
            restored,
            "reveal must settle without hover-dependent size changes"
        );
        // Removing the originating split mid-gesture must not leave the host's persistence gate closed.
        panels[1].update(visual, |panel, cx| {
            panel.visible = false;
            cx.notify();
        });
        draw(visual);
        let current = visual.debug_bounds(selector).unwrap();
        let press = if axis == Axis::Vertical {
            point(current.center().x, current.top())
        } else {
            point(current.left(), current.center().y)
        };
        visual.simulate_mouse_down(press, MouseButton::Left, Default::default());
        assert!(dock.is_dragging());
        visual.update(|window, cx| {
            area.update(cx, |area, cx| {
                area.set_center(DockLayout::tabs().panel(panels[0].clone()), window, cx);
            });
        });
        draw(visual);
        visual.simulate_mouse_up(press, MouseButton::Left, Default::default());
        assert!(!dock.is_dragging());
    }
}

/// Control case: the native divider tracks the pointer with all siblings visible.
#[gpui::test]
fn split_resize_tracks_visible_neighbors(cx: &mut TestAppContext) {
    exercise_split_resize(cx, false, Axis::Vertical);
}

/// Retaining an invisible sibling cannot redirect resize input to its stale bounds.
#[gpui::test]
fn split_resize_skips_hidden_neighbors(cx: &mut TestAppContext) {
    exercise_split_resize(cx, true, Axis::Vertical);
}

/// Horizontal tool rows share the same visible-neighbor resize semantics.
#[gpui::test]
fn split_resize_skips_hidden_columns(cx: &mut TestAppContext) {
    exercise_split_resize(cx, true, Axis::Horizontal);
}
