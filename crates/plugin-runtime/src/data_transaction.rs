//! A durable journal couples one private scope with its package registry; business data stays opaque.
use crate::package::atomic_write;
use anyhow::{Context, ensure};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize)]
struct Journal {
    /// Optional host metadata travels with registry/private-data commit and rollback.
    #[serde(default)]
    previous_files: std::collections::BTreeMap<PathBuf, Option<Vec<u8>>>,
    scope: PathBuf,
    had_scope: bool,
    previous_registry: Option<Vec<u8>>,
    committed: bool,
}

/// Unpublished copies are disposable; once a journal exists only recovery may remove their backups.
pub(crate) struct Transaction {
    _lock: std::fs::File,
    root: PathBuf,
    directory: PathBuf,
    scope: PathBuf,
}
impl Transaction {
    pub(crate) fn new(root: &Path, scope: &Path) -> anyhow::Result<Self> {
        let relative = scope.strip_prefix(root)?.to_owned();
        validate_scope(root, &relative)?;
        let transactions = root.join("data-transactions");
        std::fs::create_dir_all(&transactions)?;
        let lock = open_lock(root)?;
        lock.try_lock()
            .context("Another data transaction is active")?;
        let directory = tempfile::Builder::new()
            .prefix("update-")
            .tempdir_in(transactions)?
            .keep();
        Ok(Self {
            _lock: lock,
            root: root.into(),
            directory,
            scope: relative,
        })
    }
    pub(crate) fn candidate(&self) -> PathBuf {
        self.directory.join("candidate")
    }
    /// A globally installed package may still have no data in the current logical workspace.
    pub(crate) fn source_exists(&self) -> bool {
        self.root.join(&self.scope).exists()
    }
    /// The held transaction lock makes this registry the base for the eventual whole-file replacement.
    pub(crate) fn registry(&self) -> anyhow::Result<Option<Vec<u8>>> {
        read_optional(&self.root.join("registry.json"))
    }
    /// A surviving journal means rollback could not finish; no guest may resume against uncertain files.
    pub(crate) fn recovery_pending(&self) -> bool {
        self.directory.join("journal.json").exists()
    }
    /// The manager's exclusive cutover turn takes this final copy after all earlier guest writes.
    pub(crate) fn refresh(&self) -> anyhow::Result<()> {
        validate_scope(&self.root, &self.scope)?;
        let candidate = self.candidate();
        if candidate.exists() {
            std::fs::remove_dir_all(&candidate)?;
        }
        std::fs::create_dir_all(&candidate)?;
        let source = self.root.join(&self.scope);
        if source.exists() {
            copy_tree(&source, &candidate, &mut (0, 0), 0)?;
        }
        std::fs::create_dir_all(candidate.join("files"))?;
        Ok(())
    }
    /// Journal first, then directory swap, registry, commit marker; only then discard the rollback copy.
    pub(crate) fn publish(
        &self,
        registry: &[u8],
        control: &crate::InstallControl,
    ) -> anyhow::Result<()> {
        self.publish_metadata(registry, &Default::default(), control)
    }
    /// Couple settings/provider choices with the same durable marker as the package registry.
    /// Only bounded host metadata paths are accepted; plugin private writes still use the scope copy.
    pub(crate) fn publish_metadata(
        &self,
        registry: &[u8],
        files: &std::collections::BTreeMap<PathBuf, Vec<u8>>,
        control: &crate::InstallControl,
    ) -> anyhow::Result<()> {
        validate_scope(&self.root, &self.scope)?;
        ensure!(
            files.len() <= 16 && files.values().map(Vec::len).sum::<usize>() <= 4 * 1024 * 1024,
            "Replacement metadata exceeds quota"
        );
        let mut previous_files = std::collections::BTreeMap::new();
        let mut previous_size = 0u64;
        for path in files.keys() {
            validate_metadata(&self.root, path)?;
            if let Ok(metadata) = std::fs::metadata(self.root.join(path)) {
                previous_size = previous_size
                    .checked_add(metadata.len())
                    .context("Metadata quota overflow")?;
                ensure!(
                    previous_size <= 4 * 1024 * 1024,
                    "Previous replacement metadata exceeds quota"
                );
            }
            previous_files.insert(path.clone(), read_optional(&self.root.join(path))?);
        }
        let target = self.root.join(&self.scope);
        let mut journal = Journal {
            previous_files,
            scope: self.scope.clone(),
            had_scope: target.exists(),
            previous_registry: read_optional(&self.root.join("registry.json"))?,
            committed: false,
        };
        atomic_write(
            &self.directory.join("journal.json"),
            &serde_json::to_vec(&journal)?,
        )?;
        let result = (|| {
            std::fs::create_dir_all(target.parent().unwrap())?;
            if journal.had_scope {
                std::fs::rename(&target, self.directory.join("backup"))?;
            }
            std::fs::rename(self.candidate(), &target)?;
            control.stage(crate::InstallStage::Committing)?;
            for (path, bytes) in files {
                let target = self.root.join(path);
                std::fs::create_dir_all(target.parent().unwrap())?;
                atomic_write(&target, bytes)?;
            }
            atomic_write(&self.root.join("registry.json"), registry)?;
            journal.committed = true;
            atomic_write(
                &self.directory.join("journal.json"),
                &serde_json::to_vec(&journal)?,
            )?;
            // Cancellation after the durable marker cannot undo a committed transaction.
            let _ = control.stage(crate::InstallStage::Committed);
            Ok::<_, anyhow::Error>(())
        })();
        if let Err(error) = result {
            journal.committed = false;
            return match restore(&self.root, &self.directory, &journal) {
                Ok(()) => Err(error),
                Err(recovery) => Err(anyhow::anyhow!(
                    "Commit failed: {error:#}; recovery incomplete, data retained at {}: {recovery:#}",
                    platform_windows::display_path(&self.directory)
                )),
            };
        }
        // Cleanup is best effort after the durable commit; startup completes it without undoing success.
        let _ = std::fs::remove_dir_all(&self.directory);
        Ok(())
    }
}
impl Drop for Transaction {
    fn drop(&mut self) {
        if !self.directory.join("journal.json").exists() {
            let _ = std::fs::remove_dir_all(&self.directory);
        }
    }
}

