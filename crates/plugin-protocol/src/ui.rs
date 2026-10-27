//! Portable native UI: plugins own state, while the host owns layout, input and theme.
//! Protocol 7 composes canvases with native controls through independently negotiated capabilities.
//! No GPUI objects cross this interface.
//!
//! `ui.native` 1.2 adds [`Node::textarea_rows`]: a Textarea can grow between bounded native
//! visual-row counts while keeping its original draft, IME, selection, Undo and internal scrolling.
//! Omitted row bounds retain the default policy; fixed pixel heights cannot be combined with them.
//!
//! `ui.canvas` 1.2 adds [`Canvas::scroll_epoch`] and [`CanvasEvent::ScrollInput`].
//! Give each replacement of a canvas's ordered content a fresh nonzero epoch, including a
//! return to previously displayed content. Preserve it through painting, theme and size updates.
//! Only native range deltas/positions may cross an older UI revision with that exact epoch;
//! future revisions, disabled/removed nodes, modals and source-owned actions remain rejected.
//! Ordinary wheels, pointer, key and text events still require the exact displayed UI revision.
//! Apply [`ScrollChange`] in order, clamping against the actual viewport. Hosts may combine
//! consecutive deltas of the same sign and retain the latest consecutive absolute position.
//! `ui.canvas` 1.3 adds [`Canvas::scroll_buffer`]: a virtualized provider declares the complete
//! painted content interval, including overscan. Native frames translate retained artwork while
//! guest replies are in flight; pointer coordinates are mapped back to that drawing's offset.
//! This opt-in excludes grid/IME canvases and visual viewport transforms. See [`ScrollBuffer`].
//! `ui.canvas` 1.4 adds [`crate::Paint::TextBox`]: a clipped single-line text region whose
//! native glyph widths determine Start/Center/End alignment and end ellipsis. It uses the same
//! UTF-8, font and drawing quotas; providers must declare the newer capability when using it.
//! `ui.canvas` 1.5 adds [`Canvas::measure_text`]: native maximum advances of bounded sample groups
//! are returned as exact-scene [`CanvasEvent::TextMeasured`] replies for content-sized layouts.
//!
//! `ui.windows` 1.0 permits a workspace package to declare `Panel.position = "window"`,
//! with the ordinary `ui.panels` grant. Publish its `api::View` before requesting visibility
//! through `EditorOperation::SetPanelVisibility`. Native close hides only that surface;
//! it never terminates native processes, aborts a domain operation or discards a document.
//! Disable/trust loss/project closure revoke the surface through the normal instance lifecycle.
//! `ui.visibility` 1.0 with `ui.panels` consent opts into [`crate::api::Notification::PanelVisibility`].
//! Observe the initial state for every declared panel of a live instance and subsequent changes;
//! aggregate these states to pause optional refresh timers when no native surface is displayed.
//! Dock tab selection, containing dock collapse and platform window visibility all participate.
//! These observations never activate an instance or replace normal completion/cancellation handling.
//! A window may set `editor_layout` and `source` to mount its one exact document Editor reference
//! after negotiating `editor.layout` 2.0, `ui.code_editor` 2.0 and `editor.read`. The source must already be open in
//! this workspace; virtual sources must belong to the publishing instance. The host borrows
//! the existing document/IME/Undo session and suppresses its main-editor mount while visible.
//! Readonly source snapshots use ordinary native presentation nodes and grant no edit authority.
//!
//! `ui.code_editor` 2.0 retains [`Document::code_alignment`]. Opted-in two/three-pane `sync_group`s
//! align logical rows and native soft wraps with zero-byte visual gaps. Original editors retain
//! text, selection, composition and Undo. A scene shares 4M comparison cells, admits at most four
//! groups, and bounds each source to 1 MiB/2001 logical rows (including the empty EOF row).
//! Native measurement has a separate finite height limit. Exceeding any limit displays an explicit
//! independent-layout fallback. Native search/code-action panels pause the whole group until closed.
//! This presentation does not grant editor.read/write or relax exact document-version checks.

use serde::{Deserialize, Serialize};
use std::ops::Range;

mod controls;
pub use controls::*;
mod editor;
pub use editor::{EDITOR_CAPABILITY_VERSION, Editor, EditorActions, EditorSource};
mod styles;
pub use styles::*;
mod canvas;
#[cfg(test)]
mod text_measure_tests;
pub use canvas::*;
mod events;
#[cfg(test)]
mod scroll_tests;
#[cfg(test)]
mod text_box_tests;
#[cfg(test)]
mod textarea_tests;
mod visual_viewport;
pub use visual_viewport::*;
mod tools;
pub use tools::*;
mod menus;
pub(crate) use menus::valid_text;
pub use menus::*;
pub mod incremental;
pub use incremental::Reuse;

