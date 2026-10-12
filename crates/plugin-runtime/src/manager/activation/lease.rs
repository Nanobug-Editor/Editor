//! A taken job and its unpublished result retain one worker reservation until either is consumed or dropped.
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

/// The flight observes this counter without owning a reservation; no host-side drop can strand a slot.
pub(super) struct PreparationLease(Arc<AtomicUsize>);
impl PreparationLease {
    pub(super) fn new() -> Self {
        Self(Arc::new(AtomicUsize::new(1)))
    }
    pub(super) fn counter(&self) -> Arc<AtomicUsize> {
        self.0.clone()
    }
}
impl Clone for PreparationLease {
    fn clone(&self) -> Self {
        self.0.fetch_add(1, Ordering::Relaxed);
        Self(self.0.clone())
    }
}
impl Drop for PreparationLease {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Release);
    }
}

/// Keep already typed permission/capability/cancellation failures through detached preparation.
pub(super) fn failure(error: anyhow::Error) -> plugin_protocol::api::Failure {
    error
        .downcast_ref::<plugin_protocol::api::Failure>()
        .cloned()
        .unwrap_or_else(|| {
            plugin_protocol::api::Failure::new(
                plugin_protocol::api::ErrorCode::OperationFailed,
                format!("{error:#}"),
            )
        })
}
