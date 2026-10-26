//! Native frame leases expose no guest execution or source authority and complete at most once.
use plugin_protocol::{Rect, api, ui};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

/// Native handoff order is separate from opportunity sequences and carries no user content.
static HANDOFF_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// One runtime-issued request, retained while its native surface is hidden.
/// Adapters call `present` only from an actual visible GPUI frame opportunity, never a timer.
pub struct CanvasFrameResource {
    descriptor: ui::CanvasFrameRequest,
    owner: Arc<AtomicBool>,
    alive: AtomicBool,
    visible: AtomicBool,
    sequence: Arc<AtomicU64>,
    completion: Mutex<Option<ui::CanvasFrameOpportunity>>,
    handoff: AtomicU64,
    source: Option<api::DocumentVersion>,
    file: Option<api::FileVersion>,
    lineage: crate::ui_owners::Lineage,
}
impl CanvasFrameResource {
    /// Bind a request to this exact instance lifetime; only validated runtime admission constructs it.
    pub(crate) fn new(
        descriptor: ui::CanvasFrameRequest,
        owner: Arc<AtomicBool>,
        sequence: Arc<AtomicU64>,
        document: &ui::Document,
    ) -> Self {
        let lineage = crate::ui_owners::lineage(document, &descriptor.canvas).unwrap_or_default();
        Self {
            descriptor,
            owner,
            alive: AtomicBool::new(true),
            visible: AtomicBool::new(false),
            sequence,
            completion: Mutex::new(None),
            handoff: AtomicU64::new(0),
            source: document.source.clone(),
            file: document.file.clone(),
            lineage,
        }
    }
    /// Immutable instance-owned addressing; it cannot be retargeted by the native adapter.
    pub fn request(&self) -> &ui::CanvasFrameRequest {
        &self.descriptor
    }
    /// Prevent a newly admitted request from borrowing an older UI snapshot awaiting preparation.
    /// This predicate grants no authority and does not expose source text or native file paths.
    pub fn matches_document(&self, document: &ui::Document) -> bool {
        self.source==document.source&&self.file==document.file
            &&crate::ui_owners::lineage(document,&self.descriptor.canvas).as_ref()==Some(&self.lineage)
            &&crate::ui_owners::find(document,&self.descriptor.canvas).is_some_and(|node|
                matches!(&node.kind,ui::Kind::Canvas(canvas) if canvas.scene.as_ref().is_some_and(|scene|scene.identity==self.descriptor.identity)))
    }
    /// Least recently borrowed leases receive the next bounded native handoff batch.
    pub(crate) fn handoff_stamp(&self) -> u64 {
        self.handoff.load(Ordering::Relaxed)
    }
    /// Advance fairness only after a manager actually lends this lease to its native adapter.
    pub(crate) fn mark_handoff(&self) {
        self.handoff.store(
            HANDOFF_SEQUENCE.fetch_add(1, Ordering::Relaxed),
            Ordering::Relaxed,
        );
    }
    /// Pending includes hidden requests; completed or revoked requests cannot schedule another frame.
    pub fn is_pending(&self) -> bool {
        self.alive.load(Ordering::Acquire)
            && self.owner.load(Ordering::Acquire)
            && self.completion.lock().unwrap().is_none()
    }
    /// Native visibility pauses an existing request without ending business timers or other requests.
    pub fn set_visible(&self, visible: bool) {
        self.visible.store(visible, Ordering::Release);
    }
    /// Complete once from a visible native opportunity. Invalid/hidden receipts leave it pending.
    /// Returns InvalidState after completion/retirement, or InvalidRequest for invalid logical geometry.
    pub fn present(&self, viewport: Rect, interval_ms: f32) -> Result<(), api::Failure> {
        let mut completion = self.completion.lock().unwrap();
        if !self.alive.load(Ordering::Acquire)
            || !self.owner.load(Ordering::Acquire)
            || !self.visible.load(Ordering::Acquire)
            || completion.is_some()
        {
            return Err(api::Failure::new(
                api::ErrorCode::InvalidState,
                "Canvas frame is hidden, completed or retired",
            ));
        }
        let sequence = self
            .sequence
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_add(1))
            .map_err(|_| {
                api::Failure::new(
                    api::ErrorCode::LimitExceeded,
                    "Canvas frame sequence exhausted",
                )
            })?
            + 1;
        let opportunity = ui::CanvasFrameOpportunity {
            identity: self.descriptor.identity,
            request: self.descriptor.request,
            sequence,
            viewport,
            interval_ms,
            visible: true,
        };
        opportunity.validate()?;
        *completion = Some(opportunity);
        Ok(())
    }
    /// Consume before notifying the guest, so a callback may immediately request its next opportunity.
    pub(crate) fn take(&self) -> Option<ui::CanvasFrameOpportunity> {
        let mut completion = self.completion.lock().unwrap();
        if completion.is_some() {
            self.alive.store(false, Ordering::Release);
        }
        completion.take()
    }
    /// Replacement/retirement seals late callbacks and drops any undelivered completion.
    pub(crate) fn revoke(&self) {
        self.alive.store(false, Ordering::Release);
        self.visible.store(false, Ordering::Release);
        self.completion.lock().unwrap().take();
    }
}
