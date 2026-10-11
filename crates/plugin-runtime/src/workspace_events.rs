//! Workspace observation ingress and each instance queue are bounded independently of native UI.
use plugin_protocol::{
    api::{ErrorCode, Failure, ResourceHandle},
    workspace::{FileEvent, FileEventKind},
};
use std::collections::VecDeque;

/// Native paths are converted to portable relative observations before entering this shared queue.
#[derive(Default)]
pub struct WorkspaceEvents {
    pending: VecDeque<FileEvent>,
    last: u64,
    failure: Option<Failure>,
}

impl WorkspaceEvents {
    /// Coalesce only repeated modifications; create/delete/rename transitions keep FIFO semantics.
    pub fn push(&mut self, event: FileEvent) {
        if self.failure.is_some() || event.sequence <= self.last {
            return;
        }
        self.last = event.sequence;
        if let FileEventKind::Modified { path } = &event.kind {
            if let Some(index) = self.pending.iter().position(|old| matches!(&old.kind, FileEventKind::Modified { path: old_path } if old_path == path)) { self.pending.remove(index); }
        }
        if self.pending.len() >= 128 {
            self.fail(Failure::new(
                ErrorCode::LimitExceeded,
                "Workspace stream overflow; rescan, reread and resubscribe",
            ));
        } else {
            self.pending.push_back(event);
        }
    }

    /// One explicit terminal error replaces incomplete buffered transitions.
    pub fn fail(&mut self, failure: Failure) {
        self.pending.clear();
        self.failure = Some(failure);
    }

    /// The serial worker drains a bounded batch without changing original sequence numbers.
    pub fn take_batch(&mut self, limit: usize) -> Result<Vec<FileEvent>, Failure> {
        if let Some(failure) = self.failure.take() {
            return Err(failure);
        }
        Ok((0..limit)
            .filter_map(|_| self.pending.pop_front())
            .collect())
    }
}

/// Releasing either the observation or its original workspace root prevents further delivery.
pub(crate) struct Subscription {
    pub handle: ResourceHandle,
    pub root: crate::AuthorizedFile,
    pub events: WorkspaceEvents,
}
