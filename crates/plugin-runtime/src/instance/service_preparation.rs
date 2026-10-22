//! Readonly service preparation narrows owned immutable plans before asynchronous acquisition.
use super::*;
use api::{ErrorCode, Failure};

impl Instance {
    /// Select only a duplicate-free subset of the admitted method's declarations. Native effects,
    /// UI and persisted state are unavailable; ephemeral file handles are released on every outcome.
    pub(super) fn select_service_preparation(
        &mut self,
        call: &crate::plugin_services::Call,
        allowed: Vec<String>,
    ) -> Result<Vec<String>, Failure> {
        if !self.store.data().service_preparation_hook {
            return Ok(allowed);
        }
        let checkpoint = self.store.data().roots.checkpoint();
        let previous = self
            .store
            .data_mut()
            .plugin_services
            .context
            .replace(call.context.clone());
        self.store.data_mut().language_hook = true;
        self.store.data_mut().service_preparation_call = true;
        self.store.data_mut().language_hook_checkpoint = Some(checkpoint);
        let reply = self.call(api::Input::Event {
            panel: None,
            event: api::Notification::ServicePreparation(plugin_protocol::service::Preparation {
                contract: call.reference.contract.clone(),
                method: call.method.clone(),
                arguments: call.arguments.clone(),
                settings: self.configuration.clone(),
                services: allowed.clone(),
            }),
        });
        let state = self.store.data_mut();
        state.language_hook = false;
        state.service_preparation_call = false;
        state.language_hook_checkpoint = None;
        state.plugin_services.context = previous;
        state.roots.release_since(checkpoint);
        state
            .plugin_services
            .resources
            .retain(|slot, _| *slot < checkpoint);
        let reply = reply.map_err(|error| {
            Failure::new(
                ErrorCode::OperationFailed,
                format!("Native preparation selection failed: {error:#}"),
            )
        })?;
        if !reply.views.is_empty()
            || reply.snapshot.is_some()
            || reply.configuration.is_some()
            || reply.language_service.is_some()
            || reply.service_reply.is_some()
            || reply.language_completion.is_some()
            || reply.language_structure.is_some()
            || reply.language_direct.is_some()
        {
            return Err(Failure::new(
                ErrorCode::InvalidRequest,
                "Preparation selection returned unrelated output",
            ));
        }
        let ids = reply.service_preparation.ok_or_else(|| {
            Failure::new(ErrorCode::InvalidRequest, "Missing preparation selection")
        })?;
        let unique: BTreeSet<_> = ids.iter().collect();
        if ids.len() > allowed.len()
            || unique.len() != ids.len()
            || ids.iter().any(|id| !allowed.contains(id))
        {
            return Err(Failure::new(
                ErrorCode::PermissionDenied,
                "Preparation selection exceeded its declared services",
            ));
        }
        Ok(ids)
    }
}
