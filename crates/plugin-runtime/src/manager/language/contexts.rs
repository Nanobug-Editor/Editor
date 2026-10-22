//! Document intents map to one provider-owned context lease, independently of language identifiers.
use super::*;
use plugin_protocol::language::{DocumentContext, ProjectContext};
use std::collections::BTreeSet;

/// A context has exactly one cache owner. Its documents borrow that owner rather than duplicating
/// Prepared entries whose Drop would retire a still-shared transport or double-count native resources.
pub(in crate::manager) struct Binding {
    pub(in crate::manager) project: Option<ProjectContext>,
    pub(in crate::manager) documents: BTreeSet<DocumentContext>,
}

/// Provider IDs cannot contain these delimiters; everything after them is opaque cache identity.
pub(super) fn provider_key(key: &str) -> &str {
    key.split(['?', '!']).next().unwrap_or(key)
}

impl Manager {
    /// Clone only the publication evidence owned by this exact provider/document reservation in
    /// the current project. A new reservation gets a new Arc even if its document identity repeats.
    pub(in crate::manager) fn language_failure_publication(
        &self,
        reason: &DemandReason,
    ) -> Option<Arc<AtomicBool>> {
        let DemandReason::LanguageDocument {
            provider, document, ..
        } = reason
        else {
            return None;
        };
        self.document_failure_publication(provider, document)
    }

    /// Cache keys may change after discovery; exact bound intent, rather than key spelling or a
    /// selected UI context, identifies the reservation retained by the original request.
    fn document_failure_publication(
        &self,
        provider: &str,
        document: &DocumentContext,
    ) -> Option<Arc<AtomicBool>> {
        self.language_services.iter().find_map(|(key, prepared)| {
            (provider_key(key) == provider
                && prepared
                    .context
                    .as_ref()
                    .is_some_and(|binding| binding.documents.contains(document)))
            .then(|| prepared.failure_publication.clone())
        })
    }

    /// Snapshot exact admitted document-to-plan bindings for native actor publication. Only opaque
    /// keys and immutable intent metadata leave this cache; enumeration performs no discovery or IO.
    pub fn language_document_bindings(&mut self) -> BTreeMap<(String, DocumentContext), String> {
        let visible = self.language_services();
        let mut bindings = BTreeMap::new();
        for key in visible.keys() {
            if let Some(context) = &self.language_services[key].context {
                for document in &context.documents {
                    bindings.insert(
                        (provider_key(key).to_owned(), document.clone()),
                        key.clone(),
                    );
                }
            }
        }
        bindings
    }
    /// Return the opaque published slot selected for this exact document configuration intent.
    /// None means cold, pending or retired; callers must never reconstruct this key from language names.
    pub fn language_document_service_key(
        &mut self,
        provider: &str,
        document: &DocumentContext,
    ) -> Option<String> {
        let visible = self.language_services();
        visible
            .keys()
            .find(|key| {
                provider_key(key) == provider
                    && self.selected_context_matches(
                        provider,
                        document,
                        &self.language_services[*key],
                    )
                    && self.language_services[*key]
                        .context
                        .as_ref()
                        .is_some_and(|binding| binding.documents.contains(document))
            })
            .cloned()
    }

    /// Borrow the exact approved context plan; preparation errors remain explicit under the same intent.
    /// This read never discovers projects, downloads dependencies, starts processes or switches providers.
    pub fn language_document_service(
        &mut self,
        provider: &str,
        document: &DocumentContext,
    ) -> Option<Result<Arc<LanguageService>, String>> {
        let key = self.language_document_service_key(provider, document)?;
        Some(self.language_services[&key].service.clone())
    }

    /// Read plugin-owned identity after successful discovery. The returned identity grants no authority;
    /// feature consumers must also retain the service lease, original document revision and host intent.
    pub fn language_document_context(
        &mut self,
        provider: &str,
        document: &DocumentContext,
    ) -> Option<ProjectContext> {
        let key = self.language_document_service_key(provider, document)?;
        self.language_services[&key]
            .context
            .as_ref()?
            .project
            .clone()
    }

    /// Release one exact binding and pending waits. Other documents in that context keep its lease;
    /// a delayed release for an older generation cannot detach the current document configuration.
    pub fn release_language_document(&mut self, provider: &str, document: &DocumentContext) {
        self.cancel_language_document_waiters(provider, document);
        for (key, prepared) in &mut self.language_services {
            if provider_key(key) != provider {
                continue;
            }
            let Some(binding) = &mut prepared.context else {
                continue;
            };
            binding.documents.remove(document);
        }
        self.collect_unused_language_contexts();
    }

    /// Release a native source lease in its captured workspace, without selecting or recreating a
    /// closed scope. A late old-window release can never remove another workspace's same relative path.
    pub fn release_language_document_in_workspace(
        &mut self,
        workspace: &str,
        provider: &str,
        document: &DocumentContext,
    ) {
        let scope = scopes::workspace_key(workspace);
        let _ = self.in_project(&scope, |manager| {
            manager.release_language_document(provider, document)
        });
    }

