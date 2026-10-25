//! Native pointer capture gates GPUI's ordinary drag listener until a deliberate long press.

use super::{ReorderEvent, ReorderGroupState, state::Gesture};
use gpui_kit::{
    App, MouseButton, MouseMoveEvent, MouseUpEvent, Pixels, Point, SharedString, Window,
};
use std::{rc::Rc, time::Duration};

/// A short press keeps native button activation; moving early rejects both activation and reorder.
const HOLD_DURATION: Duration = Duration::from_millis(350);

impl ReorderGroupState {
    /// Start a tokenized timer; old timers cannot arm a later press or revive a removed item.
    pub(super) fn begin(
        &self,
        source: SharedString,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.cancel(window, cx);
        let token = {
            let mut state = self.0.borrow_mut();
            state.next_token += 1;
            let token = state.next_token;
            state.gesture = Some(Gesture {
                source,
                start: position,
                position,
                token,
                armed: false,
                dragging: false,
                cancelled: false,
                escape: None,
            });
            token
        };
        // The focused control can live outside this group (for example a menu's retained focus).
        // Scope the public interceptor to this press's native window and unsubscribe on release.
        let cancelled = Rc::downgrade(&self.0);
        let source_window = window.window_handle();
        let escape = cx.intercept_keystrokes(move |event, window, cx| {
            if event.keystroke.key == "escape"
                && window.window_handle() == source_window
                && let Some(state) = cancelled.upgrade()
            {
                ReorderGroupState(state).cancel(window, cx);
                cx.stop_propagation();
            }
        });
        if let Some(gesture) = self.0.borrow_mut().gesture.as_mut() {
            gesture.escape = Some(escape);
        }
        let state = Rc::downgrade(&self.0);
        window
            .spawn(cx, async move |cx| {
                cx.background_executor().timer(HOLD_DURATION).await;
                loop {
                    let Some(state) = state.upgrade() else { break };
                    let state = ReorderGroupState(state);
                    let keep_waiting = cx.update(|window, cx| {
                        let current =
                            state.0.borrow().gesture.as_ref().is_some_and(|gesture| {
                                gesture.token == token && !gesture.cancelled
                            });
                        if !current {
                            return false;
                        }
                        if !window.is_window_active() {
                            state.cancel(window, cx);
                            return false;
                        }
                        if let Some(gesture) = state.0.borrow_mut().gesture.as_mut() {
                            if !gesture.armed {
                                gesture.armed = true;
                                window.refresh();
                            }
                        }
                        true
                    });
                    if !matches!(keep_waiting, Ok(true)) {
                        break;
                    }
                    cx.background_executor()
                        .timer(Duration::from_millis(50))
                        .await;
                }
            })
            .detach();
    }
}

/// Listen across the window so leaving the group shows prohibition and cannot commit a move.
pub(super) fn install(
    state: &ReorderGroupState,
    sink: Rc<dyn Fn(&ReorderEvent, &mut Window, &mut App)>,
    window: &mut Window,
) {
    let moving = state.clone();
    window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
        if !phase.capture() {
            return;
        }
        let mut state = moving.0.borrow_mut();
        let Some(gesture) = state.gesture.as_mut() else {
            return;
        };
        if event.pressed_button != Some(MouseButton::Left) {
            drop(state);
            moving.cancel(window, cx);
            return;
        }
        gesture.position = event.position;
        if !gesture.armed && (event.position - gesture.start).magnitude() > 6. {
            gesture.cancelled = true;
            gesture.escape.take();
        }
        if !gesture.armed || gesture.cancelled {
            cx.stop_propagation();
        }
        if gesture.dragging {
            window.refresh();
        }
    });
    let released = state.clone();
    window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
        if !phase.bubble() || event.button != MouseButton::Left {
            return;
        }
        let (event, suppress) = {
            let mut state = released.0.borrow_mut();
            // Native release can carry a newer position than the last delivered mouse move.
            if let Some(gesture) = state.gesture.as_mut() {
                gesture.position = event.position;
            }
            let event = state.completed_move();
            let suppress = state
                .gesture
                .take()
                .is_some_and(|gesture| gesture.dragging || gesture.cancelled);
            (event, suppress)
        };
        if suppress {
            cx.stop_active_drag(window);
            cx.stop_propagation();
            window.refresh();
        }
        if let Some(event) = event {
            sink(&event, window, cx);
        }
    });
}
