//! The actor admits intent and commits finished candidates; slow guest execution is detached and bounded.
use super::*;

const MAX_WAITERS: usize = 64;
const MAX_PREPARATIONS: usize = 4;

impl State {
    /// Terminal waits free reservations immediately; running abandoned work still occupies a worker slot.
    fn prune(&mut self) {
        self.flights.retain(|_, flight| {
            if flight.running && flight.lease_count.load(Ordering::Acquire) == 0 {
                for waiter in flight.waiters.lock().unwrap().iter() {
                    waiter.completion.finish(Err(api::Failure::new(
                        api::ErrorCode::OperationFailed,
                        "Activation preparation or unpublished result was dropped",
                    )));
                }
                return false;
            }
            flight
                .waiters
                .lock()
                .unwrap()
                .retain(|waiter| !waiter.completion.status().is_terminal());
            flight.running || !flight.waiters.lock().unwrap().is_empty()
        });
        self.prune_deferred();
        // Keep terminal attempts for the same token, preventing automatic failed-activation retries.
        self.background.retain(|token, _| {
            self.records
                .get(&(token.scope().into(), token.plugin().into()))
                .is_some_and(|record| {
                    record.token == *token && record.alive.load(Ordering::Acquire)
                })
        });
    }
    pub(in crate::manager) fn invalidate_scope(&mut self, scope: &str) {
        self.records.retain(|(record_scope, _), record| {
            if record_scope == scope {
                record.alive.store(false, Ordering::Release);
                false
            } else {
                true
            }
        });
    }
}
impl Manager {
    /// Bind a service wait to its original call before the shared flight clones its completion.
    /// Direct public prepare/commit and actor polling therefore observe the same outer cancellation,
    /// source retirement and absolute deadline without withdrawing another valid coalesced caller.
    pub(crate) fn begin_service_demand(
        &mut self,
        token: &Availability,
        contract: &str,
        parent: &crate::Completion<serde_json::Value>,
        lifetimes: &[Arc<AtomicBool>],
        native_waits: &[crate::Completion<serde_json::Value>],
    ) -> Result<DemandRequest, api::Failure> {
        let mut completion = crate::Completion::new(parent.remaining_timeout_ms().min(120_000));
        completion.lifetimes.push(parent.wait_lifetime());
        completion.lifetimes.extend(lifetimes.iter().cloned());
        completion.constrain_deadline(parent);
        // A pending ancestor can expire between preparation and a direct public commit without
        // another actor tick. Completed ancestors retain valid ownership beyond their old deadline.
        for source in native_waits {
            completion.bind_wait_source(source);
        }
        self.begin_demand_inner(token, DemandReason::Service(contract.into()), completion)
    }

    /// Join shared preparation while retaining this caller's original timeout and cancellation.
    /// The 64 pending waits are shared across commands, views, selected roles, services and background purposes.
    pub fn begin_demand(
        &mut self,
        token: &Availability,
        reason: DemandReason,
        timeout_ms: u32,
    ) -> Result<DemandRequest, api::Failure> {
        if !(1..=120_000).contains(&timeout_ms) {
            return Err(api::Failure::new(
                api::ErrorCode::InvalidRequest,
                "Activation timeout must be 1..120000 ms",
            ));
        }
        self.begin_demand_inner(token, reason, crate::Completion::new(timeout_ms))
    }

