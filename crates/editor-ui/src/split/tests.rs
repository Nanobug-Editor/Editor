//! Actual Base splitter input and fresh-window restoration use the production preference adapter.
use super::*;
use gpui_kit::{
    Context, MouseButton, Render, TestAppContext, VisualTestContext, Window, point, size,
};
use std::cell::RefCell;

/// Only expected durable weights cross between windows; each native splitter state starts fresh.
struct Fixture {
    /// Choose height restoration for Column fixtures, width restoration for Row fixtures.
    vertical: bool,
    /// Persisted intent supplied to each fresh window, without sharing native resize state.
    weights: Vec<f32>,
    /// Observe only completed user gestures; restoration and viewport changes must stay silent.
    released: Rc<RefCell<Vec<Vec<f32>>>>,
}
impl Render for Fixture {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let released = self.released.clone();
        let vertical = self.vertical;
        split_container_with_layout(
            "persistent-split".into(),
            self.vertical,
            (0..3)
                .map(|index| {
                    (
                        if vertical {
                            None
                        } else {
                            [Some(190.), None, Some(340.)][index]
                        },
                        div()
                            .size_full()
                            .debug_selector(move || format!("persisted-pane-{index}").into())
                            .into_any_element(),
                    )
                })
                .collect(),
            &self.weights,
            move |weights, _, _| released.borrow_mut().push(weights),
            window,
            cx,
        )
    }
}

/// Complete native next-frame measurement and preference projection without synthetic resize receipts.
fn settle(ui: &mut VisualTestContext) {
    for _ in 0..6 {
        ui.update(|window, cx| {
            window.draw(cx).clear(cx);
            window.simulate_next_frame(cx);
        });
        ui.run_until_parked();
    }
}

/// A real drag emits once; fresh native state restores proportions at a different viewport size.
#[gpui_kit::gpui::test]
fn split_drag_restores_proportions_in_a_fresh_window(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::typography::init(cx);
        crate::theme::apply_theme(crate::theme::builtin_theme(false), cx);
    });
    let released = Rc::new(RefCell::new(Vec::new()));
    {
        let capture = released.clone();
        let (_, ui) = cx.add_window_view(|_, _| Fixture {
            vertical: false,
            weights: vec![0.25, 0.5, 0.25],
            released: capture,
        });
        ui.simulate_resize(size(px(1000.), px(250.)));
        settle(ui);
        let left = ui.debug_bounds("persisted-pane-0").unwrap();
        assert!(
            (f32::from(left.size.width) - 250.).abs() < 2.,
            "initial {left:?}"
        );
        assert!(
            released.borrow().is_empty(),
            "restoration must not emit user gestures"
        );
        let start = point(left.right(), left.center().y);
        let end = point(start.x + px(80.), start.y);
        ui.simulate_mouse_down(start, MouseButton::Left, Default::default());
        // Base starts capture after crossing the drag threshold; a following move resizes it.
        ui.simulate_mouse_move(
            point(start.x + px(8.), start.y),
            MouseButton::Left,
            Default::default(),
        );
        ui.simulate_mouse_move(end, MouseButton::Left, Default::default());
        settle(ui);
        ui.simulate_mouse_up(end, MouseButton::Left, Default::default());
        settle(ui);
        assert_eq!(released.borrow().len(), 1, "one final receipt per drag");
        let after = ui.debug_bounds("persisted-pane-0").unwrap();
        assert!(
            (f32::from(after.size.width) - 330.).abs() < 2.,
            "drag {after:?}"
        );
    }
    let saved = released.borrow()[0].clone();
    let capture = released.clone();
    let (_, ui) = cx.add_window_view(|_, _| Fixture {
        vertical: false,
        weights: saved.clone(),
        released: capture,
    });
    for width in [800., 1400.] {
        ui.simulate_resize(size(px(width), px(250.)));
        settle(ui);
        for (index, ratio) in saved.iter().enumerate() {
            let selector = ["persisted-pane-0", "persisted-pane-1", "persisted-pane-2"][index];
            let bounds = ui.debug_bounds(selector).unwrap();
            assert!(
                (f32::from(bounds.size.width) - ratio * width).abs() < 2.,
                "restored {index} at {width}: {bounds:?}"
            );
        }
    }
    assert_eq!(
        released.borrow().len(),
        1,
        "viewport changes must not write preferences"
    );
}

/// An extreme restored ratio must leave Base's drag state consistent with the painted geometry.
#[gpui_kit::gpui::test]
fn split_extreme_restore_keeps_the_first_drag_geometry_current(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::typography::init(cx);
        crate::theme::apply_theme(crate::theme::builtin_theme(false), cx);
    });
    let released = Rc::new(RefCell::new(Vec::new()));
    let capture = released.clone();
    let (_, ui) = cx.add_window_view(|_, _| Fixture {
        vertical: false,
        weights: vec![0.8, 0.1, 0.1],
        released: capture,
    });
    ui.simulate_resize(size(px(1000.), px(250.)));
    settle(ui);
    let left = ui.debug_bounds("persisted-pane-0").unwrap();
    assert!(
        (f32::from(left.size.width) - 800.).abs() < 2.,
        "restored {left:?}"
    );
    let start = point(left.right(), left.center().y);
    let end = point(start.x - px(50.), start.y);
    ui.simulate_mouse_down(start, MouseButton::Left, Default::default());
    ui.simulate_mouse_move(
        point(start.x - px(8.), start.y),
        MouseButton::Left,
        Default::default(),
    );
    ui.simulate_mouse_move(end, MouseButton::Left, Default::default());
    settle(ui);
    ui.simulate_mouse_up(end, MouseButton::Left, Default::default());
    settle(ui);
    let observed = released.borrow();
    assert_eq!(observed.len(), 1);
    for (index, expected) in [0.75, 0.15, 0.1].into_iter().enumerate() {
        assert!(
            (observed[0][index] - expected).abs() < 0.002,
            "first drag: {:?}",
            observed[0]
        );
    }
}

/// Nested preview columns restore height ratios in the dark palette without emitting save feedback.
#[gpui_kit::gpui::test]
fn split_vertical_restores_weighted_height(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::typography::init(cx);
        crate::theme::apply_theme(crate::theme::builtin_theme(true), cx);
    });
    let released = Rc::new(RefCell::new(Vec::new()));
    let captured = released.clone();
    let (_, ui) = cx.add_window_view(|_, _| Fixture {
        vertical: true,
        weights: vec![0.25, 0.5, 0.25],
        released: captured,
    });
    for height in [800., 1400.] {
        ui.simulate_resize(size(px(300.), px(height)));
        settle(ui);
        for (index, ratio) in [0.25, 0.5, 0.25].iter().enumerate() {
            let selector = ["persisted-pane-0", "persisted-pane-1", "persisted-pane-2"][index];
            let bounds = ui.debug_bounds(selector).unwrap();
            assert!(
                (f32::from(bounds.size.height) - ratio * height).abs() < 2.,
                "vertical {index}: {bounds:?}"
            );
        }
    }
    assert!(released.borrow().is_empty());
}
