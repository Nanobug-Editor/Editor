//! Publish independently selected pure language roles with version/configuration-bound leases.
use super::*;
use crate::DirectProvider;
use std::sync::Arc;

pub(super) struct Prepared {
    pub(super) fingerprint: language::Fingerprint,
    pub(super) provider: Result<Arc<DirectProvider>, String>,
}
impl Drop for Prepared {
    fn drop(&mut self) {
        if let Ok(provider) = &self.provider {
            provider.retire();
        }
    }
}
impl Manager {
    /// A user-selected native fix re-enters its original instance's public file-plan admission.
    /// Host routing lends no write permission, and queued old owners cannot target replacements.
    pub fn submit_language_fix(
        &mut self,
        origin: &crate::LanguageFixOrigin,
        document: plugin_protocol::api::DocumentVersion,
        fix: plugin_protocol::diagnostics::Fix,
    ) -> Result<plugin_protocol::api::Value, plugin_protocol::api::Failure> {
        use plugin_protocol::api::{ErrorCode, Failure};
        if !self.trusted || !origin.is_live() {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Language result owner was retired",
            ));
        }
        if let crate::LanguageFixOrigin::Lsp(service)
        | crate::LanguageFixOrigin::GuardedLsp { service, .. } = origin
        {
            self.check_language_origin(service)?;
            let guards = match origin {
                crate::LanguageFixOrigin::GuardedLsp { guards, .. } => guards.clone(),
                _ => Vec::new(),
            };
            return service.submit_language_fix(document, fix, guards);
        }
        let instance = self.live.get_mut(origin.owner()).ok_or_else(|| {
            Failure::new(ErrorCode::InvalidHandle, "Language instance is unavailable")
        })?;
        if !instance.editor_authority().same_owner(origin.authority()) {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Language result belongs to an earlier instance",
            ));
        }
        instance.submit_language_fix(document, fix)
    }
    /// Publish only explicitly demanded direct executors; metadata discovery stays cold.
    pub fn direct_language_providers(
        &mut self,
    ) -> BTreeMap<String, Result<Arc<DirectProvider>, String>> {
        let expected = self.role_fingerprints();
        self.direct_language_providers
            .retain(|key, prepared| expected.get(key) == Some(&prepared.fingerprint));
        self.direct_language_providers
            .iter()
            .map(|(key, prepared)| (key.clone(), prepared.provider.clone()))
            .collect()
    }
    pub(super) fn retire_direct_providers(&mut self, id: &str) {
        self.direct_language_providers
            .retain(|key, _| !key.starts_with(&format!("{id}/")));
    }
    pub(super) fn clear_direct_providers(&mut self) {
        self.direct_language_providers.clear();
    }
    /// Count actual pure executors, independently from process and collection root counters.
    pub(super) fn direct_resource_count(&self) -> usize {
        self.direct_language_providers
            .values()
            .filter_map(|item| item.provider.as_ref().ok())
            .filter(|provider| provider.is_active())
            .count()
    }
}
