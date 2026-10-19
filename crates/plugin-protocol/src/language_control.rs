//! Bounded native language control shares the selected provider's existing stdio connection.
//!
//! `language.control 1.0` never exposes a process handle or an arbitrary LSP method. A workspace
//! guest addresses one of its own declared language providers and a document context. The host
//! validates the provider's method schema, prepares that exact context and forwards the request
//! on the same connection used by native language features. Source revisions and context/owner
//! lifetimes continue to guard delivery; ordinary document synchronization stays host-owned.
use crate::{api, language};
use serde::{Deserialize, Serialize};

/// Most UTF-8 bytes in one control request or result, independent of the source-document stream.
pub const MAX_CONTROL_BYTES: usize = 64 * 1024;

/// The host sends `{consumers:[instance_id,...]}` before further requests when original callers
/// retire. A negotiated native control worker releases their opaque context references idempotently;
/// this notification conveys no new caller identity or authority over a still-live consumer.
/// A batch contains at most 128 bounded IDs and uses the existing owned LSP pipe.
pub const CONSUMERS_RETIRED_METHOD: &str = "nanobug/controlConsumersRetired";

/// Strict optional JSON-RPC `error.data` receipt for the matching native control request.
/// A worker may send this only after this request's new imports are no longer retained by any
/// live managed worker. Code may already have executed; no external side-effect rollback is implied.
/// Missing, malformed or future receipts communicate no such guarantee, including on cancellation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeControlFailureData {
    /// Native control receipt version, independently checked before it can release any authority.
    pub version: u32,
    /// Disposition of only this request's newly borrowed imports, never prior committed imports.
    pub imports: NativeImportDisposition,
}

/// Absence of an understood disposition must conservatively retain possibly loaded code authority.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeImportDisposition {
    /// No still-live managed worker retains the imports newly introduced by this failed request.
    Released,
}

impl NativeControlFailureData {
    /// Future versions are deliberately not interpreted as permission to release loaded code.
    pub fn valid(&self) -> bool {
        self.version == language::CONTROL_VERSION
    }
}

/// This typed context preserves the original structured Failure as the root error for callers.
impl std::fmt::Display for NativeControlFailureData {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Native control imports were released from live managed workers")
    }
}
impl std::error::Error for NativeControlFailureData {}

/// Host-sealed source identity synchronized before a context control request is dispatched.
/// Consumers use `document` as an optional public precondition; they never have to infer the
/// transport-local `version`. This vector is native metadata, not guest-supplied arguments.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceMetadata {
    /// Original open-document incarnation, workspace-relative path and source revision.
    pub document: api::DocumentVersion,
    /// Exact URI on the selected native connection, including any host-owned URI translation.
    pub uri: String,
    /// Version on that connection after synchronization; it is independent of source revision.
    pub version: i32,
}

/// Canonical metadata-only prepare signature for a delegated language provider.
/// A WASM provider may perform asynchronous public collaboration and return a host-issued token;
/// neither source text nor an arbitrary program/path is accepted as the returned binding.
pub fn binding_prepare_method() -> crate::service::Method {
    use crate::service::Schema;
    use std::collections::BTreeMap;
    let record = |fields: BTreeMap<String, Schema>| Schema::Record {
        fields,
        optional: Default::default(),
    };
    let document = record(BTreeMap::from([
        ("path".into(), Schema::String { max_bytes: 4096 }),
        (
            "generation".into(),
            Schema::Integer {
                min: 0,
                max: 9_007_199_254_740_991,
            },
        ),
    ]));
    crate::service::Method {
        parameters: record(BTreeMap::from([("document".into(), document)])),
        result: record(BTreeMap::from([(
            "binding".into(),
            Schema::String { max_bytes: 128 },
        )])),
        permissions: ["editor.read", "workspace.read", "native.modules"]
            .into_iter()
            .map(String::from)
            .collect(),
    }
}

/// A sealed, self-contained package directory loaded only in a separately managed native worker.
/// `language.modules 1.0` and the `native.modules` installation permission are both required.
/// The host verifies every neighbor in `files`; adapters must not discover project tool paths.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleExport {
    /// Package-relative immutable directory, never a private data or workspace directory.
    pub root: String,
    /// Entry relative to `root`; its bytes must also appear in `files`.
    pub entry: String,
    /// Lowercase SHA-256 of the entry; it pins code independently of the package version.
    pub sha256: String,
    /// Complete root-relative file closure, including runtime metadata and sibling dependencies.
    pub files: std::collections::BTreeMap<String, String>,
    /// Adapter interface identity interpreted by the receiving plugin, at most 128 bytes.
    pub abi: String,
    /// Named engine ranges checked by the worker before loading, not by host language branches.
    pub engine: std::collections::BTreeMap<String, semver::VersionReq>,
}