    /// Native owners outlive empty document indexes while an alias, unredeemed binding, readonly tab
    /// or recovery receipt borrows their exact authority. Collection waits for those independent needs.
    pub(in crate::manager) fn collect_unused_language_contexts(&mut self) {
        let borrowed = self
            .language_services
            .values()
            .filter(|prepared| {
                !prepared.owns_service
                    && prepared
                        .context
                        .as_ref()
                        .is_some_and(|binding| !binding.documents.is_empty())
            })
            .filter_map(|prepared| prepared.service.as_ref().ok().cloned())
            .collect::<Vec<_>>();
        self.language_services.retain(|_, prepared| {
            let Some(binding) = &prepared.context else {
                return true;
            };
            if !binding.documents.is_empty() {
                return true;
            }
            prepared.owns_service
                && prepared.service.as_ref().is_ok_and(|service| {
                    borrowed
                        .iter()
                        .any(|borrower| Arc::ptr_eq(borrower, service))
                        || self.host_resources.bindings.retains(service)
                        || service.request_resource_count() > 0
                })
        });
        self.reconcile_language_context_authority();
    }

    /// Sign only the aggregate live document intents of an owned transport, its authorized aliases
    /// and unredeemed binding pins. Updating one borrower never withdraws another borrower's source.
    pub(super) fn reconcile_language_context_authority(&self) {
        let mut services =
            BTreeMap::<usize, (Arc<LanguageService>, BTreeSet<DocumentContext>)>::new();
        for prepared in self
            .language_services
            .values()
            .filter(|prepared| prepared.owns_service)
        {
            if let Ok(service) = &prepared.service {
                services
                    .entry(Arc::as_ptr(service) as usize)
                    .or_insert_with(|| (service.clone(), BTreeSet::new()));
            }
        }
        for prepared in self.language_services.values() {
            let (Ok(service), Some(binding)) = (&prepared.service, &prepared.context) else {
                continue;
            };
            if let Some((_, documents)) = services.get_mut(&(Arc::as_ptr(service) as usize)) {
                documents.extend(binding.documents.iter().cloned());
            }
        }
        for (_, (service, mut documents)) in services {
            documents.extend(self.host_resources.bindings.documents(&service));
            service.reconcile_document_contexts(&documents);
        }
    }

    /// Capture a newer original intent before detached work runs. The reservation also rejects late
    /// publication from an older preparation, even while the new context has no usable native plan.
    pub(in crate::manager) fn admit_language_document_intent(
        &mut self,
        provider: &str,
        document: &DocumentContext,
    ) -> Result<(), api::Failure> {
        if !document.valid() {
            return Err(api::Failure::new(
                api::ErrorCode::InvalidPath,
                "Invalid language document path",
            ));
        }
        if self
            .document_intent_generation(provider, &document.path)
            .is_some_and(|latest| latest > document.generation)
        {
            return Err(api::Failure::new(
                api::ErrorCode::InvalidHandle,
                "Language document configuration was superseded",
            ));
        }
        let entry = &self.installed[provider.split('/').next().unwrap()];
        let values = self
            .effective_settings(&entry.manifest.id)
            .map_err(|error| format!("{error:#}"));
        let fingerprint = self.language_fingerprint(entry, &values);
        if self.language_services.iter().any(|(key, prepared)| {
            provider_key(key) == provider
                && prepared.fingerprint == fingerprint
                && self.selected_context_matches(provider, document, prepared)
                && prepared
                    .context
                    .as_ref()
                    .is_some_and(|binding| binding.documents.contains(document))
        }) {
            return Ok(());
        }
        // Observe declared collection epochs before withdrawing any old binding. A metadata quota
        // failure cannot tear down an otherwise healthy original context.
        let failure_inputs = Some(self.language_failure_inputs(provider)?);
        // Remove only superseded document bindings, preserving unrelated contexts and unchanged plans
        // awaiting hot revalidation. Those hidden resources cannot publish their old configuration.
        for (key, prepared) in &mut self.language_services {
            if provider_key(key) == provider
                && let Some(pending) = &prepared.delegation
            {
                pending.supersede(document);
            }
            if provider_key(key) == provider
                && let Some(binding) = &mut prepared.context
            {
                binding.documents.retain(|old| {
                    old.path != document.path || old.generation > document.generation
                });
            }
        }
        let key = format!("{provider}!{}", serde_json::to_string(document).unwrap());
        self.language_services.insert(
            key,
            Prepared {
                fingerprint,
                service: Err("Language context preparation is pending".into()),
                context: Some(Binding {
                    project: None,
                    documents: BTreeSet::from([document.clone()]),
                }),
                pending: true,
                owns_service: true,
                delegation: None,
                failure_inputs,
                failure_publication: Arc::new(AtomicBool::new(false)),
            },
        );
        self.reconcile_language_context_authority();
        Ok(())
    }

