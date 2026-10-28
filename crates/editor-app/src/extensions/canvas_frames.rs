//! Worker intents wait for their original visible native surface instead of a timer redraw.
use super::*;
#[cfg(test)]
mod tests;

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
        let visible = self.visible.get();
        let requests = if visible {
            let (Some(plugin), Some(panel)) = (self.active.as_ref(), self.surface_id.as_ref())
            else {
                return;
            };
            let state = self.worker.state.lock().unwrap();
            if state.instance_epochs.get(plugin).copied() != Some(self.instance_epoch) {
                return;
            }
            let requests = state
                .canvas_frames
                .iter()
                .filter(|(owner, request)| {
                    owner == plugin && request.request().panel == *panel && request.is_pending()
                })
                .map(|(_, request)| request.clone())
                .collect::<Vec<_>>();
            // Ordinary source toolbars have no frame intents. Their render must not update a
            // native body which may already hold the borrow that is rendering this source pane.
            if requests.is_empty() {
                return;
            }
            requests
        } else {
            Vec::new()
        };
        let owner = cx.entity().downgrade();
        let plugin = self.active.clone();
        let panel = self.surface_id.clone();
        let epoch = self.instance_epoch;
        // A composed native Editor calls back into its source toolbar while PluginView is leased.
        // Route after that render borrow ends, retaining the original surface and runtime leases.
        cx.defer(move |cx| {
            let Some(owner) = owner.upgrade() else {
                return;
            };
            let (natives, current_visible) = {
                let current = owner.read(cx);
                if current.active != plugin
                    || current.surface_id != panel
                    || current.instance_epoch != epoch
                    || (!visible && current.visible.get())
                {
                    return;
                }
                let natives = natives
                    .into_iter()
                    .filter(|native| {
                        current.native_ui.as_ref() == Some(native)
                            || current.native_toolbar.as_ref() == Some(native)
                    })
                    .collect::<Vec<_>>();
                (natives, current.visible.get())
            };
            for native in natives {
                native.update(cx, |view, cx| {
                    if current_visible {
                        // The view checks exact source/scene identity again before borrowing a
                        // pending request; publication changes cannot retarget the captured lease.
                        view.request_canvas_frames(requests.clone(), cx);
                    } else {
                        view.pause_canvas_frames(cx);
                    }
                });
            }
        });
    }
}
