//! One host-owned data root for settings, plugins, history and rebuildable caches.
//!
//! Ordinary installations store data directly in the executable directory. Explicit profiles retain their
//! isolation, and an unavailable installation path never falls back to an unrelated user store.

use std::{
    io,
    path::{Path, PathBuf},
};

/// Resolve the directory containing absolute `executable` without creating it.
/// A relative executable or a path without a parent returns `InvalidInput`.
pub fn beside_executable(executable: &Path) -> io::Result<PathBuf> {
    if !executable.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Executable path must be absolute",
        ));
    }
    let parent = executable.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Executable directory is unavailable",
        )
    })?;
    Ok(parent.to_path_buf())
}

/// Return this installation's default profile, independent of the working directory and overrides.
/// Failure to locate the running executable is returned to the caller.
pub fn default_profile_root() -> io::Result<PathBuf> {
    beside_executable(&std::env::current_exe()?)
}

/// Return the explicitly selected profile or the executable's own directory.
/// Relative `ME_EDITOR_PROFILE_HOME` values resolve once against the launching working directory.
pub fn profile_root() -> io::Result<PathBuf> {
    match std::env::var_os("ME_EDITOR_PROFILE_HOME").filter(|value| !value.is_empty()) {
        Some(root) => std::path::absolute(root),
        None => default_profile_root(),
    }
}

/// Return the selected profile's rebuildable cache directory without creating it.
pub fn cache_root() -> io::Result<PathBuf> {
    Ok(profile_root()?.join("cache"))
}

/// Create and return the selected profile's scratch directory for session-owned temporary files.
/// Creation failures propagate; callers never silently use the operating system's temporary store.
pub fn temporary_root() -> io::Result<PathBuf> {
    let root = cache_root()?.join("tmp");
    std::fs::create_dir_all(&root)?;
    Ok(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Portable roots follow the executable even when its directory contains spaces or Unicode.
    #[test]
    fn portable_root_is_beside_executable() {
        let directory = tempfile::tempdir().unwrap();
        let installed = directory.path().join("编辑器 application");
        assert_eq!(
            beside_executable(&installed.join("Nanobug.exe")).unwrap(),
            installed
        );
        assert!(
            !installed.exists(),
            "path selection must not create a profile"
        );
        assert!(beside_executable(Path::new("Nanobug.exe")).is_err());
    }
}
