//! Native transport ownership and pending configuration intent have distinct retirement boundaries.
use super::*;

/// Package, grants, original workspace and SDK seal an executor; effective values seal its next intent.
/// A configuration change may hide a native plan while preserving its owned transport for revalidation.
#[derive(Clone, PartialEq, Eq)]
pub(in crate::manager) struct Fingerprint {
    source: Vec<u8>,
    configuration: Vec<u8>,
    discovery: u64,
}
impl Fingerprint {
    /// Capture admitted inputs without exposing settings or inferring authority from a matching name.
    pub(in crate::manager) fn new(
        entry: &Installed,
        workspace: &str,
        values: &Result<plugin_protocol::settings::Effective, String>,
        sdk: &Option<Result<plugin_protocol::api::SdkDescriptor, String>>,
    ) -> Self {
        Self {
            source: serde_json::to_vec(&(&entry.digest, &entry.grants, workspace, sdk)).unwrap(),
            configuration: serde_json::to_vec(values).unwrap(),
            discovery: 0,
        }
    }

    /// Domain inputs can change independently from package bytes or user configuration.
    pub(in crate::manager) fn with_discovery_revision(mut self, revision: u64) -> Self {
        self.discovery = revision;
        self
    }

    /// This comparison permits retaining resources, never publishing a plan or admitting old intent.
    pub(in crate::manager) fn same_source(&self, other: &Self) -> bool {
        self.source == other.source && self.discovery == other.discovery
    }
}
