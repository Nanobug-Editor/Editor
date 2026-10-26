//! Native frame intents share the current document and existing Canvas entities, without redraw.
use super::*;
use std::sync::Arc;
impl PluginView {
    /// Route pending leases to active native canvases without notifying or invalidating this view.
    pub(crate) fn request_canvas_frames(
        &mut self,
        requests: Vec<Arc<plugin_runtime::CanvasFrameResource>>,
        cx: &mut Context<Self>,
    ) {
        if !self.scene_current.get() {
            self.pause_canvas_frames(cx);
            return;
        }
        let requests = requests
            .into_iter()
            .filter_map(|request| {
                if !request.matches_document(&self.document) {
                    return None;
                }
                let id = &request.request().canvas;
                self.document.active_node(id)?;
                let canvas = self.canvases.get(id)?.clone();
                Some((canvas, request))
            })
            .collect::<Vec<_>>();
        if requests.is_empty() {
            return;
        }
        let _ = self.window.update(cx, move |_, window, cx| {
            for (canvas, request) in requests {
                canvas.update(cx, |canvas, cx| {
                    canvas.request_native_frame(request, window, cx)
                });
            }
        });
    }
    /// Visibility/source withdrawal pauses requests without cancelling business work.
    pub(crate) fn pause_canvas_frames(&mut self, cx: &mut Context<Self>) {
        for canvas in self.canvases.values() {
            canvas.update(cx, |canvas, _| canvas.pause_frame_requests());
        }
    }
}
