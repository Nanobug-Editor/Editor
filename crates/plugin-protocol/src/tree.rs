//! Instance-owned native trees with bounded lazy pages and versioned callbacks.
//! Node identifiers and cursors are provider data; they grant no filesystem or document authority.
//!
//! Stable core `ui.tree 1.0`, protocol 7/base 1. The minimum compatible host must advertise
//! `ui.tree`, `ui.panels` and `ui.native`; package versions do not negotiate these capabilities.
//! Guest messages are platform independent; the current native GPUI implementation is verified on
//! Windows. Other hosts must provide the same negotiated behavior before advertising this capability.
//! Allocate with `create`, publish `ui::Kind::Tree` in an owned panel, handle
//! `ui::Action::Tree(Event::Request(..))`, and call `reply` with provider data or a typed failure.
//! Give `Kind::Tree` and its surrounding column a growing or explicitly sized layout, so Base's
//! virtual list has a bounded viewport supplied by the native panel.
//! A tree holds at most 4096 nodes, 64 levels, eight outstanding requests (15 s deadline),
//! 2 MiB node metadata and 2 MiB owned SVG artwork. Pages hold at most 128 nodes/512 KiB.
//! Selection never implicitly executes a command; activation/context commands must be typed,
//! declared by the package and negotiated through `plugin.commands`. Artwork also needs `ui.icons`
//! and the `assets.read` grant; each SVG is at most 64 KiB.
//! Activation and menu slots are separate action identities, even when command names repeat.
//! `ui.tree >=1.6,<2` adds shared hierarchical `Node::action_menu` with stable IDs, checked rows,
//! groups, lazy cascading submenus and typed actions. `Options::single_click_expand` keeps directory
//! disclosure on a single click while retaining the selected leaf gesture. Both require that floor.
//! The host resolves each slot's complete typed arguments from the current owned node and epoch.
//! `ui.tree >=1.7,<2` adds provider-owned three-state checkboxes, inline secondary labels and
//! `set_expanded` for explicit disclosure of cached directories. Checking emits `Event::Checked`
//! without selecting or expanding a row; providers decide its business effect and publish the result.
//! `ui.tree >=1.8,<2` adds `Node::muted` for totals and `Options::proportional_labels` for compact UI text.
//! `ui.tree >=1.1,<2` adds disabled/separated items and raises the per-node menu from 8 to 32.
//! Declare that explicit minimum when publishing these features; disabled commands reject invocation.
//! `ui.tree >=1.2,<2` adds `update` for cached presentation/action availability changes without
//! reloading pages. Declare a required range excluding 1.0/1.1; host inventory alone is not opt-in.
//! An update admits at most 128 nodes/512 KiB atomically. Uncached IDs are ignored after validation.
//! Existing IDs retain branch/leaf identity, activation/menu command names, typed arguments, menu
//! order and separators. Labels, descriptions, icons and node/command disabled states may change.
//! Updates preserve epoch, expansion, loaded pages, continuations and outstanding request identities;
//! identical metadata leaves the native revision unchanged. Structure requires `refresh` or the
//! separately negotiated 1.4 structural patch described below.
//! `ui.tree >=1.3,<2` adds semantic node tones and one initial selection path. Initialization lazily
//! reveals the exact ancestor IDs, never invokes a command or takes focus, and yields permanently
//! to native user input. Repeated initialization, refresh and hide/show cannot reset a user's choice.
//! `ui.tree >=1.4,<2` adds atomic structural `apply_patch`: publish only changed nodes and cached
//! child lists while retaining all other loaded pages, native selection, expansion and scrolling.
//! A changed patch seals old callbacks/pending replies without an intermediate empty tree. An
//! identical patch preserves its epoch. Patch bytes plus the request envelope remain below 2 MiB.
//! `ui.tree >=1.5,<2` adds `create_with_options`: choose single-click (the default) or double-click
//! pointer selection for the lifetime of a tree. A first press in double-click mode only focuses;
//! native keyboard selection, disclosure and typed activation keep their ordinary behavior.
//! Cursor length is 1..=128 bytes. IDs are unique throughout a refresh epoch and stable for the
//! same business node across refresh. Provider cursors/IDs are not paths or access grants.
//! Collapse, refresh, scene removal and owner retirement cancel pending page admission; cancellation
//! does not stop external provider work. A late, duplicate or forged reply yields `StaleRevision`.
//! Error pages retain the earlier accepted data and expose a native retry row. Explicit
//! `api::guest::close_resource` releases the tree; disable/cutover/trust loss revoke every clone.
//! Invalid ownership is `InvalidHandle`, missing grants `PermissionDenied`, missing capability
//! `CapabilityUnavailable`, malformed cursors `InvalidRequest`, duplicate IDs `Conflict`, and invalid
//! or oversized node metadata/quotas `LimitExceeded`. Invalid icon paths produce `InvalidPath`.
use crate::api::{ErrorCode, Failure, ResourceHandle};
use serde::{Deserialize, Serialize};

