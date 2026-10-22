//! Static checks return stable, payload-free diagnostics from the production Package validator.
use crate::Package;
use plugin_protocol::api::{Negotiated, NegotiationFailure, NegotiationKind};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{io::Read, path::Path};

/// Machine-readable diagnostic categories; messages are not used to infer these codes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Code {
    /// The requested input could not be read; no source path is exported.
    InputIo,
    /// Archive structure or compression is invalid.
    Archive,
    /// A package/archive expansion or entry count exceeds the supported limit.
    Limit,
    /// A package entry violates the canonical relative-path policy.
    Path,
    /// Manifest bytes or a version field cannot be decoded.
    Manifest,
    /// The author requires an unsupported host release.
    HostVersion,
    /// The executable wire protocol is no longer accepted.
    Protocol,
    /// A required capability/range cannot be negotiated.
    RequiredCapability,
    /// A contribution uses semantics newer than its declared minimum capability version.
    CapabilityFloor,
    /// A declaration requires missing permissions, or requests an unsupported permission.
    Permission,
    /// An optional capability is absent; this warning is intentional degradation.
    OptionalUnavailable,
    /// A referenced package resource is absent.
    MissingResource,
    /// A rejection not yet covered by a typed origin; coverage remains explicit.
    Unclassified,
}

/// A safe typed cause retained inside existing anyhow errors, including typed context values.
/// Use `anyhow::Error::downcast_ref` to recover it; std error-chain iteration omits context values.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Diagnostic {
    /// The production validation phase; static checks never imply runtime evaluation.
    pub stage: String,
    /// Host-authored explanation, never text copied from an error chain or package body.
    pub message: String,
    /// Stable machine category, independent from localized text.
    pub code: Code,
    /// Error blocks admission; warning is a declared optional degradation.
    pub severity: String,
    /// Package-local file when this is known at the rejecting validator.
    pub file: Option<String>,
    /// JSON Pointer when the field is known without inspecting an error message.
    pub pointer: Option<String>,
    /// Exact decoder line and column when available, never guessed.
    pub location: Option<(usize, usize)>,
    /// Required version/range if the rejecting validator supplies one.
    pub required: Option<String>,
    /// Available version if the capability or host exists.
    pub available: Option<String>,
    /// A rejected range can itself be oversized; its digest identifies omitted metadata.
    #[serde(default)]
    pub required_sha256: Option<String>,
    /// Digest of an omitted available version, without exporting arbitrary version text.
    #[serde(default)]
    pub available_sha256: Option<String>,
    /// Exported fields were omitted to keep reports bounded; admission policy is unchanged.
    #[serde(default)]
    pub metadata_partial: bool,
}
impl Diagnostic {
    /// Bound every version origin at the export boundary, including older host-floor validators.
    fn bound_metadata(&mut self) {
        for (value, digest) in [
            (&mut self.required, &mut self.required_sha256),
            (&mut self.available, &mut self.available_sha256),
        ] {
            if let Some(input) = value.take() {
                let metadata = bounded_metadata(&input);
                self.metadata_partial |= metadata.sha256.is_some();
                *value = metadata.value;
                *digest = metadata.sha256;
            }
        }
    }
    /// A declaration uses a capability feature introduced at `minimum`, without opting into that floor.
    pub(crate) fn floor(capability: &str, minimum: &str) -> Self {
        let pointer = safe_capability_pointer("required", capability);
        let mut issue = Self::new(Code::CapabilityFloor, pointer.as_deref());
        issue.required = Some(format!(">={minimum}"));
        issue
    }
    /// Construct a rejecting `code` at a constant package field; no raw error text is retained.
    pub(crate) fn new(code: Code, pointer: Option<&str>) -> Self {
        Self {
            stage: "package_admission".into(),
            message: code.description().into(),
            code,
            severity: "error".into(),
            file: pointer.map(|_| "manifest.json".into()),
            pointer: pointer.map(str::to_owned),
            location: None,
            required: None,
            available: None,
            required_sha256: None,
            available_sha256: None,
            metadata_partial: false,
        }
    }
}
impl std::fmt::Display for Diagnostic {
    /// Existing anyhow callers retain a readable, safe rejection without parsing it.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code.description())?;
        if let Some(pointer) = &self.pointer {
            write!(f, " at {pointer}")?;
        }
        if let Some(required) = &self.required {
            write!(f, ": requires {required}")?;
        }
        Ok(())
    }
}
impl Code {
    /// Closed descriptions are safe to serialize even when the rejected input was malicious.
    fn description(self) -> &'static str {
        match self {
            Code::HostVersion => "Unsupported minimum host version",
            Code::Protocol => "Unsupported plugin protocol; update the plugin",
            Code::Path => "Unsafe package path",
            Code::Limit => "Package resource quota exceeded",
            Code::Manifest => "Invalid plugin manifest",
            Code::Permission => "Missing or unsupported permission",
            Code::CapabilityFloor => "A capability requires an explicit minimum version",
            Code::OptionalUnavailable => {
                "Optional capability unavailable; fallback remains allowed"
            }
            Code::RequiredCapability => "Required API or capability unavailable",
            Code::InputIo => "Could not read the package input",
            Code::Archive => "Invalid package archive",
            Code::MissingResource => "Referenced package resource is missing",
            Code::Unclassified => {
                "A production validator rejected the package without a typed diagnosis"
            }
        }
    }
}
impl std::error::Error for Diagnostic {}

