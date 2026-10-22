//! Known-path migration verifies the target before retiring the sole original snapshot.

use super::LegacyEntry;
use crate::store::path_text;
use crate::{Capture, Error, FileId, History, Reason, Result, VersionId};
use rusqlite::{OptionalExtension, params};
use sha2::{Digest, Sha256};
use std::path::Path;

impl History {
    /// Lazily associate eligible retired snapshots with a host-observed original path.
    /// Unsupported or oversized sources remain available in the unassociated legacy category.
    pub fn import_known(&self, original: &Path, workspace: Option<&Path>) -> Result<u64> {
        self.import_known_inner(original, workspace, None)
    }

    /// A reliable native move proves that the old path spelling belongs to file's current identity.
    /// Import old times/paths without moving its present binding back to that retired location.
    pub fn import_known_into(
        &self,
        original: &Path,
        workspace: Option<&Path>,
        file: FileId,
    ) -> Result<u64> {
        self.location(file)?;
        self.import_known_inner(original, workspace, Some(file))
    }

    /// Eligibility and ambiguity rules apply equally to live paths and proven moved identities.
    fn import_known_inner(
        &self,
        original: &Path,
        workspace: Option<&Path>,
        file: Option<FileId>,
    ) -> Result<u64> {
        let policy = self.policy()?;
        if policy.skip_path(original).is_some() {
            return Ok(0);
        }
        if self.legacy_path_is_ambiguous(original)? {
            return Ok(0);
        }
        let hash = original
            .to_string_lossy()
            .bytes()
            .fold(0xcbf29ce484222325_u64, |hash, byte| {
                (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
            });
        let mut query = super::LegacyQuery {
            filter: format!("{hash:016x}/"),
            ..Default::default()
        };
        let mut imported = 0;
        loop {
            let entries = self.legacy(&query)?;
            if entries.is_empty() {
                break;
            }
            query.before = entries.last().map(LegacyEntry::cursor);
            for entry in entries {
                if entry.size > policy.max_file_bytes || entry.at_ms.is_none() {
                    continue;
                }
                let bytes = self.read_legacy(&entry)?;
                if policy.skip(original, &bytes).is_some() {
                    continue;
                }
                self.import_legacy_inner(&entry, original, workspace, file)?;
                imported += 1;
            }
        }
        Ok(imported)
    }

    /// Associate entry with the supplied original path only when its retired path hash matches.
    pub fn import_legacy(
        &self,
        entry: &LegacyEntry,
        original: &Path,
        workspace: Option<&Path>,
    ) -> Result<VersionId> {
        self.import_legacy_inner(entry, original, workspace, None)
    }

    /// Commit and verify the source association before its native deletion intent becomes eligible.
    fn import_legacy_inner(
        &self,
        entry: &LegacyEntry,
        original: &Path,
        workspace: Option<&Path>,
        file: Option<FileId>,
    ) -> Result<VersionId> {
        let _activity = self.activity.lock().map_err(|_| Error::Unavailable)?;
        let hash = original
            .to_string_lossy()
            .bytes()
            .fold(0xcbf29ce484222325_u64, |hash, byte| {
                (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
            });
        if !entry.id.starts_with(&format!("{hash:016x}/")) {
            return Err(Error::InvalidPath);
        }
        self.legacy_path(&entry.id)?;
        let source = path_text(original)?;
        let imported: Option<(String, Option<i64>)> = self
            .lock()?
            .query_row(
                "SELECT source_path,version FROM imports WHERE id=?1",
                [&entry.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if let Some((previous, version)) = imported {
            if previous != source {
                return Err(Error::InvalidPath);
            }
            let version = VersionId(version.ok_or(Error::Stale)?);
            self.read(version)?;
            self.drain_legacy_gc()?;
            return Ok(version);
        }
        if self.legacy_path_is_ambiguous(original)? {
            return Err(Error::Stale);
        }
        let bytes = self.read_legacy_inner(entry)?;
        let digest = Sha256::digest(&bytes).to_vec();
        let at_ms = entry.at_ms.ok_or(Error::Stale)?;
        // An interruption after the immutable version commit but before the ledger is repairable:
        // original path, original time and content key identify the same retired snapshot request.
        let existing: Option<i64> = self.lock()?.query_row(
            "SELECT id FROM versions WHERE path=?1 AND at_ms=?2 AND object=?3 AND reason='imported' LIMIT 1",
            params![source,at_ms,digest], |row| row.get(0),
        ).optional()?;
        let version = match existing {
            Some(id) => VersionId(id),
            None => self
                .capture_inner(
                    Capture {
                        file,
                        path: original.to_owned(),
                        workspace: workspace.map(Path::to_owned),
                        contents: bytes.clone(),
                        reason: Reason::Imported,
                        at_ms,
                    },
                    Some(&entry.id),
                    None,
                )?
                .version()
                .ok_or(Error::Stale)?,
        };
        if self.read(version)? != bytes {
            return Err(Error::Corrupt);
        }
        let mut connection = self.lock()?;
        let tx = connection.transaction()?;
        tx.execute(
            "INSERT INTO imports(id,source_path,hash,version) VALUES(?1,?2,?3,?4)",
            params![entry.id, source, digest, version.0],
        )?;
        tx.execute(
            "INSERT OR IGNORE INTO legacy_gc(id,raw_size,modified_ns) VALUES(?1,?2,?3)",
            params![entry.id, entry.size as i64, entry.modified_ns],
        )?;
        tx.execute("DELETE FROM legacy WHERE id=?1", [&entry.id])?;
        if !original.exists() {
            tx.execute(
                "UPDATE files SET alive=0 WHERE id=(SELECT file_id FROM versions WHERE id=?1) AND path=?2",
                params![version.0,source],
            )?;
        }
        tx.commit()?;
        drop(connection);
        self.drain_legacy_gc()?;
        Ok(version)
    }

    /// A retired path hash cannot distinguish independent incarnations already known at one path.
    fn legacy_path_is_ambiguous(&self, original: &Path) -> Result<bool> {
        self.lock()?
            .query_row(
                "SELECT count(DISTINCT file_id)>1 FROM versions WHERE path=?1 OR previous_path=?1",
                [path_text(original)?],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }
}
