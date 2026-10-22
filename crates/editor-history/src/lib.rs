//! Platform-independent, durable local history with independently decoded full content objects.

mod changes;
mod leases;
mod legacy;
mod maintenance;
mod policy;
mod profiles;
mod query;
mod schema;
mod store;
mod worker;

pub use leases::Lease;
pub use legacy::{LegacyCursor, LegacyEntry, LegacyFingerprint, LegacyIo, LegacyQuery};
pub use policy::Policy;
use std::path::PathBuf;
pub use store::History;
pub use worker::{Worker, WorkerStatus};

/// Persistent logical identity, independent of both the file's path and its content.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FileId(pub i64);

/// Stable identifier of a committed historical state in one profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VersionId(pub i64);

/// The event whose successfully observed bytes are being retained.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    /// Newly created file, with independent logical identity.
    Created,
    /// Successfully copied content, independent from its source file.
    Copied,
    /// Complete bytes successfully written by an editor save.
    Saved,
    /// Observed disk content before the editor overwrites it.
    BeforeOverwrite,
    /// Disk state observed after an external program's modification.
    External,
    /// Immutable current buffer retained before applying a historical version.
    BeforeRestore,
    /// Latest disk bytes retained before a destructive removal.
    BeforeDelete,
    /// Successfully renamed file, retaining its logical identity.
    Renamed,
    /// Successfully moved file, potentially across workspace memberships.
    Moved,
    /// Confirmed absence after removal; a content reference may still be available.
    Deleted,
    /// Historical source imported with its original observation time.
    Imported,
}

impl Reason {
    /// Stable storage label, also useful to select the host's localized source label.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Copied => "copied",
            Self::Saved => "saved",
            Self::BeforeOverwrite => "before_overwrite",
            Self::External => "external",
            Self::BeforeRestore => "before_restore",
            Self::BeforeDelete => "before_delete",
            Self::Renamed => "renamed",
            Self::Moved => "moved",
            Self::Deleted => "deleted",
            Self::Imported => "imported",
        }
    }

    /// Reject unknown on-disk labels rather than pretending the event was an ordinary save.
    pub(crate) fn parse(value: &str) -> Result<Self> {
        match value {
            "created" => Ok(Self::Created),
            "copied" => Ok(Self::Copied),
            "saved" => Ok(Self::Saved),
            "before_overwrite" => Ok(Self::BeforeOverwrite),
            "external" => Ok(Self::External),
            "before_restore" => Ok(Self::BeforeRestore),
            "before_delete" => Ok(Self::BeforeDelete),
            "renamed" => Ok(Self::Renamed),
            "moved" => Ok(Self::Moved),
            "deleted" => Ok(Self::Deleted),
            "imported" => Ok(Self::Imported),
            _ => Err(Error::Corrupt),
        }
    }
}

/// Metadata for one file operation already observed to have succeeded.
pub struct Change {
    /// Persistent file whose identity continues through the operation.
    pub file: FileId,
    /// Path after the operation, or its last path for a deletion.
    pub path: PathBuf,
    /// Membership after the operation; deletion retains the previous workspace.
    pub workspace: Option<PathBuf>,
    /// Structural event, such as a rename, move or deletion.
    pub reason: Reason,
    /// Whether the file is known to exist after this operation.
    pub present: bool,
    /// Actual UTC observation time, in milliseconds.
    pub at_ms: i64,
}

/// An immutable content capture; callers supply observation time rather than a hidden clock.
pub struct Capture {
    /// Existing identity for a known operation, or None to resolve the current path binding.
    pub file: Option<FileId>,
    /// Native adapter's validated absolute backing path.
    pub path: PathBuf,
    /// Workspace at observation time, or None for a standalone file.
    pub workspace: Option<PathBuf>,
    /// Exact uncompressed UTF-8 bytes, including BOM and original line endings.
    pub contents: Vec<u8>,
    /// Operation represented by this capture.
    pub reason: Reason,
    /// UTC milliseconds at which these bytes were observed or successfully saved.
    pub at_ms: i64,
}

