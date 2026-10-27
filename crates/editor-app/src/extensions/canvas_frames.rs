//! Worker intents wait for their original visible native surface instead of a timer redraw.
use super::*;
impl ExtensionPanel {
    /// Missing/hidden native views retain their runtime lease until a visible surface is mounted.
    pub(super) fn schedule_canvas_frames(&mut self, cx: &mut Context<Self>) {
        let natives = self
            .native_ui
            .iter()
            .chain(self.native_toolbar.iter())
            .cloned()
            .collect::<Vec<_>>();
        if natives.is_empty() {
            return;
        }
        if !self.visible.get() {
            for native in natives {
                native.update(cx, |view, cx| view.pause_canvas_frames(cx));
            }
            return;
        }
        let (Some(plugin), Some(panel)) = (self.active.as_ref(), self.surface_id.as_ref()) else {
            return;
        };
        let requests = {
            let state = self.worker.state.lock().unwrap();
            if state.instance_epochs.get(plugin).copied() != Some(self.instance_epoch) {
                return;
            }
            state
                .canvas_frames
                .iter()
                .filter(|(owner, request)| {
                    owner == plugin && request.request().panel == *panel && request.is_pending()
                })
                .map(|(_, request)| request.clone())
                .collect::<Vec<_>>()
        };
        for native in natives {
            native.update(cx, |view, cx| {
                view.request_canvas_frames(requests.clone(), cx)
            });
        }
    }
}