#[cfg(test)]
mod images_tests;
#[cfg(test)]
mod layout_tests;
#[cfg(test)]
mod presentation_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tools_tests;
mod validate;
#[cfg(test)]
mod workbench_tests;

pub const VERSION: u32 = 1;

/// Replace a panel's complete view atomically. Revision is echoed in user events.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Document {
    /// Bounded role-scoped native typography and decoration; requires `ui.styles`.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub content_styles: std::collections::BTreeMap<String, ContentStyle>,
    /// Plugin-owned RGB defaults keyed by `role.property`, overridden by user theme tokens.
    /// Requires `ui.content_colors`; native control behavior and general chrome remain host-owned.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub content_colors: std::collections::BTreeMap<String, u32>,
    /// Bottom-bar functions remain guest-owned, independently of this surface's layout contribution.
    /// Requires `ui.tools`; file targets also require the exact `Document.file` context.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ToolButton>,
    /// The selected file provider owns this complete center tree. Requires `editor.layout`.
    /// Document Editor nodes borrow the existing source session; omitting them hides its input surface.
    #[serde(default)]
    pub editor_layout: bool,
    /// File-resource authority echoed by `editor.files`; no text session or mutable bytes implied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<crate::api::FileVersion>,
    /// Preview replies echo their input version; regular panels leave it absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<crate::api::DocumentVersion>,
    pub version: u32,
    pub revision: u64,
    pub root: Node,
    /// Native controls above the source editor, bound to `source` and requiring `editor.toolbar`.
    /// Node identities and quotas share the panel's root/dialog/menu namespace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor_toolbar: Option<Node>,
    /// Claim user-initiated image paste/drop on the source pane through `editor.images`.
    /// This additive declaration requires `source`; ordinary panels and older guests leave it false.
    #[serde(default)]
    pub editor_image_input: bool,
    /// Emit clicked native rich-text links through `ui.links`; false retains inert link defaults.
    #[serde(default)]
    pub link_events: bool,
    /// Readonly CodeBlock language/text requests use the selected plugin WASM highlighter.
    /// Requires ui.code_highlighting, editor.read and the owning preview's exact source version.
    #[serde(default)]
    pub code_highlighting: bool,
    /// Align native code panes sharing a `sync_group` with display-only blank rows.
    /// Requires `ui.code_editor` 2.0; false preserves independent native pane layout.
    /// The host keeps source bytes, selections, IME and Undo in their existing editor sessions.
    /// Layout/algorithm limits are shown explicitly; alignment never pads the source text.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub code_alignment: bool,
    /// Bind one active source-mapped Scroll to native source viewport notifications and locate requests.
    /// Requires editor.viewport, ui.richtext, editor.read and this preview's exact source identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor_viewport: Option<String>,
    /// At most one modal per panel. Removing it closes the modal.
    #[serde(default)]
    pub dialog: Option<Dialog>,
    /// Optional native popup in root-local coordinates; it owns input until dismissed.
    #[serde(default)]
    pub menu: Option<PopupMenu>,
}

