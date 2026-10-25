//! Dispatch actual GPUI pointer/key events through the same public group used by the host.

use super::*;
use crate::Button;
use gpui_kit::{
    Entity, MouseButton, Pixels, Point, TestAppContext, VisualTestContext, gpui, point, size,
};
use std::{cell::RefCell, time::Duration};

/// The model and observed events live outside the group, as in the host toolbar.
struct Harness {
    /// Public item model rebuilt on each frame.
    ids: Vec<&'static str>,
    /// Persistent state used through ordinary render and membership updates.
    state: ReorderGroupState,
    /// Switch to the same axis used by overflow menus.
    vertical: bool,
    /// Restrict the painted group to a finite viewport for drop-area regression coverage.
    clipped: bool,
    /// Position the same row near either window edge for native preview placement checks.
    top: f32,
    /// Moves emitted through the public callback.
    events: Rc<RefCell<Vec<ReorderEvent>>>,
    /// Native child activations observed independently from reorder events.
    clicks: Rc<RefCell<Vec<SharedString>>>,
}

impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let sink = self.events.clone();
        let mut group = ReorderGroup::new("group", self.state.clone())
            .on_reorder(move |event, _, _| sink.borrow_mut().push(event.clone()));
        if self.vertical {
            group = group.vertical();
        }
        for id in &self.ids {
            let clicked = SharedString::from(*id);
            let sink = self.clicks.clone();
            group = group.item(
                *id,
                *id,
                Button::new(*id)
                    .label(*id)
                    .w(px(80.))
                    .on_click(move |_, _, _| sink.borrow_mut().push(clicked.clone())),
            );
        }
        div().size_full().p(px(20.)).pt(px(self.top)).child(
            div()
                .when(self.clipped, |viewport| {
                    viewport.w(px(80.)).h(px(45.)).overflow_hidden()
                })
                .child(group),
        )
    }
}

/// Initialize local theme/control behavior and return the retained model plus native test window.
fn harness(cx: &mut TestAppContext, vertical: bool) -> (Entity<Harness>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::typography::init(cx);
        crate::theme::apply_theme(crate::theme::builtin_theme(false), cx);
    });
    let retained = Rc::new(RefCell::new(None));
    let stored = retained.clone();
    let (_, ui) = cx.add_window_view(move |window, cx| {
        let view = cx.new(|_| Harness {
            ids: vec!["one", "two", "three"],
            state: ReorderGroupState::default(),
            vertical,
            clipped: false,
            top: 20.,
            events: Rc::new(RefCell::new(Vec::new())),
            clicks: Rc::new(RefCell::new(Vec::new())),
        });
        stored.replace(Some(view.clone()));
        gpui_base::Root::new(view, window, cx)
    });
    ui.update(|window, _| window.activate_window());
    ui.simulate_resize(size(px(500.), px(300.)));
    frame(ui, 0);
    (retained.borrow_mut().take().unwrap(), ui)
}

/// Advance only the finite test clock interval, then draw the refreshed native tree.
fn frame(ui: &mut VisualTestContext, millis: u64) {
    ui.executor().advance_clock(Duration::from_millis(millis));
    ui.run_until_parked();
    ui.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
}

/// Press and hold the named item, then move enough to trigger GPUI's native drag threshold.
fn begin_drag(ui: &mut VisualTestContext, id: &str) -> Point<Pixels> {
    let selector = match id {
        "one" => "reorder-item-one",
        "two" => "reorder-item-two",
        "three" => "reorder-item-three",
        _ => panic!("unknown fixture item"),
    };
    let start = ui.debug_bounds(selector).unwrap().center();
    ui.simulate_mouse_down(start, MouseButton::Left, Default::default());
    frame(ui, 0);
    frame(ui, 350);
    ui.simulate_mouse_move(
        start + point(px(8.), px(0.)),
        MouseButton::Left,
        Default::default(),
    );
    frame(ui, 0);
    assert!(ui.update(|_, cx| cx.has_active_drag()));
    start
}

