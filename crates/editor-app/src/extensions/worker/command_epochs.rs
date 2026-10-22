//! Static command discovery and live callback incarnations have separate monotonic identities.

use super::*;

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
    fn reconcile_availabilities(&mut self, manager: &mut Manager) {
        let next = manager.availabilities();
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
    /// The production actor and real-Manager application harness use this same reconciliation.
    pub(in crate::extensions) fn reconcile_instances(&mut self, manager: &Manager) {
        let next = manager
            .live
            .keys()
            .filter_map(|id| {
                manager
                    .instance_id(id)
                    .map(|identity| (id.clone(), identity.to_owned()))
            })
            .collect::<BTreeMap<_, _>>();
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

    /// Only an opened Manager supplies completion, effective entries and real instance identities.
    pub(in crate::extensions) fn publish_manager(&mut self, manager: &mut Manager) {
        self.observations = manager.runtime_observations();
        crate::plugin_development::report::checkpoint(manager, false);
        self.reconcile_availabilities(manager);
        self.reconcile_instances(manager);
        self.publish_entries(manager.published_entries());
        self.startup.clear();
        self.ready = true;
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
        self.reconcile_availabilities(manager);
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
