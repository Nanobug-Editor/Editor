//! A hot transition validates in isolation, saves once, then delivers to the same admitted owners.
use super::*;

/// Capture a logical scope and its validated values without borrowing its live Store across preparation.
struct ValidatedOwner {
    scope: String,
    values: Effective,
}

impl Manager {
    /// This branch is connected only for declared configuration-1.1 HotApply values.
    /// Validation failure precedes persistence; delivery failure follows persistence and retires that owner.
    pub(super) fn apply_hot_setting(
        &mut self,
        entry: &Installed,
        scope: Scope,
        saved: &SavedSettings,
        path: &Path,
        bytes: &[u8],
    ) -> anyhow::Result<()> {
        let id = &entry.manifest.id;
        let (assets, package) = self.read_installed_assets(entry)?;
        let component = package
            .manifest
            .component
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Hot configuration requires a component"))?;
        if self.engine.is_none() {
            self.engine = Some(Instance::engine()?);
        }
        let selected_scope = scopes::workspace_key(&self.environment.workspace);
        let mut scopes = vec![(
            if entry.manifest.scope == api::InstanceScope::Application {
                "application".into()
            } else {
                selected_scope.clone()
            },
            self.environment.clone(),
        )];
        if scope == Scope::User && entry.manifest.scope == api::InstanceScope::Workspace {
            scopes.extend(
                self.parked
                    .iter()
                    .filter(|(_, parked)| parked.trusted)
                    .map(|(key, parked)| (key.clone(), parked.environment.clone())),
            );
        }
        let mut validated = Vec::new();
        for (owner_scope, environment) in scopes {
            // The candidate receives only logical state. It never activates business or acquires the old
            // owner's committed Secret/Temporary lifetime, so rejecting it cannot clean live files.
            let snapshot = if owner_scope == "application" || owner_scope == selected_scope {
                self.live.get_mut(id).map(Instance::snapshot).transpose()?
            } else {
                self.parked
                    .get_mut(&owner_scope)
                    .and_then(|parked| parked.live.get_mut(id))
                    .map(Instance::snapshot)
                    .transpose()?
            };
            let mut candidate = Instance::prepare_with_resources(
                self.engine.as_ref().unwrap(),
                &package.files[component],
                &entry.manifest,
                &entry.grants,
                environment.clone(),
                self.data_directory_for(&entry.manifest, &environment),
                assets.clone(),
                snapshot,
                self.host_resources.clone(),
            )?;
            let values = candidate.validate_settings(
                &entry.manifest,
                saved.resolve(&entry.manifest, &environment.workspace)?,
            )?;
            validated.push(ValidatedOwner {
                scope: owner_scope,
                values,
            });
        }
        std::fs::create_dir_all(path.parent().unwrap())?;
        atomic_write(path, bytes)?;
        // Configuration revocation abandons queued old intent, but deliberately leaves the logical
        // availability seal and already bound service/resource owner intact on successful Changed.
        self.cancel_pending_demands(
            id,
            (scope == Scope::Project).then_some(selected_scope.as_str()),
        );
        let mut failures = Vec::new();
        for owner in validated {
            let instances = if owner.scope == "application" || owner.scope == selected_scope {
                &mut self.live
            } else {
                &mut self.parked.get_mut(&owner.scope).unwrap().live
            };
            let result = instances
                .get_mut(id)
                .map(|instance| instance.apply_hot_settings(&entry.manifest, owner.values));
            if let Some(Err(error)) = result {
                // Saved values are authoritative after the atomic write. A failed Changed cannot
                // promise guest-memory/external rollback, so a later explicit demand starts fresh.
                if let Some(mut instance) = instances.remove(id) {
                    instance.stop();
                }
                failures.push(format!("scope={}: {error:#}", owner.scope));
            }
        }
        // Read-only catalogs hide changed configuration until its selected role is revalidated.
        // Native transports remain owned by the unchanged business instance while comparing that plan.
        self.language_services();
        self.direct_language_providers();
        self.structure_providers();
        self.refresh_services();
        if !failures.is_empty() {
            let message = format!(
                "Settings saved, but hot application failed; failed owners retired: {}",
                failures.join("; ")
            );
            if let Some(entry) = self.installed.get_mut(id) {
                entry.error = Some(message.clone());
            }
            anyhow::bail!(message);
        }
        if let Some(entry) = self.installed.get_mut(id) {
            entry.error = None;
        }
        Ok(())
    }
}
