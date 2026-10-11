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

/// A declared typed command in the publishing package; arguments obey that command's schema.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    pub command: String,
    pub label: crate::ui::LocalizedText,
    #[serde(default)]
    pub arguments: serde_json::Value,
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
    /// Trees, toolbar tools and decorations use the same owned geometry-only SVG contract.
    #[serde(default)]
    pub icon: Option<crate::ui::ToolIcon>,
    /// Enter or a double click invokes this package's declared typed command.
    #[serde(default)]
    pub activation: Option<Command>,
    /// Native context items reuse typed command admission rather than create a second command API.
    #[serde(default)]
    pub menu: Vec<Command>,
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
            icon: None,
            activation: None,
            menu: Vec::new(),
        }
    }
    /// Bound metadata before native allocation; ownership and typed command schemas are rechecked by runtime.
    pub fn validate(&self) -> Result<(), Failure> {
        if !valid_id(&self.id)
            || self.label.is_empty()
            || self.label.len() > 1024
            || self.description.len() > 4096
            || self.menu.len() > 8
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
        Ok(())
    }
}

/// One atomic page replaces the first page or appends to the exact accepted continuation.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Page {
    pub nodes: Vec<Node>,
    /// Opaque provider cursor; `None` marks the completed branch, including an empty branch.
    pub next: Option<String>,
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

/// Callbacks use an ordinary owning UI node/revision in addition to this tree's epoch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Event {
    Request(Request),
    /// Selection is informational; it does not implicitly invoke a command or open a document.
    Selected {
        tree: ResourceHandle,
        epoch: u64,
        node: String,
    },
    /// The host resolves the current command metadata and uses the existing typed command boundary.
    Invoke {
        tree: ResourceHandle,
        epoch: u64,
        node: String,
        command: String,
    },
}
impl Event {
    /// Every native callback is bound to one owned tree, never a current-plugin lookup.
    pub fn handle(&self) -> &ResourceHandle {
        match self {
            Self::Request(request) => &request.tree,
            Self::Selected { tree, .. } | Self::Invoke { tree, .. } => tree,
        }
    }
}

/// Tree publication is independent of canvas/tree-business implementations.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    Create,
    Reply {
        request: Request,
        result: Result<Page, Failure>,
    },
    /// Invalidates pending work and reloads roots; stable node IDs let native selection recover.
    Refresh {
        tree: ResourceHandle,
    },
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