/// Native short clicks and keyboard activation stay on the child Button's original behavior.
#[gpui::test]
fn short_press_keeps_child_activation(cx: &mut TestAppContext) {
    let (view, ui) = harness(cx, false);
    let start = ui.debug_bounds("reorder-item-one").unwrap().center();
    ui.simulate_click(start, Default::default());
    frame(ui, 0);
    ui.update(|window, cx| crate::testing::press_key(window, "enter", cx));
    let (clicks, moves) = ui.update(|_, cx| {
        (
            view.read(cx).clicks.borrow().clone(),
            view.read(cx).events.borrow().clone(),
        )
    });
    assert_eq!(
        clicks,
        vec![SharedString::from("one"), SharedString::from("one")]
    );
    assert!(moves.is_empty());
}

/// A long drag shows the insertion mark, emits exactly one move, and never clicks a button.
#[gpui::test]
fn long_press_reorders_with_visible_insertion(cx: &mut TestAppContext) {
    let (view, ui) = harness(cx, false);
    begin_drag(ui, "one");
    let target = ui.debug_bounds("reorder-item-three").unwrap();
    let target = point(target.right() - px(2.), target.center().y);
    ui.simulate_mouse_move(target, MouseButton::Left, Default::default());
    frame(ui, 0);
    assert!(ui.debug_bounds("reorder-insertion").is_some());
    assert!(ui.debug_bounds("reorder-forbidden").is_none());
    ui.simulate_mouse_up(target, MouseButton::Left, Default::default());
    let (clicks, events) = ui.update(|_, cx| {
        (
            view.read(cx).clicks.borrow().clone(),
            view.read(cx).events.borrow().clone(),
        )
    });
    assert!(clicks.is_empty());
    assert_eq!(
        events,
        vec![ReorderEvent {
            source: "one".into(),
            before: None
        }]
    );
    assert!(!ui.update(|_, cx| cx.has_active_drag()));
}

/// The prohibition mark follows a drag outside the group; release there cannot reorder or click.
#[gpui::test]
fn outside_drop_is_forbidden(cx: &mut TestAppContext) {
    let (view, ui) = harness(cx, false);
    let start = begin_drag(ui, "one");
    let last = ui.debug_bounds("reorder-item-three").unwrap();
    // The flex container fills the harness width, but blank space beside its last item is invalid.
    let blank = point(last.right() + px(30.), start.y);
    ui.simulate_mouse_move(blank, MouseButton::Left, Default::default());
    frame(ui, 0);
    assert!(ui.debug_bounds("reorder-forbidden").is_some());
    assert!(ui.debug_bounds("reorder-insertion").is_none());
    let outside = point(px(350.), px(160.));
    ui.simulate_mouse_move(outside, MouseButton::Left, Default::default());
    frame(ui, 0);
    assert!(ui.debug_bounds("reorder-forbidden").is_some());
    assert!(ui.debug_bounds("reorder-insertion").is_none());
    ui.simulate_mouse_up(outside, MouseButton::Left, Default::default());
    ui.update(|_, cx| {
        assert!(view.read(cx).events.borrow().is_empty());
        assert!(view.read(cx).clicks.borrow().is_empty());
    });
}

/// Escape suppresses the eventual release, even if it returns to the original button.
#[gpui::test]
fn escape_cancels_without_click(cx: &mut TestAppContext) {
    let (view, ui) = harness(cx, false);
    let start = begin_drag(ui, "one");
    ui.simulate_keystrokes("escape");
    frame(ui, 0);
    assert!(!ui.update(|_, cx| cx.has_active_drag()));
    ui.simulate_mouse_up(start, MouseButton::Left, Default::default());
    ui.update(|_, cx| {
        assert!(view.read(cx).events.borrow().is_empty());
        assert!(view.read(cx).clicks.borrow().is_empty());
    });
}

/// A removed source and a deactivated window both revoke their live native drag.
#[gpui::test]
fn membership_change_and_window_blur_cancel(cx: &mut TestAppContext) {
    let (view, ui) = harness(cx, false);
    begin_drag(ui, "one");
    ui.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.ids.remove(0);
            cx.notify();
        })
    });
    frame(ui, 0);
    assert!(!ui.update(|_, cx| cx.has_active_drag()));
    ui.simulate_mouse_up(
        point(px(30.), px(30.)),
        MouseButton::Left,
        Default::default(),
    );
    begin_drag(ui, "two");
    ui.deactivate_window();
    frame(ui, 50);
    assert!(!ui.update(|_, cx| cx.has_active_drag()));
    ui.update(|_, cx| assert!(view.read(cx).events.borrow().is_empty()));
}

