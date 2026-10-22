//! Project membership synchronizes only explicit related snapshots and seals the whole request vector.
use super::*;
use gpui_base::input::Rope;
use plugin_runtime::plugin_protocol::{
    api::{DocumentVersion, ErrorCode, Failure},
    language_context::{Document, MAX_CANDIDATES, Members, Query},
    language_control::{MAX_CONTROL_BYTES, SourceMetadata},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::OnceLock,
};

/// An immutable EditorState snapshot handle; converting selected members to wire text is deferred.
#[derive(Clone)]
pub(crate) struct ContextSource {
    pub document: DocumentVersion,
    pub uri: Uri,
    pub language: String,
    pub text: Rope,
}

/// A single native request's candidate metadata and immutable source handles. It is never a cache or
/// editable document model; the result seal lists only members actually selected by the provider.
#[derive(Clone)]
pub(crate) struct ContextCapture {
    pub workspace: PathBuf,
    pub provider_generation: u64,
    target: String,
    sources: Arc<BTreeMap<String, ContextSource>>,
    seal: Arc<OnceLock<ContextSeal>>,
    cancellation: RequestCancellation,
    /// Native file/configuration events revoke results and previews without retiring the transport.
    guard: Arc<std::sync::atomic::AtomicBool>,
    /// A display-context reselect revokes only this native target, preserving unrelated projects.
    source_guard: Option<Arc<std::sync::atomic::AtomicBool>>,
    /// Trusted transport metadata is available only after the complete fixed point is synchronized.
    wire_sources: Arc<OnceLock<Vec<SourceMetadata>>>,
}

/// Final participating revisions and wire versions are monotonic even if text later changes back.
struct ContextSeal {
    documents: Vec<DocumentVersion>,
    graph_generation: String,
    wire_versions: BTreeMap<String, i32>,
    service: std::sync::Weak<plugin_runtime::LanguageService>,
    retired: std::sync::Weak<std::sync::atomic::AtomicBool>,
    /// Distinguish replacement pipes even when graph generations and wire versions repeat.
    connection_alive: std::sync::Weak<std::sync::atomic::AtomicBool>,
}
impl ContextCapture {
    /// Capture one workspace/selection intent without copying text or admitting foreign paths.
    pub(crate) fn new(
        workspace: PathBuf,
        provider_generation: u64,
        target: String,
        sources: Vec<ContextSource>,
    ) -> Self {
        Self {
            workspace,
            provider_generation,
            target,
            sources: Arc::new(
                sources
                    .into_iter()
                    .map(|source| (source.document.id.clone(), source))
                    .collect(),
            ),
            seal: Arc::new(OnceLock::new()),
            cancellation: RequestCancellation::default(),
            guard: Arc::new(std::sync::atomic::AtomicBool::new(true)),
            source_guard: None,
            wire_sources: Arc::new(OnceLock::new()),
        }
    }
    /// Reuse an existing native action's cancellation boundary for graph queries and synchronization.
    pub(crate) fn with_cancellation(mut self, cancellation: RequestCancellation) -> Self {
        self.cancellation = cancellation;
        self
    }
    /// Share an app-issued epoch with analysis, result presentation and eventual edit authority.
    pub(crate) fn with_guard(mut self, guard: Arc<std::sync::atomic::AtomicBool>) -> Self {
        self.guard = guard;
        self
    }
    /// The native source's monotonic display epoch also follows its result and preview authority.
    pub(crate) fn with_source_guard(mut self, guard: Arc<std::sync::atomic::AtomicBool>) -> Self {
        self.source_guard = Some(guard);
        self
    }
    /// Wrap only the original native LSP authority; no file grant or service identity is replaced.
    pub(crate) fn fix_origin(&self, server: &LanguageServer) -> plugin_runtime::LanguageFixOrigin {
        let mut guards = vec![self.guard.clone()];
        guards.extend(self.source_guard.iter().cloned());
        // A new pipe can repeat graph and wire versions. Only this completed capture's original
        // connection authorizes pending edits; an unfinished or mismatched capture fails closed.
        let connection = self
            .seal
            .get()
            .filter(|seal| {
                seal.service
                    .upgrade()
                    .is_some_and(|service| Arc::ptr_eq(&service, &server.service))
            })
            .and_then(|seal| seal.connection_alive.upgrade());
        guards.push(
            connection.unwrap_or_else(|| Arc::new(std::sync::atomic::AtomicBool::new(false))),
        );
        plugin_runtime::LanguageFixOrigin::GuardedLsp {
            service: server.service.clone(),
            guards,
            files: Vec::new(),
        }
    }
    /// Read only the actual participating vector after native analysis; unfinished captures cannot
    /// authorize presentation or edits. Callers also check workspace, selection and transport lifetime.
    pub(crate) fn documents(&self) -> Option<&[DocumentVersion]> {
        self.seal.get().map(|seal| seal.documents.as_slice())
    }
    /// Borrow the immutable preimages actually admitted by the graph. Refactors use these exact
    /// snapshots rather than resynchronizing old transport leases or taking a later disk baseline.
    pub(crate) fn selected_sources(&self) -> Option<Vec<&ContextSource>> {
        self.seal.get().map(|seal| {
            seal.documents
                .iter()
                .filter_map(|document| self.sources.get(&document.id))
                .collect()
        })
    }
    /// Native control can pair public document revisions with actual connection-owned wire versions.
    /// This contains metadata only and is never exposed as a source of caller-supplied authority.
    pub(crate) fn wire_sources(&self) -> Option<&[SourceMetadata]> {
        self.wire_sources.get().map(Vec::as_slice)
    }
    /// A native result cannot survive its original transport, native grant, or project owner.
    pub(crate) fn is_live(&self) -> bool {
        self.guard.load(std::sync::atomic::Ordering::Acquire)
            && self
                .source_guard
                .as_ref()
                .is_none_or(|guard| guard.load(std::sync::atomic::Ordering::Acquire))
            && self.seal.get().is_some_and(|seal| {
                !seal.graph_generation.is_empty()
                    && seal.wire_versions.len() == seal.documents.len()
                    && seal
                        .service
                        .upgrade()
                        .is_some_and(|service| service.is_active())
                    && seal
                        .retired
                        .upgrade()
                        .is_some_and(|retired| !retired.load(std::sync::atomic::Ordering::Acquire))
                    && seal
                        .connection_alive
                        .upgrade()
                        .is_some_and(|alive| alive.load(std::sync::atomic::Ordering::Acquire))
            })
    }
    /// Opening or closing candidates can introduce or remove dirty dependencies from the graph.
    /// Content changes remain precise: only actual participating revisions invalidate a result.
    pub(crate) fn has_candidates(&self, ids: &BTreeSet<String>) -> bool {
        self.sources.keys().eq(ids.iter())
    }
    /// File/configuration publication invalidates analysis as a typed stale result, never a fault.
    fn check_epoch(&self) -> anyhow::Result<()> {
        if self.guard.load(std::sync::atomic::Ordering::Acquire)
            && self
                .source_guard
                .as_ref()
                .is_none_or(|guard| guard.load(std::sync::atomic::Ordering::Acquire))
        {
            Ok(())
        } else {
            Err(Failure::new(
                ErrorCode::StaleRevision,
                "Language configuration or files changed",
            )
            .into())
        }
    }
}
impl LanguageServer {
    /// Only an explicitly declared graph method enables related-source discovery for this provider.
    pub(crate) fn has_related_documents(&self) -> bool {
        self.service.provider.related_documents.is_some()
    }