impl Document {
    /// Editor content and controls are immutable within one UI revision. This also binds literal
    /// edge actions to their original bytes even though those bytes carry no document authority.
    /// Call `validate` first to bound traversal; comparison borrows content without copying literals.
    pub fn validate_editor_replacement(&self, previous: &Self) -> Result<(), crate::api::Failure> {
        fn collect<'a>(node: &'a Node, result: &mut Vec<(&'a str, &'a Editor)>) {
            match &node.kind {
                Kind::Editor(editor) => {
                    result.push((&node.id, editor));
                    for action in editor.left_actions().iter().chain(editor.right_actions()) {
                        collect(&action.content, result);
                    }
                }
                Kind::Column { children } | Kind::Row { children } => {
                    for child in children {
                        collect(child, result);
                    }
                }
                Kind::Scroll { content } => collect(content, result),
                Kind::Canvas(canvas) => {
                    for overlay in &canvas.overlays {
                        collect(&overlay.child, result);
                    }
                }
                Kind::Tabs { tabs, .. } => {
                    for tab in tabs {
                        collect(&tab.content, result);
                    }
                }
                _ => {}
            }
        }
        fn editors(document: &Document) -> Vec<(&str, &Editor)> {
            let mut result = Vec::new();
            collect(&document.root, &mut result);
            if let Some(dialog) = &document.dialog {
                collect(&dialog.content, &mut result);
            }
            if let Some(toolbar) = &document.editor_toolbar {
                collect(toolbar, &mut result);
            }
            result
        }
        if self.revision <= previous.revision && editors(self) != editors(previous) {
            return Err(crate::api::Failure::new(
                crate::api::ErrorCode::StaleRevision,
                "Editor replacement must advance its UI revision",
            ));
        }
        Ok(())
    }

    /// Minimum public capabilities for all nodes, including hidden tabs, menus and dialogs.
    /// Embedding consumers use the same requirements as ordinary plugin surfaces before rendering.
    pub fn required_capabilities(
        &self,
    ) -> std::collections::BTreeMap<&'static str, semver::Version> {
        let mut required =
            std::collections::BTreeMap::from([("ui.native", semver::Version::new(1, 0, 0))]);
        let mut visit = |node: &Node| {
            if node.drop_target.is_some() || node.drag_source.is_some() {
                required.insert("ui.transfer", semver::Version::new(1, 0, 0));
            }
            if node.tooltip_anchor.is_some() {
                required.insert("ui.canvas", semver::Version::new(1, 9, 0));
            }
            if node.layout.align.is_some() || node.layout.margin != Edges::default() {
                required.insert("ui.styles", semver::Version::new(1, 0, 0));
            }
            if !node.links.is_empty() {
                required.insert("ui.links", semver::Version::new(1, 0, 0));
            }
            if node.source_range.is_some()
                || matches!(node.kind, Kind::RichText { .. } | Kind::CodeBlock { .. })
            {
                required.insert("ui.richtext", semver::Version::new(1, 0, 0));
            }
            match &node.kind {
                Kind::Editor(editor) => {
                    required.insert("ui.code_editor", EDITOR_CAPABILITY_VERSION.clone());
                    if editor.line_actions.is_some() {
                        required.insert("editor.actions", semver::Version::new(1, 0, 0));
                    }
                }
                Kind::Textarea(_) => {
                    let version = if node.textarea_rows.is_some() { 2 } else { 1 };
                    let native = required.get_mut("ui.native").unwrap();
                    *native = native.clone().max(semver::Version::new(1, version, 0));
                }
                Kind::Canvas(canvas) => {
                    if canvas.scene.as_ref().is_some_and(|scene| {
                        scene.targets.iter().any(|target| {
                            target.drop_target.is_some() || target.drag_source.is_some()
                        })
                    }) {
                        required.insert("ui.transfer", semver::Version::new(1, 0, 0));
                    }
                    let version = if canvas.scroll_2d.is_some() {
                        semver::Version::new(1, 10, 0)
                    } else if !canvas.overlays.is_empty() {
                        semver::Version::new(1, 9, 0)
                    } else if let Some(scene) = &canvas.scene {
                        scene.minimum_canvas_version()
                    } else if canvas.column_resize.is_some() {
                        semver::Version::new(1, 6, 0)
                    } else if !canvas.measure_text.is_empty() {
                        semver::Version::new(1, 5, 0)
                    } else if canvas
                        .paint
                        .iter()
                        .any(|paint| matches!(paint, crate::Paint::TextBox { .. }))
                    {
                        semver::Version::new(1, 4, 0)
                    } else if canvas.scroll_buffer.is_some() {
                        semver::Version::new(1, 3, 0)
                    } else if canvas.scroll_epoch.is_some() {
                        semver::Version::new(1, 2, 0)
                    } else if canvas.scroll.is_some() || canvas.font != Default::default() {
                        semver::Version::new(1, 1, 0)
                    } else {
                        semver::Version::new(1, 0, 0)
                    };
                    required
                        .entry("ui.canvas")
                        .and_modify(|actual| {
                            *actual = std::cmp::max(actual.clone(), version.clone())
                        })
                        .or_insert(version);
                    if canvas.grid {
                        required.insert("ui.grid", semver::Version::new(1, 0, 0));
                    }
                }
                Kind::SideTabs(_) => {
                    required.insert("ui.collections", semver::Version::new(1, 0, 0));
                }
                Kind::Tree { .. } => {
                    required.insert("ui.tree", semver::Version::new(1, 0, 0));
                }
                Kind::Image { .. } => {
                    required.insert("ui.images", semver::Version::new(1, 0, 0));
                }
                _ => {}
            }
        };
        self.root.visit(&mut visit);
        if let Some(toolbar) = &self.editor_toolbar {
            toolbar.visit(&mut visit);
        }
        if let Some(dialog) = &self.dialog {
            dialog.content.visit(&mut visit);
        }
        if self.menu.is_some() {
            required.insert("ui.collections", semver::Version::new(1, 0, 0));
        }
        if self.menu.as_ref().is_some_and(|menu| menu.anchor.is_some()) {
            required.insert("ui.canvas", semver::Version::new(1, 9, 0));
        }
        if self.link_events {
            required.insert("ui.links", semver::Version::new(1, 0, 0));
        }
        if !self.content_styles.is_empty() {
            required.insert("ui.styles", semver::Version::new(1, 0, 0));
        }
        if self.editor_toolbar.is_some() {
            required.insert("editor.toolbar", semver::Version::new(1, 0, 0));
        }
        if self.editor_image_input {
            required.insert("editor.images", semver::Version::new(1, 0, 0));
        }
        if self.code_highlighting {
            required.insert("ui.code_highlighting", semver::Version::new(1, 0, 0));
        }
        if self.code_alignment {
            // Install this after visiting nodes: a readonly node's 1.0 floor must never
            // downgrade the document-level alignment requirement.
            required.insert("ui.code_editor", EDITOR_CAPABILITY_VERSION.clone());
        }
        if self.editor_layout {
            required.insert("editor.layout", semver::Version::new(2, 0, 0));
        }
        if self.editor_viewport.is_some() {
            required.insert("editor.viewport", semver::Version::new(1, 0, 0));
        }
        required
    }
    pub fn new(root: Node) -> Self {
        Self {
            content_styles: Default::default(),
            content_colors: Default::default(),
            tools: Vec::new(),
            editor_layout: false,
            file: None,
            source: None,
            version: VERSION,
            revision: 0,
            root,
            editor_toolbar: None,
            editor_image_input: false,
            link_events: false,
            code_highlighting: false,
            code_alignment: false,
            editor_viewport: None,
            dialog: None,
            menu: None,
        }
    }

    pub fn revision(mut self, revision: u64) -> Self {
        self.revision = revision;
        self
    }

    /// Enable source-pane image input for this versioned preview without exposing native pixel bytes.
    pub fn editor_image_input(mut self, enabled: bool) -> Self {
        self.editor_image_input = enabled;
        self
    }

    /// Opt all active `sync_group` code panes into bounded display-row alignment.
    /// The caller must negotiate `ui.code_editor` 2.0; this grants no text-write authority.
    pub fn code_alignment(mut self, enabled: bool) -> Self {
        self.code_alignment = enabled;
        self
    }

    pub fn dialog(mut self, dialog: Dialog) -> Self {
        self.dialog = Some(dialog);
        self
    }

    /// Reject unsupported versions, duplicate identities and unbounded native workloads.
    pub fn validate(&self) -> Result<(), String> {
        validate::document(self)?;
        let mut images = std::collections::BTreeSet::new();
        let mut references = Vec::new();
        let mut visit = |node: &Node| match &node.kind {
            Kind::Image { .. } | Kind::FileImage { .. } => {
                images.insert(node.id.clone());
            }
            Kind::Canvas(canvas) => {
                if let Some(scene) = &canvas.scene {
                    references.extend(scene.image_nodes().into_iter().map(str::to_owned));
                }
            }
            _ => {}
        };
        self.root.visit(&mut visit);
        if let Some(toolbar) = &self.editor_toolbar {
            toolbar.visit(&mut visit);
        }
        if let Some(dialog) = &self.dialog {
            dialog.content.visit(&mut visit);
        }
        if references.iter().any(|id| !images.contains(id)) {
            return Err(
                "Canvas scene image reference must name an Image/FileImage in this document".into(),
            );
        }
        Ok(())
    }

    /// Return the editor reference actually mounted in the active root, excluding disabled/inactive trees.
    /// Validation separately requires exact source ownership and at most one reference in the full tree.
    pub fn active_native_editor(&self) -> Option<&crate::api::DocumentVersion> {
        self.active_editor().and_then(Editor::document_version)
    }

    /// Return the active borrowed-source configuration, preserving the root's single-session rule.
    pub fn active_editor(&self) -> Option<&Editor> {
        fn active(node: &Node) -> Option<&Editor> {
            if node.disabled {
                return None;
            }
            match &node.kind {
                Kind::Editor(editor) if editor.document_version().is_some() => Some(editor),
                Kind::Column { children } | Kind::Row { children } => {
                    children.iter().find_map(active)
                }
                Kind::Scroll { content } => active(content),
                Kind::Canvas(canvas) => canvas
                    .overlays
                    .iter()
                    .filter(|overlay| {
                        canvas
                            .scene
                            .as_ref()
                            .and_then(|scene| overlay.anchor.placement(scene))
                            .is_some_and(|placement| placement.visible)
                    })
                    .find_map(|overlay| active(&overlay.child)),
                Kind::Tabs { tabs, selected } => tabs
                    .iter()
                    .find(|tab| tab.id == *selected)
                    .and_then(|tab| active(&tab.content)),
                _ => None,
            }
        }
        self.editor_layout.then(|| active(&self.root)).flatten()
    }

    /// Find a live root/toolbar target, while dialogs and popups retain exclusive input ownership.
    pub fn active_node(&self, id: &str) -> Option<&Node> {
        if let Some(dialog) = &self.dialog {
            return dialog.content.find(id);
        }
        if self.menu.is_some() {
            return None;
        }
        self.root.find(id).or_else(|| {
            self.editor_toolbar
                .as_ref()
                .and_then(|toolbar| toolbar.find(id))
        })
    }
    /// Line-edge actions retain source authority in addition to the ordinary UI scene revision.
    /// Native hosts must revalidate this exact version immediately before forwarding an event.
    pub fn editor_action_source(&self, id: &str) -> Option<&crate::api::DocumentVersion> {
        fn find<'a>(node: &'a Node, id: &str) -> Option<&'a crate::api::DocumentVersion> {
            match &node.kind {
                Kind::Editor(editor)
                    if editor
                        .left_actions()
                        .iter()
                        .chain(editor.right_actions())
                        .any(|action| action.content.find(id).is_some()) =>
                {
                    editor.document_version()
                }
                Kind::Column { children } | Kind::Row { children } => {
                    children.iter().find_map(|node| find(node, id))
                }
                Kind::Scroll { content } => find(content, id),
                Kind::Canvas(canvas) => canvas
                    .overlays
                    .iter()
                    .find_map(|overlay| find(&overlay.child, id)),
                Kind::Tabs { tabs, selected } => tabs
                    .iter()
                    .find(|tab| tab.id == *selected)
                    .and_then(|tab| find(&tab.content, id)),
                _ => None,
            }
        }
        self.active_node(id).and_then(|_| find(&self.root, id))
    }
}

