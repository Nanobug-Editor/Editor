//! Private directory management stays within the existing data root and bounded migration quota.
use super::*;
use api::{ErrorCode, Failure, Value};
use plugin_protocol::storage::{Area, Entry, Listing, Operation};
use resource_roots::RootKind;

impl State {
    /// Cache/temporary roots are subdirectories of this owner, never guest-supplied native paths.
    pub(super) fn private_relative(&self, kind: RootKind, path: &str) -> Result<String, Failure> {
        if matches!(kind, RootKind::Data) {
            self.validate_data_path(path)?;
        }
        let prefix = match kind {
            RootKind::Cache => ".cache/",
            RootKind::Temporary => ".temporary/",
            _ => "",
        };
        if path.is_empty() {
            return Err(Failure::new(
                ErrorCode::InvalidPath,
                "A private file path is required",
            ));
        }
        Ok(format!("{prefix}{path}"))
    }

    /// The reserved Temporary area is reachable only through its owned handle, including Windows aliases.
    /// Data listing may still observe the reserved name; Cache and Workspace have independent relative names.
    pub(super) fn validate_data_path(&self, path: &str) -> Result<(), Failure> {
        if path.is_empty() {
            return Ok(());
        }
        let reserved = |name: &str| {
            name.trim_end_matches([' ', '.'])
                .eq_ignore_ascii_case(".temporary")
        };
        let denied = || {
            Failure::new(
                ErrorCode::InvalidPath,
                "Temporary data requires its owned area handle",
            )
        };
        if path.split('/').next().is_some_and(reserved) {
            return Err(denied());
        }
        // Resolving existing components closes 8.3 aliases as well as case aliases. For new files,
        // checked canonicalizes the parent, so a reserved directory cannot be reached by a new child.
        let canonical_root = self.data.canonicalize().map_err(io_failure)?;
        let target = checked(&canonical_root, path, false)?;
        if target
            .strip_prefix(&canonical_root)
            .ok()
            .and_then(|relative| relative.components().next())
            .and_then(|component| component.as_os_str().to_str())
            .is_some_and(reserved)
        {
            return Err(denied());
        }
        Ok(())
    }
    pub(super) fn private_request(&mut self, operation: Operation) -> Result<Value, Failure> {
        if !self
            .api
            .capabilities
            .get("storage.private")
            .is_some_and(|version| *version >= semver::Version::new(1, 2, 0))
        {
            return Err(Failure::new(
                ErrorCode::CapabilityUnavailable,
                "Private directories require storage.private 1.2",
            ));
        }
        self.file_authority(RootKind::Data, true)?;
        // Prepare/configuration/Activate and post-flush Snapshot are still unpublished. Only the trusted
        // manager's durable cutover opens directory authority; staged_writes being empty proves no commit.
        if !self.activation_committed {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "Private directories require committed activation",
            ));
        }
        match operation {
            Operation::Open { area } => {
                let (kind, directory) = match area {
                    Area::Data => (RootKind::Data, None),
                    Area::Cache => (RootKind::Cache, Some(".cache")),
                    Area::Temporary => (RootKind::Temporary, Some(".temporary")),
                };
                self.roots.ensure_capacity()?;
                if let Some(relative) = directory {
                    let path = checked(&self.data, relative, false)?;
                    reserve_entries(&self.data, [path.as_path()], self.roots.limit)?;
                    if !path.exists() {
                        std::fs::create_dir(&path).map_err(io_failure)?;
                    }
                    if !path.is_dir() {
                        return Err(Failure::new(
                            ErrorCode::InvalidPath,
                            "Private area is not a directory",
                        ));
                    }
                }
                self.roots.open(kind)
            }
            Operation::Stat { handle, path } => {
                let (root, path) = self.private_target(&handle, &path, false)?;
                let path = checked(&root, &path, true)?;
                metadata(&path).map(Value::PrivateEntry)
            }
            Operation::List {
                handle,
                path,
                after,
                limit,
            } => {
                if !(1..=128).contains(&limit)
                    || after
                        .as_ref()
                        .is_some_and(|name| name.len() > 255 || name.contains(['/', '\\', ':']))
                {
                    return Err(Failure::new(
                        ErrorCode::InvalidRequest,
                        "Invalid private listing page",
                    ));
                }
                let (root, relative) = self.private_target(&handle, &path, true)?;
                let directory = if relative.is_empty() {
                    root.clone()
                } else {
                    checked(&root, &relative, true)?
                };
                private_size(&self.data, self.roots.limit)?;
                let mut entries = vec![];
                for entry in std::fs::read_dir(directory).map_err(io_failure)? {
                    let entry = entry.map_err(io_failure)?;
                    reject_link(&entry.path())?;
                    entries.push(metadata(&entry.path())?);
                }
                entries.sort_by(|left, right| left.name.cmp(&right.name));
                entries.retain(|entry| after.as_ref().is_none_or(|after| &entry.name > after));
                let has_more = entries.len() > limit as usize;
                entries.truncate(limit.into());
                Ok(Value::PrivateListing(Listing { entries, has_more }))
            }
            Operation::CreateDirectory { handle, path } => {
                let (root, relative) = self.private_target(&handle, &path, false)?;
                let path = checked(&root, &relative, false)?;
                reserve_entries(&self.data, [path.as_path()], self.roots.limit)?;
                // A single directory must have an existing parent; this bounds mutation and rollback scope.
                std::fs::create_dir(path).map_err(io_failure)?;
                Ok(Value::Unit)
            }
            Operation::Rename {
                handle,
                path,
                destination,
            } => {
                let (root, source) = self.private_target(&handle, &path, false)?;
                let (_, target) = self.private_target(&handle, &destination, false)?;
                let source = checked(&root, &source, true)?;
                let target = checked(&root, &target, false)?;
                let usage = private_usage(&self.data, self.roots.limit)?;
                if target.exists() {
                    return Err(Failure::new(
                        ErrorCode::Conflict,
                        "Private destination already exists",
                    ));
                }
                let old_name = source.file_name().unwrap().to_string_lossy().len();
                let new_name = target.file_name().unwrap().to_string_lossy().len();
                if usage.names.saturating_sub(old_name) + new_name > MAX_NAME_BYTES {
                    return Err(Failure::new(
                        ErrorCode::LimitExceeded,
                        "Private name quota exceeded",
                    ));
                }
                // Moving a whole subtree must preserve the depth limit for every descendant.
                validate_depth(&source, root_relative_depth(&self.data, &target)?)?;
                std::fs::rename(source, target).map_err(io_failure)?;
                Ok(Value::Unit)
            }
            Operation::Delete {
                handle,
                path,
                recursive,
            } => {
                let (root, relative) = self.private_target(&handle, &path, false)?;
                let path = checked(&root, &relative, true)?;
                private_size(&self.data, self.roots.limit)?;
                if path.is_dir() {
                    if recursive {
                        std::fs::remove_dir_all(path).map_err(io_failure)?;
                    } else {
                        std::fs::remove_dir(path).map_err(io_failure)?;
                    }
                } else {
                    std::fs::remove_file(path).map_err(io_failure)?;
                }
                Ok(Value::Unit)
            }
        }
    }
    fn private_target(
        &self,
        handle: &api::ResourceHandle,
        path: &str,
        allow_root: bool,
    ) -> Result<(PathBuf, String), Failure> {
        let kind = self.roots.resolve(handle)?;
        if !matches!(kind, RootKind::Data | RootKind::Cache | RootKind::Temporary) {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Private operation requires an owned data area",
            ));
        }
        self.file_authority(kind, true)?;
        if matches!(kind, RootKind::Data) {
            self.validate_data_path(path)?;
        }
        let prefix = match kind {
            RootKind::Cache => ".cache",
            RootKind::Temporary => ".temporary",
            _ => "",
        };
        let root = if prefix.is_empty() {
            self.data.clone()
        } else {
            checked(&self.data, prefix, true)?
        };
        if path.is_empty() && !allow_root {
            return Err(Failure::new(
                ErrorCode::InvalidPath,
                "Cannot mutate a private root",
            ));
        }
        Ok((root, path.into()))
    }
    /// Temporary bytes belong to the incarnation which opened the area, even after its handle closes.
    /// Failed same-root candidates have no ownership; repeated quiesce/stop/Drop cannot delete a successor's bytes.
    pub(super) fn clear_temporary_data(&mut self) {
        if !self.roots.take_temporary() {
            return;
        }
        if let Ok(path) = checked(&self.data, ".temporary", true) {
            if std::fs::remove_dir_all(path).is_err() {
                // Access is already revoked; retain an observable safe diagnosis if native cleanup fails.
                self.host_resources.logs.append(
                    &self.plugin_services.principal.plugin,
                    crate::LogLevel::Warning,
                    "private-data",
                    "Temporary private data cleanup failed",
                );
            }
        }
    }
}