    /// Public waits and forwarded service waits share budget, identity and first-birth admission.
    /// Inherited gates must already be attached when this completion is cloned into a flight.
    fn begin_demand_inner(
        &mut self,
        token: &Availability,
        reason: DemandReason,
        mut completion: crate::Completion<ActivationReceipt>,
    ) -> Result<DemandRequest, api::Failure> {
        if self.availability_in_scope(token.plugin(), token.scope())? != *token {
            return Err(api::Failure::new(
                api::ErrorCode::InvalidHandle,
                "Queued availability changed",
            ));
        }
        self.validate_demand_reason(token, &reason)?;
        self.activation.prune();
        self.resume_deferred_demands();
        let record = &self.activation.records[&(token.scope().into(), token.plugin().into())];
        // A stopped actor cannot provide another tick. Original host/scope seals therefore retire
        // request observation immediately, while same-owner configuration changes use explicit cancellation.
        completion.lifetimes.extend([
            if token.scope() == "application" {
                self.application_alive.clone()
            } else {
                self.host_alive.clone()
            },
            record.alive.clone(),
        ]);
        // An expired forwarded parent consumes no reservation and cannot publish an already warm receipt.
        if completion.status().is_terminal() {
            return Ok(DemandRequest { completion });
        }
        if reason.is_language() {
            self.language_services();
        }
        let record = &self.activation.records[&(token.scope().into(), token.plugin().into())];
        let role = self.demanded_role_result(&reason);
        if record.materialized && (!reason.is_language() || role.is_some()) {
            completion.finish(role.unwrap_or(Ok(())).map(|_| ActivationReceipt {
                availability: token.clone(),
                owner: record.owner.clone(),
            }));
            return Ok(DemandRequest { completion });
        }
        let receipt = record.materialized.then(|| ActivationReceipt {
            availability: token.clone(),
            owner: record.owner.clone(),
        });
        let reserved = self.activation.waiter_count() + self.language_delegation_waiter_count();
        if reserved >= MAX_WAITERS {
            return Err(api::Failure::new(
                api::ErrorCode::LimitExceeded,
                "Activation waiter budget exhausted",
            ));
        }
        if let Some(receipt) = receipt
            && self.join_language_delegation(&reason, completion.clone(), receipt)
        {
            return Ok(DemandRequest { completion });
        }
        if let DemandReason::LanguageDocument {
            provider, document, ..
        } = &reason
        {
            self.admit_language_document_intent(provider, document)?;
            // Cancel old source intent immediately; the later worker result is also checked at publication.
            self.activation.for_each_waiter(|_, waiter| {
                    if matches!(&waiter.reason, DemandReason::LanguageDocument { provider: old_provider, document: old_document, .. }
                        if old_provider == provider && old_document.path == document.path && old_document.generation < document.generation)
                    {
                        let _ = waiter
                            .completion
                            .cancel(api::CancelMode::TryTerminate, api::ErrorCode::InvalidHandle);
                    }
            });
        }
        let language_workspace = reason
            .is_language()
            .then(|| scopes::workspace_key(&self.environment.workspace));
        self.join_demand_waiter(
            token,
            Waiter {
                completion: completion.clone(),
                reason,
                language_workspace,
            },
        )?;
        Ok(DemandRequest { completion })
    }

