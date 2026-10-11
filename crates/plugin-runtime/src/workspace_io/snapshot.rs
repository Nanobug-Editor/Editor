//! Bounded immutable disk preimages carry native identity, bytes and permissions, never editor state.

use super::*;
use crate::native_path::{is_reparse, pin_path};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};

/// Recovery bytes remain private memory. Nested trees share immutable file bytes across planned states.
#[derive(Clone)]
pub(super) struct Snapshot {
    pub(super) stamp: FileStamp,
    pub(super) node: Node,
}

#[derive(Clone)]
pub(super) enum Node {
    Missing,
    /// None is a planned new file: native creation supplies regular-file attributes, not parent flags.
    File {
        bytes: Arc<[u8]>,
        permissions: Option<std::fs::Permissions>,
    },
    Directory {
        entries: BTreeMap<String, Snapshot>,
        permissions: std::fs::Permissions,
    },
}

/// Budgets cover all captured targets, including duplicates, tree entries and discarded bytes.
#[derive(Default)]
pub(super) struct Budget {
    entries: usize,
    bytes: usize,
}

impl Budget {
    /// Native and virtual traversal share one entry/depth limit, including duplicated selected targets.
    fn entry(&mut self, depth: usize) -> Result<(), Failure> {
        self.entries += 1;
        if self.entries > 50_000 || depth > 64 {
            return Err(limit("Disk tree exceeds entry/depth limits"));
        }
        Ok(())
    }

    /// Reserve complete file lengths before capture or consent; proposed copies cannot hide growth.
    fn file_bytes(&mut self, bytes: usize) -> Result<(), Failure> {
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .ok_or_else(|| limit("Disk bytes exceed limit"))?;
        if self.bytes > MAX_BACKUP_BYTES {
            return Err(limit("Disk preparation exceeds 32 MiB"));
        }
        Ok(())
    }
}

/// Projected states obey the same bounded capture as actual IO, without reading or mutating disk.
pub(super) fn projection_bytes<'a>(
    snapshots: impl Iterator<Item = &'a Snapshot>,
    stop: &impl Fn() -> bool,
) -> Result<usize, Failure> {
    fn reserve(
        snapshot: &Snapshot,
        budget: &mut Budget,
        depth: usize,
        stop: &impl Fn() -> bool,
    ) -> Result<(), Failure> {
        ensure_running(stop)?;
        budget.entry(depth)?;
        match &snapshot.node {
            Node::Missing => Ok(()),
            Node::File { bytes, .. } => budget.file_bytes(bytes.len()),
            Node::Directory { entries, .. } => {
                for child in entries.values() {
                    reserve(child, budget, depth + 1, stop)?;
                }
                Ok(())
            }
        }
    }
    let mut budget = Budget::default();
    for snapshot in snapshots {
        reserve(snapshot, &mut budget, 0, stop)?;
    }
    Ok(budget.bytes)
}

impl Snapshot {
    pub(super) fn capture(
        path: &Path,
        budget: &mut Budget,
        depth: usize,
        stop: &impl Fn() -> bool,
    ) -> Result<Self, Failure> {
        ensure_running(stop)?;
        budget.entry(depth)?;
        let metadata = match std::fs::symlink_metadata(path) {
            Ok(meta) => meta,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::missing());
            }
            Err(error) => return Err(io(error)),
        };
        if is_reparse(&metadata) {
            return Err(Failure::new(
                ErrorCode::InvalidPath,
                "Disk tree contains a link or junction",
            ));
        }
        let pins = pin_path(path, false)?;
        let identity = pins.identities.last().copied();
        let permissions = metadata.permissions();
        let mut digest = Sha256::new();
        digest.update(format!("{identity:?}:{}:", permissions.readonly()));
        if metadata.is_file() {
            budget.file_bytes(
                usize::try_from(metadata.len()).map_err(|_| limit("Disk bytes exceed limit"))?,
            )?;
            // OPEN_REPARSE_POINT and the leaf pin close the metadata/open race without following a link.
            let mut options = std::fs::OpenOptions::new();
            options.read(true);
            #[cfg(windows)]
            {
                use std::os::windows::fs::OpenOptionsExt;
                options.share_mode(3).custom_flags(0x00200000);
            }
            let mut file = options.open(path).map_err(io)?;
            if is_reparse(&file.metadata().map_err(io)?) {
                return Err(Failure::new(
                    ErrorCode::InvalidPath,
                    "File redirected during preparation",
                ));
            }
            let mut bytes = Vec::with_capacity(metadata.len() as usize);
            (&mut file)
                .take(MAX_BACKUP_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(io)?;
            ensure_running(stop)?;
            if bytes.len() != metadata.len() as usize || bytes.len() > MAX_BACKUP_BYTES {
                return Err(conflict("File changed during preparation"));
            }
            digest.update(&bytes);
            Ok(Self {
                stamp: FileStamp::File {
                    version: format!("{:x}", digest.finalize()),
                    bytes: bytes.len() as u64,
                    readonly: permissions.readonly(),
                },
                node: Node::File {
                    bytes: bytes.into(),
                    permissions: Some(permissions),
                },
            })
        } else if metadata.is_dir() {
            let mut entries = BTreeMap::new();
            for entry in std::fs::read_dir(path).map_err(io)? {
                ensure_running(stop)?;
                let entry = entry.map_err(io)?;
                let name = entry
                    .file_name()
                    .into_string()
                    .map_err(|_| Failure::new(ErrorCode::InvalidPath, "Non-UTF-8 file name"))?;
                // A portable path is required even for host recovery, including device and stream aliases.
                FileTarget {
                    root: ResourceHandle {
                        instance: "validation".into(),
                        scope: "validation".into(),
                        resource: 0,
                    },
                    path: name.clone(),
                }
                .validate()?;
                entries.insert(name, Self::capture(&entry.path(), budget, depth + 1, stop)?);
            }
            for (name, snapshot) in &entries {
                digest.update(name);
                digest.update(format!("{:?}", snapshot.stamp));
            }
            Ok(Self {
                stamp: FileStamp::Directory {
                    version: format!("{:x}", digest.finalize()),
                    readonly: permissions.readonly(),
                },
                node: Node::Directory {
                    entries,
                    permissions,
                },
            })
        } else {
            Err(Failure::new(
                ErrorCode::UnsupportedOperation,
                "Only regular files and directories are supported",
            ))
        }
    }

    pub(super) fn missing() -> Self {
        Self {
            stamp: FileStamp::Missing,
            node: Node::Missing,
        }
    }

    /// New files inherit the native temporary file's regular-file attributes on actual creation.
    /// Windows directory attributes are not valid file permissions; never copy them from a parent.
    pub(super) fn file(bytes: Vec<u8>) -> Self {
        Self {
            stamp: FileStamp::File {
                version: String::new(),
                bytes: bytes.len() as u64,
                readonly: false,
            },
            node: Node::File {
                bytes: bytes.into(),
                permissions: None,
            },
        }
    }

    pub(super) fn directory(path: &Path) -> Result<Self, Failure> {
        let parent = nearest_existing_parent(path)?;
        Ok(Self {
            stamp: FileStamp::Directory {
                version: String::new(),
                readonly: false,
            },
            node: Node::Directory {
                entries: BTreeMap::new(),
                permissions: std::fs::metadata(parent).map_err(io)?.permissions(),
            },
        })
    }

    pub(super) fn bytes(&self) -> Result<&[u8], Failure> {
        match &self.node {
            Node::File { bytes, .. } => Ok(bytes),
            _ => Err(Failure::new(
                ErrorCode::UnsupportedOperation,
                "Target is not a regular file",
            )),
        }
    }

    pub(super) fn readonly(&self) -> bool {
        matches!(
            self.stamp,
            FileStamp::File { readonly: true, .. } | FileStamp::Directory { readonly: true, .. }
        )
    }
}