    /// Prepare a fixed point of related snapshots, run one ordinary operation, then revalidate graph
    /// and every native source version. Each capture is single-use; UI delivery checks its full vector.
    pub(crate) fn with_context_snapshot<T>(
        &self,
        capture: &ContextCapture,
        operation: impl FnOnce() -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        if !self.has_related_documents() {
            return operation();
        }
        if !self.service.can_read_workspace() {
            return Err(Failure::new(
                ErrorCode::PermissionDenied,
                "Related source synchronization requires workspace.read",
            )
            .into());
        }
        capture.check_epoch()?;
        ensure!(
            capture.seal.get().is_none(),
            "Context capture has already completed"
        );
        self.prepare_until_ready()?;
        let target = capture
            .sources
            .get(&capture.target)
            .context("Context target is not an open source")?;
        let mut selected = BTreeSet::from([capture.target.clone()]);
        let mut generation = None;
        let mut synchronized = BTreeSet::new();
        // Every successful round admits at least one new member. The finite open-source set bounds
        // discovery even when an unsaved dependency introduces another unsaved import.
        for _ in 0..=capture.sources.len() {
            capture.cancellation.check()?;
            capture.check_epoch()?;
            let mut connection = self
                .connection
                .lock()
                .map_err(|_| anyhow!("Language context connection lock poisoned"))?;
            let current = connection
                .as_mut()
                .context("Language context is unavailable")?;
            for id in selected.difference(&synchronized) {
                let source = &capture.sources[id];
                let document =
                    self.open_document_for_language(source.uri.clone(), &source.language);
                current.bind_document(&document)?;
                current.sync_document(source.uri.clone(), source.text.to_string())?;
            }
            synchronized = selected.clone();
            let queried = self.query_related(current, target, capture);
            self.connection_failed("related documents", &queried, &mut connection);
            let (next_generation, members) = queried?;
            generation = Some(next_generation);
            selected.extend(members);
            if selected == synchronized {
                break;
            }
        }
        ensure!(
            selected == synchronized,
            "Related document graph did not stabilize"
        );
        let graph_generation =
            generation.context("Related document graph omitted its generation")?;
        let (wire_versions, sources, connection_alive) = {
            let connection = self
                .connection
                .lock()
                .map_err(|_| anyhow!("Language context connection lock poisoned"))?;
            let current = connection
                .as_ref()
                .context("Language context is unavailable")?;
            let mut versions = BTreeMap::new();
            let mut sources = Vec::new();
            for id in &selected {
                let source = &capture.sources[id];
                let version = current
                    .diagnostics
                    .document_version(source.uri.as_str())
                    .context("Related source was not synchronized")?;
                versions.insert(source.uri.as_str().to_owned(), version);
                sources.push(SourceMetadata {
                    document: source.document.clone(),
                    uri: current.wire_uri(&source.uri).as_str().to_owned(),
                    version,
                });
            }
            (versions, sources, current.incarnation_alive.clone())
        };
        capture
            .wire_sources
            .set(sources)
            .map_err(|_| anyhow!("Context capture was synchronized concurrently"))?;
        let value = operation()?;
        capture.cancellation.check()?;
        capture.check_epoch()?;
        ensure!(self.is_active(), "Language context was retired");
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| anyhow!("Language context connection lock poisoned"))?;
        let current = connection
            .as_mut()
            .context("Language context is unavailable")?;
        // Another request can observe EOF and reconnect while this operation is outside the lock.
        // A repeated graph/version in that replacement is not evidence for the original result.
        if !Arc::ptr_eq(&connection_alive, &current.incarnation_alive) {
            return Err(Failure::new(
                ErrorCode::StaleRevision,
                "Language connection changed during analysis",
            )
            .into());
        }
        let queried = self.query_related(current, target, capture);
        self.connection_failed("related documents", &queried, &mut connection);
        let (current_generation, _) = queried?;
        let current = connection
            .as_ref()
            .context("Language context is unavailable")?;
        if current_generation != graph_generation
            || wire_versions
                .iter()
                .any(|(uri, version)| current.diagnostics.document_version(uri) != Some(*version))
        {
            return Err(Failure::new(
                ErrorCode::StaleRevision,
                "Language project snapshot changed during analysis",
            )
            .into());
        }
        capture
            .seal
            .set(ContextSeal {
                documents: selected
                    .iter()
                    .map(|id| capture.sources[id].document.clone())
                    .collect(),
                graph_generation,
                wire_versions,
                service: Arc::downgrade(&self.service),
                retired: Arc::downgrade(&self.retired),
                connection_alive: Arc::downgrade(&connection_alive),
            })
            .map_err(|_| anyhow!("Context capture was completed concurrently"))?;
        Ok(value)
    }

    /// Page metadata by both count and encoded bytes. Responses may select only supplied IDs and
    /// must describe one stable project generation across all pages of the same native graph.
    fn query_related(
        &self,
        connection: &mut LanguageServerConnection,
        target: &ContextSource,
        capture: &ContextCapture,
    ) -> anyhow::Result<(String, BTreeSet<String>)> {
        let method = self.service.provider.related_documents.as_ref().unwrap();
        let target = context_document(target);
        let mut pages = Vec::<Query>::new();
        let mut query = Query {
            target: target.clone(),
            candidates: Vec::new(),
        };
        for source in capture.sources.values() {
            let candidate = context_document(source);
            query.candidates.push(candidate);
            if query.candidates.len() > MAX_CANDIDATES
                || serde_json::to_vec(&query)?.len() > MAX_CONTROL_BYTES
            {
                let candidate = query.candidates.pop().unwrap();
                ensure!(
                    !query.candidates.is_empty(),
                    "Context metadata exceeds the bounded control channel"
                );
                pages.push(query);
                query = Query {
                    target: target.clone(),
                    candidates: vec![candidate],
                };
                ensure!(
                    serde_json::to_vec(&query)?.len() <= MAX_CONTROL_BYTES,
                    "Context document metadata exceeds the bounded control channel"
                );
            }
        }
        if !query.candidates.is_empty() {
            pages.push(query);
        }
        let mut generation = None;
        let mut documents = BTreeSet::new();
        for query in pages {
            capture.cancellation.check()?;
            plugin_runtime::plugin_protocol::language_context::related_documents_method()
                .parameters
                .accepts(&serde_json::to_value(&query)?)?;
            let parameters = json!({"arguments":query,"_nanobug":{"native":true}});
            let value =
                connection.request_cancellable(method, parameters, &capture.cancellation)?;
            if serde_json::to_vec(&value)?.len() > MAX_CONTROL_BYTES {
                return Err(Failure::new(
                    ErrorCode::LimitExceeded,
                    "Related graph response exceeds 64 KiB",
                )
                .into());
            }
            let members: Members = serde_json::from_value(value).map_err(|error| {
                Failure::new(
                    ErrorCode::InvalidRequest,
                    format!("Invalid related graph response: {error}"),
                )
            })?;
            if !members.valid_for(&query) {
                return Err(Failure::new(
                    ErrorCode::InvalidRequest,
                    "Related graph selected invalid or foreign document IDs",
                )
                .into());
            }
            if generation
                .as_ref()
                .is_some_and(|previous| previous != &members.generation)
            {
                return Err(Failure::new(
                    ErrorCode::StaleRevision,
                    "Project graph changed between metadata pages",
                )
                .into());
            }
            generation = Some(members.generation);
            documents.extend(members.documents);
        }
        Ok((
            generation.context("Related graph has no target metadata")?,
            documents,
        ))
    }
}

/// Convert an admitted native source to the public metadata-only graph query record.
fn context_document(source: &ContextSource) -> Document {
    Document {
        id: source.document.id.clone(),
        uri: source.uri.as_str().to_owned(),
        revision: source.document.revision,
    }
}
