//! Stable `storage.secrets` 1.0: system-protected values isolated by host profile and plugin identity.
#[cfg(feature = "guest")]
use crate::api::{ErrorCode, Failure};
use serde::{Deserialize, Serialize};

/// Keys select only the calling plugin's namespace; no target plugin or native path is accepted.
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    Read {
        key: String,
    },
    /// Between one and 2560 bytes. Updates are available only after activation commits.
    Write {
        key: String,
        bytes: Vec<u8>,
    },
    Delete {
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
pub fn delete(key: impl Into<String>) -> Result<(), Failure> {
    crate::api::guest::request(crate::api::Operation::Secrets {
        operation: Operation::Delete { key: key.into() },
    })
    .map(|_| ())
}
