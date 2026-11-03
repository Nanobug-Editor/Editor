//! Preserve existing user data while moving the ordinary profile beside the application.
//!
//! Import is copy-first and retryable: old files remain backups, destination files win conflicts,
//! and completion is recorded only after every staged file has been published. Caches rebuild at
//! their new location; they are never a prerequisite for preserving settings, plugins or history.

use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

#[cfg(test)]
mod tests;

/// Old application-owned stores, supplied explicitly so migration never searches arbitrary projects.
#[derive(Clone, Debug)]
pub struct LegacyLocations {
    /// Previous configuration root, including installed plugins and private development profiles.
    pub profile: Option<PathBuf>,
    /// Previous local-history store, which used to live separately from configuration.
    pub history: Option<PathBuf>,
}

impl LegacyLocations {
    /// Resolve the established MeEditor stores for the current OS user without creating them.
    pub fn current_user() -> Self {
        Self {
            profile: dirs::config_dir().map(|root| root.join("MeEditor")),
            history: dirs::data_local_dir().map(|root| root.join("MeEditor/history")),
        }
    }
}

/// Durable marker prevents later launches from resurrecting deliberately removed settings or plugins.
const COMPLETE: &str = "legacy-import-v1.complete";

/// Prepare writable `profile`, optionally importing `legacy` stores before any profile consumer starts.
/// Explicit isolated profiles pass `None`. Live legacy owners, redirected entries, unreadable files
/// and write failures return errors with their paths; they never select another storage directory.
pub fn prepare(profile: &Path, legacy: Option<LegacyLocations>) -> io::Result<()> {
    fs::create_dir_all(profile)?;
    let profile = profile.canonicalize()?;
    // Probe actual writes, not just readonly attributes: ACLs and read-only volumes can differ.
    let mut probe = tempfile::NamedTempFile::new_in(&profile)?;
    probe.write_all(b"Nanobug data directory\n")?;
    probe.as_file().sync_all()?;
    drop(probe);
    let Some(legacy) = legacy else {
        return Ok(());
    };
    let _migration = lock(&profile.join("legacy-import.lock"))?;
    if profile.join(COMPLETE).try_exists()? {
        return Ok(());
    }
    // Explicit launches selecting this same target must not open it halfway through an import.
    let gui = profile.join("gui");
    reject_redirect(&gui)?;
    fs::create_dir_all(&gui)?;
    reject_redirect(&gui.join("owner.lock"))?;
    let _target_owner = lock(&gui.join("owner.lock"))?;
    let source_profile = existing_directory(legacy.profile.as_deref())?;
    let source_history = existing_directory(legacy.history.as_deref())?;
    for source in [&source_profile, &source_history].into_iter().flatten() {
        if profile.starts_with(source) || source.starts_with(&profile) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "Legacy and portable data directories overlap: {}",
                    source.display()
                ),
            ));
        }
    }

    // These are the same OS leases held by ordinary GUI, runtime and history writers. Keep them
    // through publication so a source process cannot change the snapshot midway through import.
    let mut leases = Vec::new();
    let mut locked_paths = Vec::new();
    if let Some(source) = &source_profile {
        locked_paths.push(source.join("gui"));
        acquire(
            source.join("gui/owner.lock"),
            &mut leases,
            &mut locked_paths,
        )?;
        if source.join("runtime-plugins").is_dir() {
            acquire(
                source.join("runtime-plugins/runtime.lock"),
                &mut leases,
                &mut locked_paths,
            )?;
            acquire(
                source.join("runtime-plugins/data-transactions.lock"),
                &mut leases,
                &mut locked_paths,
            )?;
        }
    }
    if let Some(source) = &source_history {
        acquire(source.join("writer.lock"), &mut leases, &mut locked_paths)?;
    }

    let staging = tempfile::Builder::new()
        .prefix(".legacy-import-")
        .tempdir_in(&profile)?;
    if let Some(source) = &source_profile {
        copy_directory(source, staging.path(), &locked_paths)?;
    }
    if let Some(source) = &source_history {
        let target = staging.path().join("history");
        fs::create_dir_all(&target)?;
        copy_directory(source, &target, &locked_paths)?;
    }
    publish_missing(staging.path(), &profile, true)?;
    let mut marker = tempfile::NamedTempFile::new_in(&profile)?;
    marker.write_all(b"Imported legacy user data; original stores retained as backups.\n")?;
    marker.as_file().sync_all()?;
    marker
        .persist_noclobber(profile.join(COMPLETE))
        .map_err(|error| error.error)?;
    Ok(())
}

