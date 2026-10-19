//! Explicit service collections retain a whole scoped publication epoch, including the empty set.
use super::*;

/// A collection generation is shared only by one contract in one logical scope. A replacement
/// seals old references and deferred work; it never retargets them to the new member set.
pub(super) struct CollectionState {
    revision: u64,
    alive: Arc<AtomicBool>,
    preference: Option<String>,
    members: Vec<Member>,
}

/// Logical publications survive first birth; once observed, a concrete owner cannot be replaced
/// inside a captured collection. Contract/configuration changes also invalidate its preparation.
struct Member {
    identity: String,
    availability: Option<crate::Availability>,
    publication: Arc<AtomicBool>,
    declaration: String,
    owner: Option<(String, Arc<AtomicBool>)>,
    running: bool,
}
impl Member {
    fn capture(provider: &Provider, contract: &Contract) -> Self {
        Self {
            identity: provider.identity(),
            availability: provider.availability.clone(),
            publication: provider.alive.clone(),
            declaration: serde_json::to_string(contract).expect("bounded contract serializes"),
            owner: provider
                .owner_alive
                .as_ref()
                .map(|alive| (provider.caller.instance.clone(), alive.clone())),
            running: provider.running(),
        }
    }
    fn changed(&self, next: &Self) -> bool {
        self.identity != next.identity
            || self.availability != next.availability
            || !Arc::ptr_eq(&self.publication, &next.publication)
            || self.declaration != next.declaration
            || self.owner.as_ref().is_some_and(|(id, alive)| {
                next.owner.as_ref().is_none_or(|(next_id, next_alive)| {
                    id != next_id || !Arc::ptr_eq(alive, next_alive)
                }) || self.running != next.running
            })
    }
    fn live(&self) -> bool {
        self.publication.load(Ordering::Acquire)
            && (!self.running
                || self
                    .owner
                    .as_ref()
                    .is_none_or(|(_, alive)| alive.load(Ordering::Acquire)))
    }
}

impl Broker {
    /// Metadata capture alone never starts a guest. An explicit preference restricts a collection
    /// to that provider, including an unavailable preference which must not fall back to others.
    fn collection_members(&self, scope: &str, contract: &str) -> Vec<Member> {
        let preference = self.preference(scope, contract);
        self.providers
            .values()
            .filter(|provider| {
                provider.caller.scope == scope
                    && provider.alive.load(Ordering::Acquire)
                    && preference.is_none_or(|selected| selected == &provider.caller.plugin)
            })
            .filter_map(|provider| {
                provider
                    .contracts
                    .get(contract)
                    .filter(|declaration| declaration.collection)
                    .map(|declaration| Member::capture(provider, declaration))
            })
            .collect()
    }

    /// Reconcile only previously opened collections; unrelated installation cannot consume an
    /// unbounded registry. First activation records its owner without cancelling the activating call.
    pub(super) fn refresh_collections(&mut self) {
        let keys = self.collections.keys().cloned().collect::<Vec<_>>();
        for (scope, contract) in keys {
            let members = self.collection_members(&scope, &contract);
            let preference = self.preference(&scope, &contract).cloned();
            let state = self.collections.get_mut(&(scope, contract)).unwrap();
            let changed = state.preference != preference
                || state.members.len() != members.len()
                || state
                    .members
                    .iter()
                    .zip(&members)
                    .any(|(old, new)| old.changed(new));
            if changed {
                state.alive.store(false, Ordering::Release);
                state.revision = state
                    .revision
                    .checked_add(1)
                    .expect("Service collection generation exhausted");
                state.alive = Arc::new(AtomicBool::new(true));
            }
            state.preference = preference;
            state.members = members;
        }
    }

    /// Resolve all explicitly composable compatible providers, never broadcasting an ordinary Open.
    /// Returned references are owned by the caller and keep the existing first-birth/permission path.
    pub(crate) fn resolve_set(
        &mut self,
        caller: &Caller,
        contract: &str,
        dependency: &Dependency,
    ) -> Result<(u64, Vec<Reference>), Failure> {
        if !dependency.collection {
            return Err(Failure::new(
                ErrorCode::PermissionDenied,
                "Service dependency did not declare collection discovery",
            ));
        }
        self.refresh_collections();
        let candidates = if self.preference(&caller.scope, contract).is_some() {
            let selected = self.resolve(caller, contract, dependency)?;
            if !selected.provider.contracts[contract].collection {
                return Err(Failure::new(
                    ErrorCode::CapabilityUnavailable,
                    "Selected provider does not offer a composable contract",
                ));
            }
            vec![selected.provider]
        } else {
            self.providers
                .values()
                .filter(|provider| {
                    provider.caller.scope == caller.scope
                        && provider.alive.load(Ordering::Acquire)
                        && provider.contracts.get(contract).is_some_and(|declaration| {
                            declaration.collection && dependency.matches(declaration)
                        })
                })
                .cloned()
                .collect::<Vec<_>>()
        };
        if candidates.len() > plugin_protocol::service::MAX_SET_MEMBERS {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Service collection exceeds its member quota",
            ));
        }
        if candidates.is_empty() && !dependency.optional {
            return Err(Failure::new(
                ErrorCode::CapabilityUnavailable,
                "Required service collection has no compatible provider",
            ));
        }
        let key = (caller.scope.clone(), contract.to_owned());
        if !self.collections.contains_key(&key) {
            if self.collections.len() >= 1024 {
                return Err(Failure::new(
                    ErrorCode::LimitExceeded,
                    "Service collection registry is full",
                ));
            }
            self.collections.insert(
                key.clone(),
                CollectionState {
                    revision: 1,
                    alive: Arc::new(AtomicBool::new(true)),
                    preference: self.preference(&caller.scope, contract).cloned(),
                    members: self.collection_members(&caller.scope, contract),
                },
            );
        }
        let state = &self.collections[&key];
        let references = candidates
            .into_iter()
            .map(|provider| {
                let mut reference = Reference::new(
                    provider,
                    contract.into(),
                    dependency.clone(),
                    state.revision,
                    false,
                );
                reference.collection = Some(state.alive.clone());
                reference
            })
            .collect();
        Ok((state.revision, references))
    }

    /// Check the complete epoch after asynchronous preparation, also when discovery returned zero
    /// members. This comparison grants no access and cannot replace an owned reference or lease.
    pub(crate) fn validate_set(
        &self,
        caller: &Caller,
        contract: &str,
        revision: u64,
    ) -> Result<(), Failure> {
        let valid = self
            .collections
            .get(&(caller.scope.clone(), contract.into()))
            .is_some_and(|state| {
                state.revision == revision
                    && state.alive.load(Ordering::Acquire)
                    && state.members.iter().all(Member::live)
            });
        if valid {
            Ok(())
        } else {
            Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Service collection membership changed; prepare a new set",
            ))
        }
    }

    /// Borrow the monotonic epoch only after successful scoped validation. Only the owning instance
    /// records this guard after OpenSet; native operations cannot manufacture it from a number.
    pub(crate) fn set_guard(
        &self,
        caller: &Caller,
        contract: &str,
        revision: u64,
    ) -> Result<Arc<AtomicBool>, Failure> {
        self.validate_set(caller, contract, revision)?;
        Ok(self.collections[&(caller.scope.clone(), contract.into())]
            .alive
            .clone())
    }
}
