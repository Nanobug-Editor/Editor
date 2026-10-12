//! Detached compilation and business activation never borrow the actor's mutable Manager or Store.
use super::*;
use std::sync::Mutex;

/// Owned inputs for one coalesced first activation. Run off the native input/cancellation actor.
pub struct DemandPreparation {
    pub(super) token: Availability,
    pub(super) entry: Installed,
    pub(super) root: PathBuf,
    pub(super) data: PathBuf,
    pub(super) environment: Environment,
    pub(super) configuration: plugin_protocol::settings::Effective,
    pub(super) engine: Option<wasmtime::Engine>,
    pub(super) host_resources: crate::HostResources,
    pub(super) diagnostics: crate::faults::NativeDiagnostics,
    pub(super) broker: crate::plugin_services::Shared,
    pub(super) alive: Arc<AtomicBool>,
    pub(super) host_alive: Arc<AtomicBool>,
    pub(super) waiters: Arc<Mutex<Vec<Waiter>>>,
    /// Materializing a later role must never Activate the already live business a second time.
    pub(super) activate_business: bool,
    pub(super) existing_owner: Option<String>,
    pub(super) authority: Option<crate::EditorAuthority>,
    pub(super) lease: lease::PreparationLease,
}

/// A prepared candidate remains unpublished until its originating Manager rechecks every captured input.
pub struct PreparedDemand {
    pub(super) token: Availability,
    pub(super) result: Result<BuiltDemand, api::Failure>,
    /// Dropping an unpublished result releases the originating worker reservation.
    pub(super) lease: lease::PreparationLease,
}
/// A role-only job retains its original exact owner; first birth also carries its unpublished Store.
pub(super) struct BuiltDemand {
    pub(super) instance: Option<Instance>,
    pub(super) existing_owner: Option<String>,
    pub(super) roles: Vec<roles::PreparedRole>,
    pub(super) workspace: String,
    /// A failed selected language plan must not turn a cold business into an activated owner.
    pub(super) materialized: bool,
}
impl DemandPreparation {
    /// Compilation and bounded guest hooks run here; cancellation is checked before and after each seam.
    /// Failed/cancelled candidates drop their own resources and never commit staged private writes.
    pub fn run(self) -> PreparedDemand {
        // A host panic must not strand the four-worker pool or leave its waiters pending forever.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.prepare()))
            .map_err(|_| {
                api::Failure::new(
                    api::ErrorCode::OperationFailed,
                    "Activation preparation panicked",
                )
            })
            .and_then(|result| result.map_err(lease::failure));
        PreparedDemand {
            token: self.token.clone(),
            result,
            lease: self.lease.clone(),
        }
    }
    pub(super) fn check(&self) -> anyhow::Result<()> {
        if !self.host_alive.load(Ordering::Acquire) || !self.alive.load(Ordering::Acquire) {
            anyhow::bail!(api::Failure::new(
                api::ErrorCode::InvalidHandle,
                "Demand owner was retired"
            ));
        }
        if !self
            .waiters
            .lock()
            .unwrap()
            .iter()
            .any(|waiter| !waiter.completion.status().is_terminal())
        {
            anyhow::bail!(api::Failure::new(
                api::ErrorCode::Cancelled,
                "All activation waits were cancelled or expired"
            ));
        }
        Ok(())
    }
    fn prepare(&self) -> anyhow::Result<BuiltDemand> {
        self.check()?;
        let (assets, package) = package::read(&self.root, &self.entry)?;
        self.check()?;
        let Some(component) = &package.manifest.component else {
            return Ok(BuiltDemand {
                instance: None,
                existing_owner: None,
                roles: self.prepare_roles(None, None, None),
                workspace: self.environment.workspace.clone(),
                materialized: true,
            });
        };
        let engine = match &self.engine {
            Some(engine) => engine.clone(),
            None => Instance::engine()?,
        };
        let snapshot_path = self.data.parent().unwrap().join("state.json");
        let snapshot = match std::fs::File::open(&snapshot_path) {
            Ok(file) => {
                use std::io::Read;
                anyhow::ensure!(
                    file.metadata()?.len() <= 40 * 1024 * 1024,
                    "Saved state exceeds host quota"
                );
                let mut bytes = Vec::new();
                file.take(40 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
                anyhow::ensure!(
                    bytes.len() <= 40 * 1024 * 1024,
                    "Saved state exceeds host quota"
                );
                Some(serde_json::from_slice(&bytes)?)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        let mut instance = Instance::prepare_with_resources(
            &engine,
            &package.files[component],
            &self.entry.manifest,
            &self.entry.grants,
            self.environment.clone(),
            self.data.clone(),
            assets.clone(),
            snapshot,
            self.host_resources.clone(),
        )?;
        self.check()?;
        instance.configure_settings(&self.entry.manifest, self.configuration.clone())?;
        self.check()?;
        instance.connect_services(self.broker.clone())?;
        instance.connect_diagnostics(self.diagnostics.clone());
        let authority = self
            .authority
            .clone()
            .or_else(|| Some(instance.editor_authority()));
        let code = language_preparation::ComponentCode {
            engine: &engine,
            bytes: &package.files[component],
            assets: &assets,
        };
        let mut roles = self.prepare_roles(Some(code), Some(&mut instance), authority);
        // Activate owns candidate resources, but durable private writes and Secret/Temporary ownership
        // remain gated until final commit. Cancellation does not promise external side-effect rollback.
        self.check()?;
        // Only an admitted non-language demand or a successfully prepared selected role justifies Main.
        // Failed role results are published independently, so retrying discovery cannot run business.
        let activate = self.activate_business && self.business_is_demanded(&roles);
        if activate {
            instance.activate()?;
        }
        if self.activate_business && !activate {
            self.retain_unactivated_failures(&mut roles);
        }
        self.check()?;
        Ok(BuiltDemand {
            instance: activate.then_some(instance),
            existing_owner: self.existing_owner.clone(),
            roles,
            workspace: self.environment.workspace.clone(),
            materialized: !self.activate_business || activate,
        })
    }
}