/// Recovery precedes registry reads and guest activation; an unreadable journal is never silently discarded.
pub(crate) fn recover(root: &Path) -> anyhow::Result<Option<std::fs::File>> {
    let transactions = root.join("data-transactions");
    if !root.exists() {
        return Ok(None);
    }
    let lock = open_lock(root)?;
    match lock.try_lock() {
        Ok(()) => {}
        Err(std::fs::TryLockError::WouldBlock) => {
            anyhow::bail!("Private-data update is active; retry after commit")
        }
        Err(error) => return Err(error.into()),
    }
    if !transactions.exists() {
        return Ok(Some(lock));
    }
    ensure!(
        !std::fs::symlink_metadata(&transactions)?
            .file_type()
            .is_symlink(),
        "Transaction root is linked"
    );
    for entry in std::fs::read_dir(&transactions)? {
        let entry = entry?;
        ensure!(
            entry.file_type()?.is_dir()
                && entry.file_name().to_string_lossy().starts_with("update-"),
            "Unexpected transaction entry"
        );
        let directory = entry.path();
        if let Some(bytes) = read_optional(&directory.join("journal.json"))? {
            let journal: Journal = serde_json::from_slice(&bytes)
                .context("Invalid recovery journal; data preserved")?;
            restore(root, &directory, &journal).with_context(|| {
                format!(
                    "Recovery incomplete at {}; data preserved",
                    platform_windows::display_path(&directory)
                )
            })?;
        } else {
            std::fs::remove_dir_all(directory)?;
        }
    }
    Ok(Some(lock))
}

