//! Native command intentions freeze the originating document and revoke waits on real transitions.
//! This contains no text, selection editor or undo state; EditorState remains the only mutable source.

use plugin_runtime::plugin_protocol::api;
use rust_i18n::t;
use std::{
    path::PathBuf,
    sync::{
        Arc, Weak,
        atomic::{AtomicBool, Ordering},
    },
};

/// The worker retains this original target while activation runs; returning to an old selection
/// cannot revive a command cancelled by an intervening native change.
#[derive(Clone)]
pub(crate) struct CommandGuard(Arc<GuardState>, Option<plugin_runtime::EditorAuthority>);

struct GuardState {
    document: Option<api::DocumentVersion>,
    selection: Option<api::TextRange>,
    path: Option<PathBuf>,
    directory: bool,
    revision_sensitive: bool,
    scope_sensitive: bool,
    current: AtomicBool,
}

impl CommandGuard {
    /// Narrow a native intent to its publishing instance; queued callbacks cannot reach a replacement.
    pub(crate) fn with_authority(mut self, authority: plugin_runtime::EditorAuthority) -> Self {
        self.1 = Some(authority);
        self
    }
    /// Check cancellation and the captured non-document path immediately before guest invocation.
    /// A descriptive path grants no file capability; the runtime still owns all permission checks.
    pub(crate) fn admit(&self) -> Result<(), api::Failure> {
        let path_current = self.0.document.is_some()
            || self
                .0
                .path
                .as_ref()
                .is_none_or(|path| path.exists() && path.is_dir() == self.0.directory);
        if self.0.current.load(Ordering::Acquire)
            && path_current
            && self.1.as_ref().is_none_or(|authority| authority.is_live())
        {
            Ok(())
        } else {
            Err(api::Failure::new(
                api::ErrorCode::StaleRevision,
                t!("plugins.command_target_changed").to_string(),
            ))
        }
    }
}

/// The original window owns cancellation. Weak registrations do not keep completed work alive.
#[derive(Default)]
pub(crate) struct CommandIntents {
    guards: Vec<Weak<GuardState>>,
}

impl CommandIntents {
    /// Session replacement/path changes also revoke waits even when no text revision changed.
    /// Only immutable native metadata is compared; no replacement session can revive this guard.
    pub(super) fn synchronize(
        &mut self,
        current: &std::collections::BTreeMap<String, api::DocumentInfo>,
    ) {
        self.guards.retain(|weak| {
            let Some(guard) = weak.upgrade() else {
                return false;
            };
            if guard.revision_sensitive
                && let Some(document) = &guard.document
            {
                let matches = current.get(&document.id).is_some_and(|info| {
                    info.document.id == document.id
                        && info.document.path == document.path
                        && (!guard.revision_sensitive || info.document == *document)
                        && guard.selection.is_none_or(|range| info.selection == range)
                });
                if !matches {
                    guard.current.store(false, Ordering::Release);
                }
            }
            true
        });
    }

    /// Native LSP use can include an opened dependency outside SDK workspace read authority.
    /// Only these native lifetimes are inspected; no external document is published to a plugin.
    pub(super) fn synchronize_lifetimes(
        &mut self,
        current: &std::collections::BTreeMap<String, api::DocumentVersion>,
    ) {
        self.guards.retain(|weak| {
            let Some(guard) = weak.upgrade() else {
                return false;
            };
            if !guard.revision_sensitive
                && let Some(document) = &guard.document
            {
                if !current
                    .get(&document.id)
                    .is_some_and(|now| now.path == document.path)
                {
                    guard.current.store(false, Ordering::Release);
                }
            }
            true
        });
    }

    /// Reserve a bounded intention after checking the real native target, before any async demand.
    /// The returned guard is copied to the actor, never populated from a later active document.
    pub(super) fn capture(
        &mut self,
        document: Option<api::DocumentVersion>,
        selection: Option<api::TextRange>,
        path: Option<PathBuf>,
        directory: bool,
    ) -> Result<CommandGuard, api::Failure> {
        self.register(document, selection, path, directory, true, true)
    }

    /// A primary LSP is used by an open session, independently of its caret, revision and focus.
    /// Closing/reopening or renaming that original session still seals its activation waiter.
    fn capture_lifetime(
        &mut self,
        document: api::DocumentVersion,
    ) -> Result<CommandGuard, api::Failure> {
        self.register(Some(document), None, None, false, false, false)
    }

