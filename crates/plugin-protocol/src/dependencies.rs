//! Private dependency plans; native installation requires separate permission and concrete host consent.
//!
//! dependencies 1.3 accepts up to 256 MiB per source artifact and 512 MiB of extracted files
//! per ZIP (at most 10,000 entries). Streaming, hashing and extraction enforce the actual byte
//! counts, including cancellation. Older 1.2 hosts limit these to 128 MiB and 256 MiB;
//! consumers requiring the larger distribution budgets must declare a 1.3 minimum.
//!
//! dependencies 1.4 adds `Manifest::service_preparation_hook`. Before an admitted method's
//! asynchronous acquisition, `Notification::ServicePreparation` receives its immutable service
//! allowlist and effective settings. Return `Output::service_preparation` containing a unique subset;
//! undeclared IDs fail before acquisition. The bounded callback permits only granted readonly
//! observations, including tools.inspect; no downloads, native effects, UI or persisted writes.
//! Original caller authority, cancellation, exact instance and immutable cache leases remain intact.
use serde::{Deserialize, Serialize};

/// Artifacts form a bounded DAG. Paths start with an artifact ID, followed by a relative entry.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub artifacts: Vec<Artifact>,
    pub executable: String,
}

/// Immutable identity includes the version, platform and exact bytes, not a moving release label.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub id: String,
    pub version: String,
    /// Host OS and architecture, such as windows-x86_64; alternative platforms use different services.
    pub platform: String,
    pub sha256: String,
    pub source: Source,
    pub format: Format,
    #[serde(default)]
    pub requires: Vec<String>,
    /// Optional native step runs only after verification and explicit approval, before cache publication.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installer: Option<Installer>,
}

/// Paths are relative to the verified artifact; only whole `${target}`/`${source}` arguments expand.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Installer {
    pub program: String,
    pub args: Vec<String>,
    pub target: String,
    pub purpose: String,
    pub kind: InstallerKind,
}

/// Project toolchains require a separate affirmative choice beyond ordinary service installation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallerKind {
    Service,
    ProjectSdk,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Source {
    Url { url: String },
    Package { path: String },
    Local { path: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Format {
    Zip,
    File { path: String },
}
