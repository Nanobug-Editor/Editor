//! Analysis seals narrow preparation until a real effect, without becoming persistent ownership.
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// One admitted native plan shares this phase between its pending request and recovery resource.
/// Consumption never changes the independent owner/caller chain or revives a retired resource.
pub(crate) struct PreparationConditions {
    guards: Vec<Arc<AtomicBool>>,
    consumed: AtomicBool,
}

impl PreparationConditions {
    /// Retain immutable monotonic seals; the native issuer checks them again before allocation.
    pub(crate) fn new(guards: Vec<Arc<AtomicBool>>) -> Self {
        Self {
            guards,
            consumed: AtomicBool::new(false),
        }
    }

    /// Before effects every source/configuration/connection seal must remain valid. Afterwards the
    /// request reports actual outcomes and its receipt is governed solely by its original authority.
    pub(crate) fn is_live(&self) -> bool {
        self.consumed.load(Ordering::Acquire)
            || self
                .guards
                .iter()
                .all(|guard| guard.load(Ordering::Acquire))
    }

    /// Only the native coordinator calls this immediately after a real mutation succeeds. Entering
    /// execution, acknowledging a noop or cancelling a wait cannot consume analysis preconditions.
    pub(crate) fn record_effect(&self) {
        self.consumed.store(true, Ordering::Release);
    }
}