/// An import borrows only an admitted exporter in the original logical workspace.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Import {
    /// Manifest-local export name. It never names an arbitrary package directory or file.
    pub export: String,
    /// An existing service reference selects its exact live exporter. Omission selects the sealed
    /// immediate caller (or this owner for a direct call); guest identity strings are never accepted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<api::ResourceHandle>,
}

/// A previously opened service collection is an input to this native preparation, including empty sets.
/// Requires plugin.services 1.2 and the caller's explicit collection dependency. The host checks its
/// owned OpenSet record; a guessed revision cannot lend another instance's discovery authority.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceSetDependency {
    pub contract: String,
    pub revision: u64,
}

/// One provider-owned control operation; plugin identities and native paths are never routing input.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    /// Manifest-local provider identifier owned by the calling package.
    pub provider: String,
    /// Original workspace-relative document and host configuration generation. Zero requests the
    /// current host-issued generation; a positive hint must match, never advance, that generation.
    pub document: language::DocumentContext,
    /// Explicitly declared, non-reserved control method, such as `typescript/project/resolve`.
    pub method: String,
    /// Inert values checked against the provider's bounded public method schema.
    pub arguments: serde_json::Value,
    /// Opt in to the provider's declared related-source graph and native revision-vector guard.
    /// Metadata-only resolve/attach calls leave this false: attachment can legitimately change
    /// the graph it is preparing. True requires an open native source, related-source support and
    /// workspace.read from both the provider and original call chain, even for weaker method schemas.
    #[serde(default)]
    pub context_snapshot: bool,
    /// Membership/configuration changes revoke this request, its binding and every imported module.
    /// Up to 16 distinct contracts may narrow authority; no additional access is granted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub service_sets: Vec<ServiceSetDependency>,
    /// Immutable export selectors; the host issues consumer-bound leases before native delivery.
    /// Empty for metadata/analysis operations that do not attach a native module.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub imports: Vec<Import>,
    /// After a successful declared module-attachment operation, issue an opaque binding for the
    /// immediate caller's exact document intent. Only a matching delegated provider may consume it.
    /// Source text and standard LSP messages continue on the existing connection, outside this envelope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publish_binding: Option<language::DocumentContext>,
}

impl Request {
    /// Reject malformed contexts, reserved transport commands and oversized control envelopes.
    /// This checks shape only: the runtime separately enforces ownership, capability and grants.
    pub fn valid(&self) -> bool {
        !self.provider.is_empty()
            && self.provider.len() <= 100
            && self.provider.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
            })
            && self.document.valid()
            && language::valid_control_method(&self.method)
            && self.imports.len() <= 16
            && self.service_sets.len() <= 16
            && self.service_sets.iter().all(|set| {
                !set.contract.is_empty() && set.contract.len() <= 128 && set.revision > 0
            })
            && self
                .service_sets
                .iter()
                .map(|set| &set.contract)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                == self.service_sets.len()
            && self
                .imports
                .iter()
                .all(|import| valid_export_id(&import.export))
            && self
                .publish_binding
                .as_ref()
                .is_none_or(|document| document.valid() && document.path == self.document.path)
            && !self
                .arguments
                .as_object()
                .is_some_and(|args| args.contains_key("_nanobug"))
            && serde_json::to_vec(self).is_ok_and(|value| value.len() <= MAX_CONTROL_BYTES)
    }
}

/// Export names are local metadata keys, not paths or globally selected plugin identities.
pub fn valid_export_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b))
}

#[cfg(test)]
mod receipt_tests {
    use super::*;
    use serde_json::json;

    /// Only an explicit current receipt can release imports; future or ambiguous failure data
    /// remains an ordinary business error whose possible code lifetime must stay authorized.
    #[test]
    fn native_import_release_receipts_require_the_exact_current_shape() {
        let receipt: NativeControlFailureData =
            serde_json::from_value(json!({"version":1,"imports":"released"})).unwrap();
        assert!(receipt.valid());
        assert_eq!(receipt.imports, NativeImportDisposition::Released);
        for data in [
            json!(null),
            json!({"imports":"released"}),
            json!({"version":1}),
            json!({"version":2,"imports":"released"}),
            json!({"version":1,"imports":"loaded"}),
            json!({"version":1,"imports":"released","reason":"extra"}),
            json!({"nanobugControl":{"version":1,"imports":"released"}}),
        ] {
            assert!(
                !serde_json::from_value::<NativeControlFailureData>(data.clone())
                    .is_ok_and(|receipt| receipt.valid()),
                "ambiguous release receipt: {data}"
            );
        }
    }
}