/// One tree admits at most 4096 nodes, 128 nodes per page, 8 pending pages and 64 levels.
pub const MAX_NODES: usize = 4096;
/// The provider must use `Page.next` rather than exceed this native publication budget.
pub const MAX_PAGE: usize = 128;
/// Native trees share the instance's 128-resource quota and have an additional limit of eight trees.
pub const MAX_TREES: usize = 8;

/// Pointer gesture which commits native selection and emits `Event::Selected`.
/// Keyboard navigation is unchanged; this never checks out a branch or grants command authority.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionGesture {
    /// Select on the first pointer press, as in the original tree contract.
    #[default]
    SingleClick,
    /// Keep the current selection until the second press of a native double click.
    DoubleClick,
}

/// Immutable interaction preferences supplied at allocation; requires explicit ui.tree 1.5.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Options {
    /// Pointer selection gesture; absent metadata keeps ordinary single-click behavior.
    #[serde(default)]
    pub selection: SelectionGesture,
    /// Single pointer presses toggle directories without changing leaf selection; requires ui.tree 1.6.
    #[serde(default, skip_serializing_if = "is_false")]
    pub single_click_expand: bool,
    /// Use the UI font with compact explorer geometry; omitted keeps monospace labels. Requires 1.8.
    #[serde(default, skip_serializing_if = "is_false")]
    pub proportional_labels: bool,
}

/// A declared typed command in the publishing package; arguments obey that command's schema.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    pub command: String,
    pub label: crate::ui::LocalizedText,
    #[serde(default)]
    pub arguments: serde_json::Value,
    /// Unavailable actions remain visible but cannot activate; requires an explicit ui.tree 1.1 floor.
    #[serde(default, skip_serializing_if = "is_false")]
    pub disabled: bool,
    /// Draw a separator before this item; requires an explicit ui.tree 1.1 floor.
    #[serde(default, skip_serializing_if = "is_false")]
    pub separator_before: bool,
}

impl Command {
    /// Bound one declared typed command before native publication or callback resolution.
    pub fn validate(&self) -> Result<(), Failure> {
        if !valid_id(&self.command)
            || self.label.en.is_empty()
            || self.label.zh_cn.is_empty()
            || self.label.en.len() > 4096
            || self.label.zh_cn.len() > 4096
            || serde_json::to_vec(&self.arguments).map_or(true, |bytes| bytes.len() > 65536)
        {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Invalid typed menu command metadata",
            ));
        }
        Ok(())
    }
}

/// Omit additive metadata so ordinary 1.0 commands retain their original wire shape.
fn is_false(value: &bool) -> bool {
    !*value
}

#[cfg(test)]
mod incremental_tests {
    /// Structural deltas need their own wire operation; metadata-only updates cannot insert rows.
    #[test]
    fn incremental_tree_patch_has_a_public_wire_operation() {
        let value = serde_json::json!({
            "kind":"apply_patch", "tree":{"instance":"owner","scope":"workspace","resource":1},
            "patch":{"nodes":[],"branches":[{"parent":null,"children":[],"next":null}]}
        });
        assert!(
            serde_json::from_value::<super::Operation>(value).is_ok(),
            "public tree deltas must not require clearing all cached pages"
        );
    }
}