/// Report metadata has a separate budget from valid package declarations.
pub(crate) struct BoundedMetadata {
    /// Complete printable value, absent when it exceeds the export budget.
    pub value: Option<String>,
    /// Original-byte identity for an omitted value; never a digest of truncated text.
    pub sha256: Option<String>,
}
/// Keep small printable identity fields; omission carries an exact digest rather than truncation.
pub(crate) fn bounded_metadata(value: &str) -> BoundedMetadata {
    if value.len() <= 128 && !value.chars().any(char::is_control) {
        BoundedMetadata {
            value: Some(value.into()),
            sha256: None,
        }
    } else {
        BoundedMetadata {
            value: None,
            sha256: Some(format!("{:x}", Sha256::digest(value.as_bytes()))),
        }
    }
}

/// Preserve the production admission phase even when its precise rejecting origin is not typed yet.
/// Human callers retain their existing text; structured exports read only the safe typed context.
pub(crate) fn admission_error(error: anyhow::Error) -> anyhow::Error {
    if error.downcast_ref::<Diagnostic>().is_some()
        || error.downcast_ref::<NegotiationFailure>().is_some()
    {
        error
    } else {
        let message = error.to_string();
        error
            .context(Diagnostic::new(Code::Unclassified, None))
            .context(message)
    }
}

/// The exact checker binary/SDK identity; absent values mean unavailable, never the current defaults.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ToolIdentity {
    /// Human release version, insufficient by itself to identify an executable.
    pub version: String,
    /// Checker executable SHA-256.
    pub executable_sha256: Option<String>,
    /// Embedded public SDK content digest.
    pub sdk_sha256: Option<String>,
}

/// Stable report shared by the CLI and author CI; success certifies only static admission.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckReport {
    /// Report schema version, separate from plugin transport and capability versions.
    pub schema_version: u32,
    /// All performed static checks passed; dynamic behavior remains untested.
    pub passed: bool,
    /// Always false for this static entry point.
    pub executed: bool,
    /// Exact checker identity.
    pub tool: ToolIdentity,
    /// Exact ZIP hash, also available for malformed ZIP bytes within the input bound.
    pub archive_sha256: Option<String>,
    /// Production Package digest, only present after successful admission.
    pub package_digest: Option<String>,
    /// Admitted manifest identity, deliberately absent for rejected/untrusted parse results.
    pub plugin: Option<String>,
    /// Admitted package version.
    pub version: Option<String>,
    /// Exact version digest when an otherwise valid identity exceeds the report field budget.
    #[serde(default)]
    pub version_sha256: Option<String>,
    /// Static admission still completes when only printable metadata is omitted.
    #[serde(default)]
    pub metadata_partial: bool,
    /// Admitted transport protocol; unavailable when admission rejected the package.
    pub protocol: Option<u32>,
    /// Host negotiation does not grant runtime permissions.
    pub negotiated: Option<Negotiated>,
    /// Safe diagnostics and deliberate optional degradation.
    pub diagnostics: Vec<Diagnostic>,
    /// The first rejecting stage, or None when static admission completed.
    pub stopped_at: Option<String>,
    /// Explicit limits on what this command claims to have tested.
    pub coverage: Vec<String>,
}

