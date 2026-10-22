//! Cache bounded LSP preparation results by package/configuration/workspace, preserving unaffected services.
use super::*;
use crate::LanguageService;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
mod contexts;
mod delegation;
mod fingerprint;
mod selection;
pub(super) use fingerprint::Fingerprint;

pub(super) struct Prepared {
    pub(super) fingerprint: Fingerprint,
    pub(super) service: Result<Arc<LanguageService>, String>,
    pub(super) context: Option<contexts::Binding>,
    /// Pending intent remains in the same scope-owned cache, but is never published as a ready plan.
    pub(super) pending: bool,
    /// Borrowed aliases never retire or account for the original provider's native transport.
    pub(super) owns_service: bool,
    pub(super) delegation: Option<delegation::Pending>,
    /// Original collection metadata can withdraw a terminal failure after its inputs change.
    /// Pending and healthy plans retain their existing authority and lease retirement rules.
    pub(super) failure_inputs: Option<crate::plugin_services::CollectionPublication>,
    /// Original reservation evidence survives cache withdrawal for its already admitted native wait.
    /// Only an actual terminal Err publication sets it; cancellation or request admission cannot.
    pub(super) failure_publication: Arc<AtomicBool>,
}
impl Drop for Prepared {
    fn drop(&mut self) {
        if self.owns_service
            && let Ok(service) = &self.service
        {
            service.retire();
        }
    }
}
impl Manager {
    /// Read previously demanded leases only; catalog publication never compiles or runs a guest.
    /// Explicit preparation failures remain visible under their original provider key.
    pub fn language_services(&mut self) -> BTreeMap<String, Result<Arc<LanguageService>, String>> {
        self.refresh_services();
        let expected = self.role_fingerprints();
        let stale_failures = {
            let broker = self.plugin_services.lock().unwrap();
            self.language_services
                .iter()
                .filter(|(_, prepared)| {
                    !prepared.pending
                        && prepared.service.is_err()
                        && prepared.failure_inputs.as_ref().is_some_and(|original| {
                            !broker.collection_publication_is_current(original)
                        })
                })
                .map(|(key, _)| key.clone())
                .collect::<BTreeSet<_>>()
        };
        // Same-owner HotApply hides an unvalidated configuration without killing its original process.
        // Metadata/grant/workspace retirement still revokes the lease immediately.
        self.language_services.retain(|key, prepared| {
            !stale_failures.contains(key)
                && prepared
                    .service
                    .as_ref()
                    .map_or(true, |service| service.is_active())
                && expected
                    .get(contexts::provider_key(key))
                    .is_some_and(|fingerprint| fingerprint.same_source(&prepared.fingerprint))
        });
        self.reconcile_language_context_authority();
        self.language_services
            .iter()
            .filter(|(key, prepared)| {
                !prepared.pending
                    && expected.get(contexts::provider_key(key)) == Some(&prepared.fingerprint)
            })
            .map(|(key, prepared)| (key.clone(), prepared.service.clone()))
            .collect()
    }
    /// Native plans bind digest, grant set, effective configuration, scope and SDK without printing
    /// settings. Issuance and later submission use the same identity, so stale metadata cannot lend
    /// authority while a publication snapshot is waiting for the UI or worker's next tick.
    pub(super) fn language_fingerprint(
        &self,
        entry: &Installed,
        values: &Result<plugin_protocol::settings::Effective, String>,
    ) -> Fingerprint {
        Fingerprint::new(
            entry,
            &self.environment.workspace,
            values,
            &self.host_resources.sdk,
        )
    }

    /// Capture metadata at reservation, including indirect collections declared by already published
    /// scope participants. Conservatively, any such collection change may retry a failed purpose;
    /// healthy plans remain untouched. No metadata is sampled again after the failure occurs.
    /// Returns LimitExceeded if the Broker's existing bounded collection metadata table is full.
    pub(super) fn language_failure_inputs(
        &self,
        provider: &str,
    ) -> Result<crate::plugin_services::CollectionPublication, api::Failure> {
        let owner = provider.split('/').next().unwrap();
        let entry = &self.installed[owner];
        let scope = if entry.manifest.scope == api::InstanceScope::Application {
            "application".into()
        } else {
            self.host_scope()
        };
        let mut dependencies = entry
            .manifest
            .plugin_services
            .requires
            .iter()
            .filter(|(_, dependency)| dependency.collection)
            .map(|(contract, dependency)| (contract.clone(), dependency.clone()))
            .collect::<BTreeMap<_, _>>();
        let mut broker = self.plugin_services.lock().unwrap();
        for plugin in broker.scoped_provider_plugins(&scope) {
            if let Some(published) = self.installed.get(&plugin) {
                for (contract, dependency) in &published.manifest.plugin_services.requires {
                    if dependency.collection {
                        dependencies
                            .entry(contract.clone())
                            .or_insert_with(|| dependency.clone());
                    }
                }
            }
        }
        broker.capture_collection_publication(&scope, &dependencies)
    }

