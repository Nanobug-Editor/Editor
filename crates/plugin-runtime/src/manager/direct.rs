//! Publish independently selected pure language roles with version/configuration-bound leases.
use super::*;
use crate::DirectProvider;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64},
};

pub(super) struct Prepared {
    fingerprint: Vec<u8>,
    provider: Result<Arc<DirectProvider>, String>,
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
        if let crate::LanguageFixOrigin::Lsp(service) = origin {
            self.check_language_origin(service)?;
            return service.submit_language_fix(document, fix);
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
    /// Optional unsupported experimental roles are omitted without removing other contributions.
    pub fn direct_language_providers(
        &mut self,
    ) -> BTreeMap<String, Result<Arc<DirectProvider>, String>> {
        let mut wanted = BTreeSet::new();
        for entry in self
            .published_entries()
            .into_iter()
            .filter(|entry| entry.enabled && entry.error.is_none())
        {
            if crate::capabilities::negotiate(&entry.manifest).map_or(true, |api| {
                !api.capabilities.contains_key("language.direct")
            }) {
                continue;
            }
            for declaration in &entry.manifest.direct_language_providers {
                let key = format!("{}/{}", entry.manifest.id, declaration.id);
                wanted.insert(key.clone());
                let settings = self
                    .effective_settings(&entry.manifest.id)
                    .map_err(|error| format!("{error:#}"));
                let fingerprint = serde_json::to_vec(&(
                    &entry.digest,
                    &self.environment.workspace,
                    &settings,
                    &self.host_resources.sdk,
                ))
                .unwrap();
                if self
                    .direct_language_providers
                    .get(&key)
                    .is_some_and(|old| old.fingerprint == fingerprint)
                {
                    continue;
                }
                let provider = settings.and_then(|settings| {
                    self.prepare_direct(&entry, declaration.clone(), settings)
                        .map(Arc::new)
                        .map_err(|error| format!("{error:#}"))
                });
                self.direct_language_providers.insert(
                    key,
                    Prepared {
                        fingerprint,
                        provider,
                    },
                );
            }
        }
        self.direct_language_providers
            .retain(|key, _| wanted.contains(key));
        self.direct_language_providers
            .iter()
            .map(|(key, prepared)| (key.clone(), prepared.provider.clone()))
            .collect()
    }
    fn prepare_direct(
        &self,
        entry: &Installed,
        declaration: plugin_protocol::direct::Provider,
        settings: plugin_protocol::settings::Effective,
    ) -> anyhow::Result<DirectProvider> {
        anyhow::ensure!(
            entry.grants.contains("editor.read"),
            "Direct language requires editor.read"
        );
        let owner = self
            .live
            .get(&entry.manifest.id)
            .ok_or_else(|| anyhow::anyhow!("Direct owner is unavailable"))?;
        let assets = self
            .root
            .join("packages")
            .join(&entry.manifest.id)
            .join(&entry.digest);
        let bytes = std::fs::read(crate::instance::safe_path(
            &assets,
            entry
                .manifest
                .component
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("Direct language requires WASM"))?,
            true,
        )?)?;
        let instance = Instance::prepare_language_worker(
            self.engine
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Plugin engine unavailable"))?,
            &bytes,
            &entry.manifest,
            &entry.grants,
            self.environment.clone(),
            self.data_directory(&entry.manifest.id),
            assets,
            self.host_resources.clone(),
        )?;
        Ok(DirectProvider {
            owner: entry.manifest.id.clone(),
            declaration,
            instance: Mutex::new(Some(instance)),
            settings,
            authority: owner.editor_authority(),
            active: AtomicBool::new(true),
            sequence: AtomicU64::new(1),
            workspace_read: entry.grants.contains("workspace.read"),
        })
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
