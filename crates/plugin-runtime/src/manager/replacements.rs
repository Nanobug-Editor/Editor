//! Finite, explicitly approved package consolidation preserves original owners and opaque private data.
use super::*;
use crate::InstallControl;

/// Validated durable files and their in-memory choices form one replacement commit candidate.
/// No fallible file read may be required after publication has retired the original owners.
#[derive(Default)]
pub(super) struct ReplacementMetadata {
    /// Host-owned relative paths and bounded serialized contents for the data transaction.
    pub(super) files: BTreeMap<PathBuf, Vec<u8>>,
    /// A validated memory projection, installed only after those files commit successfully.
    pub(super) preferences: Option<crate::plugin_services::Preferences>,
}

impl Manager {
    /// Capture exact replaced versions; another update/enablement change invalidates the candidate.
    pub(super) fn replacement_records(&self, manifest: &Manifest) -> BTreeMap<String, Installed> {
        manifest
            .replaces
            .iter()
            .filter_map(|rule| {
                self.installed
                    .get(&rule.id)
                    .filter(|entry| entry.replaced_by.is_none())
                    .map(|entry| (rule.id.clone(), entry.clone()))
            })
            .collect()
    }

    /// Manifests request replacement; only host consent authorizes touching another installation.
    pub(super) fn validate_replacements(
        &self,
        manifest: &Manifest,
        control: &InstallControl,
    ) -> anyhow::Result<()> {
        for rule in &manifest.replaces {
            let Some(old) = self
                .installed
                .get(&rule.id)
                .filter(|old| old.replaced_by.is_none())
            else {
                continue;
            };
            anyhow::ensure!(
                control.replacement_approved(&rule.id),
                "Replacement requires confirmation: {}",
                rule.id
            );
            anyhow::ensure!(
                rule.versions.matches(&old.manifest.version.parse()?),
                "Unsupported replaced package version: {}",
                rule.id
            );
            anyhow::ensure!(
                old.manifest.scope == manifest.scope,
                "Replacement scope differs"
            );
            anyhow::ensure!(
                rule.contracts.iter().all(|id| old
                    .manifest
                    .plugin_services
                    .provides
                    .contains_key(id)),
                "Replacement service does not match"
            );
            anyhow::ensure!(
                self.parked
                    .values()
                    .all(|scope| !scope.live.contains_key(&rule.id)),
                "Close other workspace owners before replacement"
            );
        }
        Ok(())
    }

    /// Existing destination overrides win; dormant project enablement remains an explicit setting.
    pub(super) fn replacement_settings(
        &self,
        manifest: &Manifest,
    ) -> anyhow::Result<settings::SavedSettings> {
        let mut saved = settings::SavedSettings::read(
            &self
                .root
                .join("settings")
                .join(format!("{}.json", manifest.id)),
        )?;
        for rule in &manifest.replaces {
            let Some(old) = self
                .installed
                .get(&rule.id)
                .filter(|old| old.replaced_by.is_none())
            else {
                continue;
            };
            saved
                .user
                .entry(rule.enabled_setting.clone())
                .or_insert(old.enabled.into());
            for project in &old.project_enabled {
                saved
                    .projects
                    .entry(scopes::workspace_key(project))
                    .or_default()
                    .entry(rule.enabled_setting.clone())
                    .or_insert(true.into());
            }
        }
        Ok(saved)
    }

