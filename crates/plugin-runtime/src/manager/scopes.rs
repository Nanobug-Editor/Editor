//! Logical workspace ownership shares package metadata while keeping live instances and files apart.
use super::*;

pub(super) struct ParkedWorkspace {
    pub(super) environment: Environment,
    pub(super) trusted: bool,
    pub(super) live: BTreeMap<String, Instance>,
    /// Concurrent routing retains transports and native roots; legacy sequential switching does not.
    pub(super) resources: Option<super::concurrent::ProjectResources>,
}

/// Canonical roots make equivalent host paths select the same persisted scope.
pub(crate) fn workspace_key(workspace: &str) -> String {
    let path = Path::new(workspace)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(workspace));
    format!("{:x}", Sha256::digest(path.to_string_lossy().as_bytes()))
}

/// Offline migrations and admitted live instances share one source-of-truth layout calculation.
fn scoped_files(root: &Path, manifest: &Manifest, workspace: &str) -> PathBuf {
    let root = root.join("data").join(&manifest.id);
    match manifest.scope {
        api::InstanceScope::Application => root.join("application/files"),
        api::InstanceScope::Workspace => root
            .join("workspaces")
            .join(workspace_key(workspace))
            .join("files"),
    }
}

impl Manager {
    /// Publish portable unopened-file observations only to the current trusted workspace.
    /// Events are data and do not grant paths, content reads or access to parked/application instances.
    pub fn workspace_file_event(&mut self, event: plugin_protocol::workspace::FileEvent) {
        if self.trusted && self.workspace_open {
            for instance in self.live.values_mut() {
                instance.workspace_file_event(event.clone());
            }
        }
    }

    /// An overflow rejects partial history explicitly; guests recover through bounded rereads.
    pub fn workspace_events_failed(&mut self, error: api::Failure) {
        for instance in self.live.values_mut() {
            instance.workspace_events_failed(error.clone());
        }
    }
    /// Publish ordered metadata only to the current trusted workspace's explicitly opted-in streams.
    pub fn document_event(&mut self, event: api::DocumentEvent) {
        if self.trusted && self.workspace_open {
            for instance in self.live.values_mut() {
                instance.document_event(event.clone());
            }
        }
    }

    /// Source overflow terminates the rich stream independently of legacy revision subscriptions.
    pub fn document_stream_failed(&mut self, error: api::Failure) {
        for instance in self.live.values_mut() {
            instance.document_stream_failed(error.clone());
        }
    }