    /// The selected role's actual approved plan decides whether HotApply preserves its transport.
    /// Reusing the cache entry retains its exact original native authority and completed resources;
    /// replacing or failing the plan drops that entry and seals every old retained handle.
    pub(super) fn publish_language_plan(
        &mut self,
        key: String,
        fingerprint: Fingerprint,
        service: Result<Arc<LanguageService>, String>,
    ) {
        if let Some(previous) = self.language_services.get_mut(&key)
            && previous.fingerprint.same_source(&fingerprint)
            && let (Ok(original), Ok(candidate)) = (&previous.service, &service)
            && original.same_plan(candidate)
        {
            previous.fingerprint = fingerprint;
            return;
        }
        self.language_services.insert(
            key,
            Prepared {
                fingerprint,
                service,
                context: None,
                pending: false,
                owns_service: true,
                delegation: None,
                // Workspace-only pure preparation has no source reservation or service discovery.
                failure_inputs: None,
                failure_publication: Arc::new(AtomicBool::new(false)),
            },
        );
    }

    /// The public native result carries its actual cache Arc, never merely a reusable package ID.
    /// Workspace effects additionally reject application scope and any unpublished grant/version.
    pub(super) fn check_language_origin(
        &self,
        service: &Arc<LanguageService>,
    ) -> Result<(), plugin_protocol::api::Failure> {
        use plugin_protocol::api::{ErrorCode, Failure, InstanceScope};
        let entry = self.installed.get(&service.owner).ok_or_else(|| {
            Failure::new(ErrorCode::InvalidHandle, "Language package is unavailable")
        })?;
        if !self.workspace_open
            || entry.manifest.scope != InstanceScope::Workspace
            || !(entry.enabled || entry.project_enabled_in(&self.environment.workspace))
            || entry.error.is_some()
        {
            return Err(Failure::new(
                ErrorCode::PermissionDenied,
                "No published workspace language authority",
            ));
        }
        let prepared = self
            .language_services
            .values()
            .find(|prepared| {
                prepared.owns_service
                    && !prepared.pending
                    && prepared
                        .service
                        .as_ref()
                        .is_ok_and(|current| Arc::ptr_eq(current, service))
            })
            .ok_or_else(|| {
                Failure::new(
                    ErrorCode::InvalidHandle,
                    "Language service is no longer published",
                )
            })?;
        let values = self
            .effective_settings(&service.owner)
            .map_err(|error| format!("{error:#}"));
        if !prepared
            .service
            .as_ref()
            .is_ok_and(|current| Arc::ptr_eq(current, service))
            || prepared.fingerprint != self.language_fingerprint(entry, &values)
        {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Language service metadata or owner changed",
            ));
        }
        Ok(())
    }

    /// The actor transfers WASM and controlled native calls through one owned dispatcher. Each
    /// issuer remains responsible for permission, lifetime and quota checks before publication.
    pub fn take_editor_requests(&mut self) -> Vec<(String, crate::EditorRequest)> {
        let mut requests = Vec::new();
        for (id, instance) in &mut self.live {
            requests.extend(
                instance
                    .take_editor_requests()
                    .into_iter()
                    .map(|request| (id.clone(), request)),
            );
        }
        for service in self
            .language_services
            .values()
            .filter(|prepared| prepared.owns_service)
            .filter_map(|prepared| prepared.service.as_ref().ok())
        {
            requests.extend(
                service
                    .take_editor_requests()
                    .into_iter()
                    .map(|request| (service.owner.clone(), request)),
            );
        }
        // Native virtual tabs and recovery receipts can outlive their final source. Once their
        // arena has polled terminal/revoked resources, release the now-unused context owner too.
        self.collect_unused_language_contexts();
        requests
    }

    /// Called only on successful replacement or revocation; failed configuration preserves old leases.
    pub(super) fn retire_language_services(&mut self, id: &str) {
        self.language_services
            .retain(|key, _| !key.starts_with(&format!("{id}/")));
        self.collect_unused_language_contexts();
        // Every pure language role loses authority at the same package retirement boundary.
        self.retire_structure_providers(id);
        self.retire_direct_providers(id);
    }
}
