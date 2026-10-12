//! Ordered native disk plans use sealed roots, immutable preimages and observable partial receipts.
//! Preparation performs no writes. Recovery checks every captured target before starting restoration.

use crate::{AuthorizedFile, WorkspaceRequest};
use plugin_protocol::{
    api::{ErrorCode, Failure, ResourceHandle},
    workspace::{Change, FileStamp, FileTarget},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::Arc,
};

mod preparation;
mod queries;
mod snapshot;
use snapshot::{Budget, Node, Snapshot};
const MAX_BACKUP_BYTES: usize = 32 * 1024 * 1024;

/// One prepared action addresses native paths only after the runtime issued exact directional proofs.
enum Action {
    Buffer(AuthorizedFile),
    Create(AuthorizedFile),
    Write(AuthorizedFile, Snapshot),
    Copy(AuthorizedFile, AuthorizedFile, Snapshot),
    Rename(AuthorizedFile, AuthorizedFile),
    Delete(AuthorizedFile),
}

/// This disk-only journal cannot replace a DocumentSession or a native document's Undo stack.
pub struct FilePlan {
    files: BTreeMap<PathBuf, AuthorizedFile>,
    original: BTreeMap<PathBuf, Snapshot>,
    current: BTreeMap<PathBuf, Snapshot>,
    actions: BTreeMap<u16, Action>,
    mutated: BTreeSet<PathBuf>,
    observed: bool,
    /// Reserve the largest projected post-state before confirmation, so every partial step stays observable.
    reserved_bytes: usize,
}

impl FilePlan {
    /// All disk targets, including absent destinations, are checked before any mixed-plan effect.
    pub fn preflight(&self, stop: &impl Fn() -> bool) -> Result<(), Failure> {
        self.validate(&self.original, stop)
    }
    /// Native text steps share the same current disk preconditions without issuing a filesystem mutation.
    pub fn check_current(&self, stop: &impl Fn() -> bool) -> Result<(), Failure> {
        self.validate(&self.current, stop)
    }

    /// A confirmed operation reports its real outcome; a failed directory step may still have effects.
    /// Post-state inspection is retained even after failure, so Undo never blindly assumes a rollback.
    pub fn apply(&mut self, index: u16, stop: &impl Fn() -> bool) -> Result<(), Failure> {
        self.validate(&self.current, stop)?;
        let action = self
            .actions
            .get(&index)
            .ok_or_else(|| Failure::new(ErrorCode::InvalidRequest, "Unknown file operation"))?;
        let result = match action {
            Action::Buffer(file) => {
                let _guard = file.guard()?;
                Ok(())
            }
            Action::Create(file) => {
                let _guard = file.guard()?;
                ensure_running(stop)?;
                std::fs::create_dir(file.path()).map_err(io)
            }
            Action::Write(file, snapshot) => {
                let _guard = file.guard()?;
                let Node::File { bytes, permissions } = &snapshot.node else {
                    unreachable!()
                };
                snapshot::install_file(
                    file.path(),
                    bytes,
                    permissions,
                    matches!(self.current[file.path()].node, Node::Missing),
                    stop,
                )
            }
            Action::Copy(source, target, snapshot) => {
                let _source_guard = source.guard()?;
                let _target_guard = target.guard()?;
                snapshot::restore(snapshot, target.path(), stop)
            }
            Action::Rename(source, target) => {
                let _source_guard = source.guard()?;
                let _target_guard = target.guard()?;
                ensure_running(stop)?;
                // Missing destinations were checked above; Windows rename never clobbers a new target.
                rename_no_replace(source.path(), target.path())
            }
            Action::Delete(file) => {
                let _guard = file.guard()?;
                snapshot::remove(file.path(), stop)
            }
        };
        // Cancellation stops more mutations, but must not prevent recording already performed IO.
        self.observe()?;
        result
    }

    /// Returning conflicts performs no writes, so the native coordinator can check all buffers as well.
    pub fn undo_conflicts(&self) -> Vec<(FileTarget, String)> {
        if !self.observed {
            return self
                .files
                .values()
                .map(|file| {
                    (
                        file.target().clone(),
                        "Post-state could not be inspected".into(),
                    )
                })
                .collect();
        }
        let mut budget = Budget::default();
        self.files
            .iter()
            .filter_map(|(path, file)| {
                let result = file
                    .guard()
                    .and_then(|_guard| Snapshot::capture(path, &mut budget, 0, &|| false));
                match result {
                    Ok(snapshot) if snapshot.stamp == self.current[path].stamp => None,
                    Ok(_) => Some((
                        file.target().clone(),
                        "Disk target was edited or replaced".into(),
                    )),
                    Err(error) => Some((file.target().clone(), error.message)),
                }
            })
            .collect()
    }

