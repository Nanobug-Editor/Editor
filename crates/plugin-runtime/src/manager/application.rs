//! Application management routes package metadata without borrowing or fabricating a workspace.

use super::*;

impl Manager {
    /// A host-owned empty environment permits profile management, never plugin business demands.
    pub fn is_application_management(&self) -> bool {
        !self.workspace_open && self.environment.workspace.is_empty() && self.trusted
    }

    /// Park the current project's exact resources before serving a profile-only management operation.
    /// Open projects keep their transports and documents; no empty workspace is registered.
    pub fn select_application(&mut self, mut environment: Environment) {
        if self.is_application_management() {
            return;
        }
        environment.workspace.clear();
        if self.workspace_open {
            let key = self.host_scope();
            let ids = self
                .live
                .keys()
                .filter(|id| self.installed[*id].manifest.scope == api::InstanceScope::Workspace)
                .cloned()
                .collect::<Vec<_>>();
            let live = ids
                .into_iter()
                .map(|id| {
                    let instance = self.live.remove(&id).unwrap();
                    (id, instance)
                })
                .collect();
            let mut resources = concurrent::ProjectResources::new(self, false);
            resources.swap(self);
            self.parked.insert(
                key,
                ParkedWorkspace {
                    environment: self.environment.clone(),
                    trusted: self.trusted,
                    live,
                    resources: Some(resources),
                },
            );
        }
        self.environment = environment;
        self.workspace_open = false;
        self.trusted = true;
        self.host_alive
            .store(false, std::sync::atomic::Ordering::Release);
        self.activation.select_scope("application");
        self.host_resources
            .terminals
            .select_scope("application".into());
    }

    /// Pure profile-management admission preserves the same dormant update and owner restrictions.
    /// The host may use this before revoking UI epochs; no workspace or guest is created here.
    pub(super) fn validate_dormant_workspace(
        &self,
        package: &Package,
        grants: &BTreeSet<String>,
    ) -> anyhow::Result<()> {
        crate::capabilities::require_current(&package.manifest)?;
        anyhow::ensure!(
            package.manifest.permissions.is_subset(grants),
            "Permission confirmation required"
        );
        let id = &package.manifest.id;
        anyhow::ensure!(
            !self.live.contains_key(id)
                && self
                    .parked
                    .values()
                    .all(|scope| !scope.live.contains_key(id)),
            "Close other workspace instances before replacing this package"
        );
        anyhow::ensure!(
            self.replacement_records(&package.manifest).is_empty(),
            "Open a trusted project to consolidate installed packages"
        );
        // A future migration failure must still have the original package as its fallback.
        // Empty management cannot commit an upgrade before any real scope has prepared that migration.
        anyhow::ensure!(
            self.installed
                .get(id)
                .is_none_or(|old| old.digest == package.digest),
            "Open a trusted project to update workspace plugins safely"
        );
        anyhow::ensure!(
            self.installed
                .get(id)
                .is_none_or(|old| old.manifest.scope == package.manifest.scope),
            "Changing instance scope requires reinstalling the package"
        );
        anyhow::ensure!(
            package.manifest.data_format.is_some()
                || self
                    .installed
                    .get(id)
                    .is_none_or(|old| old.manifest.data_format.is_none()),
            "Removing a data format requires an explicit migration"
        );
        Ok(())
    }

    /// Admit an immutable workspace package and its enabled preference without creating private project data.
    /// Data migration and workspace-dependent preparation occur only when a real project is opened.
    /// Live instances and package consolidation still require the established project cutover path.
    pub(super) fn install_dormant_workspace(
        &mut self,
        package: &Package,
        grants: BTreeSet<String>,
        control: &crate::InstallControl,
    ) -> anyhow::Result<()> {
        self.validate_dormant_workspace(package, &grants)?;
        let id = &package.manifest.id;
        control.stage(crate::InstallStage::Preparing)?;
        if let Some(component) = package.component() {
            if self.engine.is_none() {
                self.engine = Some(Instance::engine()?);
            }
            // Compile for admission only. No guest hooks, workspace paths, processes or private scopes are created.
            wasmtime::component::Component::new(self.engine.as_ref().unwrap(), component)?;
        }
        let version = self.root.join("packages").join(id).join(&package.digest);
        package.extract(&version)?;
        if !package
            .manifest
            .language_servers
            .iter()
            .any(|provider| provider.hook)
        {
            self.prepare_dependencies(package, None, control)?;
        }
        if package.manifest.component.is_some() {
            // A first installation has no old private scope. Defer guest preparation until a real,
            // trusted directory supplies the scope, retaining the marker until preparation succeeds.
            atomic_write(
                &self
                    .root
                    .join("pending-workspace-installations")
                    .join(id)
                    .join(&package.digest),
                b"pending",
            )?;
        }
        control.stage(crate::InstallStage::Committing)?;
        let previous = self.installed.get(id).cloned();
        let (enabled, project_enabled) = self.installation_scope(&package.manifest, control);
        let record = Installed {
            manifest: package.manifest.clone(),
            digest: package.digest.clone(),
            grants,
            enabled,
            project_enabled,
            retired_ui_contract: false,
            replaced_by: None,
            global_enabled: None,
            error: None,
        };
        self.installed.insert(id.clone(), record);
        if let Err(error) = self.save_registry() {
            if let Some(old) = previous {
                self.installed.insert(id.clone(), old);
            } else {
                self.installed.remove(id);
            }
            return Err(error);
        }
        control.stage(crate::InstallStage::Committed)?;
        Ok(())
    }
    /// A cold workspace install needs the ordinary transaction once each actual scope first opens.
    pub(super) fn dormant_preparation_pending(&self, id: &str, digest: &str) -> bool {
        let pending = self
            .root
            .join("pending-workspace-installations")
            .join(id)
            .join(digest);
        pending.is_file()
            && !pending
                .with_file_name(format!(
                    "{digest}-{}",
                    scopes::workspace_key(&self.environment.workspace)
                ))
                .is_file()
    }
    /// Record readiness only after the established data/dependency transaction commits successfully.
    pub(super) fn finish_dormant_preparation(&self, id: &str, digest: &str) -> anyhow::Result<()> {
        atomic_write(
            &self
                .root
                .join("pending-workspace-installations")
                .join(id)
                .join(format!(
                    "{digest}-{}",
                    scopes::workspace_key(&self.environment.workspace)
                )),
            b"prepared",
        )
    }
    /// A newly installed cold package must initialize a resumed real scope as well as a new one.
    /// Failed preparation stays attached to that project; actor routing never retries it every tick.
    pub(super) fn prepare_dormant_scope_plugins(&mut self) -> anyhow::Result<()> {
        if !self.trusted || !self.workspace_open {
            return Ok(());
        }
        let entries = self
            .installed
            .values()
            .filter(|entry| {
                entry.error.is_none()
                    && entry.manifest.scope == api::InstanceScope::Workspace
                    && (entry.enabled || entry.project_enabled_in(&self.environment.workspace))
                    && self.dormant_preparation_pending(&entry.manifest.id, &entry.digest)
            })
            .map(|entry| (entry.manifest.id.clone(), entry.enabled))
            .collect::<Vec<_>>();
        if entries.is_empty() {
            return Ok(());
        }
        // Preparation may admit project-only availability; keep its original global default durable.
        self.restore_saved_plugins(entries);
        self.save_registry()
    }
}