    /// Return the whole commit candidate; old settings, data and receipts remain untouched.
    pub(super) fn replacement_metadata(
        &self,
        package: &Package,
        registry: &mut BTreeMap<String, Installed>,
    ) -> anyhow::Result<ReplacementMetadata> {
        let manifest = &package.manifest;
        let records = self.replacement_records(manifest);
        if records.is_empty() {
            return Ok(ReplacementMetadata::default());
        }
        let mut preferences = plugin_services::read_preferences(&self.root)?;
        for rule in &manifest.replaces {
            if !records.contains_key(&rule.id) {
                continue;
            }
            for (key, provider) in &mut preferences.user {
                if provider == &rule.id
                    && rule.contracts.iter().any(|contract| {
                        key == &crate::plugin_services::choice_key("workspace", contract)
                            || key == &crate::plugin_services::choice_key("application", contract)
                    })
                {
                    *provider = manifest.id.clone();
                }
            }
            for choices in preferences.projects.values_mut() {
                for contract in &rule.contracts {
                    if choices.get(contract) == Some(&rule.id) {
                        choices.insert(contract.clone(), manifest.id.clone());
                    }
                }
            }
            let old = registry.get_mut(&rule.id).expect("replacement was checked");
            old.replaced_by = Some(manifest.id.clone());
            old.enabled = false;
            old.project_enabled.clear();
        }
        // The transaction's aggregate allowance cannot replace individual store quotas.
        // Reject growth before the registry/data commit, while every original owner is still live.
        let preference_bytes = serde_json::to_vec_pretty(&preferences)?;
        anyhow::ensure!(
            preference_bytes.len() <= plugin_services::MAX_PREFERENCES_BYTES,
            "Service provider preferences exceed quota"
        );
        let settings_bytes = serde_json::to_vec_pretty(&self.replacement_settings(manifest)?)?;
        anyhow::ensure!(
            settings_bytes.len() <= settings::MAX_SETTINGS_BYTES,
            "Settings exceed 1 MiB"
        );
        let files = BTreeMap::from([
            (PathBuf::from("service-providers.json"), preference_bytes),
            (
                PathBuf::from("settings").join(format!("{}.json", manifest.id)),
                settings_bytes,
            ),
            (
                PathBuf::from("replacement-history").join(format!("{}.json", package.digest)),
                serde_json::to_vec_pretty(&records)?,
            ),
        ]);
        Ok(ReplacementMetadata {
            files,
            preferences: Some(preferences),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Project-only enablement survives consolidation, while explicit destination settings win.
    #[test]
    fn project_enablement_and_existing_destination_overrides_survive_replacement() {
        let root = tempfile::tempdir().unwrap();
        let project = root.path().join("project");
        std::fs::create_dir(&project).unwrap();
        let store = root.path().join("plugins");
        let mut manager = Manager::open(store.clone(), Environment::default()).unwrap();
        // A generic declaration keeps this unit contract independent of any shipped plugin source.
        let manifest: Manifest = serde_json::from_value(serde_json::json!({
            "id":"combined-provider", "name":"Combined provider", "version":"1.0.0",
            "protocol":7, "permissions":[], "storage_limit":0,
            "settings":{"debugger.enabled":{"title":"Enable provider", "value_type":{"kind":"boolean"}, "default":true, "scope":"project"}},
            "replaces":[{"id":"previous-provider", "versions":"^0.2", "contracts":["example.service"], "enabled_setting":"debugger.enabled"}]
        })).unwrap();
        let mut original = manifest.clone();
        original.id = "previous-provider".into();
        original.version = "0.2.0".into();
        original.replaces.clear();
        manager.installed.insert(
            original.id.clone(),
            Installed {
                manifest: original,
                digest: "original".into(),
                install_sequence: 0,
                grants: Default::default(),
                enabled: false,
                project_enabled: [project.display().to_string()].into(),
                global_enabled: None,
                retired_ui_contract: false,
                replaced_by: None,
                error: None,
            },
        );
        let saved = manager.replacement_settings(&manifest).unwrap();
        assert_eq!(
            saved.resolve(&manifest, "unrelated-workspace").unwrap()["debugger.enabled"].value,
            serde_json::json!(false)
        );
        assert_eq!(
            saved
                .resolve(&manifest, &project.display().to_string())
                .unwrap()["debugger.enabled"]
                .value,
            serde_json::json!(true)
        );
        // A destination project override must not be replaced by the old package's broader preference.
        let explicit = settings::SavedSettings {
            user: [("debugger.enabled".into(), serde_json::json!(true))].into(),
            projects: [(
                scopes::workspace_key(&project.display().to_string()),
                [("debugger.enabled".into(), serde_json::json!(false))].into(),
            )]
            .into(),
        };
        std::fs::create_dir_all(store.join("settings")).unwrap();
        atomic_write(
            &store.join("settings/combined-provider.json"),
            &serde_json::to_vec(&explicit).unwrap(),
        )
        .unwrap();
        let saved = manager.replacement_settings(&manifest).unwrap();
        assert_eq!(
            saved.resolve(&manifest, "unrelated-workspace").unwrap()["debugger.enabled"].value,
            serde_json::json!(true)
        );
        assert_eq!(
            saved
                .resolve(&manifest, &project.display().to_string())
                .unwrap()["debugger.enabled"]
                .value,
            serde_json::json!(false)
        );
    }
}
