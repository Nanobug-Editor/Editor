//! A full native mailbox visibly pauses the original surface until cleanup executes in order.
use super::*;
impl ExtensionPanel {
    /// Resume is an actor marker behind both admitted input and native cancellation receipts.
    pub(super) fn reconcile_input_backpressure(&mut self, cx: &mut Context<Self>) {
        let (Some(plugin), Some(panel)) = (self.active.clone(), self.surface_id.clone()) else {
            return;
        };
        let paused = self
            .worker
            .tx
            .input_paused(&plugin, self.instance_epoch, Some(&panel));
        let natives = self
            .native_ui
            .iter()
            .chain(self.native_toolbar.iter())
            .cloned()
            .collect::<Vec<_>>();
        if !natives.is_empty() {
            if paused {
                let mut cancelled = false;
                for native in natives {
                    cancelled |= native.update(cx, |view, cx| view.pause_native_input(cx));
                }
                if cancelled {
                    self.worker
                        .tx
                        .resume_input(&plugin, self.instance_epoch, Some(&panel));
                }
            } else {
                let current = self.visible.get() && self.current_document().is_some();
                for native in natives {
                    native.update(cx, |view, cx| view.resume_native_input(current, cx));
                }
            }
        } else if paused {
            // Its native owner has already unmounted; existing window-close cleanup precedes resume.
            self.worker
                .tx
                .resume_input(&plugin, self.instance_epoch, Some(&panel));
        }
    }
}
