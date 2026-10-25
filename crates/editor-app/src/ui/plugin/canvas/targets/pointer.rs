//! Native mouse capture retains its original target; unavailable device fields stay explicit.
use super::*;
use protocol::ui::{
    CanvasCancelReason, CanvasCursor, CanvasPointerDevice, CanvasPointerInput,
    CanvasPointerPhase as Phase, CanvasWheelInput, CanvasWheelUnit,
};

/// Preserve all native modifier flags without inferring unsupported physical input.
fn modifiers(value: gpui_kit::Modifiers) -> CanvasModifiers {
    CanvasModifiers {
        ctrl: value.control,
        alt: value.alt,
        shift: value.shift,
        meta: value.platform,
        function: value.function,
    }
}
impl CanvasView {
    /// Invert the same viewport/scroll basis used by native drawing, retaining fractional coordinates.
    fn target_point(&self, position: Point<Pixels>) -> (f32, f32) {
        if let Some(frame) = &self.targets.frame {
            let point = position - frame.bounds.origin;
            return (
                (point.x / px(1.) - frame.basis.x) / frame.basis.w,
                (point.y / px(1.) - frame.basis.y) / frame.basis.h,
            );
        }
        let basis = self.project(
            protocol::Rect {
                x: 0.,
                y: 0.,
                w: 1.,
                h: 1.,
            },
            self.bounds,
        );
        let point = position - self.bounds.origin;
        (
            (point.x / px(1.) - basis.x) / basis.w,
            (point.y / px(1.) - basis.y) / basis.h,
        )
    }
    /// Reject outside ordinary input; captured input deliberately retains outside coordinates.
    fn hit_target(&self, position: Point<Pixels>) -> Option<(String, u64)> {
        let frame = self.targets.frame.as_ref()?;
        if !self.enabled || !frame.bounds.contains(&position) {
            return None;
        }
        let (x, y) = self.target_point(position);
        frame.graphics.targets.hit(x, y)
    }
    /// Build one genuine native mouse observation; GPUI supplies no full button mask or pressure here.
    fn mouse_input(
        &self,
        phase: Phase,
        position: Point<Pixels>,
        button: Option<u8>,
        clicks: Option<u32>,
        native: gpui_kit::Modifiers,
    ) -> CanvasPointerInput {
        let (x, y) = self.target_point(position);
        CanvasPointerInput {
            pointer: 1,
            device: CanvasPointerDevice::Mouse,
            phase,
            x,
            y,
            button,
            buttons: None,
            clicks,
            pressure: None,
            modifiers: modifiers(native),
            cancel_reason: None,
        }
    }
    /// Enter/leave uses exact native hit paths rather than their focus bounding rectangles.
    fn target_hover(
        &mut self,
        position: Point<Pixels>,
        native: gpui_kit::Modifiers,
        cx: &mut Context<Self>,
    ) {
        let next = self.hit_target(position);
        if next == self.targets.hover {
            return;
        }
        if let Some(previous) = self.targets.hover.take() {
            let event = self.mouse_input(Phase::Leave, position, None, None, native);
            self.emit_target(previous, CanvasInputKind::Pointer(event), cx);
        }
        if let Some(next) = next {
            let event = self.mouse_input(Phase::Enter, position, None, None, native);
            if self.emit_target(next.clone(), CanvasInputKind::Pointer(event), cx) {
                self.targets.hover = Some(next);
            }
        }
        cx.notify();
    }
    /// Press captures/focuses locally before the plugin receives its ordered event.
    pub(in crate::ui::plugin::canvas) fn target_pointer_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let target = self
            .targets
            .captured
            .as_ref()
            .map(|(key, _, _)| key.clone())
            .or_else(|| self.hit_target(event.position));
        let Some(target) = target else {
            return false;
        };
        let Some(declaration) = self.drawing.scene.as_ref().and_then(|scene| {
            scene
                .targets
                .iter()
                .find(|item| item.id == target.0 && item.epoch == target.1)
        }) else {
            return false;
        };
        let capture = declaration.capture_on_down;
        let focus = declaration.focus_on_down && declaration.focusable;
        let button = button_index(event.button);
        let input = self.mouse_input(
            Phase::Down,
            event.position,
            Some(button),
            Some(event.click_count as u32),
            event.modifiers,
        );
        if !self.emit_target(target.clone(), CanvasInputKind::Pointer(input.clone()), cx) {
            return false;
        }
        if capture {
            let buttons = self
                .targets
                .captured
                .as_ref()
                .map_or(0, |(_, _, buttons)| *buttons);
            self.targets.captured = Some((target.clone(), input, buttons | (1 << button)));
        }
        if focus {
            if let Some(handle) = self.targets.focus.get(&target) {
                handle.focus(window, cx);
            }
        }
        cx.notify();
        true
    }
    /// Ordinary movement may enter another shape; a captured move is handled by the window listener.
    pub(in crate::ui::plugin::canvas) fn target_pointer_move(
        &mut self,
        event: &MouseMoveEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.targets.captured.is_some() {
            return false;
        }
        self.target_hover(event.position, event.modifiers, cx);
        let Some(target) = self.targets.hover.clone() else {
            return false;
        };
        let input = self.mouse_input(Phase::Move, event.position, None, None, event.modifiers);
        self.emit_target(target, CanvasInputKind::Pointer(input), cx)
    }
    /// Noncaptured release uses the actual hit target; captured release stops propagation earlier.
    pub(in crate::ui::plugin::canvas) fn target_pointer_up(
        &mut self,
        event: &MouseUpEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(target) = self.hit_target(event.position) else {
            return false;
        };
        let input = self.mouse_input(
            Phase::Up,
            event.position,
            Some(button_index(event.button)),
            Some(event.click_count as u32),
            event.modifiers,
        );
        self.emit_target(target, CanvasInputKind::Pointer(input), cx)
    }
    /// Hover revocation never transfers a drag to another target.
    pub(in crate::ui::plugin::canvas) fn leave_targets(
        &mut self,
        position: Point<Pixels>,
        native: gpui_kit::Modifiers,
        cx: &mut Context<Self>,
    ) {
        if self.targets.captured.is_none() {
            if let Some(previous) = self.targets.hover.take() {
                let input = self.mouse_input(Phase::Leave, position, None, None, native);
                self.emit_target(previous, CanvasInputKind::Pointer(input), cx);
                cx.notify();
            }
        }
    }
    /// Native focus/pointer loss emits one cancellation and releases capture before subsequent input.
    pub(in crate::ui::plugin::canvas) fn cancel_target_pointer(
        &mut self,
        reason: CanvasCancelReason,
        cx: &mut Context<Self>,
    ) {
        if let Some((target, mut input, _)) = self.targets.captured.take() {
            input.phase = Phase::Cancel;
            input.button = None;
            input.clicks = None;
            input.cancel_reason = Some(reason);
            self.emit_target(target, CanvasInputKind::Pointer(input), cx);
        }
        self.targets.hover = None;
        cx.notify();
    }
    /// Install frame-local window listeners so an original drag completes beyond the Canvas bounds.
    pub(in crate::ui::plugin::canvas) fn capture_target_pointer(
        &self,
        window: &mut Window,
        cx: &Context<Self>,
    ) {
        let owner = cx.entity().downgrade();
        let gesture = self.action_gesture;
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
            if phase.capture() {
                let _ = owner.update(cx, |this, cx| {
                    if this.action_gesture != gesture || !this.enabled {
                        return;
                    }
                    let Some((target, _, buttons)) = this.targets.captured.clone() else {
                        return;
                    };
                    // GPUI reports one pressed button, not a complete mask. Another reported button
                    // cannot prove that the initiating button was released; only None proves loss.
                    if event.pressed_button.is_none() {
                        this.cancel_target_pointer(CanvasCancelReason::PointerLost, cx);
                        return;
                    }
                    this.target_hover(event.position, event.modifiers, cx);
                    let input =
                        this.mouse_input(Phase::Move, event.position, None, None, event.modifiers);
                    this.emit_target(target.clone(), CanvasInputKind::Pointer(input.clone()), cx);
                    this.targets.captured = Some((target, input, buttons));
                    cx.stop_propagation();
                });
            }
        });
        let owner = cx.entity().downgrade();
        window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
            if phase.capture() {
                let _ = owner.update(cx, |this, cx| {
                    if this.action_gesture != gesture || !this.enabled {
                        return;
                    }
                    let Some((target, _, buttons)) = this.targets.captured.clone() else {
                        return;
                    };
                    let button = button_index(event.button);
                    if buttons & (1 << button) == 0 {
                        return;
                    }
                    let input = this.mouse_input(
                        Phase::Up,
                        event.position,
                        Some(button),
                        Some(event.click_count as u32),
                        event.modifiers,
                    );
                    let remaining = buttons & !(1 << button);
                    this.targets.captured =
                        (remaining != 0).then(|| (target.clone(), input.clone(), remaining));
                    this.emit_target(target, CanvasInputKind::Pointer(input), cx);
                    cx.notify();
                    cx.stop_propagation();
                });
            }
        });
        if self.targets.captured.is_some() {
            window.set_window_cursor_style(self.target_cursor());
        }
    }
    /// Preserve native wheel units and subpixel deltas; target motion is inverse-projected once.
    pub(in crate::ui::plugin::canvas) fn target_wheel(
        &mut self,
        event: &ScrollWheelEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        let target = self
            .targets
            .captured
            .as_ref()
            .map(|(key, _, _)| key.clone())
            .or_else(|| self.hit_target(event.position));
        let Some(target) = target else {
            return false;
        };
        let (x, y) = self.target_point(event.position);
        let basis = self.project(
            protocol::Rect {
                x: 0.,
                y: 0.,
                w: 1.,
                h: 1.,
            },
            self.bounds,
        );
        let (delta_x, delta_y, unit) = match event.delta {
            gpui_kit::ScrollDelta::Pixels(delta) => (
                delta.x / px(1.) / basis.w,
                delta.y / px(1.) / basis.h,
                CanvasWheelUnit::Pixels,
            ),
            gpui_kit::ScrollDelta::Lines(delta) => (delta.x, delta.y, CanvasWheelUnit::Lines),
        };
        self.emit_target(
            target,
            CanvasInputKind::Wheel(CanvasWheelInput {
                x,
                y,
                delta_x,
                delta_y,
                unit,
                modifiers: modifiers(event.modifiers),
            }),
            cx,
        )
    }
    /// Native cursor requests use the exact hovered/captured object while preserving child controls.
    pub(in crate::ui::plugin::canvas) fn target_cursor(&self) -> gpui_kit::CursorStyle {
        let key = self
            .targets
            .captured
            .as_ref()
            .map(|(key, _, _)| key)
            .or(self.targets.hover.as_ref());
        let cursor = key
            .and_then(|key| {
                self.drawing
                    .scene
                    .as_ref()?
                    .targets
                    .iter()
                    .find(|target| target.id == key.0 && target.epoch == key.1)
            })
            .map(|target| target.cursor)
            .unwrap_or_default();
        use gpui_kit::CursorStyle as Native;
        match cursor {
            CanvasCursor::Arrow => Native::Arrow,
            CanvasCursor::Text => Native::IBeam,
            CanvasCursor::Pointer => Native::PointingHand,
            CanvasCursor::Crosshair => Native::Crosshair,
            CanvasCursor::Grab => Native::OpenHand,
            CanvasCursor::Grabbing => Native::ClosedHand,
            CanvasCursor::ResizeHorizontal => Native::ResizeLeftRight,
            CanvasCursor::ResizeVertical => Native::ResizeUpDown,
            CanvasCursor::NotAllowed => Native::OperationNotAllowed,
        }
    }
}
