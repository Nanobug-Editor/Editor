//! One configurable native Editor; content provenance never changes document ownership.
use super::{EditorAction, Node};
use crate::api::{DocumentVersion, TextRange};
use serde::{Deserialize, Serialize};

/// Independently negotiated replacement for the three legacy code-editor node shapes.
pub const EDITOR_CAPABILITY_VERSION: semver::Version = semver::Version::new(2, 0, 0);

/// Optional complete line-edge feature. Empty vectors deliberately retain the native rail geometry.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditorActions {
    /// Controls anchored to UTF-8 byte offsets along the source's left edge.
    #[serde(default)]
    pub left: Vec<EditorAction>,
    /// Controls anchored to UTF-8 byte offsets along the source's right edge.
    #[serde(default)]
    pub right: Vec<EditorAction>,
}

/// A host-owned live session or bounded immutable bytes supplied by the publishing plugin.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum EditorSource {
    /// Borrow this exact open session; the host retains text, IME, selection, save and Undo.
    Document { document: DocumentVersion },
    /// Literal bytes have no document/file authority and must use readonly presentation.
    Snapshot { text: String },
}

/// Configures the same local Editor used by ordinary host documents.
/// Requires `ui.code_editor ^2`; document sources also require `editor.layout ^2` and
/// `editor.read`. Enabling line edges requires `editor.actions ^1` and retains their
/// source version as well as the containing UI revision. No option grants edit/save authority.
/// Snapshot limits are 256 KiB per pane, eight panes and 1 MiB of aggregate literal bytes per
/// UI document. At most 128 edge controls and 512 snapshot change ranges are admitted per pane.
/// Ranges are UTF-8 bytes; the host clips folded/offscreen controls using actual source geometry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Editor {
    /// Content provenance; document references must match the layout's exact `Document.source`.
    pub source: EditorSource,
    /// Restrict user input for this mount. False preserves the owner's existing restrictions.
    /// Snapshot sources must set this true; editable plugin-owned literal drafts are unsupported.
    #[serde(default)]
    pub read_only: bool,
    /// Optional left/right rails; Some(empty) preserves width while controls are suspended.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_actions: Option<EditorActions>,
    /// Snapshot-owned changed ranges; never inserted into source text or its Undo history.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changes: Vec<TextRange>,
    /// Optional shared vertical-scroll/alignment group, independent of edge controls.
    /// IDs are 1..64 ASCII letters/digits/./_/-; a document admits four groups with at most three
    /// active panes each. Mapping keeps the finite 2,000-row/4M-cell budget and explicit fallback.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync_group: Option<String>,
}

impl Editor {
    /// Borrow an exact host session with its current editing policy and no optional decoration.
    pub fn document(document: DocumentVersion) -> Self {
        Self {
            source: EditorSource::Document { document },
            read_only: false,
            line_actions: None,
            changes: Vec::new(),
            sync_group: None,
        }
    }

    /// Present immutable literal text with native selection, copying and scrolling.
    pub fn snapshot(text: impl Into<String>) -> Self {
        Self {
            source: EditorSource::Snapshot { text: text.into() },
            read_only: true,
            line_actions: None,
            changes: Vec::new(),
            sync_group: None,
        }
    }

    /// Return borrowed document authority; literals never synthesize a document identity.
    pub fn document_version(&self) -> Option<&DocumentVersion> {
        match &self.source {
            EditorSource::Document { document } => Some(document),
            EditorSource::Snapshot { .. } => None,
        }
    }

    /// Visit every edge subtree, including disabled controls, for shared quotas and identities.
    pub(super) fn visit_actions(&self, visitor: &mut impl FnMut(&Node)) {
        for action in self.left_actions().iter().chain(self.right_actions()) {
            action.content.visit(visitor);
        }
    }

    /// Get configured left controls without enabling the optional line-edge feature.
    pub fn left_actions(&self) -> &[EditorAction] {
        self.line_actions
            .as_ref()
            .map_or(&[], |actions| actions.left.as_slice())
    }

    /// Get configured right controls without enabling the optional line-edge feature.
    pub fn right_actions(&self) -> &[EditorAction] {
        self.line_actions
            .as_ref()
            .map_or(&[], |actions| actions.right.as_slice())
    }
}
