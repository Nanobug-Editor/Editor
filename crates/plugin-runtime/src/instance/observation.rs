//! Observations read the actual instance maps and never become another owner or permission gate.
use super::*;
use crate::observability::{Owner, ResourceCounts};

impl State {
    /// Preparation assigns the owner before WASM instantiation; absent metadata remains explicitly unknown.
    pub(super) fn observation_identity(&self) -> Owner {
        self.observation_owner.clone().unwrap_or_else(|| {
            let principal = &self.plugin_services.principal;
            Owner::new(&principal.plugin, &principal.instance, &principal.scope)
        })
    }
}
impl Instance {
    /// Copy scalar counts from this exact incarnation; no view or report retains runtime resources.
    pub fn resource_counts(&self) -> ResourceCounts {
        let state = self.store.data();
        let (native_pending, native_cleanup_unconfirmed) = state.processes.cleanup_evidence();
        ResourceCounts {
            committed: state.activation_committed,
            handles: state.roots.len(),
            requests: state.editor_requests.len() + state.plugin_services.pending.len(),
            http: state.http_streams.len(),
            timers: state.timers.len(),
            subscriptions: state.subscriptions.len()
                + state.document_streams.len()
                + state.file_subscriptions.len()
                + state.preference_subscriptions.len(),
            projections: state.trees.len()
                + state.virtual_documents.len()
                + state.diagnostic_collections.len()
                + state.decorations.len(),
            processes: state.process_handles.len(),
            native_pending,
            native_cleanup_unconfirmed,
            retired: state.roots.retired,
        }
    }

    /// Report a current owner projection, including the final revoked state after quiesce.
    pub(crate) fn sample_resources(&self) {
        self.store.data().host_resources.observations.resources(
            self.store.data().observation_identity(),
            self.resource_counts(),
        );
    }
}
