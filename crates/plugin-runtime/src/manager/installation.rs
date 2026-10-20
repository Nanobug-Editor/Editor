//! Pure installation admission runs before host callback retirement and repeats at mutation ingress.
use super::*;

impl Manager {
    /// Check current trust/scope, grants, cancellation and immutable package compatibility.
    /// This is the shared metadata prefix for executable and resource-only installation; it does
    /// not prepare downloads, execute guests, extract files or change an installed record. A
    /// prepared WASM token additionally needs `validate_prepared_installation` before cutover.
    pub fn validate_installation(
        &self,
        package: &Package,
        grants: &BTreeSet<String>,
        control: &crate::InstallControl,
    ) -> anyhow::Result<()> {
        control.check()?;
        crate::capabilities::require_current(&package.manifest)?;
        if self.is_application_management()
            && package.manifest.scope == api::InstanceScope::Workspace
        {
            return self.validate_dormant_workspace(package, grants);
        }
        anyhow::ensure!(
            self.trusted && (self.workspace_open || self.is_application_management()),
            "Workspace is restricted or closed"
        );
        anyhow::ensure!(
            self.parked
                .values()
                .all(|scope| !scope.live.contains_key(&package.manifest.id)),
            "Close other workspace instances before replacing this package"
        );
        // Changing ownership needs explicit data migration, not an implicit update cutover.
        anyhow::ensure!(
            self.installed
                .get(&package.manifest.id)
                .is_none_or(|old| old.manifest.scope == package.manifest.scope),
            "Changing instance scope requires reinstalling the package"
        );
        anyhow::ensure!(
            package.manifest.permissions.is_subset(grants),
            "Permission confirmation required"
        );
        if package.manifest.component.is_none() {
            anyhow::ensure!(
                self.installed
                    .get(&package.manifest.id)
                    .is_none_or(|old| old.manifest.data_format.is_none()),
                "Removing a data format requires an explicit migration"
            );
            // Package admission forbids executable service contracts and typed commands without
            // a component. This Manifest therefore proposes an empty executable publication,
            // not an empty WASM instance. Its contract removals obey the same reverse check as
            // a prepared component before either the Actor or Manager retires the old owner.
            let replacement = crate::plugin_services::ServiceReplacement {
                plugin: package.manifest.id.clone(),
                scope: match package.manifest.scope {
                    api::InstanceScope::Application => "application".into(),
                    api::InstanceScope::Workspace => self.host_scope().into(),
                },
                contracts: BTreeMap::new(),
            };
            self.validate_service_replacement(
                &replacement,
                &self.replacement_records(&package.manifest),
            )?;
        }
        Ok(())
    }

    /// Both executable and resource-only candidates preserve currently usable mandatory relations.
    /// Retiring consumers no longer need their old declarations; surviving owners resolve through
    /// the actual current broker. No snapshots, guest calls, files or publication changes occur.
    pub(super) fn validate_service_replacement(
        &self,
        replacement: &crate::plugin_services::ServiceReplacement,
        replaced_records: &BTreeMap<String, Installed>,
    ) -> anyhow::Result<()> {
        let broker = self.plugin_services.lock().unwrap();
        for (consumer, instance) in self
            .live
            .iter()
            .chain(self.parked.values().flat_map(|scope| scope.live.iter()))
        {
            if consumer != &replacement.plugin && !replaced_records.contains_key(consumer) {
                instance.validate_service_replacement(replacement, &broker)?;
            }
        }
        Ok(())
    }
}