    /// Restore only after the host completes the all-target conflict check. Actual failures remain partial.
    pub fn undo(&mut self, stop: &impl Fn() -> bool) -> Result<(), Failure> {
        self.validate(&self.current, stop)?;
        let mut roots: Vec<_> = self
            .mutated
            .iter()
            .filter(|path| {
                !self
                    .mutated
                    .iter()
                    .any(|parent| parent != *path && path.starts_with(parent))
            })
            .cloned()
            .collect();
        // Remove originally absent targets first; then restore original objects and their permissions.
        roots.sort_by_key(|path| {
            (
                !matches!(self.original[path].node, Node::Missing),
                std::cmp::Reverse(path.components().count()),
            )
        });
        let result = (|| {
            for path in roots {
                ensure_running(stop)?;
                let _guard = self.files[&path].guard()?;
                snapshot::restore(&self.original[&path], &path, stop)?;
            }
            Ok(())
        })();
        self.observe()?;
        result
    }

    /// Immutable original targets and observed post-states identify real partial disk changes.
    pub fn affected(&self) -> Vec<FileTarget> {
        self.files
            .iter()
            .filter(|(path, _)| self.original[*path].stamp != self.current[*path].stamp)
            .map(|(_, file)| file.target().clone())
            .collect()
    }
    /// Recovery is available only if every post-state is known; inability to inspect is never hidden.
    pub fn recoverable(&self) -> bool {
        self.observed
    }

    /// Recovery reserves original bytes and the largest projected post-state before any side effect.
    /// The native coordinator adds its text preimages; later disk growth cannot exceed that reservation.
    pub fn retained_bytes(&self) -> usize {
        self.reserved_bytes
    }

    fn observe(&mut self) -> Result<(), Failure> {
        self.observed = false;
        let mut budget = Budget::default();
        let mut snapshots = BTreeMap::new();
        for (path, file) in &self.files {
            let _guard = file.guard()?;
            snapshots.insert(
                path.clone(),
                Snapshot::capture(path, &mut budget, 0, &|| false)?,
            );
        }
        self.current = snapshots;
        self.observed = true;
        Ok(())
    }

    fn validate(
        &self,
        expected: &BTreeMap<PathBuf, Snapshot>,
        stop: &impl Fn() -> bool,
    ) -> Result<(), Failure> {
        if !self.observed {
            return Err(conflict(
                "Unknown post-state cannot be automatically restored",
            ));
        }
        let mut budget = Budget::default();
        for (path, file) in &self.files {
            let _guard = file.guard()?;
            let current = Snapshot::capture(path, &mut budget, 0, stop)?;
            if current.stamp != expected[path].stamp {
                return Err(conflict(&format!(
                    "Disk target changed: {}",
                    file.target().path
                )));
            }
        }
        Ok(())
    }
}

pub use queries::{discover, read};

fn rename_no_replace(source: &Path, target: &Path) -> Result<(), Failure> {
    if target.try_exists().map_err(io)? {
        return Err(conflict("Rename destination appeared"));
    }
    #[cfg(windows)]
    {
        std::fs::rename(source, target).map_err(io)
    }
    #[cfg(not(windows))]
    {
        Err(Failure::new(
            ErrorCode::UnsupportedOperation,
            "No-clobber resource rename requires Windows",
        ))
    }
}
fn ensure_running(stop: &impl Fn() -> bool) -> Result<(), Failure> {
    if stop() {
        Err(Failure::new(
            ErrorCode::Cancelled,
            "Further file work stopped",
        ))
    } else {
        Ok(())
    }
}
fn limit(message: &str) -> Failure {
    Failure::new(ErrorCode::LimitExceeded, message)
}
fn conflict(message: &str) -> Failure {
    Failure::new(ErrorCode::Conflict, message)
}
fn io(error: std::io::Error) -> Failure {
    crate::native_path::io_failure(error)
}
