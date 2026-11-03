//! Built-in plugin development workflows share the runtime's project builder with the CLI.
mod check;
mod cli;
pub(crate) mod configuration;
pub(crate) mod instance;
pub(crate) mod jobs;
#[cfg(test)]
mod native_report_tests;
pub(crate) mod report;
#[cfg(test)]
mod tests;
pub(crate) use cli::run_cli;
use std::path::{Path, PathBuf};

/// One stable development profile per workspace/configuration beneath the selected editor data root.
pub(crate) fn profile(workspace: &str, configuration: &str) -> anyhow::Result<PathBuf> {
    use sha2::{Digest, Sha256};
    let key = serde_json::to_vec(&(workspace, configuration))?;
    Ok(editor_core::storage::profile_root()?
        .join("plugin-development")
        .join(format!("{:x}", Sha256::digest(key))))
}