    /// New roles arriving after a worker snapshot retain their old completion/deadline in the same pool.
    pub(super) fn join_demand_waiter(
        &mut self,
        token: &Availability,
        waiter: Waiter,
    ) -> Result<(), api::Failure> {
        if let Some(flight) = self.activation.flights.get_mut(token) {
            if flight.running && flight.waiters.lock().unwrap().is_empty() {
                // Never add a renewed purpose to the cancelled worker's authority. The old job
                // retires first, then the actor gives these independent completions fresh inputs.
                self.activation
                    .deferred
                    .entry(token.clone())
                    .or_default()
                    .push(waiter);
                return Ok(());
            }
            flight.waiters.lock().unwrap().push(waiter);
            return Ok(());
        }
        // Required service resolution reads the same cold static publication, without awakening dependencies.
        self.refresh_services();
        let entry = self.installed[token.plugin()].clone();
        let environment = self.demand_environment(token.scope())?;
        let configuration = self
            .demand_instance(token.plugin(), token.scope())
            .map(|instance| instance.configuration.clone())
            .map(Ok)
            .unwrap_or_else(|| self.settings_for_workspace(&entry.manifest, &environment.workspace))
            .map_err(|error| {
                api::Failure::new(api::ErrorCode::InvalidState, format!("{error:#}"))
            })?;
        let record = &self.activation.records[&(token.scope().into(), token.plugin().into())];
        let waiters = Arc::new(Mutex::new(vec![waiter]));
        let lease = lease::PreparationLease::new();
        let lease_count = lease.counter();
        let preparation = DemandPreparation {
            token: token.clone(),
            data: self.data_directory_for(&entry.manifest, &environment),
            entry,
            root: self.root.clone(),
            environment,
            configuration,
            engine: self.engine.clone(),
            compiled: self
                .demand_instance(token.plugin(), token.scope())
                .map(Instance::compiled_code),
            host_resources: self.host_resources.clone(),
            diagnostics: self.native_diagnostics.clone(),
            broker: self.plugin_services.clone(),
            alive: record.alive.clone(),
            host_alive: if token.scope() == "application" {
                self.application_alive.clone()
            } else {
                self.host_alive.clone()
            },
            waiters: waiters.clone(),
            activate_business: !record.materialized,
            existing_owner: record.owner.clone(),
            authority: self
                .demand_instance(token.plugin(), token.scope())
                .map(Instance::editor_authority),
            lease,
        };
        self.activation.flights.insert(
            token.clone(),
            Flight {
                waiters,
                preparation: Some(preparation),
                running: false,
                lease_count,
            },
        );
        Ok(())
    }

    /// Take one queued owned job for a host's existing background worker. `poll` provides a bounded default.
    /// Return the result to `commit_demand` on this same Manager. Dropping either the job or result
    /// terminates its pending waiters on the next poll and releases capacity; no guest runs here.
    pub fn take_demand_preparation(&mut self) -> Option<DemandPreparation> {
        self.activation.prune();
        self.resume_deferred_demands();
        if self
            .activation
            .flights
            .values()
            .filter(|flight| flight.running)
            .count()
            >= MAX_PREPARATIONS
        {
            return None;
        }
        let token = self
            .activation
            .flights
            .iter()
            .find(|(_, flight)| !flight.running)
            .map(|(token, _)| token.clone())?;
        let flight = self.activation.flights.get_mut(&token).unwrap();
        flight.running = true;
        for waiter in flight.waiters.lock().unwrap().iter() {
            waiter.completion.begin();
        }
        flight.preparation.take()
    }

    /// Recheck original scope/configuration, commit private data, then publish one exact owner and selected roles.
    /// A later role job borrows the original admitted owner and cannot Activate business again.
    pub fn commit_demand(
        &mut self,
        prepared: PreparedDemand,
    ) -> Result<ActivationReceipt, api::Failure> {
        let target = prepared
            .result
            .as_ref()
            .ok()
            .map(|built| scopes::workspace_key(&built.workspace));
        let previous = self
            .workspace_open
            .then(|| (self.environment.clone(), self.trusted));
        let application = self
            .is_application_management()
            .then(|| self.environment.clone());
        if let Some(target) = target.filter(|scope| scope != &self.host_scope()) {
            if let Some(project) = self.parked.get(&target) {
                let environment = project.environment.clone();
                let trusted = project.trusted;
                let _ = self.select_workspace(environment, trusted);
            }
        }
        let result = self.commit_project_demand(prepared);
        if let Some((environment, trusted)) = previous {
            let _ = self.select_workspace(environment, trusted);
        } else if let Some(environment) = application {
            self.select_application(environment);
        }
        result
    }