/// Backup/candidate existence distinguishes interruptions between a rename and its next durable write.
fn restore(root: &Path, directory: &Path, journal: &Journal) -> anyhow::Result<()> {
    validate_scope(root, &journal.scope)?;
    ensure!(
        journal.previous_files.len() <= 16
            && journal
                .previous_files
                .values()
                .filter_map(Option::as_ref)
                .map(Vec::len)
                .sum::<usize>()
                <= 4 * 1024 * 1024,
        "Recovery metadata exceeds quota"
    );
    if journal.committed {
        ensure!(
            root.join(&journal.scope).is_dir() && root.join("registry.json").is_file(),
            "Committed data or registry is missing; recovery backup retained"
        );
    }
    if !journal.committed {
        for (path, bytes) in &journal.previous_files {
            validate_metadata(root, path)?;
            let target = root.join(path);
            if let Some(bytes) = bytes {
                std::fs::create_dir_all(target.parent().unwrap())?;
                atomic_write(&target, bytes)?;
            } else if target.exists() {
                std::fs::remove_file(target)?;
            }
        }
        let target = root.join(&journal.scope);
        let backup = directory.join("backup");
        if backup.exists() {
            // Copy, do not consume the backup: a second crash during rollback can retry safely.
            if target.exists() {
                std::fs::remove_dir_all(&target)?;
            }
            std::fs::create_dir_all(&target)?;
            copy_tree(&backup, &target, &mut (0, 0), 0)?;
        } else if journal.had_scope {
            ensure!(
                directory.join("candidate").is_dir() && target.is_dir(),
                "Rollback backup is missing; remaining data retained"
            );
        } else if !directory.join("candidate").exists() && target.exists() {
            std::fs::remove_dir_all(&target)?;
        }
        if let Some(bytes) = &journal.previous_registry {
            atomic_write(&root.join("registry.json"), bytes)?;
        } else if root.join("registry.json").exists() {
            std::fs::remove_file(root.join("registry.json"))?;
        }
    }
    std::fs::remove_dir_all(directory)?;
    Ok(())
}

/// Recovery journals may touch only specific host stores, never arbitrary workspace/OS paths.
fn validate_metadata(root: &Path, path: &Path) -> anyhow::Result<()> {
    let value = path.to_string_lossy().replace('\\', "/");
    crate::package::validate_relative(&value)?;
    ensure!(
        value == "service-providers.json"
            || ((value.starts_with("settings/") || value.starts_with("replacement-history/"))
                && value.split('/').count() == 2
                && value.ends_with(".json")),
        "Invalid replacement metadata path"
    );
    crate::migration::reject_links(&root.join(path))?;
    Ok(())
}

/// Journal paths must name a private scope, never an arbitrary path supplied by persisted data.
fn validate_scope(root: &Path, scope: &Path) -> anyhow::Result<()> {
    let parts = scope
        .iter()
        .map(|part| part.to_string_lossy())
        .collect::<Vec<_>>();
    ensure!(
        scope
            .components()
            .all(|part| matches!(part, std::path::Component::Normal(_)))
            && ((parts.len() == 3 && parts[2] == "application")
                || (parts.len() == 4
                    && parts[2] == "workspaces"
                    && parts[3].len() == 64
                    && parts[3].bytes().all(|b| b.is_ascii_hexdigit())))
            && parts[0] == "data"
            && !parts[1].is_empty()
            && parts[1]
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b".-".contains(&b)),
        "Invalid private scope recovery path"
    );
    let mut path = root.to_owned();
    for part in scope {
        path.push(part);
        if path.exists() {
            ensure!(
                !std::fs::symlink_metadata(&path)?.file_type().is_symlink(),
                "Linked private scope is not migratable"
            );
        }
    }
    Ok(())
}

