//! Readonly contract matching shares publication gates and never activates or publishes a candidate.
use super::*;

impl Services {
    /// Candidate and live publications expose identical platform/configuration gates and commands.
    /// This metadata snapshot owns no reference, process or publication lifetime.
    pub(super) fn contracts(
        &self,
        configuration: &plugin_protocol::settings::Effective,
    ) -> BTreeMap<String, service::Contract> {
        let mut contracts = self
            .declarations
            .provides
            .iter()
            .filter(|(_, contract)| {
                (contract.platforms.is_empty()
                    || contract.platforms.contains(&crate::development::platform()))
                    && contract.enabled_setting.as_ref().is_none_or(|key| {
                        configuration
                            .get(key)
                            .is_some_and(|value| value.value == serde_json::Value::Bool(true))
                    })
            })
            .map(|(id, contract)| (id.clone(), contract.clone()))
            .collect::<BTreeMap<_, _>>();
        for (id, signature) in &self.commands {
            contracts.insert(
                plugin_protocol::commands::contract(&self.principal.plugin, id),
                service::Contract {
                    collection: false,
                    // Typed commands need no provider-owned dependency or contribution gate.
                    preparation: BTreeMap::new(),
                    enabled_setting: None,
                    platforms: Vec::new(),
                    version: semver::Version::new(1, 0, 0),
                    methods: BTreeMap::from([("invoke".into(), signature.clone())]),
                },
            );
        }
        contracts
    }
}

impl Instance {
    /// Capture a prepared or live instance's actual gated contracts without publishing its owner.
    /// The snapshot contains only inert metadata and cannot launch or retarget a service request.
    pub(crate) fn service_replacement(&self) -> broker::ServiceReplacement {
        let services = &self.store.data().plugin_services;
        broker::ServiceReplacement {
            plugin: services.principal.plugin.clone(),
            scope: services.principal.scope.clone(),
            contracts: services.contracts(&self.configuration),
        }
    }

    /// Reject only a live mandatory consumer that currently resolves to this replaced guest/scope.
    /// Optional, unresolved and differently selected dependencies impose no new update policy.
    /// Host publications have no guest availability and cannot be mistaken for a same-named package.
    pub(crate) fn validate_service_replacement(
        &self,
        replacement: &broker::ServiceReplacement,
        broker: &broker::Broker,
    ) -> Result<(), Failure> {
        let state = self.store.data();
        if state.roots.retired || !state.active {
            return Ok(());
        }
        for (id, dependency) in &state.plugin_services.declarations.requires {
            if dependency.optional {
                continue;
            }
            let Ok(reference) = broker.resolve(&state.plugin_services.principal, id, dependency)
            else {
                // Preflight preserves usable relationships; it does not repair unrelated missing
                // dependencies or resolve an ambiguous future graph on the consumer's behalf.
                continue;
            };
            let Some(token) = &reference.provider.availability else {
                continue;
            };
            if token.plugin() == replacement.plugin
                && token.scope() == replacement.scope
                && !replacement
                    .contracts
                    .get(id)
                    .is_some_and(|contract| dependency.matches(contract))
            {
                return Err(Failure::new(
                    ErrorCode::CapabilityUnavailable,
                    format!(
                        "Service {id} replacement is incompatible with active consumer {}",
                        state.plugin_services.principal.plugin
                    ),
                ));
            }
        }
        Ok(())
    }
}
