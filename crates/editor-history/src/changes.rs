//! Structural events preserve file identity without duplicating unchanged content.

use crate::store::path_text;
use crate::{Change, Error, FileId, History, Result, VersionId};
use rusqlite::{OptionalExtension, params};
use std::path::Path;

impl History {
    /// Resolve a logical file's current or last path; old versions never redirect it to an old name.
    pub fn location(&self, file: FileId) -> Result<crate::Location> {
        self.lock()?
            .query_row(
                "SELECT path,workspace,alive FROM files WHERE id=?1",
                [file.0],
                |row| {
                    Ok(crate::Location {
                        path: std::path::PathBuf::from(row.get::<_, String>(0)?),
                        workspace: row.get::<_, Option<String>>(1)?.map(Into::into),
                        present: row.get(2)?,
                    })
                },
            )
            .map_err(Into::into)
    }

    /// Bind a known existing file without fabricating content. Disabled/excluded paths are refused.
    pub fn track(&self, path: &Path, workspace: Option<&Path>, at_ms: i64) -> Result<FileId> {
        let _activity = self.activity.lock().map_err(|_| Error::Unavailable)?;
        if path
            .canonicalize()
            .unwrap_or_else(|_| path.to_owned())
            .starts_with(&*self.root)
        {
            return Err(Error::ProtectionSkipped(crate::SkipReason::Excluded));
        }
        if let Some(reason) = self.policy()?.skip_path(path) {
            return Err(Error::ProtectionSkipped(reason));
        }
        let path = path_text(path)?;
        let workspace = workspace.map(path_text).transpose()?;
        let mut connection = self.lock()?;
        self.sync_leases(&connection)?;
        let tx = connection.transaction()?;
        let file = tx
            .query_row(
                "SELECT id FROM files WHERE path=?1 AND alive=1",
                [&path],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        let file = match file {
            Some(file) => file,
            None => {
                tx.execute(
                    "INSERT INTO files(path,workspace) VALUES(?1,?2)",
                    params![path, workspace],
                )?;
                let file = tx.last_insert_rowid();
                // A metadata baseline makes this binding a retained observation, rather than an
                // orphan removed by the next unrelated capture. No empty text is fabricated.
                tx.execute("INSERT INTO versions(file_id,path,workspace,reason,at_ms) VALUES(?1,?2,?3,'external',?4)",
                    params![file,path,workspace,at_ms])?;
                self.trim_budget(&tx, Some(VersionId(tx.last_insert_rowid())), None)?;
                file
            }
        };
        tx.commit()?;
        Ok(FileId(file))
    }

    /// Resolve live identities within a file or directory, respecting actual path-component boundaries.
    pub fn bindings_under(&self, root: &Path) -> Result<Vec<(FileId, std::path::PathBuf)>> {
        let path = path_text(root)?;
        let escaped = path
            .replace('!', "!!")
            .replace('%', "!%")
            .replace('_', "!_");
        let prefix = format!(
            "{}{}%",
            escaped.trim_end_matches(std::path::MAIN_SEPARATOR),
            std::path::MAIN_SEPARATOR
        );
        let connection = self.lock()?;
        let mut statement = connection.prepare("SELECT id,path FROM files WHERE alive=1 AND (path=?1 OR path LIKE ?2 ESCAPE '!') ORDER BY path")?;
        let mut rows = statement.query(params![path, prefix])?;
        let mut result = Vec::new();
        let mut retained = 0_usize;
        while let Some(row) = rows.next()? {
            let file = FileId(row.get(0)?);
            let path = std::path::PathBuf::from(row.get::<_, String>(1)?);
            retained += path.as_os_str().len().saturating_mul(2) + 128;
            if retained > 32 * 1024 * 1024 {
                return Err(Error::Busy);
            }
            result.push((file, path));
        }
        Ok(result)
    }

    /// Record a successful change and atomically update its live path binding.
    pub fn change(&self, change: Change) -> Result<VersionId> {
        self.change_inner(change, true)
    }

    /// Observe structure when current bytes are unsupported; an older text object is not its preimage.
    pub fn change_without_content(&self, change: Change) -> Result<VersionId> {
        self.change_inner(change, false)
    }

    /// Structural events either retain a verified head or explicitly revoke a stale content hint.
    fn change_inner(&self, change: Change, inherit_content: bool) -> Result<VersionId> {
        let _activity = self.activity.lock().map_err(|_| Error::Unavailable)?;
        if !self.policy()?.enabled {
            return Err(Error::ProtectionSkipped(crate::SkipReason::Disabled));
        }
        let path = path_text(&change.path)?;
        let workspace = change.workspace.as_deref().map(path_text).transpose()?;
        let mut connection = self.lock()?;
        self.sync_leases(&connection)?;
        let tx = connection.transaction()?;
        // The head is only a lookup hint: expired objects are not kept alive by the binding.
        let previous: String = tx.query_row(
            "SELECT path FROM files WHERE id=?1",
            [change.file.0],
            |row| row.get(0),
        )?;
        let object: Option<Vec<u8>> = if inherit_content {
            tx.query_row(
            "SELECT o.hash FROM files f LEFT JOIN objects o ON o.hash=f.disk_object WHERE f.id=?1",
            [change.file.0],
            |row| row.get(0),
        )?
        } else {
            None
        };
        tx.execute(
            "UPDATE files SET path=?1,workspace=?2,alive=?3 WHERE id=?4",
            params![path, workspace, change.present, change.file.0],
        )?;
        if !inherit_content {
            tx.execute(
                "UPDATE files SET disk_object=NULL WHERE id=?1",
                [change.file.0],
            )?;
        }
        tx.execute(
            "INSERT INTO versions(file_id,path,workspace,reason,at_ms,object,present,previous_path)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                change.file.0,
                path,
                workspace,
                change.reason.as_str(),
                change.at_ms,
                object,
                change.present,
                previous
            ],
        )?;
        let version = VersionId(tx.last_insert_rowid());
        self.trim_budget(&tx, Some(version), None)?;
        tx.commit()?;
        drop(connection);
        self.drain_legacy_gc()?;
        Ok(version)
    }

    /// Resolve the live path only; tombstones never lend their identity to an unrelated new file.
    pub fn file_at(&self, path: &Path) -> Result<Option<FileId>> {
        self.lock()?
            .query_row(
                "SELECT id FROM files WHERE path=?1 AND alive=1",
                [path_text(path)?],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map(|id| id.map(FileId))
            .map_err(Into::into)
    }
}
