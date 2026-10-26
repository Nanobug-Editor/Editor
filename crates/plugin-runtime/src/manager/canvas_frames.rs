//! Native frame adapters borrow bounded instance-issued leases without activating cold plugins.
use super::*;
use std::sync::Arc;
impl Manager {
    /// Return pending visible-frame intents, capped at 256 native handoffs per polling round.
    /// Repeated borrowing is idempotent; hidden requests remain pending until a real opportunity.
    /// Every resource seals itself on completion, source replacement or instance retirement.
    pub fn take_canvas_frame_requests(&self) -> Vec<(String, Arc<crate::CanvasFrameResource>)> {
        let mut pending = self
            .live
            .iter()
            .flat_map(|(plugin, instance)| {
                instance
                    .canvas_frame_requests()
                    .into_iter()
                    .map(|request| (plugin.clone(), request))
            })
            .collect::<Vec<_>>();
        // A hidden first batch must not starve visible requests owned by later canvases.
        pending.sort_by_key(|(_, request)| request.handoff_stamp());
        pending.truncate(256);
        for (_, request) in &pending {
            request.mark_handoff();
        }
        pending
    }
}
