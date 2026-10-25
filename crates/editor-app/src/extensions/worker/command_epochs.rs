//! Static command discovery and live callback incarnations have separate monotonic identities.

use super::*;

/// Manager-owned publication data is prepared before locking the UI-readable snapshot.
/// Keep the actor's Manager guard until this data is applied; availability preparation may read
/// package files, but it must not block readers of Published or overwrite their early retirements.
pub(super) struct ManagerPublication {
    /// Shared runtime observations belonging to the currently opened manager.
    observations: plugin_runtime::RuntimeObservations,
    /// Validated cold activation identities used to reconcile static command epochs.
    availabilities: BTreeMap<String, plugin_runtime::Availability>,
    /// Actual live owners used to detect first birth, replacement and removal.
    instance_ids: BTreeMap<String, String>,
    /// Effective installed entries retain scope enablement and publication error reporting.
    entries: Vec<Installed>,
}

impl ManagerPublication {
    /// Capture from the actor's still-locked `manager` without holding the publication mutex.
    /// Returns prepared data for atomic application to Published while the Manager guard remains held.
    pub(super) fn capture(manager: &mut Manager) -> Self {
        let observations = manager.runtime_observations();
        crate::plugin_development::report::checkpoint(manager, false);
        Self {
            observations,
            availabilities: manager.availabilities(),
            instance_ids: Self::instance_ids(manager),
            entries: manager.published_entries(),
        }
    }

    /// Actual owner IDs distinguish first birth, replacement and retirement independently of metadata.
    fn instance_ids(manager: &Manager) -> BTreeMap<String, String> {
        manager
            .live
            .keys()
            .filter_map(|id| {
                manager
                    .instance_id(id)
                    .map(|identity| (id.clone(), identity.to_owned()))
            })
            .collect()
    }
}

impl Published {
    /// Visibility observations must never treat an old retirement epoch as a live callback owner.
    pub(in crate::extensions) fn live_instance_epoch(&self, plugin: &str) -> Option<u64> {
        self.instance_ids
            .contains_key(plugin)
            .then(|| self.instance_epochs.get(plugin).copied())
            .flatten()
    }
    /// Receipt consumers bind only an owner actually present in the latest live publication.
    pub(in crate::extensions) fn owner_epoch(&self, plugin: &str, owner: &str) -> Option<u64> {
        (self.instance_ids.get(plugin).map(String::as_str) == Some(owner))
            .then(|| self.instance_epochs.get(plugin).copied())
            .flatten()
    }
    /// Advance ownership before blocking retirement; old callbacks must never reach a replacement.
    pub(super) fn retire_instance(&mut self, plugin: &str) {
        *self.instance_epochs.entry(plugin.to_owned()).or_default() += 1;
    }

    /// Seal accepted command rounds before a queued retirement, including disable→enable ABA.
    pub(super) fn retire_commands(&mut self, plugin: &str) {
        *self.command_epochs.entry(plugin.to_owned()).or_default() += 1;
    }

    /// Metadata changes revoke original demands; a live instance's first birth changes no metadata.
    fn reconcile_availabilities(&mut self, next: BTreeMap<String, plugin_runtime::Availability>) {
        let changed = self
            .availabilities
            .iter()
            .filter(|(id, availability)| next.get(*id) != Some(*availability))
            .map(|(id, _)| id.clone())
            .chain(
                next.keys()
                    .filter(|id| !self.availabilities.contains_key(*id))
                    .cloned(),
            )
            .collect::<Vec<_>>();
        for id in changed {
            if self.availabilities.contains_key(&id) {
                self.retire_commands(&id);
            } else {
                // Preserve an epoch left by removal; a fresh install cannot revive an old row.
                self.command_epochs.entry(id).or_default();
            }
        }
        self.availabilities = next;
    }

    /// Compare actual instance IDs without resetting epochs already advanced by early retirement.
    fn reconcile_instance_ids(&mut self, next: BTreeMap<String, String>) {
        let changed = next
            .iter()
            .filter(|(id, identity)| self.instance_ids.get(*id) != Some(*identity))
            .map(|(id, _)| id.clone())
            .chain(
                self.instance_ids
                    .keys()
                    .filter(|id| !next.contains_key(*id))
                    .cloned(),
            )
            .collect::<Vec<_>>();
        for id in changed {
            self.retire_instance(&id);
        }
        self.instance_ids = next;
    }

    /// Reconcile against current counters: UI retirement may have advanced them during capture.
    /// Only already-prepared data is applied while the UI-readable publication is locked.
    pub(super) fn apply_manager_publication(&mut self, snapshot: ManagerPublication) {
        self.observations = snapshot.observations;
        self.reconcile_availabilities(snapshot.availabilities);
        self.reconcile_instance_ids(snapshot.instance_ids);
        self.publish_entries(snapshot.entries);
        self.startup.clear();
        self.ready = true;
    }

    /// Real-Manager fixtures use the same preparation and reconciliation as the production actor.
    #[cfg(test)]
    pub(in crate::extensions) fn publish_manager(&mut self, manager: &mut Manager) {
        self.apply_manager_publication(ManagerPublication::capture(manager));
    }

    /// Fixture dispatch also reconciles real owners before its next full publication.
    #[cfg(test)]
    pub(in crate::extensions) fn reconcile_instances(&mut self, manager: &Manager) {
        self.reconcile_instance_ids(ManagerPublication::instance_ids(manager));
    }

