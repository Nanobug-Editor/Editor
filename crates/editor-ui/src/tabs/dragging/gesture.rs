//! Native mouse dispatch and bounded edge scrolling complete the file-tab drag transaction.

use super::{FileTabDragState, FileTabMove, state::Session};
use gpui_kit::{
    Along as _, App, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ScrollHandle,
    Window, px,
};
use std::{rc::Rc, time::Duration};

/// Private native payload prevents unrelated drag/drop controls from accepting document moves.
pub(super) struct DraggedTab(pub u64);

impl FileTabDragState {
    /// Snapshot `source` geometry after Base's drag threshold and start bounded scrolling on `scroll`.
    pub(super) fn start(
        &self,
        source: u64,
        scroll: ScrollHandle,
        window: &mut Window,
        cx: &mut App,
    ) {
        let token = {
            let mut state = self.0.borrow_mut();
            let Some(source_bounds) = state.items.get(&source).copied() else {
                return;
            };
            if state.session.is_some() || !state.ids.contains(&source) {
                return;
            }
            state.generation = state.generation.wrapping_add(1);
            let token = state.generation;
            state.suppress_release = false;
            state.session = Some(Session {
                source,
                pointer: window.mouse_position(),
                token,
                source_bounds,
            });
            token
        };
        window.refresh();
        let weak = Rc::downgrade(&self.0);
        let mut interval = window
            .refresh_interval(cx)
            .filter(|interval| !interval.is_zero())
            .unwrap_or(Duration::from_millis(16));
        window
            .spawn(cx, async move |cx| {
                loop {
                    cx.background_executor().timer(interval).await;
                    let Some(state) = weak.upgrade() else {
                        break;
                    };
                    let state = FileTabDragState(state);
                    let keep_running = cx.update(|window, cx| {
                        if !state
                            .0
                            .borrow()
                            .session
                            .as_ref()
                            .is_some_and(|session| session.token == token)
                        {
                            return None;
                        }
                        if !window.is_window_active() || !cx.has_active_drag() {
                            state.cancel(window, cx);
                            return None;
                        }
                        // Match the current display cadence and keep velocity in logical pixels
                        // per second, rather than imposing a 60 Hz movement step.
                        let (axis, velocity) = {
                            let state = state.0.borrow();
                            (state.axis, state.edge_scroll())
                        };
                        let delta = px(velocity * interval.as_secs_f32());
                        let offset = scroll.offset();
                        let next = (offset.along(axis) + delta)
                            .clamp(-scroll.max_offset().along(axis), px(0.));
                        if next != offset.along(axis) {
                            scroll.set_offset(offset.apply_along(axis, |_| next));
                            window.refresh();
                        }
                        Some(
                            window
                                .refresh_interval(cx)
                                .filter(|interval| !interval.is_zero())
                                .unwrap_or(interval),
                        )
                    });
                    match keep_running {
                        Ok(Some(next)) => interval = next,
                        _ => break,
                    }
                }
            })
            .detach();
    }
}

impl super::state::State {
    /// Return bounded logical pixels per second while the pointer stays inside either viewport edge.
    fn edge_scroll(&self) -> f32 {
        let Some(session) = self.session.as_ref() else {
            return 0.;
        };
        if !self.viewport.contains(&session.pointer) {
            return 0.;
        }
        let extent = self.viewport.size.along(self.axis);
        let edge = 28_f32.min(extent.as_f32() / 2.);
        if edge <= 0. {
            return 0.;
        }
        let leading =
            (session.pointer.along(self.axis) - self.viewport.origin.along(self.axis)).as_f32();
        let trailing = extent.as_f32() - leading;
        if leading < edge {
            600. * (1. - leading / edge)
        } else if trailing < edge {
            -600. * (1. - trailing / edge)
        } else {
            0.
        }
    }
}

/// Route pointer positions window-wide; release is intercepted before child click/drop handlers.
pub(super) fn install(
    state: &FileTabDragState,
    sink: Rc<dyn Fn(&FileTabMove, &mut Window, &mut App)>,
    window: &mut Window,
) {
    let pressed = state.clone();
    window.on_mouse_event(move |_: &MouseDownEvent, phase, _, _| {
        if phase.capture() && !pressed.active() {
            // A fresh press cannot inherit suppression from an abandoned out-of-window release.
            pressed.0.borrow_mut().suppress_release = false;
        }
    });
    let moving = state.clone();
    window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
        if !phase.capture() || !moving.active() {
            return;
        }
        if event.pressed_button != Some(MouseButton::Left) {
            moving.cancel(window, cx);
            return;
        }
        if let Some(session) = moving.0.borrow_mut().session.as_mut() {
            session.pointer = event.position;
        }
        window.refresh();
    });
    let released = state.clone();
    window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
        if !phase.bubble() || event.button != MouseButton::Left {
            return;
        }
        let (event, suppress) = {
            let mut state = released.0.borrow_mut();
            if let Some(session) = state.session.as_mut() {
                // MouseUp may carry a newer location than the last mouse move.
                session.pointer = event.position;
            }
            let event = state.completed();
            let suppress = state.session.take().is_some() || state.suppress_release;
            state.suppress_release = false;
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