/// A modal uses the same tree and theme as its owning panel. IDs share one namespace.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Dialog {
    pub id: String,
    pub title: String,
    pub width: f32,
    pub content: Node,
}

impl Dialog {
    pub fn new(id: impl Into<String>, title: impl Into<String>, content: Node) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            width: 480.,
            content,
        }
    }
}

/// Stable IDs preserve focus, IME composition and scroll position across tree replacements.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    /// ui.transfer 1.0: accept inert offers under this node's ordinary identity and lifetime.
    /// Negotiation grants no clipboard or filesystem authority; external files use imported handles.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drop_target: Option<crate::transfer::DropTarget>,
    /// ui.transfer 1.0: a declaration alone cannot start OS transfer or consume clipboard authority.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drag_source: Option<crate::transfer::DragSource>,
    /// Optional reusable visual input/projection; requires negotiated `ui.viewport`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viewport: Option<VisualViewport>,
    /// Explicit read-only link targets for native keyboard focus and linked images/alternative text.
    /// URIs use rendered href spelling, not a host-parsed domain format; requires `ui.links`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<LinkTarget>,
    /// Optional UTF-8 source bytes in the immutable version echoed by `Document.source`.
    /// Mapping does not grant document access or editing authority; it requires `ui.richtext`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_range: Option<SourceRange>,
    /// Localized hover/accessibility text; this counts toward the ordinary UI text budget.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tooltip: Option<String>,
    /// Optional scene rectangle for the existing native tooltip; requires ui.canvas 1.9.
    /// The text remains inert and the target's displayed lifetime controls hover ownership.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tooltip_anchor: Option<CanvasNodeAnchor>,
    /// Geometry-only inline SVG for an icon-only Button, requiring negotiated `ui.icons`.
    /// At most 4 KiB per icon and 64 per document; `Kind::Button.label` remains its accessible name.
    /// The runtime rejects scripts, external resources, text, CSS and XML entities before publication.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub button_icon: Option<String>,
    /// Native visual-row bounds for a Textarea, requiring `ui.native >=1.2`.
    /// Omitted fields keep the host's default; explicit pixel height cannot be combined with rows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub textarea_rows: Option<TextareaRows>,
    /// Resolves `plugins[plugin_id].ui[role]` colors and `typography[role]` fonts.
    /// If empty, the host uses the node kind (e.g. `button`) as the role.
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub layout: Layout,
    pub kind: Kind,
}

