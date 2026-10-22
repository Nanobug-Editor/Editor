//! Transfer preimages and metadata-only file observations use bounded native traversal.

use super::*;
use editor_history::{Change, FileId, Lease};

/// A completed transfer keeps preimages alive until their actual endpoint results have been indexed.
pub(crate) struct TreeLease {
    /// Every retained file is associated with its native pre-operation path.
    pub files: Vec<(FileId, PathBuf)>,
    /// Leases root content and structural-only versions through asynchronous publication.
    leases: Vec<Lease>,
}

/// A reliable directory rename includes unopened and unsupported files without inheriting Git ignores.
pub(crate) fn record_move(
    history: &History,
    old: &Path,
    new: &Path,
    root: &Path,
) -> editor_history::Result<()> {
    let policy = history.policy()?;
    if !policy.enabled {
        return Ok(());
    }
    let mut files = history.bindings_under(old)?;
    // The app's sibling temporary publication can arrive after its successful Saved observation.
    if files.is_empty() && new.is_file() && history.file_at(new)?.is_some() {
        return Ok(());
    }
    let mut pending = vec![new.to_owned()];
    let mut retained = new.as_os_str().len().saturating_mul(2) + 128;
    while let Some(path) = pending.pop() {
        if policy.skip_path(&path).is_some() {
            continue;
        }
        let metadata = std::fs::symlink_metadata(&path)?;
        #[cfg(windows)]
        let linked = {
            use std::os::windows::fs::MetadataExt as _;
            metadata.file_attributes() & 0x400 != 0
        };
        #[cfg(not(windows))]
        let linked = metadata.file_type().is_symlink();
        if linked {
            continue;
        }
        if metadata.is_dir() {
            for entry in std::fs::read_dir(&path)? {
                let path = entry?.path();
                retained += path.as_os_str().len().saturating_mul(2) + 128;
                if retained > 32 * 1024 * 1024 {
                    return Err(editor_history::Error::Busy);
                }
                pending.push(path);
            }
        } else if metadata.is_file() {
            let relative = path
                .strip_prefix(new)
                .map_err(|_| editor_history::Error::InvalidPath)?;
            let source = if relative.as_os_str().is_empty() {
                old.to_owned()
            } else {
                old.join(relative)
            };
            if policy.skip_path(&source).is_some() {
                continue;
            }
            if history.file_at(&source)?.is_none() && history.file_at(&path)?.is_none() {
                files.push((
                    history.track(&source, source.starts_with(root).then_some(root), now_ms())?,
                    source,
                ));
            }
        }
    }
    for (file, source) in files {
        let relative = source
            .strip_prefix(old)
            .map_err(|_| editor_history::Error::InvalidPath)?;
        let path = if relative.as_os_str().is_empty() {
            new.to_owned()
        } else {
            new.join(relative)
        };
        if !path.is_file() {
            continue;
        }
        if history.location(file)?.path != path {
            history.change(Change {
                file,
                path: path.clone(),
                workspace: path.starts_with(root).then(|| root.to_owned()),
                reason: if old.parent() == new.parent() {
                    Reason::Renamed
                } else {
                    Reason::Moved
                },
                present: true,
                at_ms: now_ms(),
            })?;
        }
        history.import_known_into(&source, source.starts_with(root).then_some(root), file)?;
        super::observe_disk(
            history,
            &path,
            path.starts_with(root).then_some(root),
            Reason::External,
        )?;
    }
    Ok(())
}