/// Resolve an optional legacy root; missing stores are normal, other filesystem failures propagate.
fn existing_directory(path: Option<&Path>) -> io::Result<Option<PathBuf>> {
    let Some(path) = path else {
        return Ok(None);
    };
    match path.metadata() {
        Ok(metadata) if metadata.is_dir() => path.canonicalize().map(Some),
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Legacy data root is not a directory: {}", path.display()),
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// Hold an exclusive, nonblocking OS lease at `path`; active old editors must exit normally first.
fn lock(path: &Path) -> io::Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    file.try_lock().map_err(|error| {
        io::Error::new(
            io::ErrorKind::WouldBlock,
            format!(
                "Cannot lock editor data (close other editors and retry): {}: {error}",
                path.display()
            ),
        )
    })?;
    Ok(file)
}

/// Keep each source `path` pinned in `leases` and exclude its lock bytes from the copied snapshot.
fn acquire(
    path: PathBuf,
    leases: &mut Vec<File>,
    locked_paths: &mut Vec<PathBuf>,
) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("Missing lease parent"))?;
    reject_redirect(parent)?;
    fs::create_dir_all(parent)?;
    reject_redirect(&path)?;
    leases.push(lock(&path)?);
    locked_paths.push(path);
    Ok(())
}

/// Reject symlinks and Windows reparse points without traversing them; an absent target is allowed.
fn reject_redirect(path: &Path) -> io::Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    #[cfg(windows)]
    let redirected = {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    };
    #[cfg(not(windows))]
    let redirected = metadata.file_type().is_symlink();
    if redirected {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "Data migration does not follow redirected entries: {}",
                path.display()
            ),
        ));
    }
    Ok(())
}

/// Snapshot `source` into private `target`, retaining timestamps needed by legacy history identities.
/// `locked_paths` excludes OS lease files; GUI endpoint credentials are never copied between owners.
fn copy_directory(source: &Path, target: &Path, locked_paths: &[PathBuf]) -> io::Result<()> {
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        if locked_paths.contains(&path) {
            continue;
        }
        reject_redirect(&path)?;
        let metadata = fs::metadata(&path)?;
        let destination = target.join(entry.file_name());
        if metadata.is_dir() {
            fs::create_dir_all(&destination)?;
            copy_directory(&path, &destination, locked_paths)?;
        } else if metadata.is_file() {
            let mut input = File::open(&path)?;
            let mut output = File::create(&destination)?;
            let copied = io::copy(&mut input, &mut output)?;
            let after = input.metadata()?;
            if copied != metadata.len()
                || after.len() != metadata.len()
                || after.modified()? != metadata.modified()?
            {
                return Err(io::Error::other(format!(
                    "Legacy file changed during import: {}",
                    path.display()
                )));
            }
            output.set_times(fs::FileTimes::new().set_modified(metadata.modified()?))?;
            output.sync_all()?;
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Unsupported legacy data entry: {}", path.display()),
            ));
        }
    }
    Ok(())
}

/// Publish complete staged files into `target`; existing user files win and redirected targets fail.
/// A failed publication is retryable because imported leaves are complete and the old stores survive.
/// At the profile `root`, existing multi-file stores are preserved as a unit: mixing a database or
/// registry with another store's journals and payloads would corrupt otherwise valid destination data.
fn publish_missing(staged: &Path, target: &Path, root: bool) -> io::Result<()> {
    for entry in fs::read_dir(staged)? {
        let entry = entry?;
        let destination = target.join(entry.file_name());
        reject_redirect(&destination)?;
        if destination.try_exists()? {
            if entry.file_type()?.is_dir() && destination.is_dir() {
                let owned_store = matches!(
                    entry.file_name().to_str(),
                    Some("history" | "runtime-plugins" | "plugin-development" | "run")
                );
                if root && owned_store && fs::read_dir(&destination)?.next().transpose()?.is_some()
                {
                    continue;
                }
                publish_missing(&entry.path(), &destination, false)?;
            } else if entry.file_type()?.is_dir() || destination.is_dir() {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!(
                        "Legacy data conflicts with a different entry type: {}",
                        destination.display()
                    ),
                ));
            }
            continue;
        }
        fs::rename(entry.path(), &destination)?;
    }
    Ok(())
}
