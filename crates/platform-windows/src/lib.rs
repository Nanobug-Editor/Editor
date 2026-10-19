/// Reuse the core presentation rule across native adapters and runtime diagnostics.
pub use editor_core::{DisplayPath, display_path};
use editor_core::{DocumentError, DocumentStore};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use tempfile::NamedTempFile;
use thiserror::Error;

mod staged;
pub use staged::PreparedFileStore;
mod attachments;
pub use attachments::{NewWorkspaceFileStore, create_document_attachment};
/// Per-plugin protected credentials, separate from document storage and history.
pub mod credentials;
/// File-system identity for native project routing, separate from persistent display paths.
pub mod directory_identity;
pub mod file_picker;
/// Native window placement uses physical outer bounds and the current monitor's work area.
#[cfg(windows)]
pub mod window;

#[derive(Debug, Default, Clone, Copy)]
pub struct NativeFileStore;

impl DocumentStore for NativeFileStore {
    fn read_utf8(&self, path: &Path) -> Result<String, DocumentError> {
        fs::read_to_string(path).map_err(|source| DocumentError::Read {
            path: path.to_path_buf(),
            source,
        })
    }

    fn write_utf8(&self, path: &Path, contents: &str) -> Result<(), DocumentError> {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let mut temporary =
            NamedTempFile::new_in(parent).map_err(|source| DocumentError::Write {
                path: path.to_path_buf(),
                source,
            })?;
        temporary
            .write_all(contents.as_bytes())
            .and_then(|_| temporary.as_file().sync_all())
            .map_err(|source| DocumentError::Write {
                path: path.to_path_buf(),
                source,
            })?;
        temporary
            .persist(path)
            .map_err(|error| DocumentError::Write {
                path: path.to_path_buf(),
                source: error.error,
            })?;
        Ok(())
    }
}

/// Native history diagnostics format paths without changing their retry targets or I/O causes.
#[derive(Debug, Error)]
pub enum HistoryError {
    #[error("a local data directory is unavailable")]
    DataDirectoryUnavailable,
    #[error("failed to create history directory {}: {source}", display_path(.path))]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to read {}: {source}", display_path(.path))]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to write history snapshot {}: {source}", display_path(.path))]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Debug, Clone)]
pub struct LocalHistory {
    root: PathBuf,
}

impl LocalHistory {
    pub fn for_current_user() -> Result<Self, HistoryError> {
        // Development profiles retain history independently from the user's normal editor data.
        if let Some(root) =
            std::env::var_os("ME_EDITOR_PROFILE_HOME").filter(|root| !root.is_empty())
        {
            return Self::at(PathBuf::from(root).join("history"));
        }
        let root = dirs::data_local_dir()
            .ok_or(HistoryError::DataDirectoryUnavailable)?
            .join("MeEditor")
            .join("history");
        Self::at(root)
    }

    pub fn at(root: impl Into<PathBuf>) -> Result<Self, HistoryError> {
        let root = root.into();
        fs::create_dir_all(&root).map_err(|source| HistoryError::CreateDirectory {
            path: root.clone(),
            source,
        })?;
        Ok(Self { root })
    }

    pub fn snapshot_file(&self, source_path: &Path) -> Result<PathBuf, HistoryError> {
        let contents = fs::read(source_path).map_err(|source| HistoryError::Read {
            path: source_path.to_path_buf(),
            source,
        })?;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let safe_name = source_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("document")
            .replace(
                |character: char| !character.is_ascii_alphanumeric() && character != '.',
                "_",
            );
        let directory = self
            .root
            .join(format!("{:016x}", stable_path_hash(source_path)));
        fs::create_dir_all(&directory).map_err(|source| HistoryError::CreateDirectory {
            path: directory.clone(),
            source,
        })?;
        let destination = directory.join(format!("{timestamp}-{safe_name}"));
        fs::write(&destination, contents).map_err(|source| HistoryError::Write {
            path: destination.clone(),
            source,
        })?;
        Ok(destination)
    }
}

fn stable_path_hash(path: &Path) -> u64 {
    path.to_string_lossy()
        .bytes()
        .fold(0xcbf29ce484222325, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Native read errors expose a readable target while retaining the exact canonical I/O path.
    #[cfg(windows)]
    #[test]
    fn native_path_errors_display_without_verbatim_prefixes() {
        let directory = tempfile::tempdir().unwrap();
        let canonical = directory.path().canonicalize().unwrap();
        let target = canonical.join("missing 中文.txt");
        let error = NativeFileStore.read_utf8(&target).unwrap_err();
        let message = error.to_string();
        assert!(
            !message.contains(r"\\?\"),
            "native read error leaked its path: {message}"
        );
        assert!(message.contains(&display_path(&target).to_string()));
        let DocumentError::Read { path, .. } = error else {
            panic!("the native read must retain its typed target and cause");
        };
        assert_eq!(path, target);

        let blocker = canonical.join("file.txt");
        fs::write(&blocker, "not a directory").unwrap();
        let target = blocker.join("history");
        let error = LocalHistory::at(&target).unwrap_err();
        let message = error.to_string();
        assert!(
            !message.contains(r"\\?\"),
            "history error leaked its path: {message}"
        );
        let HistoryError::CreateDirectory { path, .. } = error else {
            panic!("history creation must retain its typed target and cause");
        };
        assert_eq!(path, target);
    }

    #[test]
    fn snapshot_is_stored_outside_the_project() {
        let project = tempfile::tempdir().unwrap();
        let history = tempfile::tempdir().unwrap();
        let source = project.path().join("main.rs");
        fs::write(&source, "fn main() {}").unwrap();

        let destination = LocalHistory::at(history.path())
            .unwrap()
            .snapshot_file(&source)
            .unwrap();

        assert!(destination.starts_with(history.path()));
        assert_eq!(fs::read_to_string(destination).unwrap(), "fn main() {}");
    }
}
