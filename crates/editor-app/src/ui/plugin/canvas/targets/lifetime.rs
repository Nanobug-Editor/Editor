//! Withdraw native ownership before replacing declarations, retaining only original-scoped cleanup.
use super::*;
use protocol::ui::CanvasCancelReason;

impl CanvasView {
    /// Native window deactivation cancels even nonfocusable pointer owners. Logical focus may stay
    /// on the same handle, so reactivation publishes a fresh observation without reviving old capture.
    pub(in crate::ui::plugin::canvas) fn reconcile_target_activation(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        if window.is_window_active() {
            let focused = self
                .targets
                .focus
                .iter()
                .filter(|(_, handle)| handle.is_focused(window))
                .map(|(key, _)| key.clone())
                .collect::<Vec<_>>();
            for key in focused {
                self.emit_target(key, CanvasInputKind::Focus { focused: true }, cx);
            }
        } else {
            self.cancel_target_pointer(CanvasCancelReason::FocusLost, cx);
            let sessions = self.targets.sessions.keys().cloned().collect::<Vec<_>>();
            for key in sessions {
                self.cancel_target_text(&key, cx);
            }
            let focused = self.targets.focused.iter().cloned().collect::<Vec<_>>();
            for key in focused {
                self.emit_target(key, CanvasInputKind::Focus { focused: false }, cx);
            }
        }
        cx.notify();
    }

    /// Reconcile against the next active Canvas before source/gesture/document identities change.
    /// Stable target generations preserve focus, drafts and capture; revoked native endpoints cannot revive.
    pub(in crate::ui::plugin) fn reconcile_target_lifetimes(
        &mut self,
        next: Option<&Canvas>,
        source_current: bool,
        reason: CanvasCancelReason,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let scene = next
            .and_then(|canvas| canvas.scene.as_ref())
            .filter(|scene| {
                source_current
                    && self
                        .drawing
                        .scene
                        .as_ref()
                        .is_some_and(|old| old.identity == scene.identity)
            });
        let declaration = |key: &(String, u64)| {
            scene.and_then(|scene| {
                scene
                    .targets
                    .iter()
                    .find(|target| target.id == key.0 && target.epoch == key.1)
            })
        };
        let mut retired = self
            .targets
            .sessions
            .keys()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>();
        retired.extend(self.targets.focus.keys().cloned());
        for key in retired {
            let target = declaration(&key);
            let text_retired = target.is_none_or(|target| target.text_input.is_none());
            let focus_retired = target.is_none_or(|target| !target.focusable);
            if text_retired || focus_retired {
                self.cancel_target_text(&key, cx);
                // Drop the revocation owner before any platform callback can see the next declaration.
                self.targets.sessions.remove(&key);
                self.targets.text_errors.remove(&key);
            }
            if focus_retired {
                if self
                    .targets
                    .focus
                    .get(&key)
                    .is_some_and(|focus| focus.is_focused(window))
                {
                    self.emit_target(key.clone(), CanvasInputKind::Focus { focused: false }, cx);
                }
                self.targets.subscriptions.remove(&key);
                self.targets.focus.remove(&key);
            }
        }
        if let Some((key, _, _)) = &self.targets.captured {
            if declaration(key).is_none_or(|target| !target.pointer_events) {
                let reason = if scene.is_some_and(|scene| {
                    scene
                        .targets
                        .iter()
                        .any(|target| target.id == key.0 && target.epoch != key.1)
                }) {
                    CanvasCancelReason::EpochReplaced
                } else if scene.is_some() {
                    CanvasCancelReason::TargetRemoved
                } else {
                    reason
                };
                self.cancel_target_pointer(reason, cx);
            }
        }
        if self
            .targets
            .hover
            .as_ref()
            .is_some_and(|key| declaration(key).is_none_or(|target| !target.pointer_events))
        {
            self.targets.hover = None;
        }
        if scene.is_none() {
            self.targets.clear();
        }
    }
}
