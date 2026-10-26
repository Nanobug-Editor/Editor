//! Explicit native input cancellation restores only guest-committed state after mailbox overload.
use super::*;
impl PluginView {
    /// Withdraw pending drafts/capture before the producer resumes. Existing cleanup receipts are
    /// delivered through the mailbox's finite terminal reserve, after its admitted FIFO prefix.
    pub(crate) fn pause_native_input(&mut self, cx: &mut Context<Self>) -> bool {
        if self.input_backpressure {
            return false;
        }
        self.transfers.suspend(&self.document);
        self.cancel_form_delivery();
        let canvases = self.canvases.values().cloned().collect::<Vec<_>>();
        let focus = self.view_focus.clone();
        let _ = self.window.update(cx, move |_, window, cx| {
            for canvas in canvases {
                canvas.update(cx, |canvas, cx| {
                    canvas.reconcile_target_lifetimes(
                        None,
                        false,
                        ui::CanvasCancelReason::FocusLost,
                        window,
                        cx,
                    );
                    canvas.deactivate();
                    canvas.enabled = false;
                    cx.notify();
                });
            }
            focus.focus(window, cx);
        });
        self.inputs.clear();
        self.textareas.clear();
        self.input_backpressure = true;
        self.set_scene_current(false, cx);
        cx.notify();
        true
    }
    /// Rebuild editing entities from current declarations after the actor consumes cancellation.
    /// No unconfirmed native draft or old capture survives this transition.
    pub(crate) fn resume_native_input(&mut self, current: bool, cx: &mut Context<Self>) {
        if !self.input_backpressure {
            return;
        }
        self.input_backpressure = false;
        self.set_scene_current(current, cx);
        let owner = cx.weak_entity();
        let window = self.window;
        cx.defer(move |cx| {
            let _ = window.update(cx, move |_, window, cx| {
                let _ = owner.update(cx, |view, cx| {
                    if !view.input_backpressure {
                        view.sync_native(window, cx);
                        cx.notify();
                    }
                });
            });
        });
        cx.notify();
    }
}