/// One bounded native activation target. The guest supplies its visible caption and destination;
/// `Document.link_events` enables events, while navigation remains a separately authorized request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinkTarget {
    /// Exact rendered href, at most 4096 bytes; no controls or implicit URL effect.
    pub uri: String,
    /// Author/user-localized accessible caption, at most 256 UTF-8 bytes; empty uses host locale.
    pub label: String,
}

/// Half-open UTF-8 byte offsets for one rendered block in its source document version.
/// `start <= end <= 1 MiB`; the host must check actual source length and character boundaries
/// before using the range. Blocks retain their identity through the enclosing node ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRange {
    /// Inclusive byte offset in the current immutable source text.
    pub start: usize,
    /// Exclusive byte offset in the same source text.
    pub end: usize,
}

/// Layout contains geometry only; role presentation and the user theme resolve color and typography.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    /// Optional horizontal content alignment, requiring `ui.styles`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<Alignment>,
    /// Nonnegative physical-pixel margins, requiring `ui.styles` when nonzero.
    #[serde(default, skip_serializing_if = "Edges::is_zero")]
    pub margin: Edges,
    /// Native adjustable panes on a Row/Column, requiring `ui.native >=1.1`; cannot combine with wrap.
    #[serde(default)]
    pub resizable: bool,
    pub width: Option<f32>,
    pub height: Option<f32>,
    #[serde(default)]
    pub grow: bool,
    /// Rows may wrap children into additional lines; the host derives height from their content.
    #[serde(default)]
    pub wrap: bool,
    #[serde(default)]
    pub padding: f32,
    #[serde(default)]
    pub gap: f32,
}

