//! Stable workspace.files 1.2 discovery and mixed native plans. Cancellation stops further work;
//! receipts describe actual effects and current-session recovery, without global atomic rollback.
//! The native implementation is verified on Windows; other hosts are compilation-only targets.
//!
//! Acquire a workspace root with `api::Operation::WorkspaceRoot`, then use portable relative
//! [`FileTarget`] values; guests never receive native paths. Reads require `workspace.read`,
//! mutations require `workspace.write`, and an opened unsaved buffer additionally requires
//! `editor.documents 1.1` plus `editor.read`. Buffer changes require `editor.edit 1.1` and
//! `editor.write`. External targets use `files.selection 1.1` with an explicit ReadWrite
//! selection; legacy selections remain read-only. Save permits only the exact Write intent.
//!
//! Use [`Operation::Stat`] to capture opaque disk stamps and native document versions, then
//! [`submit`] a complete [`Plan`]. All targets, permissions and revisions are checked before
//! preview and again after explicit confirmation. An opened text target uses its unique
//! DocumentSession and remains unsaved; Copy cannot read a dirty opened source directly,
//! Delete refuses opened targets, and Rename follows the normal native path reconciliation.
//! Use a document snapshot followed by Write when copying current unsaved text.
//!
//! Pages contain 1..=256 entries. Read returns at most 1 MiB; directory listing admits at
//! most 4,096 entries. Recursive discovery respects its FileQuery budget and ignore state.
//! Capture for one disk preparation is bounded to 32 MiB, 50,000 entries and depth 64;
//! retained native and disk recovery data share a 64 MiB / 16-batch host quota. Each instance
//! may retain eight batches, which also consume the shared 128-resource handle budget.
//! A native preview admits at most 256 text ranges for each edited document; all ranges use
//! half-open UTF-8 byte coordinates in the original revision, with selection in the final text.
//!
//! Invalid paths, links/junctions, device names and alternate streams fail with InvalidPath;
//! missing grants with PermissionDenied, revoked ownership with InvalidHandle, stale discovery
//! or document revisions with StaleRevision, quotas with LimitExceeded, and native IO with
//! OperationFailed or NotFound. A plan conflict reports its affected targets in [`Receipt`].
//! Cancellation stops later operations, never implies rollback, and may terminate only the
//! original wait while a started atomic disk replacement finishes. Retain the separately
//! returned batch handle to inspect BatchStatus or request UndoBatch. Undo first checks all
//! current text/disk states; later typing, local Undo/Redo or disk changes prevent all automatic
//! restoration. Partial IO recovery can be retried while its observed post-state still matches.
//! CloseResource releases requests, batches and subscriptions; retirement or trust loss revokes
//! them. Recovery is limited to the current session and does not survive an application restart.

use crate::api::{
    DocumentVersion, ErrorCode, Failure, FileQuery, ResourceHandle, TextEdit, TextRange,
};
use serde::{Deserialize, Serialize};

/// Monotonic sequence belongs to one host workspace session. Events never confer file authority.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileEvent {
    pub sequence: u64,
    pub kind: FileEventKind,
}

/// Portable workspace-relative paths include unopened targets. Repeated Modified events may coalesce.
/// Paired renames retain source/target; unmatched halves are delete/create. A RescanRequired observation
/// means an incomplete stream: restart bounded discovery and reread relevant files before trusting state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum FileEventKind {
    Created { path: String },
    Modified { path: String },
    Deleted { path: String },
    Renamed { source: String, target: String },
    RescanRequired,
}

#[cfg(feature = "guest")]
/// Observe one owned workspace root with workspace.read and workspace.files 1.2.
/// The shared native watcher supplies unopened create/modify/delete/rename and explicit rescan notices.
/// A subscription has a 128-event queue, drains eight per tick, and coalesces repeated Modified paths.
/// Overflow ends with SubscriptionFailed: call Rescan, rediscover/reread, then acquire a fresh subscription.
/// Up to eight subscriptions belong to one instance; CloseResource and retirement stop delivery immediately.
pub fn subscribe(root: crate::api::ResourceHandle) -> Result<crate::api::ResourceHandle, Failure> {
    match crate::api::guest::request(crate::api::Operation::SubscribeWorkspaceFiles { root })? {
        crate::api::Value::Resource(handle) => Ok(handle),
        _ => Err(Failure::new(
            ErrorCode::InvalidState,
            "Expected workspace observation resource",
        )),
    }
}