    /// Return a static command epoch without creating or requiring a live instance.
    /// Trust is checked by the caller's worker authority, outside this immutable publication.
    pub(in crate::extensions) fn command_epoch(&self, plugin: &str, command: &str) -> Option<u64> {
        if !self.ready
            || self.startup.contains_key(plugin)
            || !self.availabilities.contains_key(plugin)
        {
            return None;
        }
        self.entries
            .iter()
            .any(|entry| {
                entry.manifest.id == plugin
                    && entry.enabled
                    && entry.error.is_none()
                    && entry
                        .manifest
                        .commands
                        .iter()
                        .any(|item| item.id == command)
            })
            .then(|| self.command_epochs.get(plugin).copied())
            .flatten()
    }

    /// Reject delayed commands before Manager invocation, preserving the original queued epoch.
    /// Reconciliation here also catches lifecycle operations performed between fixture publications.
    pub(in crate::extensions) fn admit_command(
        &mut self,
        manager: &mut Manager,
        trusted: bool,
        plugin: &str,
        command: &str,
        expected_epoch: u64,
    ) -> anyhow::Result<()> {
        // Admission remains a live Manager check; prepared UI snapshots never authorize execution.
        self.reconcile_availabilities(manager.availabilities());
        anyhow::ensure!(
            trusted && self.command_epoch(plugin, command) == Some(expected_epoch),
            api::Failure::new(
                api::ErrorCode::StaleRevision,
                rust_i18n::t!("plugins.command_owner_changed").to_string()
            )
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real availability and owner identities survive split preparation without reviving old work.
    /// Timing stays in the production profiler; this test protects the lock-boundary semantics.
    #[test]
    #[ignore = "opt-in frozen Package/WASM fixtures; see crates/plugin-runtime/tests/fixtures/packages/README.md"]
    fn prepared_publication_preserves_early_retirement_and_disable_enable_epochs() {
        const PLUGIN: &str = "capability-example";
        const COMMAND: &str = "check-errors";
        let directory = tempfile::tempdir().unwrap();
        let package = crate::test_packages::package("capability-example-0.19.1");
        // Exercise a real open project, as the GUI actor does. An empty workspace is the
        // application-management projection and intentionally publishes no workspace availability.
        let environment = Environment {
            workspace: directory.path().display().to_string(),
            ..Default::default()
        };
        let mut manager = Manager::open(directory.path().join("plugins"), environment).unwrap();
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
        let mut published = Published::default();
        published.startup.insert(PLUGIN.into(), "loading".into());
        published.apply_manager_publication(ManagerPublication::capture(&mut manager));
        assert!(published.ready);
        assert!(published.startup.is_empty());
        let cold_epoch = published.command_epoch(PLUGIN, COMMAND).unwrap();
        assert_eq!(published.live_instance_epoch(PLUGIN), None);

        // Consume the actual installed package so owner reconciliation has a real live identity.
        crate::test_demand::demand(
            &mut manager,
            PLUGIN,
            plugin_runtime::DemandReason::View("welcome".into()),
        )
        .unwrap();
        published.apply_manager_publication(ManagerPublication::capture(&mut manager));
        assert_eq!(published.command_epoch(PLUGIN, COMMAND), Some(cold_epoch));
        let owner = manager.instance_id(PLUGIN).unwrap().to_owned();
        let owner_epoch = published.owner_epoch(PLUGIN, &owner).unwrap();

        // UI retirement can happen while the worker prepares metadata. Applying that prepared
        // snapshot must reconcile with the current counters, not restore captured counters.
        let prepared = ManagerPublication::capture(&mut manager);
        published.retire_commands(PLUGIN);
        published.retire_instance(PLUGIN);
        published.apply_manager_publication(prepared);
        let retired_command_epoch = cold_epoch + 1;
        assert_eq!(
            published.command_epoch(PLUGIN, COMMAND),
            Some(retired_command_epoch)
        );
        assert_eq!(published.owner_epoch(PLUGIN, &owner), Some(owner_epoch + 1));
        assert!(
            published
                .admit_command(&mut manager, true, PLUGIN, COMMAND, cold_epoch)
                .is_err()
        );
        published.apply_manager_publication(ManagerPublication::capture(&mut manager));
        assert_eq!(
            published.command_epoch(PLUGIN, COMMAND),
            Some(retired_command_epoch)
        );
        assert_eq!(published.owner_epoch(PLUGIN, &owner), Some(owner_epoch + 1));
        published
            .admit_command(&mut manager, true, PLUGIN, COMMAND, retired_command_epoch)
            .unwrap();

        // Durable disable/enable must keep the old command revoked, even though IDs are reused.
        manager.disable(PLUGIN).unwrap();
        published.apply_manager_publication(ManagerPublication::capture(&mut manager));
        assert_eq!(published.command_epoch(PLUGIN, COMMAND), None);
        assert_eq!(published.live_instance_epoch(PLUGIN), None);
        assert_eq!(published.owner_epoch(PLUGIN, &owner), None);
        manager.enable(PLUGIN).unwrap();
        published.apply_manager_publication(ManagerPublication::capture(&mut manager));
        let enabled_epoch = published.command_epoch(PLUGIN, COMMAND).unwrap();
        assert!(enabled_epoch > retired_command_epoch);
        assert_eq!(published.live_instance_epoch(PLUGIN), None);
        assert!(
            published
                .admit_command(&mut manager, true, PLUGIN, COMMAND, retired_command_epoch)
                .is_err()
        );
        published
            .admit_command(&mut manager, true, PLUGIN, COMMAND, enabled_epoch)
            .unwrap();
        published.apply_manager_publication(ManagerPublication::capture(&mut manager));
        assert_eq!(
            published.command_epoch(PLUGIN, COMMAND),
            Some(enabled_epoch)
        );
    }
}