    /// Admission runs in the preparation's captured project, while each language waiter keeps its own scope.
    fn commit_project_demand(
        &mut self,
        prepared: PreparedDemand,
    ) -> Result<ActivationReceipt, api::Failure> {
        // Retain the result reservation through final admission, even when a failed candidate is dropped.
        let _reservation = prepared.lease;
        let token = prepared.token;
        let Some(flight) = self.activation.flights.remove(&token) else {
            return Err(api::Failure::new(
                api::ErrorCode::InvalidHandle,
                "Activation preparation is no longer owned",
            ));
        };
        // Admission failure belongs to the abandoned original purpose, not the installation. In
        // particular a cancelled/expired first document must not withdraw static recognition while
        // this detached result is being discarded. Real preparation/Activate failures below still
        // publish their normal package diagnostic.
        let admission = self
            .availability_in_scope(token.plugin(), token.scope())
            .and_then(|current| {
                if current != token
                    || !flight
                        .waiters
                        .lock()
                        .unwrap()
                        .iter()
                        .any(|waiter| !waiter.completion.status().is_terminal())
                {
                    Err(api::Failure::new(
                        api::ErrorCode::InvalidHandle,
                        "Activation original intent was revoked",
                    ))
                } else {
                    Ok(())
                }
            });
        // Abandonment is a host lifecycle outcome, not a guest/preparation failure. Keep it
        // explicit so a guest returning the same error code cannot bypass normal diagnostics.
        enum CommitOutcome {
            Published(ActivationReceipt),
            Abandoned(api::Failure),
        }
        let result = (|| -> Result<CommitOutcome, api::Failure> {
            if let Err(error) = admission {
                return Ok(CommitOutcome::Abandoned(error));
            }
            let mut built = prepared.result?;
            if built.instance.is_none()
                && self.demand_owner(token.plugin(), token.scope())
                    != built.existing_owner.as_deref()
            {
                return Err(api::Failure::new(
                    api::ErrorCode::InvalidHandle,
                    "Role preparation owner was replaced",
                ));
            }
            let owner = if let Some(instance) = built.instance.as_mut() {
                let originals = instance.commit_data().map_err(|error| {
                    api::Failure::new(api::ErrorCode::OperationFailed, format!("{error:#}"))
                })?;
                // Cancellation racing the atomic write abandons publication and restores only our staged files.
                if !flight
                    .waiters
                    .lock()
                    .unwrap()
                    .iter()
                    .any(|waiter| !waiter.completion.status().is_terminal())
                {
                    Instance::rollback_data(&originals).map_err(|error| {
                        api::Failure::new(api::ErrorCode::OperationFailed, format!("{error:#}"))
                    })?;
                    // Only a successful rollback makes this a harmless abandoned purpose. A
                    // rollback error above remains a genuine failure and must be published.
                    return Ok(CommitOutcome::Abandoned(api::Failure::new(
                        api::ErrorCode::Cancelled,
                        "Activation wait was cancelled before publication",
                    )));
                }
                instance.commit_activation();
                instance.instance_id().map(str::to_owned)
            } else {
                built.existing_owner
            };
            if let Some(instance) = built.instance {
                if token.scope() == "application"
                    || token.scope() == scopes::workspace_key(&self.environment.workspace)
                {
                    self.live.insert(token.plugin().into(), instance);
                } else {
                    self.parked
                        .get_mut(token.scope())
                        .unwrap()
                        .live
                        .insert(token.plugin().into(), instance);
                }
            }
            let record = self
                .activation
                .records
                .get_mut(&(token.scope().into(), token.plugin().into()))
                .unwrap();
            record.owner = owner.clone();
            record.materialized = built.materialized;
            if let Some(entry) = self.installed.get_mut(token.plugin()) {
                entry.error = None;
            }
            // Native catalogs belong to the selected document workspace, independently of parked service owners.
            if self.workspace_open
                && self.trusted
                && scopes::workspace_key(&built.workspace)
                    == scopes::workspace_key(&self.environment.workspace)
            {
                roles::retain_waited_documents(&mut built.roles, &flight.waiters.lock().unwrap());
                self.publish_demanded_roles(built.roles);
            }
            Ok(CommitOutcome::Published(ActivationReceipt {
                availability: token.clone(),
                owner,
            }))
        })();
        let result = match result {
            Ok(CommitOutcome::Abandoned(error)) => {
                for waiter in std::mem::take(&mut *flight.waiters.lock().unwrap()) {
                    waiter.completion.finish(Err(error.clone()));
                }
                self.refresh_services();
                return Err(error);
            }
            Ok(CommitOutcome::Published(receipt)) => Ok(receipt),
            Err(error) => Err(error),
        };
        let waiters = std::mem::take(&mut *flight.waiters.lock().unwrap());
        for waiter in waiters {
            if waiter.completion.status().is_terminal() {
                continue;
            }
            match &result {
                Ok(receipt) if waiter.reason.is_language() => {
                    let target = waiter.language_workspace.clone().unwrap_or_default();
                    let delivered = self.in_project(&target, |manager| {
                        if let Some(role) = manager.demanded_role_result(&waiter.reason) {
                            waiter.completion.finish(role.map(|_| receipt.clone()));
                        } else if manager.join_language_delegation(
                            &waiter.reason,
                            waiter.completion.clone(),
                            receipt.clone(),
                        ) {
                            // The async declared service now owns this original wait and deadline.
                        } else if let Err(error) =
                            manager.join_demand_waiter(&token, waiter.clone())
                        {
                            waiter.completion.finish(Err(error));
                        }
                    });
                    if delivered.is_none() {
                        waiter.completion.finish(Err(api::Failure::new(
                            api::ErrorCode::InvalidHandle,
                            "Original language workspace has closed",
                        )));
                    }
                }
                _ => waiter.completion.finish(result.clone()),
            }
        }
        if let Err(error) = &result {
            if token.scope() != "application" && token.scope() != self.host_scope() {
                // A failed or closed A preparation cannot poison B's current publication for the same package.
                if let Some(resources) = self
                    .parked
                    .get_mut(token.scope())
                    .and_then(|project| project.resources.as_mut())
                {
                    resources.record_error(token.plugin(), error.to_string());
                }
            } else if let Some(entry) = self.installed.get_mut(token.plugin()) {
                entry.error = Some(error.to_string());
            }
        }
        self.refresh_services();
        result
    }

