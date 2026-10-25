//! Comparison preparations borrow the sole native file session without creating a file-tab view.
use super::*;
use crate::editor::tabs::PreparedSource;
use editor_core::DocumentStore;
use std::time::Duration;

impl EditorApp {
    /// Load/reuse a checked workspace text session without navigation, focus or preview replacement.
    /// Preparations are bounded and revocable; retained comparisons keep their own source lifetime.
    pub(crate) fn open_plugin_document_source(
        &mut self,
        path: &str,
        request: &EditorRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Value, Failure> {
        api::ResourceIdentity::Local { path: path.into() }.validate()?;
        let target = self
            .workspace
            .root()
            .join(path)
            .canonicalize()
            .map_err(|error| Failure::new(ErrorCode::NotFound, error.to_string()))?;
        if !target.starts_with(self.workspace.root()) {
            return Err(Failure::new(
                ErrorCode::PermissionDenied,
                "Document escapes its workspace",
            ));
        }
        if !target.is_file() {
            return Err(Failure::new(
                ErrorCode::UnsupportedOperation,
                "Source is not a text file",
            ));
        }
        if self.history_delete_blocks(&target) {
            return Err(Failure::new(
                ErrorCode::Cancelled,
                "Document is being removed",
            ));
        }
        let existing = self.tabs.iter().position(|tab| tab.path() == target);
        if existing.is_none()
            && self
                .tabs
                .iter()
                .filter(|tab| tab.source_only.is_some())
                .count()
                >= 32
        {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Prepared source quota exceeded",
            ));
        }
        if !request.enter_side_effect() {
            return Err(Failure::new(
                ErrorCode::Cancelled,
                "Source preparation did not execute",
            ));
        }
        let index = if let Some(index) = existing {
            self.refresh_clean_document_source(index, &target, window, cx)?;
            index
        } else {
            let opened = DocumentSession::open(&self.file_store, target.clone())
                .map_err(|error| Failure::new(ErrorCode::OperationFailed, error.to_string()))?;
            let language = self.language_for_path(&target);
            let text = self.create_native_text_tab(opened, window, cx);
            let editor = text.editor.clone();
            self.tabs.push(OpenTab {
                preview: false,
                source_only: Some(PreparedSource {
                    owners: Vec::new(),
                    expires_at: Instant::now(),
                }),
                virtual_document: None,
                path: target,
                file_id: NEXT_FILE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                file_revision: 0,
                opened_at: Instant::now(),
                file_digest: None,
                text: Some(text),
                file_error: None,
            });
            self.sync_watched_documents();
            editor.update(cx, |editor, cx| editor.set_highlighter(language, cx));
            self.tabs.len() - 1
        };
        if let Some(source) = &mut self.tabs[index].source_only {
            if !source
                .owners
                .iter()
                .any(|owner| owner.same_owner(request.authority()))
            {
                source.owners.push(request.authority().clone());
            }
            // A guest has 30s per native request; allow its follow-up comparison one full interval.
            source.expires_at = Instant::now() + Duration::from_secs(60);
        }
        self.plugin_document_info(index, cx)
            .map(Value::DocumentOpened)
    }

    /// Explicit comparison opens must observe current disk bytes even before a watcher publication.
    /// Reuse ordinary reconciliation, but protect dirty text and silent native IME/preedit first.
    fn refresh_clean_document_source(
        &mut self,
        index: usize,
        path: &Path,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), Failure> {
        let text = self.tabs[index].text.as_ref().ok_or_else(|| {
            Failure::new(
                ErrorCode::UnsupportedOperation,
                "Resource has no text session",
            )
        })?;
        let native_digest: [u8; 32] =
            Sha256::digest(text.editor.read(cx).text().to_string().as_bytes()).into();
        if text.session.is_dirty() || native_digest != text.disk_digest {
            return Ok(());
        }
        let read_at = Instant::now();
        let contents = self
            .file_store
            .read_utf8(path)
            .map_err(|error| Failure::new(ErrorCode::OperationFailed, error.to_string()))?;
        let disk_digest: [u8; 32] = Sha256::digest(contents.as_bytes()).into();
        if disk_digest != text.disk_digest {
            self.apply_reconciliation(
                crate::editor::file_watch::Reconciliation {
                    file_events: Vec::new(),
                    snapshot: None,
                    renames: Vec::new(),
                    native: true,
                    documents: vec![(
                        path.to_owned(),
                        Ok(crate::editor::file_watch::DiskContent::Text(contents)),
                        read_at,
                    )],
                },
                window,
                cx,
            );
            if self.tabs[index]
                .text
                .as_ref()
                .is_none_or(|text| text.disk_digest != disk_digest)
            {
                return Err(Failure::new(
                    ErrorCode::StaleRevision,
                    "Source reload is still pending",
                ));
            }
        }
        // Reject an older in-flight watcher read, including the interval before this explicit reload.
        self.tabs[index].opened_at = read_at;
        Ok(())
    }

    /// Release only this issuer's preparation; visible files and retained comparisons are untouched.
    pub(crate) fn release_plugin_document_source(
        &mut self,
        document: &api::DocumentVersion,
        request: &EditorRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Value, Failure> {
        if !request.enter_side_effect() {
            return Err(Failure::new(
                ErrorCode::Cancelled,
                "Source release did not execute",
            ));
        }
        if let Some(index) = (0..self.tabs.len()).find(|index| {
            self.plugin_document_version(*index)
                .is_ok_and(|current| current.id == document.id && current.path == document.path)
        }) {
            if let Some(source) = &mut self.tabs[index].source_only {
                source
                    .owners
                    .retain(|owner| !owner.same_owner(request.authority()));
            }
        }
        self.sync_prepared_document_sources(window, cx);
        Ok(Value::Unit)
    }

    /// Reclaim unheld clean sources, or reveal unsaved edits instead of silently discarding them.
    /// A pending intent gets a bounded grace period; instance retirement cancels that grace.
    pub(crate) fn sync_prepared_document_sources(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let now = Instant::now();
        let retired = self
            .tabs
            .iter()
            .enumerate()
            .filter_map(|(index, tab)| {
                let source = tab.source_only.as_ref()?;
                let pending =
                    source.expires_at > now && source.owners.iter().any(|owner| owner.is_live());
                let held = self
                    .plugin_document_version(index)
                    .is_ok_and(|version| self.comparison_holds_source(&version.id));
                (!pending && !held).then_some(index)
            })
            .collect::<Vec<_>>();
        for index in retired.into_iter().rev() {
            let tab = &self.tabs[index];
            let edited = tab.is_dirty()
                || tab.text.as_ref().is_some_and(|text| {
                    let digest: [u8; 32] =
                        Sha256::digest(text.editor.read(cx).text().to_string().as_bytes()).into();
                    digest != text.disk_digest
                });
            if edited {
                self.tabs[index].source_only = None;
                self.persist_session();
                cx.notify();
            } else {
                self.close_tab(self.tabs[index].path().to_path_buf(), window, cx);
            }
        }
    }
}
