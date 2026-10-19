//! Collection discovery allocates ordinary caller-owned references atomically, without guest execution.
use super::*;

impl State {
    /// Explicit 1.2 opt-in is separate from services.call: old consumers keep single-provider Open.
    fn collection_dependency(&self, contract: &str) -> Result<service::Dependency, Failure> {
        if self
            .api
            .capabilities
            .get("plugin.services")
            .is_none_or(|version| *version < semver::Version::new(1, 2, 0))
        {
            return Err(Failure::new(
                ErrorCode::CapabilityUnavailable,
                "Service collections require plugin.services 1.2",
            ));
        }
        self.plugin_services
            .declarations
            .requires
            .get(contract)
            .filter(|dependency| dependency.collection)
            .cloned()
            .ok_or_else(|| {
                Failure::new(
                    ErrorCode::PermissionDenied,
                    "Service collection dependency was not declared",
                )
            })
    }

    /// No partial collection escapes if the caller lacks handle capacity. Opened members remain
    /// ordinary instance resources and close through the existing CloseResource operation.
    pub(super) fn open_service_set(&mut self, contract: &str) -> Result<Value, Failure> {
        let dependency = self.collection_dependency(contract)?;
        let (revision, references) = self.plugin_services.broker.lock().unwrap().resolve_set(
            &self.plugin_services.principal,
            contract,
            &dependency,
        )?;
        let guard = self.plugin_services.broker.lock().unwrap().set_guard(
            &self.plugin_services.principal,
            contract,
            revision,
        )?;
        if self.roots.len().saturating_add(references.len()) > 128 {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Insufficient handles for the complete service collection",
            ));
        }
        let mut members: Vec<service::SetMember> = Vec::new();
        for reference in references {
            let handle = match self.roots.open(RootKind::ServiceReference) {
                Ok(Value::Resource(handle)) => handle,
                Ok(_) => unreachable!(),
                Err(error) => {
                    for member in members {
                        self.roots.remove(&member.reference);
                        self.plugin_services
                            .references
                            .remove(&member.reference.resource);
                    }
                    return Err(error);
                }
            };
            members.push(service::SetMember {
                provider: reference.provider.caller.plugin.clone(),
                reference: handle.clone(),
            });
            self.plugin_services
                .references
                .insert(handle.resource, reference);
        }
        self.plugin_services
            .sets
            .insert(contract.into(), (revision, guard));
        Ok(Value::ServiceSet(service::ServiceSet { revision, members }))
    }

    /// A revision is a comparison token only. Validation grants no resource and also covers an
    /// optional empty set, whose asynchronous preparation must notice a newly installed provider.
    pub(super) fn validate_service_set(
        &self,
        contract: &str,
        revision: u64,
    ) -> Result<Value, Failure> {
        self.service_set_guard(contract, revision)?;
        Ok(Value::Unit)
    }

    /// Captured membership is local authority, not a transferable or guessable revision label.
    pub(in crate::instance) fn service_set_guard(
        &self,
        contract: &str,
        revision: u64,
    ) -> Result<std::sync::Arc<std::sync::atomic::AtomicBool>, Failure> {
        self.collection_dependency(contract)?;
        let (_, guard) = self
            .plugin_services
            .sets
            .get(contract)
            .filter(|(owned, _)| *owned == revision)
            .ok_or_else(|| {
                Failure::new(
                    ErrorCode::InvalidHandle,
                    "Service collection was not opened by this instance",
                )
            })?;
        self.plugin_services.broker.lock().unwrap().validate_set(
            &self.plugin_services.principal,
            contract,
            revision,
        )?;
        Ok(guard.clone())
    }
}