    /// Keep the common bounded registration and window cleanup for both native purpose kinds.
    fn register(
        &mut self,
        document: Option<api::DocumentVersion>,
        selection: Option<api::TextRange>,
        path: Option<PathBuf>,
        directory: bool,
        revision_sensitive: bool,
        scope_sensitive: bool,
    ) -> Result<CommandGuard, api::Failure> {
        self.guards.retain(|guard| guard.strong_count() != 0);
        if self.guards.len() >= 64 {
            return Err(api::Failure::new(
                api::ErrorCode::LimitExceeded,
                t!("plugins.demand_queue_full").to_string(),
            ));
        }
        let state = Arc::new(GuardState {
            document,
            selection,
            path,
            directory,
            revision_sensitive,
            scope_sensitive,
            current: AtomicBool::new(true),
        });
        self.guards.push(Arc::downgrade(&state));
        Ok(CommandGuard(state, None))
    }

    /// Invalidate before the ordered plugin stream is queued, closing the UI→actor race.
    /// Viewport/focus changes do not lend authority from another document or revoke a nonactive tab.
    pub(super) fn observe(&mut self, event: &api::DocumentEventKind) {
        self.guards.retain(|weak| {
            let Some(guard) = weak.upgrade() else {
                return false;
            };
            let changed = match event {
                api::DocumentEventKind::ContentChanged(info) => {
                    guard.revision_sensitive
                        && guard
                            .document
                            .as_ref()
                            .is_some_and(|original| original.id == info.document.id)
                }
                api::DocumentEventKind::Closed(document) => guard
                    .document
                    .as_ref()
                    .is_some_and(|original| original.id == document.id),
                api::DocumentEventKind::SelectionChanged {
                    document,
                    selection,
                } => {
                    guard
                        .document
                        .as_ref()
                        .is_some_and(|original| original.id == document.id)
                        && guard
                            .selection
                            .is_some_and(|original| original != *selection)
                }
                // A→B→A remains cancelled even if all final metadata equals the first visit.
                api::DocumentEventKind::ActiveChanged(_) => guard.scope_sensitive,
                _ => false,
            };
            if changed {
                guard.current.store(false, Ordering::Release);
            }
            true
        });
    }

    /// Trust revocation and window disposal permanently cancel the original round, including ABA.
    pub(crate) fn revoke(&mut self) {
        for guard in self.guards.drain(..).filter_map(|guard| guard.upgrade()) {
            guard.current.store(false, Ordering::Release);
        }
    }
}

impl Drop for CommandIntents {
    fn drop(&mut self) {
        self.revoke();
    }
}

impl crate::EditorApp {
    /// Internal native identity for an actual text session, including external dependency files.
    /// This value is only a waiter guard; plugin_document_version keeps its stricter SDK boundary.
    pub(crate) fn native_language_document_version(
        &self,
        index: usize,
    ) -> Option<api::DocumentVersion> {
        let tab = self.tabs.get(index)?;
        if tab.virtual_document.is_some() {
            return None;
        }
        let text = tab.text.as_ref()?;
        Some(api::DocumentVersion {
            id: format!("{:?}", text.editor.entity_id()),
            path: tab.path().to_string_lossy().into_owned(),
            revision: text.capability_revision,
        })
    }

    /// Capture the actual open native session as the purpose for primary-server preparation.
    pub(crate) fn capture_language_document_guard(
        &mut self,
        document: api::DocumentVersion,
    ) -> Result<CommandGuard, api::Failure> {
        if !(0..self.tabs.len())
            .any(|index| self.native_language_document_version(index).as_ref() == Some(&document))
        {
            return Err(api::Failure::new(
                api::ErrorCode::StaleRevision,
                t!("plugins.document_target_changed").to_string(),
            ));
        }
        self.plugin_command_intents.capture_lifetime(document)
    }

    /// Document-wide readonly purposes freeze a revision without treating cursor movement as edits.
    /// The window still owns revocation; closed/reopened paths cannot borrow an earlier session.
    pub(crate) fn capture_document_intent_guard(
        &mut self,
        document: api::DocumentVersion,
    ) -> Result<CommandGuard, api::Failure> {
        if !(0..self.tabs.len())
            .any(|index| self.plugin_document_version(index).ok().as_ref() == Some(&document))
        {
            return Err(api::Failure::new(
                api::ErrorCode::StaleRevision,
                t!("plugins.document_target_changed").to_string(),
            ));
        }
        self.plugin_command_intents
            .capture(Some(document), None, None, false)
    }
}
