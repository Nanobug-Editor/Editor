//! Portable updates validate the complete payload and retain rollback copies of replaced files.

use anyhow::{Context, Result, ensure};
use std::{
    collections::HashSet,
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};

/// Only distribution-owned files may be replaced; user files and plugin stores are outside this set.
fn payload_path(name: &str) -> Result<PathBuf> {
    ensure!(!name.is_empty() && !name.contains('\\'), "Invalid ZIP path");
    let parts: Vec<_> = name.trim_end_matches('/').split('/').collect();
    ensure!(
        parts.iter().all(|part| {
            !part.is_empty()
                && *part != "."
                && *part != ".."
                && !part.ends_with(['.', ' '])
                && !part
                    .chars()
                    .any(|ch| ch.is_control() || ":*?\"<>|".contains(ch))
                && !matches!(
                    part.split('.')
                        .next()
                        .unwrap_or("")
                        .to_ascii_uppercase()
                        .as_str(),
                    "CON"
                        | "PRN"
                        | "AUX"
                        | "NUL"
                        | "COM1"
                        | "COM2"
                        | "COM3"
                        | "COM4"
                        | "COM5"
                        | "COM6"
                        | "COM7"
                        | "COM8"
                        | "COM9"
                        | "LPT1"
                        | "LPT2"
                        | "LPT3"
                        | "LPT4"
                        | "LPT5"
                        | "LPT6"
                        | "LPT7"
                        | "LPT8"
                        | "LPT9"
                )
        }),
        "Unsafe ZIP path"
    );
    ensure!(
        matches!(
            parts.as_slice(),
            ["Nanobug.exe" | "Nanobug.ico" | "build-info.json" | "INTERNAL-TEST.txt"]
        ) || parts.first() == Some(&"licenses"),
        "ZIP contains a file outside the host distribution"
    );
    Ok(parts.iter().collect())
}

/// Reject symlinks and Windows reparse points before following a destination component.
fn ordinary(path: &Path) -> Result<()> {
    if let Ok(metadata) = fs::symlink_metadata(path) {
        ensure!(
            !metadata.file_type().is_symlink(),
            "Update path is a symlink"
        );
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt as _;
            ensure!(
                metadata.file_attributes() & 0x400 == 0,
                "Update path is a reparse point"
            );
        }
    }
    Ok(())
}

/// Extract only bounded regular files into a fresh private staging tree, returning their relative paths.
fn unpack(package: &Path, destination: &Path) -> Result<Vec<PathBuf>> {
    let mut zip = zip::ZipArchive::new(File::open(package)?)?;
    ensure!(zip.len() <= 512, "Too many update ZIP entries");
    let mut seen = HashSet::new();
    let mut files = Vec::new();
    let mut total = 0_u64;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index)?;
        let relative = payload_path(entry.name())?;
        ensure!(
            seen.insert(relative.to_string_lossy().to_ascii_lowercase()),
            "Duplicate ZIP path"
        );
        ensure!(
            entry
                .unix_mode()
                .is_none_or(|mode| mode & 0o170000 != 0o120000),
            "ZIP symlinks are forbidden"
        );
        let target = destination.join(&relative);
        if entry.is_dir() {
            fs::create_dir_all(target)?;
            continue;
        }
        total = total
            .checked_add(entry.size())
            .context("ZIP size overflow")?;
        ensure!(
            entry.size() <= 512 * 1024 * 1024 && total <= 2 * 1024 * 1024 * 1024,
            "Update ZIP exceeds extraction limit"
        );
        fs::create_dir_all(target.parent().context("Missing ZIP parent")?)?;
        let mut output = File::create(&target)?;
        let expected = entry.size();
        let actual = std::io::copy(&mut (&mut entry).take(expected + 1), &mut output)?;
        ensure!(actual == expected, "Invalid ZIP file size");
        output.sync_all()?;
        files.push(relative);
    }
    ensure!(
        files.iter().any(|path| path == Path::new("Nanobug.exe")),
        "ZIP has no Nanobug executable"
    );
    ensure!(
        files.iter().any(|path| path.starts_with("licenses")),
        "ZIP has no distribution licenses"
    );
    Ok(files)
}

/// Copy through a same-directory temporary file so a replacement exposes complete bytes atomically.
fn replace(source: &Path, destination: &Path) -> Result<()> {
    let parent = destination
        .parent()
        .context("Update destination has no parent")?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    std::io::copy(&mut File::open(source)?, &mut temporary)?;
    temporary.flush()?;
    temporary.as_file().sync_all()?;
    temporary
        .persist(destination)
        .map_err(|error| error.error)?;
    Ok(())
}