/// Result of a capture, distinct from success of the project's file write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureOutcome {
    /// The durable version that was created.
    Recorded(VersionId),
    /// A consecutive identical disk state already has this retained version.
    Unchanged(VersionId),
    /// Policy intentionally excludes this observation; no content was recorded.
    Skipped(SkipReason),
}

impl CaptureOutcome {
    /// Return the retained version when capture produced a historical state.
    pub fn version(self) -> Option<VersionId> {
        match self {
            Self::Recorded(version) | Self::Unchanged(version) => Some(version),
            Self::Skipped(_) => None,
        }
    }
}

/// History errors never imply that the user's project file was rolled back.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Database operation or commit failed; the pending transaction is rolled back.
    #[error("history database: {0}")]
    Database(#[from] rusqlite::Error),
    /// File access, compression or decompression failed.
    #[error("history I/O: {0}")]
    Io(#[from] std::io::Error),
    /// A stored object could not be reconstructed without changing its bytes.
    #[error("history object is damaged")]
    Corrupt,
    /// A native path cannot be represented without losing its identity.
    #[error("history path is not valid Unicode")]
    InvalidPath,
    /// A poisoned lock cannot be used to silently continue an interrupted operation.
    #[error("history writer is unavailable")]
    Unavailable,
    /// User input must be corrected before replacing the active policy.
    #[error("invalid history policy: {0}")]
    InvalidPolicy(String),
    /// Restore must not bypass a policy decision that prevented its protective version.
    #[error("history protection was not recorded: {0:?}")]
    ProtectionSkipped(SkipReason),
    /// The budget cannot hold the required version or currently leased data.
    #[error("history capacity is insufficient")]
    Quota,
    /// A preview or migration no longer describes the selected immutable source.
    #[error("history selection changed; refresh before retrying")]
    Stale,
    /// Old raw snapshots may be streamed out but cannot be materialized above the recovery limit.
    #[error("history content exceeds the preview limit")]
    TooLarge,
    /// A foreign or newer database is preserved rather than initialized as an empty history.
    #[error("unsupported history storage format")]
    UnsupportedFormat,
    /// The bounded background queue or its retained-byte allowance is currently full.
    #[error("history worker is busy; retry after the pending operation")]
    Busy,
    /// Opening the profile failed on the worker; the underlying diagnostic remains visible.
    #[error("history initialization: {0}")]
    Initialization(String),
}

/// Result type shared by the platform-independent history interface.
pub type Result<T> = std::result::Result<T, Error>;

/// A visible policy decision, not a failure of the project's save operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkipReason {
    /// The user paused new captures in this profile.
    Disabled,
    /// The path is excluded or belongs to the history store itself.
    Excluded,
    /// Original bytes exceed the selected single-file limit.
    TooLarge,
    /// Content is binary, a known non-text file, or invalid UTF-8.
    Unsupported,
}

/// Profile history filtering; it never requires decoding content objects.
#[derive(Clone, Debug)]
pub struct Query {
    /// Only identities known deleted now; includes their earlier recoverable versions.
    pub deleted_only: bool,
    /// Restrict to one persistent identity, including all of its historical paths.
    pub file: Option<FileId>,
    /// Workspace membership at capture time.
    pub scope: Scope,
    /// Literal substring of the current or historical path.
    pub path: String,
    /// Exclusive position returned by the last item of the previous page.
    pub before: Option<Cursor>,
    /// Bounded number of metadata records, clamped to 1 through 200.
    pub limit: usize,
}

impl Default for Query {
    /// Start with the newest fifty profile records.
    fn default() -> Self {
        Self {
            deleted_only: false,
            file: None,
            scope: Scope::All,
            path: String::new(),
            before: None,
            limit: 50,
        }
    }
}