    /// Completion lookup distinguishes a pending reservation from a stale generation and a failed plan.
    pub(in crate::manager) fn document_language_result(
        &self,
        provider: &str,
        document: &DocumentContext,
        fingerprint: &Fingerprint,
    ) -> Option<Result<(), String>> {
        if self
            .document_intent_generation(provider, &document.path)
            .is_some_and(|latest| latest > document.generation)
        {
            return Some(Err("Language document configuration was superseded".into()));
        }
        let broker = self.plugin_services.lock().unwrap();
        self.language_services.iter().find_map(|(key, prepared)| {
            (provider_key(key) == provider
                && !prepared.pending
                && (prepared.service.is_ok()
                    || prepared
                        .failure_inputs
                        .as_ref()
                        .is_none_or(|original| broker.collection_publication_is_current(original)))
                && self.selected_context_matches(provider, document, prepared)
                && prepared
                    .service
                    .as_ref()
                    .map_or(true, |service| service.is_active())
                && prepared.fingerprint == *fingerprint
                && prepared
                    .context
                    .as_ref()
                    .is_some_and(|binding| binding.documents.contains(document)))
            .then(|| prepared.service.as_ref().map(|_| ()).map_err(Clone::clone))
        })
    }

    /// Publish only an admitted live document intent. The context key/root/plugin generation own one
    /// plan; an equal plan reuses the original transport and a changed generation retires its old lease.
    pub(in crate::manager) fn publish_document_language_plan(
        &mut self,
        provider: String,
        document: DocumentContext,
        project: Option<ProjectContext>,
        fingerprint: Fingerprint,
        service: Result<Arc<LanguageService>, String>,
    ) {
        self.publish_document_language_lease(
            provider,
            document,
            project,
            fingerprint,
            service,
            true,
        );
    }

    /// Publish an owned native plan or a separately authorized borrowed alias through one intent gate.
    pub(in crate::manager) fn publish_document_language_lease(
        &mut self,
        provider: String,
        document: DocumentContext,
        project: Option<ProjectContext>,
        fingerprint: Fingerprint,
        service: Result<Arc<LanguageService>, String>,
        owns_service: bool,
    ) {
        if self.document_intent_generation(&provider, &document.path) != Some(document.generation) {
            return;
        }
        let failed = service.is_err();
        let failure_publication = self
            .document_failure_publication(&provider, &document)
            .expect("a published document plan retains its admitted reservation");
        let intent = format!("{provider}!{}", serde_json::to_string(&document).unwrap());
        // Failure publication must keep the reservation's original inputs. Sampling them now could
        // permanently cache a late error against the newer set that already repaired its cause.
        let failure_inputs = service
            .is_err()
            .then(|| {
                self.language_services
                    .get(&intent)
                    .and_then(|prepared| prepared.failure_inputs.clone())
            })
            .flatten();
        let key = match (&project, &service) {
            (Some(project), Ok(service)) => format!(
                "{provider}?{}",
                serde_json::to_string(&(project, &service.root)).unwrap()
            ),
            _ => format!("{provider}!{}", serde_json::to_string(&document).unwrap()),
        };
        let reuse = self.language_services.get(&key).is_some_and(|previous| {
            previous.fingerprint.same_source(&fingerprint)
                && previous.owns_service == owns_service
                && matches!((&previous.service, &service), (Ok(original), Ok(candidate)) if
                    if owns_service { original.is_active() && original.same_plan(candidate) } else { Arc::ptr_eq(original, candidate) })
        });
        self.language_services.retain(|old_key, previous| {
            if old_key == &key || provider_key(old_key) != provider {
                return true;
            }
            let Some(binding) = &mut previous.context else {
                return true;
            };
            // A plugin generation supersedes the whole discovered context, not just the triggering file.
            if let (Some(old), Some(next), Ok(old_service), Ok(next_service)) =
                (&binding.project, &project, &previous.service, &service)
                && old.key == next.key
                && old.generation < next.generation
                && old_service.root == next_service.root
            {
                return false;
            }
            binding.documents.retain(|old| old.path != document.path);
            previous.owns_service || !binding.documents.is_empty()
        });
        if reuse {
            let previous = self.language_services.get_mut(&key).unwrap();
            previous.fingerprint = fingerprint;
            previous
                .context
                .as_mut()
                .unwrap()
                .documents
                .replace(document);
        } else {
            self.language_services.insert(
                key,
                Prepared {
                    fingerprint,
                    service,
                    context: Some(Binding {
                        project,
                        documents: BTreeSet::from([document]),
                    }),
                    pending: false,
                    owns_service,
                    delegation: None,
                    failure_inputs,
                    failure_publication: failure_publication.clone(),
                },
            );
        }
        if failed {
            // Publish before recording this monotonic evidence. Removed/recreated cache entries
            // never grant it to older requests; those retain only their original reservation Arc.
            failure_publication.store(true, Ordering::Release);
        }
        self.collect_unused_language_contexts();
    }

    /// The newest pending or published host intent is the authority for a path within this provider.
    pub(super) fn document_intent_generation(&self, provider: &str, path: &str) -> Option<u64> {
        self.language_services
            .iter()
            .filter(|(key, _)| provider_key(key) == provider)
            .filter_map(|(_, prepared)| prepared.context.as_ref())
            .flat_map(|binding| &binding.documents)
            .filter(|document| document.path == path)
            .map(|document| document.generation)
            .max()
    }
}