    /// Resolve an installation's persisted files without activating a component or granting access.
    /// `root` is the host's plugin store; `workspace` uses the same canonical identity as live instances.
    /// Invalid path-component identities are rejected. The result is metadata, not a guest capability.
    pub fn persisted_data_directory(
        root: &Path,
        manifest: &Manifest,
        workspace: &str,
    ) -> anyhow::Result<PathBuf> {
        anyhow::ensure!(
            !manifest.id.is_empty()
                && !matches!(manifest.id.as_str(), "." | "..")
                && manifest
                    .id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte)),
            "Invalid plugin identity"
        );
        Ok(scoped_files(root, manifest, workspace))
    }
    /// Only the selected trusted workspace can publish document events to its owners.
    pub fn document_changed(&mut self, change: api::DocumentChange) {
        // Legacy SDK subscriptions know only local workspace-relative paths.
        if change.document.path.starts_with("nanobug-virtual://") {
            return;
        }
        self.invalidate_document_images(&change);
        if change.closed {
            self.host_resources
                .contexts
                .source_closed(&self.host_scope(), &change.document.path);
        }
        if self.trusted && self.workspace_open {
            for instance in self.live.values_mut() {
                instance.document_changed(change.clone());
            }
        }
    }
    /// Source overflow is propagated as a terminal error, not a deceptively complete partial stream.
    pub fn document_events_failed(&mut self, error: api::Failure) {
        for instance in self.live.values_mut() {
            instance.document_events_failed(error.clone());
        }
    }
    /// Effective publication is separate from persisted user preferences and cannot grant trust.
    pub fn published_entries(&self) -> Vec<Installed> {
        self.installed
            .values()
            .map(|entry| {
                let mut visible = entry.clone();
                visible.global_enabled = Some(entry.enabled);
                visible.enabled = entry.compatibility_error().is_none()
                    && self.trusted
                    && self.workspace_open
                    && (entry.enabled
                        || (entry.manifest.scope == api::InstanceScope::Workspace
                            && entry.project_enabled_in(&self.environment.workspace)));
                visible
            })
            .collect()
    }
    /// Count resources across logical workspaces and the application owner for shutdown diagnostics.
    pub fn resource_count(&self) -> usize {
        self.images.len()
            + self.native_executions.resource_count()
            + self.structure_resource_count()
            + self.direct_resource_count()
            + self
                .language_services
                .values()
                .filter(|item| item.owns_service)
                .filter_map(|item| item.service.as_ref().ok())
                .map(|service| service.process_count() + service.request_resource_count())
                .sum::<usize>()
            + self
                .live
                .values()
                .map(Instance::resource_count)
                .sum::<usize>()
            + self
                .parked
                .values()
                .filter_map(|scope| scope.resources.as_ref())
                .map(|resources| resources.resource_count())
                .sum::<usize>()
            + self
                .parked
                .values()
                .flat_map(|scope| scope.live.values())
                .map(Instance::resource_count)
                .sum::<usize>()
    }
    /// Private files are isolated by instance scope; old layouts are handled only by the data importer.
    pub(super) fn instance_data_directory(&self, manifest: &Manifest) -> PathBuf {
        self.data_directory_for(manifest, &self.environment)
    }

    pub(super) fn data_directory_for(
        &self,
        manifest: &Manifest,
        environment: &Environment,
    ) -> PathBuf {
        scoped_files(&self.root, manifest, &environment.workspace)
    }

    /// Switch the published workspace without discarding other logical workspace owners.
    pub fn switch_workspace(
        &mut self,
        mut environment: Environment,
        trusted: bool,
    ) -> anyhow::Result<()> {
        let path = Path::new(&environment.workspace).canonicalize()?;
        anyhow::ensure!(path.is_dir(), "Workspace must be a directory");
        environment.workspace = path.display().to_string();
        let old_key = workspace_key(&self.environment.workspace);
        let next_key = workspace_key(&environment.workspace);
        if self.workspace_open && old_key == next_key {
            return self.set_workspace_trust(trusted);
        }
        // Confirmed project changes cannot detach old programs in a parked scope. Drive their
        // normal/force cleanup while their original providers and scope are still dispatchable.
        self.stop_owned_programs();
        self.retire_workspace_images();
        self.retire_workspace_image_inputs();
        let ids = self
            .live
            .keys()
            .filter(|id| self.installed[*id].manifest.scope == api::InstanceScope::Workspace)
            .cloned()
            .collect::<Vec<_>>();
        let mut live = BTreeMap::new();
        for id in ids {
            live.insert(id.clone(), self.live.remove(&id).unwrap());
        }
        if self.workspace_open {
            // Host protocol transports are recreated when this window selects another workspace.
            self.language_services.clear();
            self.clear_structure_providers();
            self.clear_direct_providers();
            self.parked.insert(
                old_key,
                ParkedWorkspace {
                    environment: self.environment.clone(),
                    trusted: self.trusted,
                    live,
                    resources: None,
                },
            );
        }
        self.environment = environment;
        self.activation.select_scope(&next_key);
        self.host_resources.terminals.select_scope(next_key.clone());
        self.trusted = trusted;
        self.workspace_open = true;
        self.native_executions.set_trusted(trusted);
        if let Some(mut parked) = self.parked.remove(&next_key) {
            if trusted && parked.trusted {
                self.live.append(&mut parked.live);
            } else {
                for instance in parked.live.values_mut() {
                    instance.stop();
                }
            }
        }
        self.start_scope_plugins();
        Ok(())
    }

    /// Revocation retires workspace guests immediately; application owners retain their own scope.
    pub fn set_workspace_trust(&mut self, trusted: bool) -> anyhow::Result<()> {
        self.native_executions.set_trusted(trusted);
        // Authority is revoked even when a guest fails to save its final snapshot.
        self.trusted = trusted;
        if !trusted {
            let result = self.close_workspace(&self.environment.workspace.clone());
            self.workspace_open = true;
            result?;
        }
        if trusted {
            self.start_scope_plugins();
        }
        Ok(())
    }

    pub(super) fn start_scope_plugins(&mut self) {
        if !self.trusted || !self.workspace_open {
            return;
        }
        let entries = self
            .installed
            .iter()
            .filter(|(_, entry)| {
                entry.enabled
                    || (entry.manifest.scope == api::InstanceScope::Workspace
                        && entry.project_enabled_in(&self.environment.workspace))
            })
            .map(|(id, entry)| (id.clone(), entry.enabled))
            .collect::<Vec<_>>();
        self.restore_saved_plugins(entries);
        if let Err(error) = self.save_registry() {
            for entry in self.installed.values_mut() {
                entry.error = Some(format!("{error:#}"));
            }
        }
    }

    /// Closing one scope removes only its views and resources, never the application singleton.
    pub fn close_workspace(&mut self, workspace: &str) -> anyhow::Result<()> {
        let key = workspace_key(workspace);
        // A reopened path gets a new static generation; old preparations never borrow its new lease.
        self.activation.invalidate_scope(&key);
        let current = workspace_key(&self.environment.workspace) == key;
        if current {
            self.native_executions.set_trusted(false);
            // Trust revocation skips guest callbacks; ordinary close retains its bounded cleanup.
            if self.trusted {
                self.stop_owned_programs();
            } else {
                // Revocation cannot call guests, but must revoke native resources even for global owners.
                self.retire_workspace_terminals();
            }
            self.retire_workspace_images();
            self.retire_workspace_image_inputs();
            self.language_services.clear();
            self.clear_structure_providers();
            self.clear_direct_providers();
        }
        let (environment, mut live) = if current {
            self.workspace_open = false;
            let ids = self
                .live
                .keys()
                .filter(|id| self.installed[*id].manifest.scope == api::InstanceScope::Workspace)
                .cloned()
                .collect::<Vec<_>>();
            let mut live = BTreeMap::new();
            for id in ids {
                live.insert(id.clone(), self.live.remove(&id).unwrap());
            }
            (self.environment.clone(), live)
        } else if let Some(parked) = self.parked.remove(&key) {
            (parked.environment, parked.live)
        } else {
            return Ok(());
        };
        let mut failures = vec![];
        for (id, instance) in &mut live {
            if current && !self.trusted {
                // Revocation cannot re-enter guest code with its former authority, even to save state.
                instance.stop();
                continue;
            }
            if let Err(error) = instance
                .snapshot()
                .and_then(|snapshot| self.save_scope_snapshot(id, &environment, &snapshot))
            {
                failures.push(format!("{id}: {error:#}"));
            }
            // Cleanup is unconditional even if the guest traps while checkpointing.
            instance.stop();
        }
        anyhow::ensure!(failures.is_empty(), "{}", failures.join("; "));
        Ok(())
    }

    fn save_scope_snapshot(
        &self,
        id: &str,
        environment: &Environment,
        snapshot: &Snapshot,
    ) -> anyhow::Result<()> {
        let manifest = &self.installed[id].manifest;
        anyhow::ensure!(
            snapshot.data.len() <= manifest.storage_limit,
            "Plugin snapshot exceeds declared quota"
        );
        let data = self.data_directory_for(manifest, environment);
        let path = data.parent().unwrap().join("state.json");
        atomic_write(&path, &serde_json::to_vec(snapshot)?)
    }

    /// Global disable/uninstall invalidates every outstanding workspace owner for the package.
    pub(super) fn retire_parked_plugin(&mut self, id: &str) {
        for scope in self.parked.values_mut() {
            if let Some(resources) = &mut scope.resources {
                resources.retire_plugin(id);
            }
        }
        let removed = self
            .parked
            .values_mut()
            .filter_map(|scope| {
                scope
                    .live
                    .remove(id)
                    .map(|instance| (scope.environment.clone(), instance))
            })
            .collect::<Vec<_>>();
        for (environment, mut instance) in removed {
            let _ = instance
                .snapshot()
                .and_then(|snapshot| self.save_scope_snapshot(id, &environment, &snapshot));
            instance.stop();
        }
    }

    /// Parked scopes remain live and checkpointed although only the active scope is rendered.
    pub(super) fn poll_parked(&mut self) {
        for scope in self.parked.values_mut() {
            for instance in scope.live.values_mut() {
                if instance.poll().is_err() {
                    instance.stop();
                }
            }
        }
    }

    /// Shutdown retires application resources as well as all workspace resources.
    pub fn shutdown(&mut self) {
        // Drop cannot report an error, but must retire resources even when checkpointing fails.
        if self.shutdown_checked().is_err() {
            let _ = self.retire_for_shutdown();
        }
    }

    /// Checkpoint private state and release all owned scopes, returning observable close failures.
    /// Checkpoint errors retain live instances for retry; native cleanup errors retain their supervisor.
    pub fn shutdown_checked(&mut self) -> anyhow::Result<()> {
        let mut failures = Vec::new();
        if let Err(error) = self.checkpoint() {
            failures.push(format!("{error:#}"));
        }
        // Parked scopes also own private state. Save before removing them, so failed writes are retryable.
        let parked = self
            .parked
            .iter()
            .flat_map(|(key, scope)| scope.live.keys().map(move |id| (key.clone(), id.clone())))
            .collect::<Vec<_>>();
        for (key, id) in parked {
            let (environment, snapshot) = {
                let scope = self.parked.get_mut(&key).unwrap();
                (
                    scope.environment.clone(),
                    scope.live.get_mut(&id).unwrap().snapshot(),
                )
            };
            if let Err(error) =
                snapshot.and_then(|snapshot| self.save_scope_snapshot(&id, &environment, &snapshot))
            {
                failures.push(format!("{id}: {error:#}"));
            }
        }
        anyhow::ensure!(failures.is_empty(), "{}", failures.join("; "));
        self.retire_for_shutdown()?;
        // A completed application retirement releases its unique private-data owner as well as its processes.
        self._runtime_lock.take();
        Ok(())
    }

    /// Retirement is idempotent; a failed native join can be retried without replaying programs.
    fn retire_for_shutdown(&mut self) -> anyhow::Result<()> {
        // Detached preparation observes this seal before activation and cannot publish after shutdown.
        self.host_alive
            .store(false, std::sync::atomic::Ordering::Release);
        self.application_alive
            .store(false, std::sync::atomic::Ordering::Release);
        self.retire_workspace_images();
        self.retire_workspace_image_inputs();
        // Programs get a shared bounded cleanup window and then forceful tree termination. The UI
        // awaits the actor's shutdown acknowledgement; only afterwards may its window disappear.
        self.stop_owned_programs();
        let mut native_failures = Vec::new();
        if let Err(error) = self.native_executions.shutdown() {
            native_failures.push(format!("{error:#}"));
        }
        // Revocation must invalidate workers retained by a host before its last Arc is released.
        // Declarative services have no Main WASM in `live`; dropping cached preparations
        // retires their native seals, published requests and retained batch receipts now.
        self.language_services.clear();
        self.clear_structure_providers();
        self.clear_direct_providers();
        // Host execution sessions end with the window that owns them; no start is left queued.
        self.host_sessions.retire();
        for instance in self.live.values_mut() {
            instance.stop();
        }
        self.live.clear();
        let scopes = self.parked.keys().cloned().collect::<Vec<_>>();
        for key in scopes {
            if let Some(scope) = self.parked.get_mut(&key) {
                let mut failed = false;
                if let Some(resources) = &mut scope.resources {
                    resources
                        .alive
                        .store(false, std::sync::atomic::Ordering::Release);
                    resources.host_sessions.retire();
                    if let Err(error) = resources.native_executions.shutdown() {
                        native_failures.push(format!("{error:#}"));
                        failed = true;
                    }
                    resources.clear_after_retirement();
                }
                for instance in scope.live.values_mut() {
                    instance.stop();
                }
                scope.live.clear();
                scope.trusted = false;
                // Keep a failed supervisor registered so a subsequent Quit can retry its actual join.
                if !failed {
                    self.parked.remove(&key);
                }
            }
        }
        self.workspace_open = false;
        self.trusted = false;
        anyhow::ensure!(native_failures.is_empty(), "{}", native_failures.join("; "));
        Ok(())
    }
}