/// No project temporary exists before confirmation. Persist is atomic for one file on its own volume.
pub(super) fn install_file(
    path: &Path,
    bytes: &[u8],
    permissions: &Option<std::fs::Permissions>,
    missing: bool,
    stop: &impl Fn() -> bool,
) -> Result<(), Failure> {
    ensure_running(stop)?;
    let _pins = parent_pin(path)?;
    let mut temporary = tempfile::NamedTempFile::new_in(
        path.parent()
            .ok_or_else(|| conflict("Target has no parent"))?,
    )
    .map_err(io)?;
    temporary
        .write_all(bytes)
        .and_then(|_| temporary.as_file().sync_all())
        .map_err(io)?;
    if let Some(permissions) = permissions {
        temporary
            .as_file()
            .set_permissions(permissions.clone())
            .map_err(io)?;
    }
    ensure_running(stop)?;
    if missing {
        temporary
            .persist_noclobber(path)
            .map_err(|error| io(error.error))?;
    } else {
        temporary.persist(path).map_err(|error| io(error.error))?;
    }
    Ok(())
}

/// Restoration traverses only captured portable children and rejects new links at every actual step.
pub(super) fn restore(
    snapshot: &Snapshot,
    path: &Path,
    stop: &impl Fn() -> bool,
) -> Result<(), Failure> {
    ensure_running(stop)?;
    let _pins = parent_pin(path)?;
    match &snapshot.node {
        Node::Missing => remove(path, stop),
        Node::File { bytes, permissions } => {
            let missing = !path.try_exists().map_err(io)?;
            if path.is_dir() {
                remove(path, stop)?;
            }
            install_file(path, bytes, permissions, missing || !path.exists(), stop)
        }
        Node::Directory {
            entries,
            permissions,
        } => {
            if path.try_exists().map_err(io)? {
                remove(path, stop)?;
            }
            std::fs::create_dir(path).map_err(io)?;
            for (name, child) in entries {
                restore(child, &path.join(name), stop)?;
            }
            std::fs::set_permissions(path, permissions.clone()).map_err(io)
        }
    }
}

/// Recursive deletes do not follow links; each parent remains pinned while a child is removed.
pub(super) fn remove(path: &Path, stop: &impl Fn() -> bool) -> Result<(), Failure> {
    ensure_running(stop)?;
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(io(error)),
    };
    if is_reparse(&metadata) {
        return Err(Failure::new(
            ErrorCode::InvalidPath,
            "Removal target became a link",
        ));
    }
    let _pins = parent_pin(path)?;
    if metadata.is_dir() {
        for child in std::fs::read_dir(path).map_err(io)? {
            remove(&child.map_err(io)?.path(), stop)?;
        }
        std::fs::remove_dir(path).map_err(io)
    } else {
        std::fs::remove_file(path).map_err(io)
    }
}

fn parent_pin(path: &Path) -> Result<crate::native_path::CheckedPath, Failure> {
    let mut pins = pin_path(path, false).or_else(|error| {
        if error.code == ErrorCode::NotFound {
            pin_path(path, true)
        } else {
            Err(error)
        }
    })?;
    pins.release_leaf();
    Ok(pins)
}

fn nearest_existing_parent(path: &Path) -> Result<&Path, Failure> {
    path.ancestors()
        .skip(1)
        .find(|parent| parent.is_dir())
        .ok_or_else(|| conflict("Parent is unavailable"))
}
