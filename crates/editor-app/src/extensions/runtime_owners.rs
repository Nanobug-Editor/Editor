//! Application-owned plugin stores share data locks and transactions, while actors retain project messages.

use plugin_runtime::{Manager, plugin_protocol::Environment};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

/// Standalone tooling owns an entire runtime; GUI projects release only their own selected scope.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Lifetime {
    Application,
    Actor,
}

/// Exactly one executable Manager owns each canonical plugin store for this application profile.
pub(crate) struct RuntimePool {
    managers: Mutex<BTreeMap<PathBuf, Arc<Mutex<Manager>>>>,
    lifetime: Lifetime,
    closing: AtomicBool,
    /// Slow candidate preparation owns the one persistent mutation lane across project actors.
    installation: Arc<AtomicBool>,
}

impl Default for RuntimePool {
    fn default() -> Self {
        Self {
            managers: Default::default(),
            lifetime: Lifetime::Application,
            closing: AtomicBool::new(false),
            installation: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl RuntimePool {
    /// Other projects keep commands and transports live while a candidate holds the store transaction.
    pub(super) fn installation_active(&self) -> bool {
        self.installation.load(Ordering::Acquire)
    }

    /// Called under the shared Manager lock; the reservation lives through candidate commit or cancellation.
    pub(super) fn reserve_installation(&self) -> Option<InstallationLease> {
        self.installation
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()?;
        Some(InstallationLease(self.installation.clone()))
    }
    /// A direct background actor is a complete independent host, preserving existing tooling behavior.
    pub(super) fn standalone() -> Self {
        Self {
            lifetime: Lifetime::Actor,
            ..Default::default()
        }
    }

    /// Create the store owner on a worker, or join its existing owner without opening a second data lock.
    pub(super) fn open(
        &self,
        root: PathBuf,
        environment: Environment,
        trusted: bool,
        resources: plugin_runtime::HostResources,
    ) -> anyhow::Result<Arc<Mutex<Manager>>> {
        anyhow::ensure!(
            !self.closing.load(Ordering::Acquire),
            "Plugin runtime is closing"
        );
        std::fs::create_dir_all(&root)?;
        let root = root.canonicalize()?;
        let mut managers = self.managers.lock().unwrap();
        anyhow::ensure!(
            !self.closing.load(Ordering::Acquire),
            "Plugin runtime is closing"
        );
        if let Some(manager) = managers.get(&root) {
            return Ok(manager.clone());
        }
        let manager = Arc::new(Mutex::new(Manager::open_with_resources(
            root.clone(),
            environment,
            trusted,
            resources,
        )?));
        managers.insert(root, manager.clone());
        Ok(manager)
    }

    /// A GUI project's explicit close keeps application declarations and the shared store owner alive.
    pub(super) fn close_project(&self, manager: &mut Manager) -> anyhow::Result<()> {
        match self.lifetime {
            Lifetime::Application => manager.close_selected_workspace_checked(),
            Lifetime::Actor => manager.shutdown_checked(),
        }
    }

    /// Unexpected view disposal still retires its project, without discarding another project's state.
    pub(super) fn abandon_project(&self, manager: &mut Manager) {
        if self.lifetime == Lifetime::Actor {
            manager.shutdown();
        } else if self.close_project(manager).is_err() {
            let workspace = manager.workspace().to_owned();
            let _ = manager.close_workspace(&workspace);
        }
    }

    /// Only application exit retires all store owners; failed cleanup remains available for retry.
    pub(crate) fn shutdown(&self) -> Result<(), String> {
        self.closing.store(true, Ordering::Release);
        let mut failures = Vec::new();
        for manager in self.managers.lock().unwrap().values() {
            let mut manager = manager.lock().unwrap();
            if let Err(error) = manager.shutdown_checked() {
                failures.push(format!("{error:#}"));
            } else {
                crate::plugin_development::report::checkpoint(&manager, true);
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(failures.join("\n"))
        }
    }
}

/// Dropping an unpublished or failed candidate releases the lane without changing any other project.
pub(super) struct InstallationLease(Arc<AtomicBool>);
impl Drop for InstallationLease {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

struct ApplicationRuntimes(Arc<RuntimePool>);
impl gpui_kit::Global for ApplicationRuntimes {}

/// Obtain the same application owner from every native project window, without a process-global test cache.
pub(crate) fn for_application(cx: &mut gpui_kit::App) -> Arc<RuntimePool> {
    if !cx.has_global::<ApplicationRuntimes>() {
        cx.set_global(ApplicationRuntimes(Arc::new(RuntimePool::default())));
    }
    cx.global::<ApplicationRuntimes>().0.clone()
}