/// Vertical overflow lists resolve the successor by Y and preserve the same public move event.
#[gpui::test]
fn vertical_group_uses_vertical_insertion(cx: &mut TestAppContext) {
    let (view, ui) = harness(cx, true);
    begin_drag(ui, "three");
    let first = ui.debug_bounds("reorder-item-one").unwrap();
    let target = point(first.center().x, first.top() + px(2.));
    ui.simulate_mouse_move(target, MouseButton::Left, Default::default());
    frame(ui, 0);
    let line = ui.debug_bounds("reorder-insertion").unwrap();
    assert_eq!(line.size.height, px(2.));
    ui.simulate_mouse_up(target, MouseButton::Left, Default::default());
    ui.update(|_, cx| {
        assert_eq!(
            *view.read(cx).events.borrow(),
            vec![ReorderEvent {
                source: "three".into(),
                before: Some("one".into())
            }]
        )
    });
}

/// A release outside the clipped viewport must reject a previously valid hover target.
#[gpui::test]
fn release_position_and_scroll_clip_are_authoritative(cx: &mut TestAppContext) {
    let (view, ui) = harness(cx, true);
    ui.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.clipped = true;
            cx.notify();
        })
    });
    frame(ui, 0);
    begin_drag(ui, "one");
    let visible = point(px(50.), px(60.));
    ui.simulate_mouse_move(visible, MouseButton::Left, Default::default());
    frame(ui, 0);
    assert!(ui.debug_bounds("reorder-insertion").is_some());
    // No intervening move: the release event itself leaves the scroll viewport.
    ui.simulate_mouse_up(
        point(px(50.), px(90.)),
        MouseButton::Left,
        Default::default(),
    );
    ui.update(|_, cx| {
        assert!(view.read(cx).events.borrow().is_empty());
        assert!(view.read(cx).clicks.borrow().is_empty());
    });
}

/// Moving before the hold threshold cannot turn into a delayed reorder or accidental activation.
#[gpui::test]
fn early_movement_rejects_drag_and_click(cx: &mut TestAppContext) {
    let (view, ui) = harness(cx, false);
    let first = ui.debug_bounds("reorder-item-one").unwrap().center();
    ui.simulate_mouse_down(first, MouseButton::Left, Default::default());
    ui.simulate_mouse_move(
        first + point(px(10.), px(0.)),
        MouseButton::Left,
        Default::default(),
    );
    frame(ui, 400);
    ui.simulate_mouse_move(first, MouseButton::Left, Default::default());
    assert!(!ui.update(|_, cx| cx.has_active_drag()));
    ui.simulate_mouse_up(first, MouseButton::Left, Default::default());
    ui.update(|_, cx| {
        assert!(view.read(cx).events.borrow().is_empty());
        assert!(view.read(cx).clicks.borrow().is_empty());
    });
}

/// A left-edge insertion keeps the full native preview inside the window and off the drop line.
#[gpui::test]
fn drag_preview_leaves_left_edge_insertion_visible(cx: &mut TestAppContext) {
    let (view, ui) = harness(cx, false);
    for top in [20., 240.] {
        ui.update(|_, cx| {
            view.update(cx, |view, cx| {
                view.top = top;
                cx.notify();
            })
        });
        frame(ui, 0);
        begin_drag(ui, "three");
        let first = ui.debug_bounds("reorder-item-one").unwrap();
        let target = point(first.left() + px(2.), first.center().y);
        ui.simulate_mouse_move(target, MouseButton::Left, Default::default());
        frame(ui, 0);
        let preview = ui.debug_bounds("reorder-drag-preview").unwrap();
        let line = ui.debug_bounds("reorder-insertion").unwrap();
        assert!(preview.left() >= px(0.));
        assert!(preview.right() <= px(500.));
        assert!(!preview.intersects(&line));
        if top > 100. {
            assert!(preview.bottom() < first.top());
        }
        ui.simulate_mouse_up(target, MouseButton::Left, Default::default());
    }
}