/// Reject reparse points at every existing component before canonical deletion or directory traversal.
pub(super) fn checked(root: &Path, relative: &str, existing: bool) -> Result<PathBuf, Failure> {
    if relative.split('/').count() > 16 || relative.len() > 4096 {
        return Err(Failure::new(
            ErrorCode::InvalidPath,
            "Private path exceeds bounds",
        ));
    }
    let result = safe_path(root, relative, existing).map_err(|error| {
        Failure::new(
            if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound)
            {
                ErrorCode::NotFound
            } else {
                ErrorCode::InvalidPath
            },
            "Private path is missing or escapes its owner",
        )
    })?;
    let mut current = root.to_path_buf();
    for component in relative.split('/') {
        current.push(component);
        if std::fs::symlink_metadata(&current).is_ok() {
            reject_link(&current)?;
        }
    }
    Ok(result)
}
fn reject_link(path: &Path) -> Result<(), Failure> {
    let metadata = std::fs::symlink_metadata(path).map_err(io_failure)?;
    let mut reparse = metadata.file_type().is_symlink();
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        reparse |= metadata.file_attributes() & 0x400 != 0;
    }
    if reparse {
        return Err(Failure::new(
            ErrorCode::InvalidPath,
            "Private reparse points are not supported",
        ));
    }
    Ok(())
}
fn metadata(path: &Path) -> Result<Entry, Failure> {
    reject_link(path)?;
    let metadata = path.metadata().map_err(io_failure)?;
    Ok(Entry {
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        directory: metadata.is_dir(),
        bytes: if metadata.is_dir() { 0 } else { metadata.len() },
    })
}
/// Traversal, not just file bytes, is quota-bound. External symlinks and unknown native file types fail closed.
const MAX_ENTRIES: usize = 1024;
const MAX_NAME_BYTES: usize = 64 * 1024;
struct Usage {
    bytes: u64,
    entries: usize,
    names: usize,
}
pub(super) fn private_size(root: &Path, limit: usize) -> Result<u64, Failure> {
    private_usage(root, limit).map(|usage| usage.bytes)
}
fn private_usage(root: &Path, limit: usize) -> Result<Usage, Failure> {
    fn walk(root: &Path, depth: usize, usage: &mut Usage, limit: usize) -> Result<(), Failure> {
        if depth > 16 {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Private directory depth exceeded",
            ));
        }
        for entry in std::fs::read_dir(root).map_err(io_failure)? {
            let path = entry.map_err(io_failure)?.path();
            reject_link(&path)?;
            usage.entries += 1;
            usage.names += path.file_name().unwrap().to_string_lossy().len();
            if usage.entries > MAX_ENTRIES || usage.names > MAX_NAME_BYTES {
                return Err(Failure::new(
                    ErrorCode::LimitExceeded,
                    "Private entry/name quota exceeded",
                ));
            }
            let metadata = path.metadata().map_err(io_failure)?;
            if metadata.is_dir() {
                walk(&path, depth + 1, usage, limit)?;
            } else if metadata.is_file() {
                usage.bytes = usage.bytes.checked_add(metadata.len()).ok_or_else(|| {
                    Failure::new(ErrorCode::LimitExceeded, "Private byte quota exceeded")
                })?;
            } else {
                return Err(Failure::new(
                    ErrorCode::InvalidPath,
                    "Unsupported private file type",
                ));
            }
            if usage.bytes > limit as u64 {
                return Err(Failure::new(
                    ErrorCode::LimitExceeded,
                    "Private byte quota exceeded",
                ));
            }
        }
        Ok(())
    }
    let mut usage = Usage {
        bytes: 0,
        entries: 0,
        names: 0,
    };
    walk(root, 0, &mut usage, limit)?;
    Ok(usage)
}
/// Reserve distinct missing entries before mutation, including staged files which are not on disk yet.
pub(super) fn reserve_entries<'a>(
    root: &Path,
    paths: impl IntoIterator<Item = &'a Path>,
    limit: usize,
) -> Result<(), Failure> {
    let mut usage = private_usage(root, limit)?;
    let paths = paths
        .into_iter()
        .map(|path| {
            // Area-relative bounds omit the reserved prefix. Count from the common data owner
            // before creating anything, so one invalid cache/temp entry cannot poison every area.
            validate_depth(path, root_relative_depth(root, path)?)?;
            Ok(path)
        })
        .collect::<Result<std::collections::BTreeSet<_>, Failure>>()?
        .into_iter()
        .filter(|path| !path.exists())
        .collect::<std::collections::BTreeSet<_>>();
    usage.entries += paths.len();
    usage.names += paths
        .iter()
        .map(|path| path.file_name().unwrap().to_string_lossy().len())
        .sum::<usize>();
    if usage.entries > MAX_ENTRIES || usage.names > MAX_NAME_BYTES {
        return Err(Failure::new(
            ErrorCode::LimitExceeded,
            "Private entry/name quota exceeded",
        ));
    }
    Ok(())
}
/// Mutations and subtree moves measure depth from the same canonical owner, including reserved areas.
fn root_relative_depth(root: &Path, path: &Path) -> Result<usize, Failure> {
    let canonical = root.canonicalize().map_err(io_failure)?;
    path.strip_prefix(canonical)
        .map(|relative| relative.components().count())
        .map_err(|_| Failure::new(ErrorCode::InvalidPath, "Private path escapes its owner"))
}
fn validate_depth(path: &Path, depth: usize) -> Result<(), Failure> {
    if depth > 16 {
        return Err(Failure::new(
            ErrorCode::LimitExceeded,
            "Private directory depth exceeded",
        ));
    }
    if path.is_dir() {
        for entry in std::fs::read_dir(path).map_err(io_failure)? {
            let path = entry.map_err(io_failure)?.path();
            reject_link(&path)?;
            validate_depth(&path, depth + 1)?;
        }
    }
    Ok(())
}
fn io_failure(error: std::io::Error) -> Failure {
    Failure::new(
        if error.kind() == std::io::ErrorKind::NotFound {
            ErrorCode::NotFound
        } else {
            ErrorCode::OperationFailed
        },
        format!("Private IO failed: {}", error.kind()),
    )
}
