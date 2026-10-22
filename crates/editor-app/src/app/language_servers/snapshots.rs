//! Capture immutable native project candidates and reject stale complete context result vectors.
use super::*;
use gpui_base::input::Rope;
use gpui_kit::EntityId;
use language_navigation::{ContextCapture, ContextSource};
use plugin_runtime::plugin_protocol::api::{DocumentVersion, ErrorCode, Failure};
use std::collections::BTreeSet;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// This epoch revokes issued result authority without restarting any unrelated native service.
pub(super) struct SnapshotEpoch {
    guard: Arc<AtomicBool>,
    configuration: Option<u64>,
    /// Source epochs contain only lifetime bits; no editor text or second document model is stored.
    sources: std::sync::Mutex<std::collections::BTreeMap<String, Arc<AtomicBool>>>,
}
impl Default for SnapshotEpoch {
    fn default() -> Self {
        Self {
            guard: Arc::new(AtomicBool::new(true)),
            configuration: None,
            sources: Default::default(),
        }
    }
}
impl SnapshotEpoch {
    fn revoke(&mut self) {
        self.guard.store(false, Ordering::Release);
        self.guard = Arc::new(AtomicBool::new(true));
        for (_, guard) in std::mem::take(self.sources.get_mut().unwrap()) {
            guard.store(false, Ordering::Release);
        }
    }
    fn source_guard(&self, path: &str) -> Result<Arc<AtomicBool>, Failure> {
        let mut sources = self.sources.lock().unwrap();
        if !sources.contains_key(path) && sources.len() >= 1024 {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Source context epoch quota exceeded",
            ));
        }
        Ok(sources
            .entry(path.into())
            .or_insert_with(|| Arc::new(AtomicBool::new(true)))
            .clone())
    }
    /// Reopening the same path gets a new bit; restoring its former context cannot revive results.
    pub(super) fn revoke_source(&self, path: &str) {
        if let Some(guard) = self.sources.lock().unwrap().remove(path) {
            guard.store(false, Ordering::Release);
        }
    }
}

