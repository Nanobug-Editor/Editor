//! Profile-owned startup preferences and the last successfully closed project.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests;

/// Startup policy is user-scoped; a project cannot grant itself automatic reopening.
#[derive(Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Preferences {
    /// Reopen the saved project on an ordinary primary launch without an explicit target.
    pub(crate) restore_last_project: bool,
    /// Canonical root of the last closed project; empty windows leave this record intact.
    pub(crate) last_project: Option<PathBuf>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            restore_last_project: true,
            last_project: None,
        }
    }
}

impl Preferences {
    /// Read `profile`'s startup policy, defaulting only when absent and preserving damaged data.
    pub(crate) fn load(profile: &Path) -> anyhow::Result<Self> {
        match std::fs::read(profile.join("startup.json")) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error.into()),
        }
    }

    /// Atomically save this policy inside `profile`; a failed write retains the previous file.
    pub(crate) fn save(&self, profile: &Path) -> anyhow::Result<()> {
        use std::io::Write as _;
        std::fs::create_dir_all(profile)?;
        let mut temporary = tempfile::NamedTempFile::new_in(profile)?;
        temporary.write_all(&serde_json::to_vec_pretty(self)?)?;
        temporary.as_file().sync_all()?;
        temporary
            .persist(profile.join("startup.json"))
            .map_err(|error| error.error)?;
        Ok(())
    }
}

/// Resolve a primary launch in `profile`; explicit targets always win, missing saved folders stay empty.
/// Call after single-instance admission so a secondary no-target launch still activates its live owner.
pub(crate) fn target(profile: &Path, explicit: Option<PathBuf>) -> Option<PathBuf> {
    explicit.or_else(|| {
        let preferences = Preferences::load(profile).ok()?;
        preferences
            .last_project
            .filter(|path| preferences.restore_last_project && path.is_absolute() && path.is_dir())
    })
}

/// Retain a successfully closed canonical `project` in `profile`, independently of the restore switch.
/// Reload before writing so another window's latest preference is preserved; errors never overwrite it.
pub(crate) fn remember_project(profile: &Path, project: &Path) -> anyhow::Result<()> {
    let mut preferences = Preferences::load(profile)?;
    preferences.last_project = Some(project.to_owned());
    preferences.save(profile)
}
