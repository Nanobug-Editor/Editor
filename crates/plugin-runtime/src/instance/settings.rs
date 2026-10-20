//! Optional configuration hooks run before activation and cannot grant authority or replace explicit values.
use super::*;
use plugin_protocol::settings::{Effective, EffectiveValue, Phase, Source};

impl Instance {
    /// Pure workers accept one typed source operation; initialization cannot manufacture its result.
    pub(crate) fn complete_snapshot(
        &mut self,
        request: language::CompletionRequest,
    ) -> anyhow::Result<api::Output> {
        anyhow::ensure!(
            self.store.data().language_pure,
            "Completion requires a pure language worker"
        );
        self.store.data_mut().language_completion_call = true;
        let result = self.notify(None, api::Notification::LanguageCompletion(request));
        self.store.data_mut().language_completion_call = false;
        result
    }
    /// The hook sees only isolated files; ephemeral handles cannot escape into the activated instance.
    pub(crate) fn migrate_data(
        &mut self,
        from: u32,
        to: u32,
        snapshot: Option<Snapshot>,
    ) -> anyhow::Result<Option<Snapshot>> {
        let checkpoint = self.store.data().roots.checkpoint();
        self.store.data_mut().migrating = true;
        let result = self.call(api::Input::Event {
            panel: None,
            event: api::Notification::MigrateData {
                from,
                to,
                snapshot: snapshot.clone(),
            },
        });
        self.store.data_mut().migrating = false;
        self.store.data_mut().roots.release_since(checkpoint);
        Ok(result?.snapshot.or(snapshot))
    }
    /// After the durable directory switch, file handles resolve against the committed scope rather than its copy.
    pub(crate) fn retarget_data(&mut self, data: PathBuf) {
        self.store.data_mut().data = data;
    }
    /// Discovery is bounded by the normal fuel/deadline and cannot mutate files, processes or UI.
    pub(crate) fn prepare_language(
        &mut self,
        mut context: language::Context,
    ) -> anyhow::Result<language::Proposal> {
        // These roots were admitted for this instance, not supplied by a guest. Canonicalize the
        // current candidate/committed directories on every call so native caches never keep a stale root.
        context.package_root = self
            .store
            .data()
            .assets
            .canonicalize()?
            .display()
            .to_string();
        context.data_root = self.store.data().data.canonicalize()?.display().to_string();
        let checkpoint = self.store.data().roots.checkpoint();
        self.store.data_mut().language_hook = true;
        self.store.data_mut().language_hook_checkpoint = Some(checkpoint);
        let reply = self.call(api::Input::Event {
            panel: None,
            event: api::Notification::LanguageService(context),
        });
        self.store.data_mut().language_hook = false;
        self.store.data_mut().language_hook_checkpoint = None;
        self.store.data_mut().roots.release_since(checkpoint);
        let reply = reply?;
        anyhow::ensure!(
            reply.views.is_empty() && reply.snapshot.is_none() && reply.configuration.is_none(),
            "LSP hook can return only a language proposal"
        );
        reply
            .language_service
            .ok_or_else(|| anyhow::anyhow!("LSP hook returned no proposal"))
    }
    /// Resolve discoveries on an isolated candidate without applying values or granting active authority.
    /// Hot validation uses this same bounded hook before saving, and initial preparation applies its result.
    pub(crate) fn validate_settings(
        &mut self,
        manifest: &Manifest,
        mut values: Effective,
    ) -> anyhow::Result<Effective> {
        if manifest.settings.is_empty() {
            return Ok(values);
        }
        anyhow::ensure!(
            self.store
                .data()
                .api
                .capabilities
                .contains_key("configuration"),
            "configuration was not negotiated"
        );
        anyhow::ensure!(
            !self.store.data().active,
            "Configuration requires a prepared candidate"
        );
        if manifest.settings_hook {
            let reply = self.call(api::Input::Event {
                panel: None,
                event: api::Notification::Configuration {
                    phase: Phase::Validate,
                    values: values.clone(),
                },
            })?;
            anyhow::ensure!(
                reply.views.is_empty() && reply.snapshot.is_none(),
                "Configuration validation cannot publish UI or snapshots"
            );
            let proposal = reply
                .configuration
                .ok_or_else(|| anyhow::anyhow!("Configuration hook returned no proposal"))?;
            anyhow::ensure!(
                proposal.errors.is_empty(),
                "Configuration rejected: {:?}",
                proposal.errors
            );
            anyhow::ensure!(
                proposal.discovered.len() <= 64,
                "Too many discovered settings"
            );
            for (key, value) in proposal.discovered {
                let definition = manifest
                    .settings
                    .get(&key)
                    .ok_or_else(|| anyhow::anyhow!("Undeclared discovered setting: {key}"))?;
                anyhow::ensure!(
                    definition.accepts(&value),
                    "Invalid discovered setting: {key}"
                );
                if values
                    .get(&key)
                    .is_some_and(|current| current.source == Source::Default)
                {
                    values.insert(
                        key,
                        EffectiveValue {
                            value,
                            source: Source::Discovered,
                        },
                    );
                }
            }
        }
        Ok(values)
    }

    /// Initial/replacement configuration applies only after isolated validation succeeds.
    pub(crate) fn configure_settings(
        &mut self,
        manifest: &Manifest,
        values: Effective,
    ) -> anyhow::Result<()> {
        self.configuration = self.validate_settings(manifest, values)?;
        self.reapply_settings()
    }

    /// Deliver a validated, saved map to the same committed configuration-1.1 owner.
    /// The manager retires this owner on failure; neither memory nor external effects are rolled back.
    pub(crate) fn apply_hot_settings(
        &mut self,
        manifest: &Manifest,
        values: Effective,
    ) -> anyhow::Result<()> {
        if !manifest.settings_hook
            || !self
                .negotiated()
                .capabilities
                .get("configuration")
                .is_some_and(|version| version.major == 1 && version.minor >= 1)
        {
            anyhow::bail!(api::Failure::new(
                api::ErrorCode::CapabilityUnavailable,
                "Hot configuration requires the negotiated 1.1 hook"
            ));
        }
        anyhow::ensure!(
            self.store.data().active && self.store.data().activation_committed,
            "Hot configuration requires a committed active owner"
        );
        self.store
            .data()
            .host_resources
            .contexts
            .revoke_owner(self.instance_id().unwrap_or(""));
        let reply = self.call(api::Input::Event {
            panel: None,
            event: api::Notification::Configuration {
                phase: Phase::Changed,
                values: values.clone(),
            },
        })?;
        if let Some(proposal) = reply.configuration {
            anyhow::ensure!(
                proposal.errors.is_empty() && proposal.discovered.is_empty(),
                "Changed cannot reject or replace already validated saved values"
            );
        }
        self.configuration = values;
        Ok(())
    }

    /// Rollback reinitializes guest memory, so the previous resolved configuration must be delivered again.
    pub(super) fn reapply_settings(&mut self) -> anyhow::Result<()> {
        self.store
            .data()
            .host_resources
            .contexts
            .revoke_owner(self.instance_id().unwrap_or(""));
        if !self.configuration.is_empty() {
            self.call(api::Input::Event {
                panel: None,
                event: api::Notification::Configuration {
                    phase: Phase::Apply,
                    values: self.configuration.clone(),
                },
            })?;
        }
        Ok(())
    }
}
