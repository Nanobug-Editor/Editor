//! Static scoped service discovery and user selection are independent of first guest birth.
use super::*;

impl Broker {
    /// Every participant that offers contracts, whether a runtime instance or the host itself.
    ///
    /// A guest is discoverable from compatible static metadata before first birth; the host's own session
    /// services are offered for as long as the runtime they belong to is alive. Both are published
    /// through this one list, so a consumer's selection and this registry cannot disagree about who
    /// offers a contract.
    pub fn reconcile(&mut self, guests: Vec<Provider>, host: Vec<Provider>) {
        let mut defaults = BTreeMap::<(String, String), BTreeSet<String>>::new();
        for provider in &host {
            for contract in provider.contracts.keys() {
                defaults
                    .entry((provider.caller.scope.clone(), contract.clone()))
                    .or_default()
                    .insert(provider.caller.plugin.clone());
            }
        }
        // Several host providers for one contract require explicit selection too; order is never authority.
        self.host_defaults = defaults
            .into_iter()
            .filter_map(|(key, choices)| {
                (choices.len() == 1).then(|| (key, choices.into_iter().next().unwrap()))
            })
            .collect();
        self.providers = guests
            .into_iter()
            .chain(host)
            .map(|p| (p.identity(), p))
            .collect();
        self.refresh_selections();
    }
    /// Selection and publication changes withdraw original waits before Manager commits a candidate.
    /// Dropping the last invalid wait cancels its demand without retiring unrelated consumers.
    fn retire_calls(&mut self) {
        let calls = std::mem::take(&mut self.queue);
        self.queue = calls
            .into_iter()
            .filter(|call| {
                let valid = self.context_alive(&call.context) && self.validate_call(call).is_ok();
                if !valid {
                    call.completion.finish(Err(Failure::new(
                        ErrorCode::InvalidHandle,
                        "Service participant exited or was replaced",
                    )));
                }
                valid && !call.completion.status().is_terminal()
            })
            .collect();
        let waits = std::mem::take(&mut self.waiting);
        self.waiting = waits
            .into_iter()
            .filter(|wait| {
                let valid = self.context_alive(&wait.call.context)
                    && self.validate_call(&wait.call).is_ok();
                if !valid {
                    wait.call.completion.finish(Err(Failure::new(
                        ErrorCode::InvalidHandle,
                        "Waiting service publication was retired",
                    )));
                }
                valid && !wait.call.completion.status().is_terminal()
            })
            .collect();
    }
    /// Track changes per contract and logical scope, without invalidating unrelated service handles.
    pub fn refresh_selections(&mut self) {
        let mut available: BTreeMap<(String, String), Vec<String>> = self
            .selections
            .keys()
            .cloned()
            .map(|key| (key, Vec::new()))
            .collect();
        for provider in self.providers.values() {
            for contract in provider.contracts.keys() {
                // Command ownership is explicit; these entries are not selectable service providers.
                if contract.starts_with(plugin_protocol::commands::CONTRACT_PREFIX) {
                    continue;
                }
                available
                    .entry((provider.caller.scope.clone(), contract.clone()))
                    .or_default()
                    .push(provider.identity());
            }
        }
        for ((scope, contract), providers) in available {
            let preference = self.preference(&scope, &contract).cloned();
            let selection = self.selections.entry((scope, contract)).or_default();
            if selection.providers != providers || selection.preference != preference {
                selection.revision = selection
                    .revision
                    .checked_add(1)
                    .expect("Service selection revision exhausted");
                selection.providers = providers;
                selection.preference = preference;
            }
        }
        self.retire_calls();
    }
    fn preference(&self, scope: &str, contract: &str) -> Option<&String> {
        self.preferences
            .projects
            .get(scope)
            .and_then(|map| map.get(contract))
            .or_else(|| self.preferences.user.get(&choice_key(scope, contract)))
            .or_else(|| self.host_defaults.get(&(scope.into(), contract.into())))
    }
    /// Package identity of the provider a start in this scope would use; descriptive, never routing.
    pub fn selected_provider(&self, scope: &str, contract: &str) -> Option<String> {
        if let Some(id) = self.preference(scope, contract)
            && self.providers.values().any(|provider| {
                &provider.caller.plugin == id
                    && provider.caller.scope == scope
                    && provider.contracts.contains_key(contract)
            })
        {
            return Some(id.clone());
        }
        let mut candidates = self
            .providers
            .values()
            .filter(|provider| {
                provider.caller.scope == scope && provider.contracts.contains_key(contract)
            })
            .map(|provider| provider.caller.plugin.clone());
        let first = candidates.next()?;
        // An ambiguous contract has no single owner; report nothing rather than guessing one.
        candidates.next().is_none().then_some(first)
    }
    pub fn resolve(
        &self,
        caller: &Caller,
        contract: &str,
        dependency: &Dependency,
    ) -> Result<Reference, Failure> {
        let candidates: Vec<_> = self
            .providers
            .values()
            .filter(|p| {
                p.alive.load(Ordering::Acquire)
                    && p.caller.scope == caller.scope
                    && p.contracts
                        .get(contract)
                        .is_some_and(|c| dependency.matches(c))
            })
            .collect();
        let selected = if let Some(id) = self.preference(&caller.scope, contract) {
            candidates
                .iter()
                .find(|p| &p.caller.plugin == id)
                .copied()
                .ok_or_else(|| {
                    Failure::new(
                        ErrorCode::CapabilityUnavailable,
                        "Selected service provider is unavailable or incompatible",
                    )
                })?
        } else if candidates.len() == 1 {
            candidates[0]
        } else {
            return Err(Failure::new(
                if candidates.is_empty() {
                    ErrorCode::CapabilityUnavailable
                } else {
                    ErrorCode::Conflict
                },
                format!(
                    "Service {contract} has {} compatible providers; choose one in plugin settings",
                    candidates.len()
                ),
            ));
        };
        Ok(Reference::new(
            selected.clone(),
            contract.into(),
            dependency.clone(),
            self.selections
                .get(&(caller.scope.clone(), contract.into()))
                .map_or(0, |selection| selection.revision),
            false,
        ))
    }
    /// Resolve an existing session's exact incarnation without consulting future-launch preferences.
    pub fn resolve_pinned(
        &self,
        caller: &Caller,
        contract: &str,
        dependency: &Dependency,
        instance: &str,
    ) -> Result<Reference, Failure> {
        let provider = self
            .providers
            .values()
            .find(|provider| provider.caller.instance == instance && provider.running())
            .filter(|provider| {
                provider.caller.scope == caller.scope
                    && provider.alive.load(Ordering::Acquire)
                    && provider
                        .contracts
                        .get(contract)
                        .is_some_and(|shape| dependency.matches(shape))
            })
            .ok_or_else(|| {
                Failure::new(
                    ErrorCode::InvalidHandle,
                    "Session provider exited or changed its contract",
                )
            })?;
        Ok(Reference::new(
            provider.clone(),
            contract.into(),
            dependency.clone(),
            0,
            true,
        ))
    }
    /// Open references retain their provider incarnation; changed user choices cannot retarget them.
    pub fn validate_reference(
        &self,
        caller: &Caller,
        reference: &Reference,
    ) -> Result<(), Failure> {
        let provider = self.reference_provider(reference)?;
        if provider.caller.scope != caller.scope {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Service reference belongs to another scope",
            ));
        }
        if reference.pinned {
            return Ok(());
        }
        let current = self
            .resolve(caller, &reference.contract, &reference.dependency)
            .map_err(|_| {
                Failure::new(
                    ErrorCode::InvalidHandle,
                    "Service reference is no longer available",
                )
            })?;
        if current.revision != reference.revision
            || current.provider.identity() != reference.provider.identity()
        {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Service provider changed; open a new reference",
            ));
        }
        Ok(())
    }
    /// A persistent reference may survive HotApply, but each queued invocation keeps its input epoch.
    /// First birth preserves that epoch; a changed configuration refuses old work before side effects.
    pub(crate) fn validate_call(&self, call: &Call) -> Result<(), Failure> {
        self.validate_reference(&call.context.caller, &call.reference)?;
        let provider = self.reference_provider(&call.reference)?;
        if call.availability.get() != Some(&provider.availability) {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Original service configuration changed",
            ));
        }
        Ok(())
    }
    /// Describe active scoped participants without granting a reference or changing selection.
    pub(crate) fn participants(&self, scope: &str, contract: &str) -> Vec<Provider> {
        self.providers
            .values()
            .filter(|provider| {
                provider.caller.scope == scope && provider.contracts.contains_key(contract)
            })
            .cloned()
            .collect()
    }
    pub fn choices(&self, scope: &str) -> Vec<Choice> {
        let mut contracts: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for provider in self.providers.values().filter(|p| p.caller.scope == scope) {
            for contract in provider.contracts.keys() {
                // Explicit command ownership is never a user-selectable service preference.
                if contract.starts_with(plugin_protocol::commands::CONTRACT_PREFIX) {
                    continue;
                }
                contracts
                    .entry(contract.clone())
                    .or_default()
                    .insert(provider.caller.plugin.clone());
            }
        }
        contracts
            .into_iter()
            .map(|(contract, candidates)| {
                let selected = self.preference(scope, &contract).cloned().or_else(|| {
                    (candidates.len() == 1).then(|| candidates.iter().next().unwrap().clone())
                });
                Choice {
                    user: self
                        .preferences
                        .user
                        .get(&choice_key(scope, &contract))
                        .cloned(),
                    project: self
                        .preferences
                        .projects
                        .get(scope)
                        .and_then(|values| values.get(&contract))
                        .cloned(),
                    scope: if scope == "application" {
                        plugin_protocol::api::InstanceScope::Application
                    } else {
                        plugin_protocol::api::InstanceScope::Workspace
                    },
                    contract,
                    candidates: candidates.into_iter().collect(),
                    selected,
                }
            })
            .collect()
    }
}