/// Workspace ownership is distinct from the file's present location.
#[derive(Clone, Debug, Default)]
pub enum Scope {
    /// Every workspace and standalone file in this profile.
    #[default]
    All,
    /// Only records captured in this validated workspace.
    Workspace(PathBuf),
    /// Only files observed without a workspace.
    Standalone,
}

/// Stable ordering position, independent of duplicate timestamps.
#[derive(Clone, Copy, Debug)]
pub struct Cursor {
    /// UTC timestamp of the last returned row.
    pub at_ms: i64,
    /// Row identity disambiguating equal timestamps.
    pub id: VersionId,
}

/// List item with enough metadata to select a version without loading its content.
#[derive(Clone, Debug)]
pub struct Version {
    /// Durable version identity.
    pub id: VersionId,
    /// Logical source file, independent of a live editor tab.
    pub file: FileId,
    /// Source path at capture time.
    pub path: PathBuf,
    /// A structural operation's previous path, including files without a content baseline.
    pub previous_path: Option<PathBuf>,
    /// Source version when a recovered document was saved as an independent copy.
    pub origin: Option<VersionId>,
    /// Workspace membership at capture time.
    pub workspace: Option<PathBuf>,
    /// UTC observation timestamp.
    pub at_ms: i64,
    /// Event source for the native history list.
    pub reason: Reason,
    /// Existence at this point; deletion does not discard its recoverable prior content.
    pub present: bool,
    /// Uncompressed length, or None if this structural point has no retained content.
    pub size: Option<u64>,
}

/// Current or last known location of a logical file, independent of a selected historical path.
#[derive(Clone, Debug)]
pub struct Location {
    /// Existing path after rename/move, or last path before deletion.
    pub path: PathBuf,
    /// Current workspace, or the workspace immediately before deletion.
    pub workspace: Option<PathBuf>,
    /// False after a known deletion; unrelated path reuse has its own identity.
    pub present: bool,
}

impl Version {
    /// Continue after this exact row even if several observations share its timestamp.
    pub fn cursor(&self) -> Cursor {
        Cursor {
            at_ms: self.at_ms,
            id: self.id,
        }
    }
}

/// Observable storage usage, used by settings and cleanup previews.
#[derive(Clone, Copy, Debug)]
pub struct Stats {
    /// Number of retained timeline points.
    pub versions: u64,
    /// Number of distinct retained complete contents.
    pub objects: u64,
    /// Compressed payload bytes, excluding database overhead.
    pub compressed_bytes: u64,
    /// Distinct uncompressed payload bytes.
    pub raw_bytes: u64,
    /// Actual persistent SQLite file length, including its indexes and allocated free pages.
    pub disk_bytes: u64,
    /// Additional current write-ahead log bytes, reported separately from steady-state capacity.
    pub wal_bytes: u64,
    /// Indexed raw old-format snapshot bytes, counted in the profile budget.
    pub legacy_bytes: u64,
    /// Old raw bytes durably selected for deletion but not yet physically reclaimed.
    pub pending_gc_bytes: u64,
}

/// Explicit user-selected history deletion scope; it never authorizes deleting project files.
#[derive(Clone, Debug)]
pub enum ClearScope {
    /// All retained history in this profile.
    All,
    /// One exact timeline item.
    Version(VersionId),
    /// All historical paths of one logical file.
    File(FileId),
    /// Only rows captured as members of this workspace.
    Workspace(PathBuf),
    /// One unassociated retired snapshot, identified by its validated relative bucket/name.
    Legacy(String),
}

/// Cleanup preview and execution result, excluding versions in active use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cleanup {
    /// Timeline rows selected for deletion.
    pub versions: u64,
    /// Raw old-format entries included in the same confirmed operation.
    pub legacy: u64,
    /// Compressed content bytes whose last reference would be removed; metadata is additional.
    pub reclaimable_bytes: u64,
    /// Durable metadata generation to reject confirmations after the selected history changes.
    pub epoch: i64,
}
