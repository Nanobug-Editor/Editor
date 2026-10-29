//! File identities and optional native text sessions shared by document lifecycle operations.
mod panel;
mod source_cache;
mod view;
pub(crate) use source_cache::SourceCache;

pub(crate) use panel::OpenFilesPanel;

use editor_core::DocumentSession;
use gpui_base::input::{EditorState, TextDecorationCollection};
use gpui_kit::{Entity, Subscription};
use std::{
    path::{Path, PathBuf},
    time::Instant,
};

/// Reopening a path issues a fresh identity, preventing delayed results from targeting its new tab.
pub(crate) static NEXT_FILE_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// A file tab owns its identity independently of native text editing resources.
pub(crate) struct OpenTab {
    /// Only one unedited explorer preview may be replaced; saving and Undo never restore this flag.
    pub(crate) preview: bool,
    /// A comparison-only local source shares this native session without a file-tab presentation.
    pub(crate) source_only: Option<PreparedSource>,
    /// Virtual identities carry revocable ownership and never represent a backing disk file.
    pub(crate) virtual_document: Option<VirtualTab>,
    pub(crate) path: PathBuf,
    pub(crate) file_id: u64,
    pub(crate) file_revision: u64,
    /// Background checks predating this incarnation or its latest explicit disk read cannot reload it.
    pub(crate) opened_at: Instant,
    /// The watcher retains a hash rather than a second mutable copy of image bytes.
    pub(crate) file_digest: Option<[u8; 32]>,
    pub(crate) text: Option<TextTab>,
    /// File failures retain the tab and its retry target, including unavailable viewer authority.
    pub(crate) file_error: Option<String>,
}

/// Pending source requests have bounded lifetimes; retained comparison views separately borrow text.
pub(crate) struct PreparedSource {
    pub(crate) owners: Vec<plugin_runtime::EditorAuthority>,
    pub(crate) expires_at: Instant,
}

/// Explicit navigation keeps a document; explorer browsing reuses the single preview slot.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum FileOpenMode {
    Fixed,
    Preview,
}

impl OpenTab {
    /// All tab presentations and navigation use the same source visibility rule.
    pub(crate) fn is_visible(&self) -> bool {
        self.source_only.is_none()
            && self
                .virtual_document
                .as_ref()
                .is_none_or(|source| !source.hidden)
    }
    /// Native presentation uses the provider's title independently of the opaque resource URI.
    pub(crate) fn title(&self) -> String {
        self.virtual_document
            .as_ref()
            .map(|virtual_tab| virtual_tab.title.clone())
            .unwrap_or_else(|| {
                self.path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            })
    }
    /// File navigation does not require consulting a text editing session.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
    /// Text capability identity belongs to the open document, not reusable native paint state.
    /// Virtual resource URIs include a unique issuer/resource pair; local documents retain their
    /// native entity identity. Binary tabs return None, and callers still check resource liveness.
    pub(crate) fn text_document_id(&self) -> Option<String> {
        let text = self.text.as_ref()?;
        Some(self.virtual_document.as_ref().map_or_else(
            || format!("{:?}", text.editor.entity_id()),
            |source| source.resource.uri(),
        ))
    }
    /// Read-only files never acquire a dirty text revision or a text save obligation.
    pub(crate) fn is_dirty(&self) -> bool {
        self.text
            .as_ref()
            .is_some_and(|text| text.session.is_dirty())
    }
    /// Native entity matching excludes binary tabs rather than lending the previously active editor.
    pub(crate) fn owns_editor(&self, editor: &Entity<EditorState>) -> bool {
        self.text
            .as_ref()
            .is_some_and(|text| text.editor == *editor)
    }
    /// Match callbacks to their original text entity, excluding file-only tabs.
    pub(crate) fn owns_editor_id(&self, id: gpui_kit::EntityId) -> bool {
        self.text
            .as_ref()
            .is_some_and(|text| text.editor.entity_id() == id)
    }
}

/// Provider metadata contains authority and display hints, never a second mutable document value.
pub(crate) struct VirtualTab {
    /// Issuer lifetime scopes resource-free reuse after a hidden preview is replaced.
    pub(crate) owner: plugin_runtime::EditorAuthority,
    /// Hidden readonly sources participate in document APIs without adding standalone chrome.
    pub(crate) hidden: bool,
    pub(crate) resource: std::sync::Arc<plugin_runtime::VirtualDocumentResource>,
    pub(crate) title: String,
    /// Canonical language used by metadata and native highlighting; explicit IDs retain their identity.
    pub(crate) language: String,
    /// Inert original filename for reevaluating automatic recognition after provider changes.
    /// Present only when the caller omitted an explicit language ID.
    pub(crate) language_path: Option<String>,
}

impl Drop for VirtualTab {
    /// Window/workspace teardown seals authority even without an explicit user close operation.
    fn drop(&mut self) {
        self.resource.revoke();
    }
}

/// Native text state retains its session, selection/IME entity and derived diagnostics.
pub(crate) struct TextTab {
    /// Unlike the dirty revision, this also advances on disk reloads and other programmatic changes.
    pub(crate) capability_revision: u64,
    /// Notify-only IME changes revoke capabilities without marking the document dirty or adding Undo.
    pub(super) capability_content: super::native_text::NativeTextIdentity,
    pub(crate) session: DocumentSession,
    pub(crate) editor: Entity<EditorState>,
    /// Hash of the last disk text, avoiding a second full copy of every open document.
    pub(crate) disk_digest: [u8; 32],
    /// Ignore worker reads that began before the latest successful local save.
    pub(crate) last_saved_at: Instant,
    pub(crate) disk_state: DiskState,
    pub(crate) suppress_change: bool,
    pub(crate) overwrite_confirmed: bool,
    /// A separate decoration layer keeps a definition jump visible for two seconds.
    pub(crate) definition_highlight: TextDecorationCollection,
    pub(crate) definition_highlight_generation: u64,
    /// One native adapter shares grammar/fold arbitration and current plugin marker coordinates.
    pub(crate) highlighter: crate::outline::folding::Controller,
    /// Diagnostics retain only derived parser state; EditorState owns the editable text.
    pub(crate) diagnostics: super::diagnostics::DocumentDiagnostics,
    pub(crate) _subscription: Subscription,
    pub(crate) _observer: Subscription,
}

impl TextTab {
    /// Text-specific operations use the session's canonical path after narrowing the file capability.
    pub(crate) fn path(&self) -> &Path {
        self.session.path()
    }
}
impl Drop for TextTab {
    /// Closing the document invalidates markers even when another UI entity retains its editor.
    fn drop(&mut self) {
        self.highlighter.markers().borrow().revoke();
    }
}

/// Open tabs remain present when their backing file changes or disappears.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DiskState {
    Synced,
    Conflict,
    Deleted,
}