/// Replace a validated ZIP after editor processes exit; restore every earlier replacement on failure.
/// `directory` is the original portable app directory and `job` owns staging/rollback data.
pub(super) fn apply(package: &Path, directory: &Path, job: &Path) -> Result<()> {
    ordinary(directory)?;
    let payload = tempfile::Builder::new()
        .prefix("payload-")
        .tempdir_in(job)?;
    let files = unpack(package, payload.path())?;
    let backup = tempfile::Builder::new()
        .prefix("rollback-")
        .tempdir_in(job)?;
    let mut replaced: Vec<(PathBuf, bool)> = Vec::new();
    let result = (|| {
        for relative in files {
            let destination = directory.join(&relative);
            let mut component = directory.to_owned();
            for part in relative.components() {
                component.push(part);
                ordinary(&component)?;
            }
            fs::create_dir_all(destination.parent().context("Missing application parent")?)?;
            let existed = destination.exists();
            if existed {
                ensure!(
                    destination.is_file(),
                    "Update destination is not a regular file"
                );
                let saved = backup.path().join(&relative);
                fs::create_dir_all(saved.parent().unwrap())?;
                fs::copy(&destination, saved)?;
            }
            replace(&payload.path().join(&relative), &destination)?;
            replaced.push((relative, existed));
        }
        Ok(())
    })();
    if let Err(error) = result {
        for (relative, existed) in replaced.into_iter().rev() {
            let destination = directory.join(&relative);
            let restored = if existed {
                replace(&backup.path().join(&relative), &destination)
            } else {
                fs::remove_file(&destination).map_err(Into::into)
            };
            if let Err(restore) = restored {
                let retained = backup.keep();
                return Err(error).context(format!(
                    "Rollback failed: {restore}; backups retained at {}",
                    retained.display()
                ));
            }
        }
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a tiny actual ZIP to exercise extraction and filesystem replacement, not a mocked installer.
    fn package(path: &Path, entries: &[(&str, &[u8])]) {
        let mut zip = zip::ZipWriter::new(File::create(path).unwrap());
        for (name, contents) in entries {
            zip.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(contents).unwrap();
        }
        zip.finish().unwrap();
    }

    /// Traversal, alternate streams, Windows device paths and non-payload files are rejected.
    #[test]
    fn host_update_portable_paths_are_bounded() {
        for name in [
            "../Nanobug.exe",
            "/Nanobug.exe",
            "licenses/../evil",
            "licenses/file:stream",
            "licenses/CON.txt",
            "licenses/a.",
            "C:\\evil",
            "plugins/evil",
            ".git/config",
        ] {
            assert!(payload_path(name).is_err(), "{name}");
        }
    }

    /// Successful replacement preserves user files; a later destination failure restores the old EXE.
    #[test]
    fn host_update_portable_replaces_and_rolls_back() {
        let root = tempfile::tempdir().unwrap();
        let app = root.path().join("app");
        fs::create_dir(&app).unwrap();
        fs::write(app.join("Nanobug.exe"), b"old").unwrap();
        fs::write(app.join("user.txt"), b"keep").unwrap();
        let zip = root.path().join("update.zip");
        package(
            &zip,
            &[("Nanobug.exe", b"new"), ("licenses/test.txt", b"license")],
        );
        fs::write(app.join("licenses"), b"blocked parent").unwrap();
        assert!(apply(&zip, &app, root.path()).is_err());
        assert_eq!(fs::read(app.join("Nanobug.exe")).unwrap(), b"old");
        fs::remove_file(app.join("licenses")).unwrap();
        apply(&zip, &app, root.path()).unwrap();
        assert_eq!(fs::read(app.join("Nanobug.exe")).unwrap(), b"new");
        assert_eq!(fs::read(app.join("user.txt")).unwrap(), b"keep");
        assert_eq!(fs::read(app.join("licenses/test.txt")).unwrap(), b"license");
    }

    /// A malicious archive fails before any running application file is touched.
    #[test]
    fn host_update_portable_rejects_bad_archive_before_replacement() {
        let root = tempfile::tempdir().unwrap();
        let app = root.path().join("app");
        fs::create_dir(&app).unwrap();
        fs::write(app.join("Nanobug.exe"), b"old").unwrap();
        let zip = root.path().join("update.zip");
        package(&zip, &[("Nanobug.exe", b"new"), ("../escape", b"evil")]);
        assert!(apply(&zip, &app, root.path()).is_err());
        assert_eq!(fs::read(app.join("Nanobug.exe")).unwrap(), b"old");
        assert!(!root.path().join("escape").exists());
    }
}
