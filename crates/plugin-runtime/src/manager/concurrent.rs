//! Route a shared data owner to live project resources without pausing another project's native work.

use super::*;
use std::sync::{Arc, atomic::AtomicBool};

#[cfg(test)]
mod log_tests;

/// Only inactive routing projections reside here; ownership moves, and no instance or mutable state is copied.
pub(super) struct ProjectResources {
    diagnostic_history: BTreeMap<String, Vec<crate::faults::Diagnostic>>,
    native_diagnostics: crate::faults::NativeDiagnostics,
    language_services: BTreeMap<String, language::Prepared>,
    structure_providers: BTreeMap<String, structure::Prepared>,
    direct_language_providers: BTreeMap<String, direct::Prepared>,
    images: BTreeMap<String, crate::images::Entry>,
    retired_image_sources: BTreeMap<String, api::ContentVersion>,
    pub(super) host_sessions: HostSessions,
    pub(super) native_executions: native_execution::Executions,
    debug_sessions: debug_services::Sessions,
    service_preparations: Vec<service_preparation::Pending>,
    pub(super) alive: Arc<AtomicBool>,
    errors: BTreeMap<String, String>,
}

impl ProjectResources {
    /// Parked native language calls retain their original place in the actor's global waiter pool.
    pub(super) fn language_waiter_count(&self) -> usize {
        self.language_services
            .values()
            .filter_map(|role| role.delegation.as_ref())
            .map(|pending| pending.waiter_count())
            .sum()
    }
    /// Application diagnostics include every live project's native roots and bounded role requests.
    pub(super) fn resource_count(&self) -> usize {
        self.images.len()
            + self.native_executions.resource_count()
            + self
                .language_services
                .values()
                .filter(|role| role.owns_service)
                .filter_map(|role| role.service.as_ref().ok())
                .map(|service| service.process_count() + service.request_resource_count())
                .sum::<usize>()
            + self
                .structure_providers
                .values()
                .filter_map(|role| role.provider.as_ref().ok())
                .filter(|provider| provider.is_active())
                .count()
            + self
                .direct_language_providers
                .values()
                .filter_map(|role| role.provider.as_ref().ok())
                .filter(|provider| provider.is_active())
                .count()
    }
    /// Global package retirement withdraws cached roles in every project, without touching unrelated providers.
    pub(super) fn retire_plugin(&mut self, id: &str) {
        let prefix = format!("{id}/");
        self.language_services
            .retain(|key, _| !key.starts_with(&prefix));
        self.structure_providers
            .retain(|key, _| !key.starts_with(&prefix));
        self.direct_language_providers
            .retain(|key, _| !key.starts_with(&prefix));
        self.images.retain(|key, _| !key.starts_with(&prefix));
        self.errors.remove(id);
    }
    /// Native shutdown may require retry; revoke other resources without dropping that supervisor.
    pub(super) fn clear_after_retirement(&mut self) {
        self.language_services.clear();
        self.structure_providers.clear();
        self.direct_language_providers.clear();
        self.images.clear();
        self.service_preparations.clear();
    }
    /// Allocate one native root for a newly opened project; subsequent routing swaps reuse it.
    pub(super) fn new(manager: &Manager, trusted: bool) -> Self {
        let alive = Arc::new(AtomicBool::new(true));
        Self {
            diagnostic_history: BTreeMap::new(),
            native_diagnostics: crate::faults::NativeDiagnostics::with_logs(
                manager.host_resources.logs.clone(),
            ),
            language_services: BTreeMap::new(),
            structure_providers: BTreeMap::new(),
            direct_language_providers: BTreeMap::new(),
            images: BTreeMap::new(),
            retired_image_sources: BTreeMap::new(),
            host_sessions: HostSessions::new(alive.clone()),
            native_executions: native_execution::Executions::new(trusted),
            debug_sessions: Default::default(),
            service_preparations: Vec::new(),
            alive,
            errors: BTreeMap::new(),
        }
    }