    /// Input/cancellation polling only launches bounded jobs and receives already finished candidates.
    pub(in crate::manager) fn poll_demands(&mut self) {
        while let Ok(prepared) = self.activation.completed.try_recv() {
            let _ = self.commit_demand(prepared);
        }
        let tokens = self.activation.flights.keys().cloned().collect::<Vec<_>>();
        for token in tokens {
            if self
                .availability_in_scope(token.plugin(), token.scope())
                .map_or(true, |current| current != token)
            {
                self.activation.for_each_waiter(|candidate, waiter| {
                    if candidate == &token {
                        let _ = waiter
                            .completion
                            .cancel(api::CancelMode::TryTerminate, api::ErrorCode::InvalidHandle);
                    }
                });
            }
        }
        self.activation.prune();
        self.schedule_background_demands();
        while let Some(preparation) = self.take_demand_preparation() {
            let token = preparation.token.clone();
            let lease = preparation.lease.clone();
            let sender = self.activation.sender.clone();
            if let Err(error) = std::thread::Builder::new()
                .name("plugin-demand".into())
                .spawn(move || {
                    let _ = sender.send(preparation.run());
                })
            {
                let _ = self.commit_demand(PreparedDemand {
                    token,
                    lease,
                    result: Err(api::Failure::new(
                        api::ErrorCode::OperationFailed,
                        error.to_string(),
                    )),
                });
            }
        }
    }