/// File authority resides in an owned workspace or explicitly selected root, never in a native path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileTarget {
    pub root: ResourceHandle,
    /// Portable root-relative path. Empty addresses a directory root or an exact selected file.
    pub path: String,
}

impl FileTarget {
    /// Validate syntax before native ancestor, link, instance and access checks.
    pub fn validate(&self) -> Result<(), Failure> {
        if self.path.len() > 4096
            || self.path.contains(['\\', ':', '?', '*'])
            || self.path.chars().any(char::is_control)
            || (!self.path.is_empty()
                && self.path.split('/').any(|part| {
                    part.is_empty()
                        || part == "."
                        || part == ".."
                        || part.ends_with(['.', ' '])
                        || crate::api::is_windows_device_segment(part)
                }))
        {
            return Err(Failure::new(
                ErrorCode::InvalidPath,
                "Invalid portable file target",
            ));
        }
        Ok(())
    }
}

/// A native identity and content/tree fingerprint. Versions are opaque and valid only for comparison.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum FileStamp {
    Missing,
    File {
        version: String,
        bytes: u64,
        readonly: bool,
    },
    Directory {
        version: String,
        readonly: bool,
    },
    /// Refer to an earlier operation's resulting object in this plan, for ordered create/rename chains.
    /// Stat never returns this form. A forward reference or a different produced target is a conflict.
    FromPlan {
        change: u16,
    },
}

/// Disk metadata remains distinct from an opened session's unsaved text and monotonic revision.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metadata {
    pub target: FileTarget,
    pub stamp: FileStamp,
    pub document: Option<DocumentVersion>,
}

/// Continuations are invalid after the directory, query or ignore state changes; restart discovery then.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cursor {
    pub version: String,
    pub after: String,
}

/// A page contains complete entries; next=None means discovery completed within its explicit budget.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Page {
    pub entries: Vec<Metadata>,
    pub next: Option<Cursor>,
    pub skipped: Vec<String>,
}

/// Resource actions address the ordered planned state. Rename chains are supported; collisions fail.
/// Writes to opened text use its unique buffer and remain unsaved. Opaque opened targets fail.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Change {
    EditDocument {
        document: DocumentVersion,
        edits: Vec<TextEdit>,
        selection: TextRange,
    },
    CreateDirectory {
        target: FileTarget,
    },
    /// Atomic disk replacement, or an atomic native buffer edit for an opened text target.
    /// Missing is required for creation. Omit expected only for an exact Save selection intent.
    Write {
        target: FileTarget,
        expected: Option<FileStamp>,
        bytes: Vec<u8>,
    },
    Copy {
        source: FileTarget,
        target: FileTarget,
        expected_source: FileStamp,
    },
    Rename {
        source: FileTarget,
        target: FileTarget,
        expected_source: FileStamp,
    },
    /// Open targets are refused; closing a dirty buffer must remain the user's explicit action.
    Delete {
        target: FileTarget,
        expected: FileStamp,
    },
}

/// At most 128 ordered operations and 1 MiB submitted bytes enter one native preview.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub title: String,
    pub changes: Vec<Change>,
}

impl Plan {
    /// Validate bounded transport shape before acquiring resources or reading any native target.
    pub fn validate(&self) -> Result<(), Failure> {
        if self.title.is_empty()
            || self.title.len() > 256
            || self.title.chars().any(char::is_control)
            || self.changes.is_empty()
            || self.changes.len() > 128
        {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Plan requires a title and 1..128 operations",
            ));
        }
        let mut bytes = 0usize;
        for change in &self.changes {
            match change {
                Change::EditDocument {
                    document,
                    edits,
                    selection,
                } => {
                    crate::api::ResourceIdentity::Local {
                        path: document.path.clone(),
                    }
                    .validate()?;
                    crate::api::validate_text_edits(edits, *selection)?;
                    bytes = bytes
                        .checked_add(edits.iter().map(|edit| edit.text.len()).sum())
                        .ok_or_else(limit)?;
                }
                Change::Write {
                    target,
                    bytes: data,
                    ..
                } => {
                    target.validate()?;
                    bytes = bytes.checked_add(data.len()).ok_or_else(limit)?;
                }
                Change::CreateDirectory { target } | Change::Delete { target, .. } => {
                    target.validate()?
                }
                Change::Copy { source, target, .. } | Change::Rename { source, target, .. } => {
                    source.validate()?;
                    target.validate()?;
                }
            }
        }
        if bytes > 1024 * 1024 {
            return Err(limit());
        }
        Ok(())
    }
}