    /// Swap the dispatch projection as one ownership move; transports, queues and revocation flags stay alive.
    pub(super) fn swap(&mut self, manager: &mut Manager) {
        std::mem::swap(
            &mut self.diagnostic_history,
            &mut manager.diagnostic_history,
        );
        std::mem::swap(
            &mut self.native_diagnostics,
            &mut manager.native_diagnostics,
        );
        std::mem::swap(&mut self.language_services, &mut manager.language_services);
        std::mem::swap(
            &mut self.structure_providers,
            &mut manager.structure_providers,
        );
        std::mem::swap(
            &mut self.direct_language_providers,
            &mut manager.direct_language_providers,
        );
        std::mem::swap(&mut self.images, &mut manager.images);
        std::mem::swap(
            &mut self.retired_image_sources,
            &mut manager.retired_image_sources,
        );
        std::mem::swap(&mut self.host_sessions, &mut manager.host_sessions);
        std::mem::swap(&mut self.native_executions, &mut manager.native_executions);
        std::mem::swap(&mut self.debug_sessions, &mut manager.debug_sessions);
        std::mem::swap(
            &mut self.service_preparations,
            &mut manager.service_preparations,
        );
        std::mem::swap(&mut self.alive, &mut manager.host_alive);
        let incoming = std::mem::take(&mut self.errors);
        for (id, entry) in &mut manager.installed {
            // Application failures belong to the shared owner and must remain visible in every project.
            if entry.manifest.scope == api::InstanceScope::Application {
                continue;
            }
            if let Some(error) = entry.error.take() {
                self.errors.insert(id.clone(), error);
            }
            entry.error = incoming.get(id).cloned();
        }
    }

    /// Inactive scope failures remain attached to that scope instead of poisoning the current window.
    pub(super) fn record_error(&mut self, id: &str, error: String) {
        self.errors.insert(id.into(), error);
    }
}

impl Manager {
    /// Route asynchronous work to its captured live project, then restore the caller's projection.
    /// Missing scopes return `None` and are never recreated by a late completion.
    pub(super) fn in_project<T>(
        &mut self,
        scope: &str,
        operation: impl FnOnce(&mut Self) -> T,
    ) -> Option<T> {
        if scope == self.host_scope() && self.workspace_open {
            return Some(operation(self));
        }
        let target = self
            .parked
            .get(scope)
            .filter(|project| project.resources.is_some())?;
        let environment = target.environment.clone();
        let trusted = target.trusted;
        let previous = self
            .workspace_open
            .then(|| (self.environment.clone(), self.trusted));
        let application = self
            .is_application_management()
            .then(|| self.environment.clone());
        self.select_workspace(environment, trusted).ok()?;
        let result = operation(self);
        if let Some((environment, trusted)) = previous {
            // The original scope remains live unless this operation explicitly retired it.
            let _ = self.select_workspace(environment, trusted);
        } else if let Some(environment) = application {
            // Polling a real project's deferred result must not lend its queues or settings to
            // the profile actor that happened to drive this tick.
            self.select_application(environment);
        }
        Some(result)
    }
}