/// Check `path` without builds, network, extraction, installation or WASM instantiation.
/// Reads at most 64 MiB plus one sentinel byte, even if a file grows after its metadata was read.
pub fn check(path: &Path, tool: ToolIdentity) -> CheckReport {
    let read = (|| -> std::io::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        std::fs::File::open(path)?
            .take(64 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        Ok(bytes)
    })();
    match read {
        Ok(bytes) => check_bytes(&bytes, tool),
        Err(_) => failed(tool, None, Diagnostic::new(Code::InputIo, None)),
    }
}

/// Inspect immutable archive `bytes` using the same Package gate as installations.
pub fn check_bytes(bytes: &[u8], tool: ToolIdentity) -> CheckReport {
    if bytes.len() > 64 * 1024 * 1024 {
        return failed(tool, None, Diagnostic::new(Code::Limit, None));
    }
    let digest = Some(format!("{:x}", Sha256::digest(bytes)));
    match Package::from_bytes(bytes) {
        Ok(package) => {
            let version = bounded_metadata(&package.manifest.version);
            let negotiated = crate::capabilities::negotiate(&package.manifest).ok();
            let mut diagnostics = Vec::new();
            if let (Some(requirements), Some(api)) = (&package.manifest.api, &negotiated) {
                for id in requirements
                    .optional
                    .keys()
                    .filter(|id| !api.capabilities.contains_key(*id))
                {
                    let mut issue = Diagnostic::new(
                        Code::OptionalUnavailable,
                        safe_capability_pointer("optional", id).as_deref(),
                    );
                    issue.severity = "warning".into();
                    diagnostics.push(issue);
                }
            }
            CheckReport {
                schema_version: 1,
                passed: true,
                executed: false,
                tool,
                archive_sha256: digest,
                package_digest: Some(package.digest),
                plugin: Some(package.manifest.id),
                metadata_partial: version.sha256.is_some(),
                version: version.value,
                version_sha256: version.sha256,
                protocol: Some(package.manifest.protocol),
                negotiated,
                diagnostics,
                stopped_at: None,
                coverage: coverage(),
            }
        }
        Err(error) => {
            let diagnostic = error
                .downcast_ref::<Diagnostic>()
                .cloned()
                .or_else(|| {
                    error
                        .downcast_ref::<NegotiationFailure>()
                        .map(negotiation_diagnostic)
                })
                .unwrap_or_else(|| Diagnostic::new(Code::Unclassified, None));
            failed(tool, digest, diagnostic)
        }
    }
}

/// Translate a typed negotiation cause without depending on its Display wording.
fn negotiation_diagnostic(error: &NegotiationFailure) -> Diagnostic {
    let code = match error.kind {
        NegotiationKind::Required | NegotiationKind::Base => Code::RequiredCapability,
        NegotiationKind::Limit => Code::Limit,
        _ => Code::Manifest,
    };
    let pointer = error
        .capability
        .as_deref()
        .and_then(|id| safe_capability_pointer("required", id));
    let mut issue = Diagnostic::new(code, pointer.as_deref().or(Some("/api")));
    issue.required = error.required.as_ref().map(ToString::to_string);
    issue.available = error.available.as_ref().map(ToString::to_string);
    issue
}
/// Use only schema-like capability names in field pointers; arbitrary declarations are not exported.
fn safe_capability_pointer(kind: &str, id: &str) -> Option<String> {
    (!id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte)))
    .then(|| format!("/api/{kind}/{id}"))
}
/// Failed reports omit raw paths, strings and chained error messages supplied by untrusted packages.
fn failed(tool: ToolIdentity, digest: Option<String>, mut issue: Diagnostic) -> CheckReport {
    issue.bound_metadata();
    CheckReport {
        schema_version: 1,
        passed: false,
        executed: false,
        tool,
        archive_sha256: digest,
        package_digest: None,
        plugin: None,
        version: None,
        version_sha256: None,
        metadata_partial: issue.metadata_partial,
        protocol: None,
        negotiated: None,
        stopped_at: Some("package_admission".into()),
        diagnostics: vec![issue],
        coverage: coverage(),
    }
}
/// Static shape and resource declarations are covered; dynamic calls are deliberately not certified.
fn coverage() -> Vec<String> {
    [
        "production_package_admission",
        "no_execution",
        "dynamic_calls_not_exercised",
        "runtime_grants_not_evaluated",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}
