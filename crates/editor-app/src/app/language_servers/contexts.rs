//! Source-specific analysis routing shares unique native adapters across document and plugin demands.
use super::demands::{ContextRecord, ControlSourceLease};
use super::*;
use plugin_runtime::plugin_protocol::language::DocumentContext;
use std::collections::{BTreeMap, BTreeSet};

impl EditorApp {
    /// A completed control may have returned a project token that a later call must describe or use.
    /// Retain its exact source/configuration intent for independently live authorized caller chains;
    /// transient completion deadlines cannot own persistent native state.
    pub(crate) fn retain_control_source_context(
        &mut self,
        provider: &str,
        document: &DocumentContext,
        source: &plugin_runtime::plugin_protocol::api::DocumentVersion,
        authority: &plugin_runtime::EditorAuthority,
        cx: &App,
    ) -> Result<(), plugin_runtime::plugin_protocol::api::Failure> {
        use plugin_runtime::plugin_protocol::api::{ErrorCode, Failure};
        if !authority.is_live() {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Language control caller retired",
            ));
        }
        let key = (provider.to_owned(), document.clone());
        if !self
            .language_server_demands
            .control_sources
            .contains_key(&key)
            && self.language_server_demands.control_sources.len() >= 1024
        {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Too many native control sources",
            ));
        }
        let lease = self.acquire_language_document_lease(provider, document, cx);
        let retained = self
            .language_server_demands
            .control_sources
            .entry(key)
            .or_insert_with(|| ControlSourceLease {
                incarnation: source.id.clone(),
                callers: Vec::new(),
                _lease: lease,
            });
        if retained.incarnation != source.id {
            // Reopening a path cannot lend the previous source's caller chain to a new document.
            retained.incarnation = source.id.clone();
            retained.callers.clear();
        }
        retained
            .callers
            .retain(plugin_runtime::EditorAuthority::is_live);
        if !retained.callers.iter().any(|old| old.same_owner(authority)) {
            if retained.callers.len() >= 128 {
                return Err(Failure::new(
                    ErrorCode::LimitExceeded,
                    "Too many native control caller chains",
                ));
            }
            retained.callers.push(authority.clone());
        }
        Ok(())
    }

    /// Share the exact source demand lifetime across UI bindings and concurrent control requests.
    /// The weak index owns no resource; the final strong guard queues release in its original scope.
    pub(crate) fn acquire_language_document_lease(
        &mut self,
        provider: &str,
        document: &DocumentContext,
        cx: &App,
    ) -> Arc<crate::extensions::LanguageDocumentLease> {
        self.language_server_demands
            .document_leases
            .retain(|_, lease| lease.strong_count() != 0);
        let key = (
            self.workspace.root().display().to_string(),
            provider.to_owned(),
            document.clone(),
        );
        if let Some(lease) = self
            .language_server_demands
            .document_leases
            .get(&key)
            .and_then(std::sync::Weak::upgrade)
        {
            return lease;
        }
        let lease = Arc::new(
            self.extensions
                .read(cx)
                .language_document_lease(provider.into(), document.clone()),
        );
        self.language_server_demands
            .document_leases
            .insert(key, Arc::downgrade(&lease));
        lease
    }
    /// Keep a source-scoped auxiliary binding until that native source closes or its configuration
    /// is replaced. This records routing metadata only and never takes over primary diagnostics.
    pub(crate) fn retain_auxiliary_language_context(
        &mut self,
        provider: &str,
        document: &DocumentContext,
        cx: &App,
    ) {
        let path = self.workspace.root().join(&document.path);
        if self.tabs.iter().any(|tab| tab.path() == path) {
            let lease = self.acquire_language_document_lease(provider, document, cx);
            self.language_server_demands
                .auxiliary
                .insert((provider.into(), document.clone()), (path, lease));
        }
    }

    /// Verify formatting against its independently selected provider and exact source context.
    pub(crate) fn language_formatter_is_current(
        &self,
        path: &Path,
        server: &Arc<language_navigation::LanguageServer>,
        cx: &App,
    ) -> bool {
        let language = self.language_for_path(path);
        let Some(provider) = self
            .providers
            .formatters()
            .get(&language)
            .and_then(Option::as_ref)
            .cloned()
        else {
            return false;
        };
        let panel = self.extensions.read(cx);
        if panel.language_provider_has_contexts(&provider) {
            let Some(token) = panel.availability(provider.split('/').next().unwrap()) else {
                return false;
            };
            let Ok(relative) = path.strip_prefix(self.workspace.root()) else {
                return false;
            };
            let document = DocumentContext {
                path: relative.to_string_lossy().replace('\\', "/"),
                generation: token.configuration_generation(),
            };
            panel
                .language_document_service(&provider, &document)
                .and_then(Result::ok)
                .is_some_and(|plan| server.is_active() && server.uses_service(&plan))
        } else {
            self.language_edits
                .formatters
                .get(&language)
                .is_some_and(|current| Arc::ptr_eq(current, server) && server.is_active())
        }
    }
    /// Language preparation badges retain their owning provider even before a native adapter exists.
    pub(crate) fn language_context_status_owner(&self, key: &str) -> Option<(String, String)> {
        let path = key.strip_prefix("context:")?;
        let record = self
            .language_server_demands
            .contexts
            .values()
            .find(|record| record.document.path == path)?;
        Some((record.availability.plugin().into(), record.provider.clone()))
    }
    /// Resolve the actual source context. A pending source never borrows a different project's
    /// adapter merely because both documents have the same recognized language.
    pub(crate) fn language_server_for_path(
        &self,
        path: &Path,
    ) -> Option<&Arc<language_navigation::LanguageServer>> {
        if let Some(record) = self.language_server_demands.contexts.get(path) {
            return record.server.as_ref().filter(|server| server.is_active());
        }
        self.language_servers.get(&self.language_for_path(path))
    }

    /// Reuse the exact runtime plan's single adapter, including control operations before a native
    /// document attaches. The pointer key is local lifetime metadata and never crosses the protocol.
    pub(crate) fn acquire_language_context_server(
        &mut self,
        plan: Arc<plugin_runtime::LanguageService>,
    ) -> Option<Arc<language_navigation::LanguageServer>> {
        if !plan.is_active() {
            return None;
        }
        let key = Arc::as_ptr(&plan) as usize;
        if let Some(server) = self
            .language_server_demands
            .transports
            .get(&key)
            .filter(|server| server.is_active())
        {
            return Some(server.clone());
        }
        let server = self
            .language_servers
            .values()
            .chain(self.language_edits.formatters.values())
            .find(|server| server.is_active() && server.uses_service(&plan))
            .cloned()
            .or_else(|| language_navigation::LanguageServer::from_service(plan).map(Arc::new))?;
        self.language_server_demands
            .transports
            .insert(key, server.clone());
        Some(server)
    }

    /// Each original open source demands its selected provider's own project discovery. Settings
    /// changes replace only the matching availability; text revisions do not rebuild context intent.
    pub(super) fn sync_language_contexts(
        &mut self,
        selected: &BTreeMap<String, Option<String>>,
        cx: &mut Context<Self>,
    ) -> bool {
        let open_sources = self
            .tabs
            .iter()
            .enumerate()
            .filter_map(|(index, tab)| {
                self.plugin_document_version(index)
                    .ok()
                    .map(|source| (tab.path().to_owned(), source.id))
            })
            .collect::<BTreeMap<_, _>>();
        let trusted = self.session_state.workspace_trusted;
        let root = self.workspace.root();
        let panel = self.extensions.read(cx);
        self.language_server_demands
            .control_sources
            .retain(|(provider, document), retained| {
                retained
                    .callers
                    .retain(plugin_runtime::EditorAuthority::is_live);
                trusted
                    && !retained.callers.is_empty()
                    && open_sources.get(&root.join(&document.path)) == Some(&retained.incarnation)
                    && panel
                        .availability(provider.split('/').next().unwrap())
                        .is_some_and(|token| {
                            token.configuration_generation() == document.generation
                        })
            });
        let released = self
            .language_server_demands
            .auxiliary
            .iter()
            .filter(|((provider, document), (path, _))| {
                !self.session_state.workspace_trusted
                    || !self.tabs.iter().any(|tab| tab.path() == *path)
                    || self
                        .extensions
                        .read(cx)
                        .availability(provider.split('/').next().unwrap())
                        .is_none_or(|token| token.configuration_generation() != document.generation)
            })
            .map(|(binding, _)| binding.clone())
            .collect::<Vec<_>>();
        for (provider, document) in released {
            self.language_server_demands
                .auxiliary
                .remove(&(provider.clone(), document.clone()));
        }
        let mut wanted = BTreeMap::new();
        if self.session_state.workspace_trusted {
            let panel = self.extensions.read(cx);
            for (index, tab) in self.tabs.iter().enumerate() {
                if tab.text.is_none() || tab.virtual_document.is_some() {
                    continue;
                }
                let language = self.language_for_path(tab.path());
                let Some(provider) = selected
                    .get(&language)
                    .and_then(Option::as_ref)
                    .filter(|provider| panel.language_provider_has_contexts(provider))
                else {
                    continue;
                };
                let plugin = provider.split('/').next().unwrap();
                let Some(availability) = panel.availability(plugin) else {
                    continue;
                };
                let Some(source) = self.native_language_document_version(index) else {
                    continue;
                };
                let Ok(relative) = tab.path().strip_prefix(self.workspace.root()) else {
                    continue;
                };
                let document = DocumentContext {
                    path: relative.to_string_lossy().replace('\\', "/"),
                    generation: availability.configuration_generation(),
                };
                if !document.valid() {
                    continue;
                }
                wanted.insert(
                    tab.path().to_path_buf(),
                    (provider.clone(), source, document, availability),
                );
            }
        }
        let removed = self
            .language_server_demands
            .contexts
            .iter()
            .filter(|(path, record)| {
                record
                    .server
                    .as_ref()
                    .is_some_and(|server| !server.is_active())
                    || !wanted.get(*path).is_some_and(
                        |(provider, source, document, availability)| {
                            provider == &record.provider
                                && source.id == record.incarnation
                                && (record.error.is_none()
                                    || source.revision == record.source_revision)
                                && document == &record.document
                                && availability == &record.availability
                        },
                    )
            })
            .map(|(path, _)| path.clone())
            .collect::<Vec<_>>();
        let mut changed = !removed.is_empty();
        for path in removed {
            let record = self.language_server_demands.contexts.remove(&path).unwrap();
            if let Some(server) = record.server {
                Self::close_server_document(server, &path, cx);
            }
        }
        for (path, (provider, source, document, availability)) in wanted {
            if !self.language_server_demands.contexts.contains_key(&path) {
                let lease = self.acquire_language_document_lease(&provider, &document, cx);
                let result = self
                    .capture_language_document_guard(source.clone())
                    .and_then(|guard| {
                        self.extensions.read(cx).demand_language_document(
                            &provider,
                            document.clone(),
                            plugin_runtime::LanguageRole::Lsp,
                            Some(guard),
                        )
                    });
                let (pending, error) = match result {
                    Ok(wait) => (Some(wait), None),
                    Err(error) => (None, Some(error.to_string())),
                };
                self.language_server_demands.contexts.insert(
                    path.clone(),
                    ContextRecord {
                        _lease: lease,
                        provider: provider.clone(),
                        document: document.clone(),
                        incarnation: source.id,
                        source_revision: source.revision,
                        availability,
                        pending,
                        error,
                        server: None,
                    },
                );
                changed = true;
            }
            let record = self
                .language_server_demands
                .contexts
                .get_mut(&path)
                .unwrap();
            if let Some(result) = record
                .pending
                .as_mut()
                .and_then(crate::extensions::DemandWait::try_take)
            {
                record.pending.take();
                record.error = result.err().map(|error| error.to_string());
            }
            let state_key = format!("context:{}", record.document.path);
            if let Some(error) = &record.error {
                self.language_service_states
                    .insert(state_key, ServiceLoadState::Failed(error.clone()));
            } else if record.pending.is_some() || record.server.is_none() {
                self.language_service_states
                    .insert(state_key, ServiceLoadState::Loading);
            }
            if record.pending.is_some()
                || record.error.is_some()
                || record
                    .server
                    .as_ref()
                    .is_some_and(|server| server.is_active())
            {
                continue;
            }
            let Some(plan) = self
                .extensions
                .read(cx)
                .language_document_service(&provider, &document)
            else {
                continue;
            };
            let plan = match plan {
                Ok(plan) => plan,
                Err(error) => {
                    self.language_server_demands
                        .contexts
                        .get_mut(&path)
                        .unwrap()
                        .error = Some(error);
                    continue;
                }
            };
            let Some(server) = self.acquire_language_context_server(plan.clone()) else {
                continue;
            };
            self.language_server_demands
                .contexts
                .get_mut(&path)
                .unwrap()
                .server = Some(server.clone());
            changed = true;
            let key = format!("context:{}", document.path);
            self.language_service_states
                .insert(key.clone(), ServiceLoadState::Loading);
            cx.spawn(async move |this, cx| {
                let active = server.clone();
                let result = cx
                    .background_executor()
                    .scheduler_executor()
                    .spawn_dedicated(move |_| async move { server.prepare_until_ready() })
                    .await;
                let _ =
                    this.update(cx, |app, cx| {
                        if !app.session_state.workspace_trusted
                            || !active.is_active()
                            || !app.language_server_demands.contexts.get(&path).is_some_and(
                                |record| {
                                    record.document == document
                                        && record
                                            .server
                                            .as_ref()
                                            .is_some_and(|server| Arc::ptr_eq(server, &active))
                                },
                            )
                        {
                            return;
                        }
                        app.language_service_states.insert(
                            key,
                            match result {
                                Ok(()) => ServiceLoadState::Ready,
                                Err(error) => ServiceLoadState::Failed(format!("{error:#}")),
                            },
                        );
                        cx.notify();
                    });
            })
            .detach();
        }
        let keys = self
            .language_server_demands
            .contexts
            .values()
            .map(|record| format!("context:{}", record.document.path))
            .collect::<BTreeSet<_>>();
        self.language_service_states
            .retain(|key, _| !key.starts_with("context:") || keys.contains(key));
        self.language_server_demands.transports.retain(|_, server| {
            if server.is_active() && self.session_state.workspace_trusted {
                true
            } else {
                server.retire();
                false
            }
        });
        changed
    }
}