/// A stable provider node. Branches may initially have no children; the host asks on expansion.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub branch: bool,
    #[serde(default)]
    pub disabled: bool,
    /// Optional semantic text color, resolved by the native theme; requires explicit ui.tree 1.3.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tone: Option<Tone>,
    /// Use the theme's secondary foreground, taking precedence over tone; requires ui.tree 1.8.
    #[serde(default, skip_serializing_if = "is_false")]
    pub muted: bool,
    /// Optional provider-owned checkbox; absent keeps the ordinary row. Requires ui.tree 1.7.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked: Option<CheckState>,
    /// Secondary inline text such as a count; descriptions remain tooltips. Requires ui.tree 1.7.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub suffix: String,
    /// Trees, toolbar tools and decorations use the same owned geometry-only SVG contract.
    #[serde(default)]
    pub icon: Option<crate::ui::ToolIcon>,
    /// Enter or a double click invokes this package's declared typed command.
    #[serde(default)]
    pub activation: Option<Command>,
    /// Native context items reuse typed command admission rather than create a second command API.
    #[serde(default)]
    pub menu: Vec<Command>,
    /// Cascading native context actions; requires explicit ui.tree 1.6 and replaces the flat menu.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub action_menu: Vec<crate::ui::ActionMenuItem>,
}
impl Node {
    /// Create a leaf or branch with a stable identifier; optional command and icon metadata start empty.
    pub fn new(id: impl Into<String>, label: impl Into<String>, branch: bool) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            description: String::new(),
            branch,
            disabled: false,
            tone: None,
            muted: false,
            checked: None,
            suffix: String::new(),
            icon: None,
            activation: None,
            menu: Vec::new(),
            action_menu: Vec::new(),
        }
    }
    /// Bound metadata before native allocation; ownership and typed command schemas are rechecked by runtime.
    pub fn validate(&self) -> Result<(), Failure> {
        if !valid_id(&self.id)
            || self.label.is_empty()
            || self.label.len() > 1024
            || self.description.len() > 4096
            || self.suffix.len() > 1024
            || self.menu.len() > 32
        {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Tree node metadata exceeds its budget",
            ));
        }
        if let Some(icon) = &self.icon {
            icon.validate()
                .map_err(|error| Failure::new(ErrorCode::InvalidPath, error))?;
        }
        // Repeated command names are valid; action identity is the activation or exact menu slot.
        for command in self.activation.iter().chain(&self.menu) {
            if !valid_id(&command.command)
                || command.label.en.is_empty()
                || command.label.zh_cn.is_empty()
                || command.label.en.len() > 256
                || command.label.zh_cn.len() > 256
                || serde_json::to_vec(&command.arguments).map_or(true, |bytes| bytes.len() > 65536)
            {
                return Err(Failure::new(
                    ErrorCode::LimitExceeded,
                    "Invalid tree command metadata",
                ));
            }
        }
        crate::ui::validate_action_menu(&self.action_menu)?;
        Ok(())
    }
}

/// Controlled checkbox value; activating a mixed row requests Checked, never guesses descendants.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckState {
    /// No items represented by this row are included.
    Unchecked,
    /// Every item represented by this row is included.
    Checked,
    /// Only some items or part of one item are included.
    Mixed,
}

/// Domain-neutral text meanings; literal-color artwork and selection backgrounds remain unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    /// Positive change or successful state, conventionally green.
    Positive,
    /// Removed content or negative state, conventionally red.
    Negative,
    /// Changed content or informational state, conventionally blue.
    Informative,
}

/// One atomic page replaces the first page or appends to the exact accepted continuation.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Page {
    pub nodes: Vec<Node>,
    /// Opaque provider cursor; `None` marks the completed branch, including an empty branch.
    pub next: Option<String>,
}

/// Replacement of one already cached parent's loaded child prefix; requires ui.tree 1.4.
/// Unchanged child lists are omitted. `next` remains the provider's bounded continuation cursor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BranchPatch {
    /// `None` addresses roots; `Some` must identify an existing or newly attached branch.
    pub parent: Option<String>,
    /// Ordered complete loaded prefix; unmentioned parents retain their accepted children.
    pub children: Vec<String>,
    /// Cursor after that prefix, or no cursor once the directory is complete.
    pub next: Option<String>,
}

/// One atomic tree delta. Nodes contain only inserted/changed metadata; branches contain only
/// changed cached child prefixes. Unreachable former subtrees are removed, never reloaded.
/// Every referenced child must exist in the resulting bounded forest, without cycles/duplicate
/// parents. New branch children remain lazy until requested; stable IDs retain native state.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Patch {
    /// New or changed nodes; existing IDs retain branch kind and typed command intent.
    pub nodes: Vec<Node>,
    /// Changed cached child prefixes; omitted branch/page state is preserved.
    pub branches: Vec<BranchPatch>,
}
impl Patch {
    /// Validate transport budgets before allocation, icon reads or mutation. Returns ordinary
    /// InvalidRequest/Conflict/LimitExceeded errors; runtime also validates the resulting forest.
    pub fn validate(&self) -> Result<(), Failure> {
        let limit = || Failure::new(ErrorCode::LimitExceeded, "Tree patch quota exceeded");
        if self.nodes.len() > MAX_NODES
            || self.branches.len() > MAX_NODES
            || self
                .branches
                .iter()
                .map(|b| b.children.len())
                .sum::<usize>()
                > MAX_NODES
            || serde_json::to_vec(self)
                .map_or(true, |b| b.len() > crate::api::MAX_REQUEST_BYTES - 4096)
        {
            return Err(limit());
        }
        let mut ids = std::collections::BTreeSet::new();
        for node in &self.nodes {
            node.validate()?;
            if !ids.insert(&node.id) {
                return Err(Failure::new(
                    ErrorCode::Conflict,
                    "Duplicate tree patch node",
                ));
            }
        }
        let mut parents = std::collections::BTreeSet::new();
        for branch in &self.branches {
            if branch.parent.as_ref().is_some_and(|id| !valid_id(id))
                || branch.children.iter().any(|id| !valid_id(id))
                || branch
                    .next
                    .as_ref()
                    .is_some_and(|next| next.is_empty() || next.len() > 128)
            {
                return Err(Failure::new(
                    ErrorCode::InvalidRequest,
                    "Invalid tree patch branch",
                ));
            }
            if !parents.insert(&branch.parent) {
                return Err(Failure::new(
                    ErrorCode::Conflict,
                    "Duplicate tree patch parent",
                ));
            }
        }
        Ok(())
    }
}