/// A small native control group anchored to the existing source's UTF-8 offset.
/// The host clips it with the source viewport and never inserts rows into the source text.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EditorAction {
    pub offset: usize,
    pub content: Box<Node>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Kind {
    /// The configurable local Editor: exact document or readonly snapshot plus optional features.
    Editor(Editor),
    /// Native lazy tree backed by an instance-owned `ui.tree` resource; the handle is not a file grant.
    Tree {
        /// Created through the formal tree interface and revoked by CloseResource or owner retirement.
        handle: crate::api::ResourceHandle,
    },
    /// Display this context's file using controlled decoding. Requires `ui.file_images` and
    /// `workspace.read`; the guest chooses sizing, while the host performs native layout/painting.
    FileImage {
        alt: String,
        sizing: ImageSizing,
    },
    /// A keyed, reorderable native item list can be placed anywhere in the ordinary layout tree.
    SideTabs(SideTabs),
    /// Any position in the same layout tree can hold a drawing surface; it has no implicit grid.
    Canvas(Canvas),
    Column {
        children: Vec<Node>,
    },
    Row {
        children: Vec<Node>,
    },
    Scroll {
        content: Box<Node>,
    },
    Text {
        text: String,
    },
    /// Read-only native rich markup, requiring `ui.richtext` in addition to `ui.native`.
    /// The host renders a restricted HTML subset and disables implicit URL/image access;
    /// this is neither a WebView nor a request to parse Markdown in the host.
    RichText {
        html: String,
    },
    /// Read-only literal code with preserved whitespace and a monospace presentation.
    /// The optional language is an informational ID, not authority to start a language tool.
    CodeBlock {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        language: Option<String>,
    },
    /// Source-bound native image, requiring `ui.images`; loading is a controlled host resource task.
    /// Document-relative paths require `workspace.read`, HTTP(S) requires `network.images`.
    /// The host limits reads to 8 MiB and 30 seconds and never follows redirects or uses credentials.
    /// Failures affect this image only; markup image tags remain unable to perform ambient reads.
    /// Without a visual viewport, Layout width/height size the bitmap area.
    /// One omitted dimension preserves the intrinsic ratio; both dimensions choose the displayed ratio.
    /// The image fits a narrower container proportionally; omitted sizes do not enlarge intrinsic pixels.
    Image {
        /// Relative percent-encoded URI or credential-free HTTP(S) URL, at most 4096 UTF-8 bytes.
        source: String,
        /// Accessible image name and loading/failure fallback; never a caption on a loaded bitmap.
        alt: String,
    },
    Button {
        label: String,
    },
    Input(Input),
    /// ui.native 1.1: ordinary multiline text using the same input revision/event contract.
    /// Newlines remain literal; the host performs no command or script parsing.
    Textarea(Input),
    Checkbox {
        label: String,
        checked: bool,
    },
    /// A native radio group, selected by stable option ID rather than array index.
    Choice {
        options: Vec<OptionItem>,
        selected: Option<String>,
    },
    Tabs {
        tabs: Vec<Tab>,
        selected: String,
    },
    /// Read-only text rows. Use ordinary buttons in a column for action lists.
    List {
        items: Vec<String>,
    },
    /// Read-only table. Each row must have exactly as many cells as headers.
    Table {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    Separator,
    /// Determinate percentage, from zero to one hundred.
    Progress {
        label: String,
        value: f32,
    },
    Spacer,
}

/// Generic image geometry policy chosen by a plugin; this has no file-format business rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageSizing {
    /// Preserve intrinsic dimensions when they fit; otherwise contain both axes without enlarging.
    OriginalContain,
    /// Contain the available area, permitting an explicit enlargement of small images.
    Contain,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Input {
    pub value: String,
    /// Increment to replace the native draft intentionally (clear/reset/load).
    /// Ordinary Change replies leave this unchanged, preventing stale echoes from erasing typing.
    #[serde(default)]
    pub value_revision: u64,
    #[serde(default)]
    pub placeholder: String,
}

/// Auto-grow bounds measured in native visual lines, including soft wrapping at the current width.
/// Border and padding surround these rows; content exceeding `max` scrolls inside the same editor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextareaRows {
    /// Minimum visible lines; validation requires `1 <= min <= max`.
    pub min: u16,
    /// Maximum visible lines; validation caps this at 64.
    pub max: u16,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OptionItem {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub disabled: bool,
}

impl OptionItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            disabled: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tab {
    pub id: String,
    pub label: String,
    pub content: Node,
}

impl Tab {
    pub fn new(id: impl Into<String>, label: impl Into<String>, content: Node) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            content,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UiEvent {
    pub revision: u64,
    pub node: String,
    pub action: Action,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum Action {
    /// Owned lazy tree callbacks use both the scene revision and the tree request epoch.
    Tree(crate::tree::Event),
    /// Input on a visual viewport; guests own zoom, pan, limits and automatic sizing decisions.
    ViewportInput(ViewportInput),
    /// Coalesced live geometry of the bound preview scroll; it never mutates its derived block.
    Viewport(crate::api::PreviewViewport),
    /// URI supplied by a clicked native rich-text link, never by parsing or layout alone.
    Link {
        uri: String,
    },
    Canvas(CanvasEvent),
    Click,
    Change(String),
    Submit(String),
    Toggle(bool),
    Select(String),
    Close(String),
    Rename {
        id: String,
        value: String,
    },
    Move {
        from: String,
        to: String,
    },
    Resize(f32),
    /// Pointer coordinates relative to the side tab list's top-left corner.
    Context {
        id: String,
        x: f32,
        y: f32,
    },
    /// Escape, backdrop or close button. The plugin should remove Document.dialog.
    Dismiss,
}

impl Node {
    pub fn new(id: impl Into<String>, kind: Kind) -> Self {
        Self {
            id: id.into(),
            viewport: None,
            links: Vec::new(),
            source_range: None,
            tooltip: None,
            drop_target: None,
            drag_source: None,
            tooltip_anchor: None,
            button_icon: None,
            textarea_rows: None,
            role: String::new(),
            disabled: false,
            layout: Layout::default(),
            kind,
        }
    }
    pub fn column(id: impl Into<String>, children: Vec<Node>) -> Self {
        Self::new(id, Kind::Column { children })
    }
    pub fn row(id: impl Into<String>, children: Vec<Node>) -> Self {
        Self::new(id, Kind::Row { children })
    }
    pub fn scroll(id: impl Into<String>, content: Node) -> Self {
        Self::new(
            id,
            Kind::Scroll {
                content: Box::new(content),
            },
        )
    }
    pub fn text(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self::new(id, Kind::Text { text: text.into() })
    }
    /// Create a read-only rich block with stable `id` and restricted `html` markup.
    /// The returned node needs `ui.richtext`; text quotas are checked by `Document::validate`.
    pub fn rich_text(id: impl Into<String>, html: impl Into<String>) -> Self {
        Self::new(id, Kind::RichText { html: html.into() })
    }
    /// Create a literal code block from `text` and an optional ASCII language ID.
    /// Returns a node requiring `ui.richtext`; the language does not imply highlighting support.
    pub fn code_block(
        id: impl Into<String>,
        text: impl Into<String>,
        language: Option<String>,
    ) -> Self {
        Self::new(
            id,
            Kind::CodeBlock {
                text: text.into(),
                language,
            },
        )
    }
    /// Declare a native image with stable `id`, resource `source` and localized alternative text.
    /// `Document.source` and negotiated `ui.images` are mandatory. Loading never grants WASM bytes
    /// or file/network authority; unavailable grants become a failure for this node only.
    pub fn image(id: impl Into<String>, source: impl Into<String>, alt: impl Into<String>) -> Self {
        Self::new(
            id,
            Kind::Image {
                source: source.into(),
                alt: alt.into(),
            },
        )
    }
    /// Attach half-open UTF-8 `range` offsets into the version in `Document.source`.
    /// Returns the mapped node; invalid bounds are rejected by `Document::validate`.
    /// Even ordinary text or layout nodes need `ui.richtext` when carrying this metadata.
    pub fn source_range(mut self, range: Range<usize>) -> Self {
        self.source_range = Some(SourceRange {
            start: range.start,
            end: range.end,
        });
        self
    }

    /// Attach localized hover/accessibility text; `Document::validate` enforces shared text quotas.
    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    /// Let a row wrap its children instead of clipping a toolbar at a narrow editor width.
    pub fn wrap(mut self) -> Self {
        self.layout.wrap = true;
        self
    }
    pub fn button(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::new(
            id,
            Kind::Button {
                label: label.into(),
            },
        )
    }
    /// Replace a button's visible label with themed geometric `svg`, retaining native focus and Click.
    /// Returns this node; validation rejects other kinds, empty labels, excessive bytes or unsafe SVG.
    /// Guests must declare `ui.icons ^1`; this grants no file or network access.
    pub fn icon(mut self, svg: impl Into<String>) -> Self {
        self.button_icon = Some(svg.into());
        self
    }
    pub fn input(id: impl Into<String>, input: Input) -> Self {
        Self::new(id, Kind::Input(input))
    }
    /// Create a multiline field requiring ui.native 1.1. Stable value_revision retains native
    /// selection, composition and undo across acknowledgements; bump it for explicit resets.
    pub fn textarea(id: impl Into<String>, input: Input) -> Self {
        Self::new(id, Kind::Textarea(input))
    }

    /// Auto-grow a Textarea between `min` and `max` native visual lines (`1..=64`).
    /// The returned node requires `ui.native 1.2`; invalid ranges or fixed heights fail validation.
    pub fn textarea_rows(mut self, min: u16, max: u16) -> Self {
        self.textarea_rows = Some(TextareaRows { min, max });
        self
    }
    pub fn checkbox(id: impl Into<String>, label: impl Into<String>, checked: bool) -> Self {
        Self::new(
            id,
            Kind::Checkbox {
                label: label.into(),
                checked,
            },
        )
    }
    pub fn role(mut self, role: impl Into<String>) -> Self {
        self.role = role.into();
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    pub fn gap(mut self, gap: f32) -> Self {
        self.layout.gap = gap;
        self
    }
    pub fn padding(mut self, padding: f32) -> Self {
        self.layout.padding = padding;
        self
    }
    pub fn width(mut self, width: f32) -> Self {
        self.layout.width = Some(width);
        self
    }
    pub fn height(mut self, height: f32) -> Self {
        self.layout.height = Some(height);
        self
    }
    /// Request native pane handles for this Row/Column without handing scroll or document state to the guest.
    pub fn resizable(mut self) -> Self {
        self.layout.resizable = true;
        self
    }

    pub fn grow(mut self) -> Self {
        self.layout.grow = true;
        self
    }

    pub fn theme_role(&self) -> &str {
        if !self.role.is_empty() {
            return &self.role;
        }
        match &self.kind {
            Kind::Tree { .. } => "tree",
            Kind::Editor(_) => "editor",
            Kind::SideTabs(_) => "tab_bar",
            Kind::Canvas(_) => "canvas",
            Kind::Column { .. } | Kind::Row { .. } => "container",
            Kind::Scroll { .. } => "scroll",
            Kind::Text { .. } => "text",
            Kind::RichText { .. } => "rich_text",
            Kind::CodeBlock { .. } => "code_block",
            Kind::Image { .. } | Kind::FileImage { .. } => "image",
            Kind::Button { .. } => "button",
            Kind::Input(_) | Kind::Textarea(_) => "input",
            Kind::Checkbox { .. } => "checkbox",
            Kind::Choice { .. } => "choice",
            Kind::Tabs { .. } => "tabs",
            Kind::List { .. } => "list",
            Kind::Table { .. } => "table",
            Kind::Separator => "separator",
            Kind::Progress { .. } => "progress",
            Kind::Spacer => "spacer",
        }
    }

    /// Walk all nodes, including inactive tabs, so identities can be validated together.
    pub fn visit(&self, visitor: &mut impl FnMut(&Node)) {
        visitor(self);
        match &self.kind {
            Kind::Editor(editor) => editor.visit_actions(visitor),
            Kind::Column { children } | Kind::Row { children } => {
                children.iter().for_each(|n| n.visit(visitor))
            }
            Kind::Scroll { content } => content.visit(visitor),
            Kind::Canvas(canvas) => canvas
                .overlays
                .iter()
                .for_each(|overlay| overlay.child.visit(visitor)),
            Kind::Tabs { tabs, .. } => tabs.iter().for_each(|t| t.content.visit(visitor)),
            _ => {}
        }
    }

    /// A bound viewport cannot report a block owned by a nested independent Scroll.
    fn viewport_block(&self, id: &str) -> Option<&Self> {
        if self.disabled || matches!(self.kind, Kind::Scroll { .. }) {
            return None;
        }
        if self.id == id {
            return Some(self);
        }
        match &self.kind {
            Kind::Column { children } | Kind::Row { children } => {
                children.iter().find_map(|node| node.viewport_block(id))
            }
            Kind::Tabs { tabs, selected } => tabs
                .iter()
                .find(|tab| &tab.id == selected)
                .and_then(|tab| tab.content.viewport_block(id)),
            _ => None,
        }
    }

    fn find(&self, id: &str) -> Option<&Self> {
        if self.disabled {
            return None;
        }
        if self.id == id {
            return Some(self);
        }
        match &self.kind {
            Kind::Column { children } | Kind::Row { children } => {
                children.iter().find_map(|n| n.find(id))
            }
            Kind::Scroll { content } => content.find(id),
            Kind::Canvas(canvas) => canvas
                .overlays
                .iter()
                .filter(|overlay| {
                    canvas
                        .scene
                        .as_ref()
                        .and_then(|scene| overlay.anchor.placement(scene))
                        .is_some_and(|placement| placement.visible)
                })
                .find_map(|overlay| overlay.child.find(id)),
            Kind::Editor(editor) => editor
                .left_actions()
                .iter()
                .chain(editor.right_actions())
                .find_map(|action| action.content.find(id)),
            Kind::Tabs { tabs, selected } => tabs
                .iter()
                .find(|t| &t.id == selected)
                .and_then(|t| t.content.find(id)),
            _ => None,
        }
    }
}
