//! Native dimensions are resampled after actor rejection, never retargeted to a newer scene.
use super::*;
use futures::channel::oneshot;

#[cfg(test)]
mod tests;

/// `true` means the exact original Resize was rejected as stale. Success, retirement and other
/// failures do not request another sample; standalone controls retain their ordinary event sink.
pub(crate) type ResizeSink = Rc<dyn Fn(UiEvent, &mut App) -> oneshot::Receiver<bool>>;

/// Both ordinary and projected canvases report native dimensions through the same admission gate.
pub(crate) fn is_canvas_resize(action: &Action) -> bool {
    matches!(
        action,
        Action::Canvas(ui::CanvasEvent::Resize { .. })
            | Action::ViewportInput(ui::ViewportInput {
                event: ui::CanvasEvent::Resize { .. },
                ..
            })
    )
}

impl PluginView {
    /// Attach actor feedback to this surface's original plugin incarnation and visibility owner.
    pub(crate) fn with_resize_receipts(mut self, sink: Option<ResizeSink>) -> Self {
        self.resize_sink = sink;
        self
    }

    /// Only native Resize uses this transport. A stale receipt invalidates the still-owned cache;
    /// its next real layout produces new coordinates and credentials instead of replaying input.
    pub(super) fn deliver_resize(&self, event: &UiEvent, cx: &mut Context<Self>) -> bool {
        if !is_canvas_resize(&event.action) {
            return false;
        }
        let Some(sink) = &self.resize_sink else {
            return false;
        };
        let Some(canvas) = self.canvases.get(&event.node) else {
            return false;
        };
        let identity = canvas.entity_id();
        let id = event.node.clone();
        let revision = event.revision;
        let geometry = match &event.action {
            Action::Canvas(event) | Action::ViewportInput(ui::ViewportInput { event, .. }) => {
                event.clone()
            }
            _ => unreachable!(),
        };
        let receipt = sink(event.clone(), cx);
        cx.spawn(async move |owner, cx| {
            if receipt.await != Ok(true) {
                return;
            }
            let _ = owner.update(cx, |owner, cx| {
                let Some(canvas) = owner
                    .canvases
                    .get(&id)
                    .filter(|canvas| canvas.entity_id() == identity)
                else {
                    return;
                };
                let redraw = owner.scene_current.get() && owner.document.revision > revision;
                canvas.update(cx, |canvas, cx| {
                    // A newer native size already supersedes this receipt. A removed view or
                    // changed incarnation owns a different entity and cannot be revived here.
                    if canvas.invalidate_dimensions(&geometry) && redraw && canvas.enabled {
                        cx.notify();
                    }
                });
            });
        })
        .detach();
        true
    }
}
