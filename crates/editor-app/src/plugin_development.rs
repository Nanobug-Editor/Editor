//! Built-in plugin development workflows share the runtime's project builder with the CLI.
mod cli;
pub(crate) mod configuration;
pub(crate) mod instance;
pub(crate) mod jobs;
#[cfg(test)]
mod tests;
pub(crate) use cli::run_cli;
use std::path::{Path, PathBuf};

/// One stable, private profile per workspace/configuration; never stores data in the project.
pub(crate) fn profile(workspace: &str, configuration: &str) -> anyhow::Result<PathBuf> {
    use sha2::{Digest, Sha256};
    let key = serde_json::to_vec(&(workspace, configuration))?;
    Ok(dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("User configuration directory unavailable"))?
        .join("MeEditor/plugin-development")
        .join(format!("{:x}", Sha256::digest(key))))
}