impl Manager {
    /// An actor can wait for an application transaction before bootstrapping a new project projection.
    pub fn workspace_is_open(&self, workspace: &str) -> bool {
        let key = scopes::workspace_key(workspace);
        (self.workspace_open && self.host_scope() == key)
            || self
                .parked
                .get(&key)
                .is_some_and(|project| project.resources.is_some())
    }
    /// Select the project used by the next host operation while every other open project keeps running.
    /// New host-owned environments must name a real directory; `trusted` cannot come from project files.
    /// Registered projections retain their identity after external moves, so routing and retirement do not reopen a path.
    /// New-directory validation and trust revocation errors are returned without retiring another project's resources.
    pub fn select_workspace(
        &mut self,
        mut environment: Environment,
        trusted: bool,
    ) -> anyhow::Result<()> {
        if self.workspace_open && self.environment.workspace == environment.workspace {
            if self.trusted != trusted {
                self.set_workspace_trust(trusted)?;
            }
            return Ok(());
        }
        let next_key = if let Some((key, _)) = self.parked.iter().find(|(_, project)| {
            project.resources.is_some() && project.environment.workspace == environment.workspace
        }) {
            // A live actor already owns this immutable scope. Only a genuinely new open depends on the current pathname.
            key.clone()
        } else {
            let path = Path::new(&environment.workspace).canonicalize()?;
            anyhow::ensure!(path.is_dir(), "Workspace must be a directory");
            environment.workspace = path.display().to_string();
            scopes::workspace_key(&environment.workspace)
        };
        let old_key = self.host_scope();
        if self.workspace_open && old_key == next_key {
            if self.trusted != trusted {
                self.set_workspace_trust(trusted)?;
            }
            return Ok(());
        }
        let restoring = self.parked.contains_key(&next_key);
        let mut next = self
            .parked
            .remove(&next_key)
            .unwrap_or_else(|| ParkedWorkspace {
                environment: environment.clone(),
                trusted,
                live: BTreeMap::new(),
                resources: None,
            });
        let next_trusted = next.trusted;
        let next_live = std::mem::take(&mut next.live);
        // Application instances remain in `live`; only this selected project's instances move.
        let ids = self
            .live
            .keys()
            .filter(|id| self.installed[*id].manifest.scope == api::InstanceScope::Workspace)
            .cloned()
            .collect::<Vec<_>>();
        for id in ids {
            next.live.insert(id.clone(), self.live.remove(&id).unwrap());
        }
        let mut resources = next
            .resources
            .take()
            .unwrap_or_else(|| super::concurrent::ProjectResources::new(self, trusted));
        resources.swap(self);
        next.resources = Some(resources);
        std::mem::swap(&mut next.environment, &mut self.environment);
        std::mem::swap(&mut next.trusted, &mut self.trusted);
        // Delayed workers retain their cloned origin while the manager routes new work here.
        self.host_resources.logs = self
            .host_resources
            .logs
            .for_workspace(&self.environment.workspace);
        self.native_diagnostics
            .set_log_sink(self.host_resources.logs.clone());
        if self.workspace_open {
            self.parked.insert(old_key, next);
        }
        self.live.extend(next_live);
        self.workspace_open = true;
        self.activation.select_scope(&next_key);
        self.host_resources.terminals.select_scope(next_key);
        // Existing projects retain their own latest environment; routing does not restore an actor's startup snapshot.
        if next_trusted != trusted {
            self.set_workspace_trust(trusted)?;
        }
        if !restoring {
            self.start_scope_plugins();
        } else {
            self.prepare_dormant_scope_plugins()?;
        }
        Ok(())
    }

    /// Checkpoint and retire only the currently routed project, leaving application and other project owners alive.
    /// Snapshot errors retain live instances for retry; native cleanup errors retain their same supervisor.
    pub fn close_selected_workspace_checked(&mut self) -> anyhow::Result<()> {
        if !self.workspace_open {
            return Ok(());
        }
        let ids = self
            .live
            .keys()
            .filter(|id| self.installed[*id].manifest.scope == api::InstanceScope::Workspace)
            .cloned()
            .collect::<Vec<_>>();
        let mut failures = Vec::new();
        for id in &ids {
            if let Err(error) = self
                .live
                .get_mut(id)
                .unwrap()
                .snapshot()
                .and_then(|snapshot| self.save_snapshot(id, &snapshot))
            {
                failures.push(format!("{id}: {error:#}"));
            }
        }
        anyhow::ensure!(failures.is_empty(), "{}", failures.join("; "));
        // Checkpoint failure retains authority; after consent enters native retirement, queued effects are sealed.
        self.host_alive
            .store(false, std::sync::atomic::Ordering::Release);
        self.stop_owned_programs();
        self.native_executions.shutdown()?;
        self.host_alive
            .store(false, std::sync::atomic::Ordering::Release);
        self.activation.invalidate_scope(&self.host_scope());
        self.retire_workspace_images();
        self.retire_workspace_image_inputs();
        self.language_services.clear();
        self.clear_structure_providers();
        self.clear_direct_providers();
        self.host_sessions.retire();
        for id in ids {
            self.live.remove(&id).unwrap().stop();
        }
        self.workspace_open = false;
        self.refresh_services();
        Ok(())
    }
}
