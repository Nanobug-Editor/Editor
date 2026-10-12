//! Recent project records are user-owned; removing a record never accesses the project's files.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Canonical identity is independent from its display name; list order records recency.
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Recent {
    pub(super) path: PathBuf,
}

/// Damaged storage is retained rather than overwritten by a fresh home window.
pub(super) struct RecentProjects {
    pub(super) entries: Vec<Recent>,
    path: PathBuf,
    pub(super) error: Option<String>,
}
impl RecentProjects {
    /// Read one explicit user profile, without scanning project directories.
    pub(super) fn load(profile: &Path) -> Self {
        let path = profile.join("recent-projects.json");
        let (entries, error) = match std::fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice::<Vec<Recent>>(&bytes) {
                Ok(entries) => (entries, None),
                Err(error) => (Vec::new(), Some(error.to_string())),
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (Vec::new(), None),
            Err(error) => (Vec::new(), Some(error.to_string())),
        };
        Self {
            entries,
            path,
            error,
        }
    }
    /// Commit atomically; failed writes leave visible list order and original storage untouched.
    fn replace(&mut self, entries: Vec<Recent>) -> anyhow::Result<()> {
        use std::io::Write as _;
        anyhow::ensure!(
            self.error.is_none(),
            "Recent projects are protected after a failed load"
        );
        let parent = self
            .path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Invalid profile path"))?;
        std::fs::create_dir_all(parent)?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        temporary.write_all(&serde_json::to_vec_pretty(&entries)?)?;
        temporary.as_file().sync_all()?;
        temporary.persist(&self.path).map_err(|error| error.error)?;
        self.entries = entries;
        Ok(())
    }
    /// Successful opens move one canonical identity to the front.
    pub(super) fn opened(&mut self, path: &Path) -> anyhow::Result<()> {
        let mut entries = self.entries.clone();
        entries.retain(|entry| entry.path != path);
        entries.insert(
            0,
            Recent {
                path: path.to_owned(),
            },
        );
        self.replace(entries)
    }
    /// Forget only a list record; settings, trust, history and project files remain owned elsewhere.
    pub(super) fn remove(&mut self, path: &Path) -> anyhow::Result<()> {
        self.replace(
            self.entries
                .iter()
                .filter(|entry| entry.path != path)
                .cloned()
                .collect(),
        )
    }
}
