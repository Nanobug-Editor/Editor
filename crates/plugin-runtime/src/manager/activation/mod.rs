//! Static availability is distinct from the revocable owner produced by an actual demand.
use super::*;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc,
};
mod background;
mod deferred;
mod driver;
mod language_preparation;
mod lease;
mod package;
mod preparation;
mod roles;
mod types;
pub use preparation::{DemandPreparation, PreparedDemand};
pub use types::{ActivationReceipt, Availability, DemandReason, DemandRequest, LanguageRole};

/// Identity-only records share the existing Manager's installed metadata and live owner maps.
pub(super) struct State {
    identity: u64,
    selected_scope: String,
    next: u64,
    records: BTreeMap<(String, String), Record>,
    flights: BTreeMap<Availability, Flight>,
    /// Renewed purposes wait outside the abandoned worker's shared authority and original budget.
    deferred: BTreeMap<Availability, Vec<Waiter>>,
    completed: mpsc::Receiver<PreparedDemand>,
    sender: mpsc::Sender<PreparedDemand>,
    /// Continuous purposes are ordinary bounded waiters; the Manager retains their cancellation owners.
    background: BTreeMap<Availability, DemandRequest>,
}
struct Record {
    token: Availability,
    metadata: Vec<u8>,
    configuration: Vec<u8>,
    owner: Option<String>,
    alive: Arc<AtomicBool>,
    materialized: bool,
}
struct Flight {
    waiters: Arc<Mutex<Vec<Waiter>>>,
    preparation: Option<DemandPreparation>,
    running: bool,
    /// Job/result ownership, not the flight itself, keeps an external worker reservation alive.
    lease_count: Arc<std::sync::atomic::AtomicUsize>,
}
#[derive(Clone)]
struct Waiter {
    completion: crate::Completion<ActivationReceipt>,
    reason: DemandReason,
    /// Application owners may span workspaces; a language plan still belongs to its original workspace.
    language_workspace: Option<String>,
}
impl Default for State {
    fn default() -> Self {
        static NEXT_MANAGER: AtomicU64 = AtomicU64::new(1);
        let (sender, completed) = mpsc::channel();
        Self {
            identity: NEXT_MANAGER.fetch_add(1, Ordering::Relaxed),
            selected_scope: String::new(),
            next: 1,
            records: BTreeMap::new(),
            flights: BTreeMap::new(),
            deferred: BTreeMap::new(),
            completed,
            sender,
            background: BTreeMap::new(),
        }
    }
}
impl State {
    /// Canonicalize only at scope selection; immutable final admission never performs filesystem I/O.
    pub(super) fn new(workspace: &str) -> Self {
        Self {
            selected_scope: scopes::workspace_key(workspace),
            ..Default::default()
        }
    }
    pub(super) fn select_scope(&mut self, scope: &str) {
        self.selected_scope = scope.into();
    }
    /// Retirement removes an identity instead of allowing disable/enable to restore its old generation.
    pub(super) fn invalidate(&mut self, plugin: &str, scope: Option<&str>) {
        self.records.retain(|(record_scope, id), record| {
            let keep = id != plugin || scope.is_some_and(|scope| scope != record_scope);
            if !keep {
                record.alive.store(false, Ordering::Release);
            }
            keep
        });
    }
}
impl Manager {
    /// Publish opaque identities only for enabled, trusted, open, currently compatible installed entries.
    /// Reading metadata does not instantiate a Store, execute a hook or acquire guest resources.
    pub fn availabilities(&mut self) -> BTreeMap<String, Availability> {
        let ids = self
            .published_entries()
            .into_iter()
            .filter(|entry| entry.enabled)
            .map(|entry| entry.manifest.id)
            .collect::<Vec<_>>();
        ids.into_iter()
            .filter_map(|id| self.availability(&id).ok().map(|token| (id, token)))
            .collect()
    }
    /// Capture the selected logical scope and its original effective configuration, without waking it.
    /// Disabled, restricted, closed or incompatible packages return a typed unavailable failure.
    pub fn availability(&mut self, id: &str) -> Result<Availability, api::Failure> {
        let entry = self.installed.get(id).ok_or_else(|| {
            api::Failure::new(api::ErrorCode::NotFound, "Plugin is not installed")
        })?;
        if !self.trusted || !self.workspace_open {
            return Err(api::Failure::new(
                api::ErrorCode::PermissionDenied,
                "Workspace is restricted or closed",
            ));
        }
        let scope = if entry.manifest.scope == api::InstanceScope::Application {
            "application".into()
        } else {
            scopes::workspace_key(&self.environment.workspace)
        };
        self.availability_in_scope(id, &scope)
    }
    /// Broker calls retain their original logical scope, including parked open trusted workspaces.
    /// This lookup never selects another workspace or borrows its current-window authority.
    pub(crate) fn availability_in_scope(
        &mut self,
        id: &str,
        scope: &str,
    ) -> Result<Availability, api::Failure> {
        let environment = self.demand_environment(scope)?;
        let entry = self.installed.get(id).cloned().ok_or_else(|| {
            api::Failure::new(api::ErrorCode::NotFound, "Plugin is not installed")
        })?;
        if entry.compatibility_error().is_some()
            || !(entry.enabled
                || (entry.manifest.scope == api::InstanceScope::Workspace
                    && entry.project_enabled_in(&environment.workspace)))
            || (entry.manifest.scope == api::InstanceScope::Application) != (scope == "application")
        {
            return Err(api::Failure::new(
                api::ErrorCode::CapabilityUnavailable,
                "Plugin is not available in this scope",
            ));
        }
        package::metadata(&self.root, &entry).map_err(|error| {
            api::Failure::new(api::ErrorCode::CapabilityUnavailable, format!("{error:#}"))
        })?;
        let configuration = serde_json::to_vec(
            &self
                .settings_for_workspace(&entry.manifest, &environment.workspace)
                .map_err(|error| {
                    api::Failure::new(api::ErrorCode::InvalidState, format!("{error:#}"))
                })?,
        )
        .unwrap();
        let metadata = serde_json::to_vec(&(&entry.digest, &entry.grants)).unwrap();
        let owner = self.demand_owner(id, scope).map(str::to_owned);
        let key = (scope.to_owned(), id.to_owned());
        let changed = self.activation.records.get(&key).is_some_and(|record| {
            record.metadata != metadata
                || record
                    .owner
                    .as_ref()
                    .is_some_and(|old| owner.as_ref() != Some(old))
        });
        if changed {
            self.activation.invalidate(id, Some(&scope));
        }
        if !self.activation.records.contains_key(&key) {
            let generation = self.activation.next;
            self.activation.next += 1;
            self.activation.records.insert(
                key.clone(),
                Record {
                    token: Availability {
                        manager: self.activation.identity,
                        plugin: id.into(),
                        scope: scope.into(),
                        generation,
                        configuration: generation,
                    },
                    metadata,
                    configuration: configuration.clone(),
                    owner: owner.clone(),
                    alive: Arc::new(AtomicBool::new(true)),
                    materialized: owner.is_some(),
                },
            );
        }
        let record = self.activation.records.get_mut(&key).unwrap();
        if record.configuration != configuration {
            // HotApply may keep the owner and its already bound resources, while pending intent becomes stale.
            record.configuration = configuration;
            record.token.configuration = self.activation.next;
            self.activation.next += 1;
        }
        record.owner = owner;
        // Cutover can publish a cold record before rollback inserts an already activated owner.
        // Adopt that live business incarnation before preparing a later role; activating it again
        // would replace the owner that signs the new plan. Declaration-only success stays intact.
        // demand_owner only returns Instance::instance_id after Activate, never an isolated worker.
        record.materialized |= record.owner.is_some();
        Ok(record.token.clone())
    }
    /// Recheck the original static token and exact admitted owner before executing original arguments.
    pub fn admit_receipt(
        &mut self,
        original: &Availability,
        receipt: &ActivationReceipt,
    ) -> Result<(), api::Failure> {
        if receipt.availability() != original
            || self.availability_in_scope(original.plugin(), original.scope())? != *original
            || self.demand_owner(original.plugin(), original.scope()) != receipt.owner()
            || !self
                .activation
                .records
                .get(&(original.scope().into(), original.plugin().into()))
                .is_some_and(|record| record.materialized)
        {
            return Err(api::Failure::new(
                api::ErrorCode::InvalidHandle,
                "Activation receipt was retired or replaced",
            ));
        }
        Ok(())
    }
    /// Logical resource seals outlive configuration generations but never availability retirement.
    pub(crate) fn availability_alive(&self, token: &Availability) -> Option<Arc<AtomicBool>> {
        self.activation
            .records
            .get(&(token.scope().into(), token.plugin().into()))
            .filter(|record| {
                record.token.generation == token.generation && record.token.manager == token.manager
            })
            .map(|record| record.alive.clone())
    }
    /// Pure final publication check; it performs no I/O, preparation or mutable resampling.
    pub(crate) fn availability_current(&self, token: &Availability) -> bool {
        let selected = self.workspace_open
            && self.trusted
            && (token.scope() == "application" || token.scope() == self.activation.selected_scope);
        let parked = if token.scope() == "application" {
            self.parked.values().any(|parked| parked.trusted)
        } else {
            self.parked
                .get(token.scope())
                .is_some_and(|parked| parked.trusted)
        };
        (selected || parked)
            && self
                .activation
                .records
                .get(&(token.scope().into(), token.plugin().into()))
                .is_some_and(|record| {
                    record.token == *token && record.alive.load(Ordering::Acquire)
                })
    }
    /// Publish all eligible service scopes from the existing installed/parked maps, without running hooks.
    pub(crate) fn service_availabilities(
        &mut self,
    ) -> Vec<(Installed, Availability, Arc<AtomicBool>)> {
        let mut scopes = self
            .parked
            .iter()
            .filter(|(_, parked)| parked.trusted)
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>();
        if self.trusted && self.workspace_open {
            scopes.push(scopes::workspace_key(&self.environment.workspace));
        }
        if !scopes.is_empty() {
            scopes.push("application".into());
        }
        let entries = self.installed.values().cloned().collect::<Vec<_>>();
        let mut result = vec![];
        for scope in scopes {
            for entry in &entries {
                if (entry.manifest.scope == api::InstanceScope::Application)
                    != (scope == "application")
                {
                    continue;
                }
                if let Ok(token) = self.availability_in_scope(&entry.manifest.id, &scope) {
                    let alive = self.availability_alive(&token).unwrap();
                    result.push((entry.clone(), token, alive));
                }
            }
        }
        result
    }
    fn demand_environment(&self, scope: &str) -> Result<Environment, api::Failure> {
        if self.workspace_open
            && self.trusted
            && (scope == "application"
                || scope == scopes::workspace_key(&self.environment.workspace))
        {
            return Ok(self.environment.clone());
        }
        let parked = if scope == "application" {
            self.parked.values().find(|parked| parked.trusted)
        } else {
            self.parked.get(scope).filter(|parked| parked.trusted)
        };
        parked
            .map(|parked| parked.environment.clone())
            .ok_or_else(|| {
                api::Failure::new(
                    api::ErrorCode::InvalidHandle,
                    "Original scope is closed or restricted",
                )
            })
    }
    fn demand_owner(&self, id: &str, scope: &str) -> Option<&str> {
        self.demand_instance(id, scope)
            .and_then(Instance::instance_id)
    }
    fn demand_instance(&self, id: &str, scope: &str) -> Option<&Instance> {
        if scope == "application" || scope == scopes::workspace_key(&self.environment.workspace) {
            self.live.get(id)
        } else {
            self.parked
                .get(scope)
                .and_then(|parked| parked.live.get(id))
        }
    }
    /// Reopened registry metadata is checked against its real installed host-version contribution.
    pub(in crate::manager) fn revalidate_installed_metadata(
        &self,
        entry: &Installed,
    ) -> anyhow::Result<()> {
        package::metadata(&self.root, entry)
    }
    /// Finite migration and fresh rollback share full current archive admission of immutable saved assets.
    pub(in crate::manager) fn read_installed_assets(
        &self,
        entry: &Installed,
    ) -> anyhow::Result<(PathBuf, Package)> {
        package::read(&self.root, entry)
    }
}