/// Protect eligible existing files before a transfer publication; Git ignores are deliberately unused.
pub(crate) fn retain_tree(
    history: &History,
    path: &Path,
    workspace: &Path,
    reason: Reason,
) -> editor_history::Result<TreeLease> {
    let policy = history.policy()?;
    let mut retained = TreeLease {
        files: Vec::new(),
        leases: Vec::new(),
    };
    if !policy.enabled {
        return Ok(retained);
    }
    let mut pending = vec![path.to_owned()];
    let mut metadata_bytes = path.as_os_str().len().saturating_mul(2) + 128;
    while let Some(path) = pending.pop() {
        if policy.skip_path(&path).is_some() {
            continue;
        }
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        #[cfg(windows)]
        let linked = {
            use std::os::windows::fs::MetadataExt as _;
            metadata.file_attributes() & 0x400 != 0
        };
        #[cfg(not(windows))]
        let linked = metadata.file_type().is_symlink();
        if linked {
            continue;
        }
        if metadata.is_dir() {
            for entry in std::fs::read_dir(path)? {
                let path = entry?.path();
                metadata_bytes += path.as_os_str().len().saturating_mul(2) + 128;
                if metadata_bytes > 32 * 1024 * 1024 {
                    return Err(editor_history::Error::Busy);
                }
                pending.push(path);
            }
            continue;
        }
        let membership = path.starts_with(workspace).then_some(workspace);
        let file = history.track(&path, membership, super::now_ms())?;
        let mut bytes = Vec::new();
        if metadata.len() <= policy.max_file_bytes {
            std::fs::File::open(&path)?
                .take(policy.max_file_bytes + 1)
                .read_to_end(&mut bytes)?;
        }
        let lease =
            if metadata.len() <= policy.max_file_bytes && policy.skip(&path, &bytes).is_none() {
                history.capture_pinned(Capture {
                    file: Some(file),
                    path: path.clone(),
                    workspace: membership.map(Path::to_owned),
                    contents: bytes,
                    reason,
                    at_ms: now_ms(),
                })?
            } else {
                let version = history.change_without_content(Change {
                    file,
                    path: path.clone(),
                    workspace: membership.map(Path::to_owned),
                    reason,
                    present: true,
                    at_ms: now_ms(),
                })?;
                history.pin(version)?
            };
        retained.files.push((file, path));
        retained.leases.push(lease);
    }
    Ok(retained)
}

/// Record only observed postconditions. A failed move with both endpoints present remains a copy.
pub(crate) fn finish_transfer(
    history: &History,
    root: &Path,
    before: Vec<TreeLease>,
    paths: Vec<PathBuf>,
    movement: Option<(PathBuf, PathBuf)>,
    copied: bool,
) -> editor_history::Result<()> {
    if !history.policy()?.enabled {
        return Ok(());
    }
    let mut moved = std::collections::HashSet::new();
    if let Some((old, new)) = movement {
        for (file, path) in before.iter().flat_map(|tree| tree.files.iter()) {
            let Ok(relative) = path.strip_prefix(&old) else {
                continue;
            };
            let target = if relative.as_os_str().is_empty() {
                new.clone()
            } else {
                new.join(relative)
            };
            if path.exists() || !target.is_file() {
                continue;
            }
            if let Some(previous) = history
                .file_at(&target)?
                .filter(|previous| previous != file)
            {
                let location = history.location(previous)?;
                history.change(Change {
                    file: previous,
                    path: target.clone(),
                    workspace: location.workspace,
                    reason: Reason::Deleted,
                    present: false,
                    at_ms: now_ms(),
                })?;
            }
            history.change(Change {
                file: *file,
                path: target.clone(),
                workspace: target.starts_with(root).then(|| root.to_owned()),
                reason: Reason::Moved,
                present: true,
                at_ms: now_ms(),
            })?;
            moved.insert(*file);
        }
    }
    for (file, path) in before.iter().flat_map(|tree| tree.files.iter()) {
        if moved.contains(file) {
            continue;
        }
        if !path.exists() || (copied && paths.iter().any(|target| path.starts_with(target))) {
            let location = history.location(*file)?;
            if location.present {
                history.change(Change {
                    file: *file,
                    path: path.clone(),
                    workspace: location.workspace,
                    reason: Reason::Deleted,
                    present: false,
                    at_ms: now_ms(),
                })?;
            }
        }
    }
    for path in paths {
        let state = retain_tree(
            history,
            &path,
            root,
            if copied {
                Reason::Copied
            } else {
                Reason::External
            },
        )?;
        drop(state);
    }
    Ok(())
}
