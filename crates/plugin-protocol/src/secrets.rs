//! Stable `storage.secrets` 1.0: system-protected values isolated by host profile and plugin identity.
//!
//! Minimum host: wire protocol 7, `api.base = ^1`, and negotiated `storage.secrets = ^1`.
//! Installation separately grants `secrets`; this grants neither network authority nor another
//! plugin's namespace. Required missing capability rejects the package; optional absence disables
//! the feature or uses a plugin fallback, never plaintext credential files.
//!
//! With the `guest` feature, `read` returns None for a missing key, `write` replaces one key, and
//! `delete` is idempotent.
//! Keys contain 1–128 UTF-8 bytes without controls; values contain 1–2,560 bytes. Windows Generic
//! Credentials persist for the current OS user, isolated by canonical host profile, plugin ID and key.
//! No namespace enumeration or caller-selected foreign target is exposed. Non-Windows hosts return
//! `UnsupportedOperation`; an unavailable Windows facility returns `OperationFailed`, without fallback.
//!
//! Writes/deletion require committed activation. Preparation, migration, unpublished candidate
//! Snapshot and delegated services cannot mutate credentials. Delegated services cannot read provider
//! secrets either. Disable/trust revocation removes access but preserves stored keys for an authorized
//! later activation; explicit deletion removes them. Calls are synchronous and do not allocate handles;
//! completed OS mutations have no cancellation or private-file snapshot rollback.
//!
//! Missing negotiation/consent returns `CapabilityUnavailable`/`PermissionDenied`; malformed keys or
//! values return `InvalidRequest`; preactivation returns `InvalidState`. Treat returned bytes as
//! sensitive application data: use a password native input; do not place tokens in settings, logs,
//! views or snapshots, and do not forward provider secrets through public service calls. Debug omits
//! both keys and values; HTTP still independently checks the original caller's exact target grants.
#[cfg(feature = "guest")]
use crate::api::{ErrorCode, Failure};
use serde::{Deserialize, Serialize};

/// Keys select only the calling plugin's namespace; no target plugin or native path is accepted.
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    /// Return api::Value::Secret(Some(bytes)) or None from this caller's system namespace.
    Read {
        /// Nonempty UTF-8 key of at most 128 bytes without controls.
        key: String,
    },
    /// Between one and 2560 bytes. Updates are available only after activation commits.
    Write {
        /// Key in this plugin/profile namespace; no foreign target can be selected.
        key: String,
        /// One to 2,560 secret bytes, never part of ordinary file snapshots or Debug output.
        bytes: Vec<u8>,
    },
    /// Delete only this namespace's key; an absent key is already success.
    Delete {
        /// Nonempty UTF-8 key of at most 128 bytes without controls.
        key: String,
    },
}
impl std::fmt::Debug for Operation {
    /// Secret values and their identifying keys are absent from debug diagnostics.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretOperation(<redacted>)")
    }
}
#[cfg(feature = "guest")]
/// Read from the system facility. Missing keys return None; unsupported/unavailable storage is an error.
/// Requires the separate secrets grant; the returned bytes are sensitive and carry no network authority.
pub fn read(key: impl Into<String>) -> Result<Option<Vec<u8>>, Failure> {
    match crate::api::guest::request(crate::api::Operation::Secrets {
        operation: Operation::Read { key: key.into() },
    })? {
        crate::api::Value::Secret(value) => Ok(value),
        _ => Err(Failure::new(
            ErrorCode::InvalidRequest,
            "Unexpected secret result",
        )),
    }
}
#[cfg(feature = "guest")]
/// Replace a system-protected key; `secrets` consent grants no network authority. Never log these bytes.
/// Returns unit after a committed OS write; unpublished/delegated calls return InvalidState/PermissionDenied.
pub fn write(key: impl Into<String>, bytes: Vec<u8>) -> Result<(), Failure> {
    crate::api::guest::request(crate::api::Operation::Secrets {
        operation: Operation::Write {
            key: key.into(),
            bytes,
        },
    })
    .map(|_| ())
}
#[cfg(feature = "guest")]
/// Delete only this plugin's key; disabling revokes access but preserves persistent credentials.
/// Deletion is idempotent, requires committed activation, and cannot be undone through a private-data snapshot.
pub fn delete(key: impl Into<String>) -> Result<(), Failure> {
    crate::api::guest::request(crate::api::Operation::Secrets {
        operation: Operation::Delete { key: key.into() },
    })
    .map(|_| ())
}