/// A host-issued page identity. Fold, refresh, suspend, close and retirement invalidate late replies.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub tree: ResourceHandle,
    pub epoch: u64,
    pub sequence: u64,
    pub parent: Option<String>,
    pub cursor: Option<String>,
    pub max_nodes: u32,
}

/// One action in an immutable node publication, resolved only inside its current owned tree epoch.
/// Command names are reusable schema identities; they do not identify a particular menu invocation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum NodeAction {
    /// Enter or double click selects the node's optional activation command.
    Activation,
    /// Zero-based position in `Node.menu` (8 items in 1.0, 32 in 1.1). Missing/disabled slots are stale.
    Menu { index: u32 },
    /// Stable row identity within the current cascading menu; disabled ancestors reject invocation.
    MenuItem { id: String },
}

/// Callbacks use an ordinary owning UI node/revision in addition to this tree's epoch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Event {
    Request(Request),
    /// Explicit checkbox intent against the displayed value; this does not change native selection.
    Checked {
        tree: ResourceHandle,
        epoch: u64,
        node: String,
        /// Captured value rejects a press whose provider metadata changed before admission.
        previous: CheckState,
        /// The requested binary result; mixed controls activate to true.
        checked: bool,
    },
    /// Selection is informational; it does not implicitly invoke a command or open a document.
    Selected {
        tree: ResourceHandle,
        epoch: u64,
        node: String,
    },
    /// The host resolves the exact current action metadata and uses the existing typed command boundary.
    Invoke {
        tree: ResourceHandle,
        epoch: u64,
        node: String,
        action: NodeAction,
    },
}
impl Event {
    /// Every native callback is bound to one owned tree, never a current-plugin lookup.
    pub fn handle(&self) -> &ResourceHandle {
        match self {
            Self::Request(request) => &request.tree,
            Self::Selected { tree, .. }
            | Self::Invoke { tree, .. }
            | Self::Checked { tree, .. } => tree,
        }
    }
}

/// Tree publication is independent of canvas/tree-business implementations.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    Create,
    /// Set disclosure for at most MAX_NODES cached branch IDs. Missing IDs are ignored;
    /// expanding loads ordinary bounded pages, while collapsing revokes descendant pending replies.
    /// Requires explicit ui.tree 1.7; no selection, focus or command activation is implied.
    SetExpanded {
        tree: ResourceHandle,
        nodes: Vec<String>,
        expanded: bool,
    },
    /// Allocate with immutable pointer preferences; requires explicit ui.tree 1.5.
    CreateWithOptions {
        options: Options,
    },
    /// One default root-to-target ID path, at most 64 unique IDs; requires explicit ui.tree 1.3.
    /// A prior initialization or native interaction makes this an accepted no-op.
    Initialize {
        tree: ResourceHandle,
        path: Vec<String>,
    },
    Reply {
        request: Request,
        result: Result<Page, Failure>,
    },
    /// Invalidates pending work and reloads roots; stable node IDs let native selection recover.
    Refresh {
        tree: ResourceHandle,
    },
    /// Atomically change cached node presentation and availability; requires explicit ui.tree 1.2.
    /// Unknown IDs are validated then ignored. Structure and command intent must remain unchanged.
    Update {
        /// Current instance-owned tree; this conveys no authority over another provider's tree.
        tree: ResourceHandle,
        /// At most 128 complete nodes/512 KiB, matched against cached IDs without loading pages.
        nodes: Vec<Node>,
    },
    /// Atomically reconcile cached structure without clearing it; requires explicit ui.tree 1.4.
    ApplyPatch {
        tree: ResourceHandle,
        patch: Patch,
    },
}