fn limit() -> Failure {
    Failure::new(
        ErrorCode::LimitExceeded,
        "Plan submitted bytes exceed 1 MiB",
    )
}

/// State survives a stopped wait while the owning instance remains live; no restart recovery is promised.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchState {
    Preparing,
    Preview,
    Applying,
    Complete,
    Cancelled,
    Partial,
    Undone,
    UndoPartial,
}

/// A conflict identifies a target without granting access to its current content.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Conflict {
    pub target: String,
    pub reason: String,
}

/// Operation indices report real completed work, including partial commit/Undo and cancellation.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub batch: ResourceHandle,
    pub state: BatchState,
    pub completed: Vec<u16>,
    /// The operation that stopped, which may itself have performed a partial directory operation.
    pub stopped_at: Option<u16>,
    /// Disk targets whose state actually differs from the captured preimage, including a partial step.
    pub affected: Vec<FileTarget>,
    pub failed: Option<Failure>,
    pub conflicts: Vec<Conflict>,
    pub recoverable: bool,
}

/// Native file work is scoped by the original instance and root. Mutations always require preview.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    Stat {
        target: FileTarget,
    },
    /// Read current unsaved native text if open, otherwise bounded disk bytes; Save intent has no read.
    Read {
        target: FileTarget,
    },
    List {
        target: FileTarget,
        page_size: u16,
        cursor: Option<Cursor>,
    },
    Discover {
        root: ResourceHandle,
        query: FileQuery,
        page_size: u16,
        cursor: Option<Cursor>,
    },
    ApplyPlan {
        plan: Plan,
    },
    /// Inspect actual effects after cancellation or I/O failure without resuming or undoing anything.
    BatchStatus {
        batch: ResourceHandle,
    },
    /// All text and disk targets are checked first. One conflict prevents any automatic restoration.
    UndoBatch {
        batch: ResourceHandle,
    },
    /// Request a full workspace reconciliation after an overflow or stale discovery continuation.
    Rescan,
}

/// File results never return native paths or expand the original root's access intent.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Value {
    Metadata(Metadata),
    Read { metadata: Metadata, bytes: Vec<u8> },
    Page(Page),
    Batch(Receipt),
    RescanRequested,
}

#[cfg(feature = "guest")]
/// Start bounded native file work. workspace.files 1.2 is required; only granted roots are admitted.
/// For ApplyPlan prefer submit(), which also returns the recovery handle independently of its wait.
pub fn start(
    operation: Operation,
    timeout_ms: u32,
) -> Result<crate::api::guest::EditorTask, Failure> {
    crate::api::guest::EditorTask::start(
        crate::api::EditorOperation::Workspace { operation },
        timeout_ms,
    )
}

#[cfg(feature = "guest")]
/// Submit a preview plan, returning both its wait and its independently releasable recovery resource.
/// Notifications use request; batch status/Undo use batch even after the wait is cancelled.
pub fn submit(plan: Plan, timeout_ms: u32) -> Result<(ResourceHandle, ResourceHandle), Failure> {
    plan.validate()?;
    match crate::api::guest::request(crate::api::Operation::Editor {
        operation: crate::api::EditorOperation::Workspace {
            operation: Operation::ApplyPlan { plan },
        },
        timeout_ms,
    })? {
        crate::api::Value::BatchAccepted { request, batch } => Ok((request, batch)),
        _ => Err(Failure::new(
            ErrorCode::InvalidState,
            "Expected accepted plan and recovery resource",
        )),
    }
}