    fn validate_demand_reason(
        &self,
        token: &Availability,
        reason: &DemandReason,
    ) -> Result<(), api::Failure> {
        let manifest = &self.installed[token.plugin()].manifest;
        if let DemandReason::Language { role, .. } | DemandReason::LanguageDocument { role, .. } =
            reason
        {
            let capability = match role {
                LanguageRole::Lsp => "language.lsp",
                LanguageRole::Formatter => "language.formatting",
                LanguageRole::Direct(_) => "language.direct",
                LanguageRole::Structure => "language.structure",
            };
            if !crate::capabilities::negotiate(manifest)
                .is_ok_and(|api| api.capabilities.contains_key(capability))
            {
                return Err(api::Failure::new(
                    api::ErrorCode::CapabilityUnavailable,
                    "Selected language role was not negotiated",
                ));
            }
        }
        if let DemandReason::LanguageDocument { role, document, .. } = reason {
            if !matches!(role, LanguageRole::Lsp | LanguageRole::Formatter)
                || !crate::capabilities::negotiate(manifest).is_ok_and(|api| {
                    api.capabilities
                        .get("language.lsp")
                        .is_some_and(|version| version >= &semver::Version::new(1, 5, 0))
                })
            {
                return Err(api::Failure::new(
                    api::ErrorCode::CapabilityUnavailable,
                    "Document contexts require language.lsp 1.5 and a native role",
                ));
            }
            if !document.valid() {
                return Err(api::Failure::new(
                    api::ErrorCode::InvalidPath,
                    "Invalid language document path",
                ));
            }
        }
        let declared = match reason {
            DemandReason::Command(id) => manifest.commands.iter().any(|command| &command.id == id),
            DemandReason::View(id) => manifest.panels.iter().any(|panel| &panel.id == id),
            DemandReason::Service(id) => {
                manifest.plugin_services.provides.contains_key(id)
                    || manifest.commands.iter().any(|command| {
                        command.signature.is_some()
                            && plugin_protocol::commands::contract(&manifest.id, &command.id) == *id
                    })
            }
            DemandReason::Background => manifest.activation.background.is_some(),
            DemandReason::Language { provider, role }
            | DemandReason::LanguageDocument { provider, role, .. } => {
                let id = provider.strip_prefix(&format!("{}/", manifest.id));
                id.is_some_and(|id| match role {
                    LanguageRole::Lsp => manifest
                        .language_servers
                        .iter()
                        .any(|item| item.id == id && item.primary),
                    LanguageRole::Formatter => manifest
                        .language_servers
                        .iter()
                        .any(|item| item.id == id && item.formatting),
                    LanguageRole::Direct(feature) => manifest
                        .direct_language_providers
                        .iter()
                        .any(|item| item.id == id && item.features.contains(feature)),
                    LanguageRole::Structure => manifest
                        .structure_providers
                        .iter()
                        .any(|item| item.id == id),
                })
            }
        };
        if !declared {
            return Err(api::Failure::new(
                api::ErrorCode::InvalidRequest,
                "Demand entry or role is not declared",
            ));
        }
        Ok(())
    }

    /// Pending original intent is cancelled by a configuration transition without sealing live references.
    pub(in crate::manager) fn cancel_language_document_waiters(
        &mut self,
        provider: &str,
        document: &plugin_protocol::language::DocumentContext,
    ) {
        self.activation.for_each_waiter(|_, waiter| {
                if matches!(&waiter.reason, DemandReason::LanguageDocument { provider: candidate, document: target, .. }
                    if candidate == provider && target == document)
                {
                    let _ = waiter
                        .completion
                        .cancel(api::CancelMode::TryTerminate, api::ErrorCode::Cancelled);
                }
        });
    }

    /// Pending original intent is cancelled by a configuration transition without sealing live references.
    pub(in crate::manager) fn cancel_pending_demands(&mut self, id: &str, scope: Option<&str>) {
        self.activation.for_each_waiter(|token, waiter| {
            if token.plugin() == id && scope.is_none_or(|scope| scope == token.scope()) {
                let _ = waiter
                    .completion
                    .cancel(api::CancelMode::TryTerminate, api::ErrorCode::InvalidHandle);
            }
        });
    }
}