#[cfg(feature = "guest")]
/// Set `nodes`' disclosure in the owned `tree` to `expanded`; lazy loads retain ordinary quotas.
/// Returns ownership, capability, malformed-ID or resource-lifetime failures without partial mutation.
pub fn set_expanded(
    tree: ResourceHandle,
    nodes: Vec<String>,
    expanded: bool,
) -> Result<(), Failure> {
    crate::api::guest::request(crate::api::Operation::Tree {
        operation: Operation::SetExpanded {
            tree,
            nodes,
            expanded,
        },
    })
    .map(|_| ())
}

#[cfg(feature = "guest")]
/// Set `tree`'s one initial selection using exact root-to-target `path` IDs. Lazy ancestors/page
/// continuations are loaded within normal quotas. Missing/disabled paths stop without activation;
/// native input wins over late defaults. Returns normal ownership, grant, capability or path errors.
pub fn initialize(tree: ResourceHandle, path: Vec<String>) -> Result<(), Failure> {
    crate::api::guest::request(crate::api::Operation::Tree {
        operation: Operation::Initialize { tree, path },
    })
    .map(|_| ())
}

/// IDs exclude host-reserved sentinels and controls; cursors are separately bounded opaque strings.
pub fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && !value.starts_with('@')
        && !value.chars().any(char::is_control)
}

#[cfg(feature = "guest")]
/// Allocate an owned lazy tree. Requires `ui.tree`, `ui.panels`, `ui.native` and the `ui.panels` grant.
pub fn create() -> Result<ResourceHandle, Failure> {
    match crate::api::guest::request(crate::api::Operation::Tree {
        operation: Operation::Create,
    })? {
        crate::api::Value::Resource(handle) => Ok(handle),
        _ => Err(Failure::new(
            ErrorCode::InvalidRequest,
            "Unexpected tree allocation result",
        )),
    }
}

#[cfg(feature = "guest")]
/// Allocate an owned tree using `options` for its entire lifetime. Requires explicit ui.tree >=1.5,
/// ui.panels/ui.native and the ui.panels grant. Returns the same ownership/quota errors as `create`;
/// older capability floors are rejected before allocation. Refresh and hide/show retain the options.
pub fn create_with_options(options: Options) -> Result<ResourceHandle, Failure> {
    match crate::api::guest::request(crate::api::Operation::Tree {
        operation: Operation::CreateWithOptions { options },
    })? {
        crate::api::Value::Resource(handle) => Ok(handle),
        _ => Err(Failure::new(
            ErrorCode::InvalidRequest,
            "Unexpected tree allocation result",
        )),
    }
}
#[cfg(feature = "guest")]
/// Complete an accepted page once. Errors become native retry rows; obsolete replies fail without mutation.
pub fn reply(request: Request, result: Result<Page, Failure>) -> Result<(), Failure> {
    crate::api::guest::request(crate::api::Operation::Tree {
        operation: Operation::Reply { request, result },
    })
    .map(|_| ())
}
#[cfg(feature = "guest")]
/// Invalidate pending pages and request current roots at the next native projection.
pub fn refresh(tree: ResourceHandle) -> Result<(), Failure> {
    crate::api::guest::request(crate::api::Operation::Tree {
        operation: Operation::Refresh { tree },
    })
    .map(|_| ())
}
#[cfg(feature = "guest")]
/// Update `tree`'s cached `nodes` without reloading provider pages; requires explicit ui.tree 1.2.
/// Full node metadata is validated atomically, including uncached IDs which are then ignored.
/// Returns ownership/capability/grant, metadata/budget, or `Conflict` for changed structure/commands.
pub fn update(tree: ResourceHandle, nodes: Vec<Node>) -> Result<(), Failure> {
    crate::api::guest::request(crate::api::Operation::Tree {
        operation: Operation::Update { tree, nodes },
    })
    .map(|_| ())
}

#[cfg(feature = "guest")]
/// Apply a bounded structural delta to the owned tree. Retains unaffected pages and native view
/// state; stale callbacks are sealed on change. Returns capability/grant/ownership, invalid forest,
/// node/artwork or quota errors atomically, preserving the last complete tree on every failure.
pub fn apply_patch(tree: ResourceHandle, patch: Patch) -> Result<(), Failure> {
    crate::api::guest::request(crate::api::Operation::Tree {
        operation: Operation::ApplyPatch { tree, patch },
    })
    .map(|_| ())
}
