//! Domain discovery epochs retire nested metadata independently from package and text revisions.
use super::*;

/// Each replacement retires the prior immutable guard; revision zero is the undiscovered state.
#[derive(Default)]
pub(super) struct Discovery {
    pub(super) revision: u64,
    pub(super) alive: Option<Arc<AtomicBool>>,
}

impl LanguageContexts {
    /// Read current domain inputs without creating a guard during metadata-only catalog scans.
    pub(crate) fn discovery_revision(&self, owner: &str) -> u64 {
        self.0
            .lock()
            .unwrap()
            .discovery
            .get(owner)
            .map_or(0, |epoch| epoch.revision)
    }

    /// Borrow all actual participants' discovery epochs; guest-supplied caller strings are unused.
    pub(crate) fn borrow_discovery(
        &self,
        mut authority: EditorAuthority,
        owners: &[String],
    ) -> Result<EditorAuthority, Failure> {
        let owners = owners.iter().collect::<std::collections::BTreeSet<_>>();
        let mut state = self.0.lock().unwrap();
        let added = owners
            .iter()
            .filter(|owner| !state.discovery.contains_key(owner.as_str()))
            .count();
        if state.discovery.len() + added > 4096 {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Language discovery epoch quota exceeded",
            ));
        }
        for owner in owners {
            let epoch = state.discovery.entry(owner.clone()).or_default();
            let guard = epoch
                .alive
                .get_or_insert_with(|| Arc::new(AtomicBool::new(true)));
            authority = authority.with_guard(guard.clone());
        }
        Ok(authority)
    }

    /// Domain refresh revokes owned registrations and every nested borrower before the next tick.
    pub(crate) fn invalidate(&self, owner: &str) -> Result<(), Failure> {
        let mut state = self.0.lock().unwrap();
        if !state.discovery.contains_key(owner) && state.discovery.len() >= 4096 {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Language discovery epoch quota exceeded",
            ));
        }
        let epoch = state.discovery.entry(owner.into()).or_default();
        let revision = epoch.revision.checked_add(1).ok_or_else(|| {
            Failure::new(
                ErrorCode::LimitExceeded,
                "Language discovery revision exhausted",
            )
        })?;
        if let Some(alive) = epoch.alive.take() {
            alive.store(false, Ordering::Release);
        }
        epoch.revision = revision;
        for context in state
            .references
            .values()
            .filter(|context| context.owner == owner)
        {
            context.seal.store(false, Ordering::Release);
        }
        state.sweep();
        Ok(())
    }
}