/// Copy only bounded regular files; native connections and linked/project files cannot become rollback data.
pub(crate) fn copy_tree(
    source: &Path,
    target: &Path,
    budget: &mut (u64, usize),
    depth: usize,
) -> anyhow::Result<()> {
    ensure!(
        depth <= 16 && !std::fs::symlink_metadata(source)?.file_type().is_symlink(),
        "Linked or excessively nested data cannot be migrated"
    );
    for entry in std::fs::read_dir(source)? {
        budget.1 += 1;
        ensure!(
            budget.1 <= 16_384,
            "Private migration file count exceeds quota"
        );
        let entry = entry?;
        let kind = entry.file_type()?;
        ensure!(!kind.is_symlink(), "Linked data cannot be migrated");
        let next = target.join(entry.file_name());
        if kind.is_dir() {
            std::fs::create_dir_all(&next)?;
            copy_tree(&entry.path(), &next, budget, depth + 1)?;
        } else {
            ensure!(kind.is_file(), "Only regular files can be migrated");
            budget.0 += entry.metadata()?.len();
            ensure!(
                budget.0 <= 128 * 1024 * 1024,
                "Private migration copy exceeds quota"
            );
            atomic_write(&next, &std::fs::read(entry.path())?)?;
        }
    }
    Ok(())
}
pub(crate) fn read_optional(path: &Path) -> anyhow::Result<Option<Vec<u8>>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

/// Registry-only readers must not mistake a live preparation for an interrupted transaction.
fn open_lock(root: &Path) -> anyhow::Result<std::fs::File> {
    Ok(std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(root.join("data-transactions.lock"))?)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A restart observes the same version of private data, registry and replacement metadata;
    /// only the durable commit marker decides whether partially published files survive.
    #[test]
    fn restart_recovers_replacement_metadata_with_its_private_scope() {
        for committed in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let scope = PathBuf::from("data/replacement/application");
            let target = root.path().join(&scope);
            let directory = root.path().join("data-transactions/update-interrupted");
            std::fs::create_dir_all(target.join("files")).unwrap();
            std::fs::create_dir_all(directory.join("backup/files")).unwrap();
            std::fs::write(target.join("files/private"), b"new").unwrap();
            std::fs::write(directory.join("backup/files/private"), b"old").unwrap();
            let files = std::collections::BTreeMap::from([
                (
                    PathBuf::from("settings/replacement.json"),
                    Some(b"old".to_vec()),
                ),
                (
                    PathBuf::from("service-providers.json"),
                    Some(b"old".to_vec()),
                ),
                (PathBuf::from("replacement-history/version.json"), None),
            ]);
            for path in files.keys() {
                let target = root.path().join(path);
                std::fs::create_dir_all(target.parent().unwrap()).unwrap();
                std::fs::write(target, b"new").unwrap();
            }
            std::fs::write(root.path().join("registry.json"), b"new").unwrap();
            let journal = Journal {
                previous_files: files.clone(),
                scope,
                had_scope: true,
                previous_registry: Some(b"old".to_vec()),
                committed,
            };
            // Simulate process loss after publication, before cleanup. Recovery runs before the
            // manager reads a registry or activates any guest, and must be safe to invoke again.
            atomic_write(
                &directory.join("journal.json"),
                &serde_json::to_vec(&journal).unwrap(),
            )
            .unwrap();
            drop(recover(root.path()).unwrap());
            let expected = if committed { b"new" } else { b"old" };
            assert_eq!(
                std::fs::read(target.join("files/private")).unwrap(),
                expected
            );
            assert_eq!(
                std::fs::read(root.path().join("registry.json")).unwrap(),
                expected
            );
            for (path, previous) in files {
                assert_eq!(
                    read_optional(&root.path().join(path)).unwrap(),
                    if committed {
                        Some(b"new".to_vec())
                    } else {
                        previous
                    }
                );
            }
            assert!(!directory.exists());
            drop(recover(root.path()).unwrap());
        }
    }
}
