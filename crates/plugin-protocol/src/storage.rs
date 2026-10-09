//! Stable `storage.private` 1.2: bounded directory operations and explicit persistent/cache/temporary roots.
use crate::api::ResourceHandle;
use serde::{Deserialize, Serialize};

/// Persistent data survives disable; cache is disposable; temporary data is deleted on instance retirement.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Area {
    Data,
    Cache,
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
    Open {
        area: Area,
    },
    Stat {
        handle: ResourceHandle,
        path: String,
    },
    List {
        handle: ResourceHandle,
        path: String,
        after: Option<String>,
        limit: u16,
    },
    CreateDirectory {
        handle: ResourceHandle,
        path: String,
    },
    Rename {
        handle: ResourceHandle,
        path: String,
        destination: String,
    },
    /// Recursive deletion applies only to this plugin's owned private root, never workspace data.
    Delete {
        handle: ResourceHandle,
        path: String,
        recursive: bool,
    },
}
#[cfg(feature = "guest")]
/// Open an owned area; existing read_file/write_file apply to these handles under the same total quota.
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
pub fn operate(operation: Operation) -> Result<crate::api::Value, crate::api::Failure> {
    crate::api::guest::request(crate::api::Operation::Private { operation })
}
