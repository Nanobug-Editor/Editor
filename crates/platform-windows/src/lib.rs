/// Reuse the core presentation rule across native adapters and runtime diagnostics.
pub use editor_core::{DisplayPath, display_path};
use editor_core::{DocumentError, DocumentStore};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use tempfile::NamedTempFile;

mod staged;
pub use staged::PreparedFileStore;
mod attachments;
pub use attachments::{NewWorkspaceFileStore, PreparedNewFile, create_document_attachment};
/// Bounded inert clipboard formats; enumeration never forces delayed rendering.
#[cfg(windows)]
pub mod clipboard;
/// Per-plugin protected credentials, separate from document storage and history.
pub mod credentials;
/// File-system identity for native project routing, separate from persistent display paths.
pub mod directory_identity;
/// Apartment-safe lazy OLE data and native drag lifecycle; no guest filesystem authority.
#[cfg(windows)]
pub mod drag;
pub mod file_picker;
/// User-profile paths and guarded parent creation for native history recovery.
pub mod history;
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
    }
}