impl EditorApp {
    /// Native result preparation follows monotonic configuration/disk and source-selection epochs.
    /// These bits retain no text and do not extend original plugin authority or a document revision.
    pub(crate) fn language_result_conditions(
        &self,
        path: &str,
    ) -> Result<Vec<Arc<AtomicBool>>, Failure> {
        Ok(vec![
            self.language_server_demands.snapshot_epoch.guard.clone(),
            self.language_server_demands
                .snapshot_epoch
                .source_guard(path)?,
        ])
    }
    /// Every observed workspace file event may change dependency membership or configuration. Only
    /// outstanding result guards expire; open contexts and native process leases remain intact.
    pub(crate) fn invalidate_language_context_snapshots(&mut self) {
        self.language_server_demands.snapshot_epoch.revoke();
    }
    /// Context choice and native close revoke one source without invalidating another graph's work.
    pub(crate) fn invalidate_source_language_context(&self, path: &Path) {
        if let Ok(relative) = path.strip_prefix(self.workspace.root()) {
            self.language_server_demands
                .snapshot_epoch
                .revoke_source(&relative.to_string_lossy().replace('\\', "/"));
        }
    }
    /// Provider/configuration publication must revoke preview authority before a new native round.
    pub(super) fn sync_language_snapshot_configuration(&mut self) {
        let generation = self.providers.feature_generation();
        let epoch = &mut self.language_server_demands.snapshot_epoch;
        if epoch.configuration != Some(generation) {
            epoch.revoke();
            epoch.configuration = Some(generation);
        }
    }
    /// Resolve the exact input entity after its current update has drained. Production navigation
    /// uses this deferred entry so reading other dirty snapshots never reenters the active input.
    pub(crate) fn capture_editor_language_context(
        &self,
        server: &language_navigation::LanguageServer,
        editor: EntityId,
        source: &str,
        cx: &App,
    ) -> anyhow::Result<Option<ContextCapture>> {
        let index = self
            .tabs
            .iter()
            .position(|tab| tab.owns_editor_id(editor))
            .ok_or_else(|| anyhow::anyhow!("Language source has closed"))?;
        let text = self.tabs[index]
            .text
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Language source has no text session"))?;
        anyhow::ensure!(
            text.editor.read(cx).text().to_string() == source,
            "Language source has changed"
        );
        if !self.tabs[index].path().starts_with(self.workspace.root()) {
            return Ok(None);
        }
        let target = self.plugin_document_version(index)?;
        self.capture_language_context(server, &target, source, cx)
            .map_err(Into::into)
    }
    /// Retain candidate metadata and immutable Rope handles for one source-bound native request.
    /// Only provider-selected graph members become strings or leave the host. `source` is the
    /// caller's existing target snapshot, avoiding a reentrant read of the active input entity.
    pub(crate) fn capture_language_context(
        &self,
        server: &language_navigation::LanguageServer,
        target: &DocumentVersion,
        source: &str,
        cx: &App,
    ) -> Result<Option<ContextCapture>, Failure> {
        if !server.has_related_documents() {
            return Ok(None);
        }
        if !self.session_state.workspace_trusted {
            return Err(Failure::new(
                ErrorCode::PermissionDenied,
                "Language workspace is restricted",
            ));
        }
        let mut sources = Vec::new();
        for (index, tab) in self.tabs.iter().enumerate() {
            if tab.virtual_document.is_some() || !tab.path().starts_with(self.workspace.root()) {
                continue;
            }
            let Some(text) = &tab.text else {
                continue;
            };
            let document = self.plugin_document_version(index)?;
            let Some(uri) = language_navigation::file_uri(tab.path()) else {
                continue;
            };
            let snapshot = if document.id == target.id {
                if document != *target {
                    return Err(Failure::new(
                        ErrorCode::StaleRevision,
                        "Language context target changed",
                    ));
                }
                Rope::from(source)
            } else {
                text.editor.read(cx).text().clone()
            };
            sources.push(ContextSource {
                document,
                uri,
                language: self.language_for_path(tab.path()),
                text: snapshot,
            });
        }
        if !sources.iter().any(|source| source.document == *target) {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Language context target has closed",
            ));
        }
        Ok(Some(
            ContextCapture::new(
                self.workspace.root().to_owned(),
                self.providers.feature_generation(),
                target.id.clone(),
                sources,
            )
            .with_guard(self.language_server_demands.snapshot_epoch.guard.clone())
            .with_source_guard(
                self.language_server_demands
                    .snapshot_epoch
                    .source_guard(&target.path)?,
            ),
        ))
    }

    /// Presentation and edit selection validate every actual related revision, including close/reopen
    /// identity and project/provider changes; editing an unrelated candidate does not invalidate it.
    pub(crate) fn language_context_is_current(&self, capture: &ContextCapture, _cx: &App) -> bool {
        let candidates = self
            .tabs
            .iter()
            .enumerate()
            .filter(|(_, tab)| {
                tab.virtual_document.is_none()
                    && tab.text.is_some()
                    && tab.path().starts_with(self.workspace.root())
            })
            .filter_map(|(index, _)| {
                self.plugin_document_version(index)
                    .ok()
                    .map(|document| document.id)
            })
            .collect::<BTreeSet<_>>();
        self.session_state.workspace_trusted
            && capture.is_live()
            && capture.has_candidates(&candidates)
            && capture.workspace == self.workspace.root()
            && capture.provider_generation == self.providers.feature_generation()
            && capture.documents().is_some_and(|documents| {
                documents.iter().all(|expected| {
                    self.tabs.iter().enumerate().any(|(index, _)| {
                        self.plugin_document_version(index).ok().as_ref() == Some(expected)
                    })
                })
            })
    }
}
