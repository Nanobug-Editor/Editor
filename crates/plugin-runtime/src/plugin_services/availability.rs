//! Logical service publications bind one first incarnation without retargeting open references.
use super::*;

impl Provider {
    /// Guest selection is keyed by logical scope/package; host participants retain their sealed identity.
    /// The length prefix prevents a workspace path separator from colliding with a plugin identifier.
    pub(crate) fn identity(&self) -> String {
        if let Some(token) = &self.availability {
            format!(
                "guest:{}:{}:{}",
                token.scope().len(),
                token.scope(),
                token.plugin()
            )
        } else {
            self.caller.instance.clone()
        }
    }

    /// A static publication owns no WASM; an exact running incarnation has an additional revocation flag.
    pub(crate) fn running(&self) -> bool {
        self.availability.is_none()
            || self
                .owner_alive
                .as_ref()
                .is_some_and(|alive| alive.load(Ordering::Acquire))
    }
}

impl Reference {
    /// Each open reference shares its one binding with all queued clones, never with a later open.
    pub(super) fn new(
        provider: Provider,
        contract: String,
        dependency: Dependency,
        revision: u64,
        pinned: bool,
    ) -> Self {
        let binding = Arc::new(OnceLock::new());
        if provider.running() {
            let _ = binding.set(BoundOwner {
                instance: provider.caller.instance.clone(),
                alive: provider
                    .owner_alive
                    .clone()
                    .unwrap_or_else(|| provider.alive.clone()),
            });
        }
        Self {
            collection: None,
            provider,
            contract,
            dependency,
            revision,
            pinned,
            binding,
        }
    }
    /// A pending native consumer sees no owner until first birth; clones then see the same exact identity.
    pub(crate) fn owner(&self) -> Option<&str> {
        self.binding.get().map(|owner| owner.instance.as_str())
    }
    /// Exact incarnation retirement is additional to removal of its static contribution.
    pub(crate) fn owner_alive(&self) -> bool {
        self.binding
            .get()
            .is_none_or(|owner| owner.alive.load(Ordering::Acquire))
    }
}

impl Broker {
    /// An explicit host contributor retains its logical identity before birth, bypassing future-launch preferences.
    /// Exact contract shape, original scope and aliveness still precede creation of the reference.
    pub(crate) fn resolve_named(
        &self,
        caller: &Caller,
        contract: &str,
        dependency: &Dependency,
        plugin: &str,
    ) -> Result<Reference, Failure> {
        let provider = self
            .providers
            .values()
            .find(|provider| {
                provider.caller.plugin == plugin
                    && provider.caller.scope == caller.scope
                    && provider.alive.load(Ordering::Acquire)
                    && provider
                        .contracts
                        .get(contract)
                        .is_some_and(|shape| dependency.matches(shape))
            })
            .ok_or_else(|| {
                Failure::new(
                    ErrorCode::CapabilityUnavailable,
                    "Named service contributor is unavailable or incompatible",
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
    /// Find the same logical publication and declaration, preserving its original revocation boundary.
    /// Configuration can change on one owner; removal, replacement and scope closure cannot revive it.
    pub(crate) fn reference_provider(&self, reference: &Reference) -> Result<&Provider, Failure> {
        let current = self
            .providers
            .get(&reference.provider.identity())
            .filter(|current| {
                current.alive.load(Ordering::Acquire)
                    && reference.provider.alive.load(Ordering::Acquire)
                    && Arc::ptr_eq(&current.alive, &reference.provider.alive)
                    && current
                        .contracts
                        .get(&reference.contract)
                        .is_some_and(|shape| reference.dependency.matches(shape))
            })
            .ok_or_else(|| {
                Failure::new(
                    ErrorCode::InvalidHandle,
                    "Service publication was retired or changed",
                )
            })?;
        if let Some(owner) = reference.owner()
            && (!reference.owner_alive() || !current.running() || current.caller.instance != owner)
        {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Service reference incarnation was retired",
            ));
        }
        Ok(current)
    }

    /// Bind the receipt's exact incarnation once after Manager admission; never resolve a new provider here.
    pub(crate) fn bind_reference(
        &self,
        reference: &Reference,
        owner: &str,
    ) -> Result<Reference, Failure> {
        let current = self.reference_provider(reference)?;
        if !current.running() || current.caller.instance != owner {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "First service birth does not match its receipt",
            ));
        }
        let _ = reference.binding.set(BoundOwner {
            instance: owner.into(),
            alive: current
                .owner_alive
                .clone()
                .unwrap_or_else(|| current.alive.clone()),
        });
        if reference.owner() != Some(owner) {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Service reference cannot be rebound",
            ));
        }
        let mut bound = reference.clone();
        bound.provider = current.clone();
        Ok(bound)
    }
}
