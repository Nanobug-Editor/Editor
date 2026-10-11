//! Stable `storage.private` 1.2: bounded directory operations and explicit persistent/cache/temporary roots.
//!
//! Minimum host: wire protocol 7, `api.base = ^1`, and negotiated `storage.private = ^1.2`.
//! Installation must separately grant `storage`. A missing required capability rejects the package;
//! a missing optional capability must disable the feature or use a plugin-owned fallback.
//! Directory operations require committed activation: Prepare, migration, configuration candidates,
//! Activate and unpublished Snapshot return `InvalidState`, even after staged writes have flushed.
//! Earlier `api::guest::open_data/read_file/write_file` retain their staged initialization semantics.
//!
//! With the `guest` feature, open an [`Area`] with `open` and use its instance/scope-owned handle with
//! `operate` or the
//! ordinary guest file helpers. Data and Cache persist outside the workspace and participate in
//! managed data replacement. Temporary is owned only by the committed incarnation which opens it:
//! closing its handle releases access, while retirement removes its bytes. Failed same-root candidates
//! cannot delete a live owner's Temporary area. Disable, trust revocation, restart and update retire
//! handles; recovery creates a new identity rather than restoring handles or external side effects.
//! Native cleanup failure is reported after access has already been revoked.
//!
//! Data cannot access the reserved `.temporary` child through file, directory or editor operations,
//! including Windows case, trailing-dot/space and existing 8.3 aliases. Obtain an Area::Temporary
//! handle instead. The same relative name remains ordinary content inside Cache or Workspace.
//! Data root listing may report the reserved name, but does not grant access to it.
//!
//! All areas share the manifest byte quota, 1 MiB per file, 1,024 total entries, 64 KiB of combined
//! entry-name bytes and depth 16 from the common Data root. Cache/Temporary reserve one depth level.
//! Paths are at most 4,096 bytes. Empty directories count; capacity is reserved before mutation.
//! Initialization permits at most 64 staged direct-child file writes. Absolute paths, traversal,
//! backslashes, Windows devices, alternate data streams, symlinks and reparse points are rejected.
//! Calls finish synchronously: cancellation does not undo an already completed native mutation.
//!
//! `CapabilityUnavailable` or `PermissionDenied` reports negotiation/consent failures. Malformed or
//! reserved paths return `InvalidPath`; missing entries return `NotFound`; a used destination returns
//! `Conflict`; quotas return `LimitExceeded`; retired/foreign handles return `InvalidHandle`; native
//! failures return `OperationFailed`. Release handles with `api::guest::close_resource`.
use crate::api::ResourceHandle;
use serde::{Deserialize, Serialize};

/// Persistent data survives disable; cache is disposable; temporary data is deleted on instance retirement.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Area {
    /// Persistent logical plugin files; the host's Temporary child is not ordinary Data authority.
    Data,
    /// Persistent but disposable cached files, sharing Data's byte/entry/depth quotas.
    Cache,
    /// Disposable bytes owned by the committed current incarnation until it retires.
    Temporary,
}
/// Metadata never exposes an absolute host path or another plugin's namespace.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    /// One relative entry name, excluding the parent directory.
    pub name: String,
    /// Whether this entry is a directory; symbolic links and reparse points are rejected.
    pub directory: bool,
    /// File byte length, or zero for a directory; enumeration never reads file content.
    pub bytes: u64,
}
/// Directory lists contain at most 128 records. Continue after the last returned name if has_more is true.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Listing {
    /// Records sorted by name, beginning strictly after the requested continuation name.
    pub entries: Vec<Entry>,
    /// Continue with the last returned name when more entries remain in this directory.
    pub has_more: bool,
}
/// Paths are relative slash-separated components; an empty path is permitted only for root listing.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    /// Return `api::Value::Resource`; requires committed activation and separate storage consent.
    Open {
        /// Select this caller's area; no native root or other plugin namespace is accepted.
        area: Area,
    },
    /// Return `api::Value::PrivateEntry` for one existing relative file or directory.
    Stat {
        /// An owned Data, Cache or Temporary handle.
        handle: ResourceHandle,
        /// A nonempty path relative to this area.
        path: String,
    },
    /// Return `api::Value::PrivateListing`, sorted by name, without reading file contents.
    List {
        /// An owned area handle; no workspace or selected-file handle is accepted.
        handle: ResourceHandle,
        /// Empty lists the area root; otherwise this must name an existing directory.
        path: String,
        /// Continue strictly after the last returned name; at most 255 bytes, without separators.
        after: Option<String>,
        /// From one to 128 entries; inspect Listing::has_more to continue.
        limit: u16,
    },
    /// Create one directory with an existing parent; return `api::Value::Unit`.
    CreateDirectory {
        /// An owned area handle under the common private quota.
        handle: ResourceHandle,
        /// Nonempty relative destination; recursive parent creation is not implicit.
        path: String,
    },
    /// Move within the same area without overwrite; return Unit, or Conflict if the destination exists.
    Rename {
        /// An owned area handle, which cannot be retargeted to another area.
        handle: ResourceHandle,
        /// Existing nonempty relative source; moving the area root is forbidden.
        path: String,
        /// Nonempty destination with an existing parent; descendant depths must still fit the quota.
        destination: String,
    },
    /// Recursive deletion applies only to this plugin's owned private root, never workspace data.
    Delete {
        /// An owned area handle; workspace data is never removed by this operation.
        handle: ResourceHandle,
        /// Existing nonempty relative entry; deleting the area root is forbidden.
        path: String,
        /// Explicitly authorize a whole directory subtree; false permits only files or empty directories.
        recursive: bool,
    },
}
#[cfg(feature = "guest")]
/// Open an owned area; existing read_file/write_file apply to these handles under the same total quota.
/// Returns a disposable handle after committed activation; see the module for grants, limits and errors.
pub fn open(area: Area) -> Result<ResourceHandle, crate::api::Failure> {
    match crate::api::guest::request(crate::api::Operation::Private {
        operation: Operation::Open { area },
    })? {
        crate::api::Value::Resource(handle) => Ok(handle),
        _ => Err(crate::api::Failure::new(
            crate::api::ErrorCode::InvalidRequest,
            "Unexpected private root",
        )),
    }
}
#[cfg(feature = "guest")]
/// Execute one bounded directory operation; values return typed metadata or a unit result.
/// This synchronous call preserves the selected handle's area; completed native mutations are not rolled back.
pub fn operate(operation: Operation) -> Result<crate::api::Value, crate::api::Failure> {
    crate::api::guest::request(crate::api::Operation::Private { operation })
}
