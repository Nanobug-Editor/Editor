//! Confirmed-scope deletion respects shared content and in-flight restore protection.

use crate::store::path_text;
use crate::{Cleanup, ClearScope, Error, History, Result, VersionId};
use rusqlite::{Connection, OptionalExtension, params};

impl History {
    /// Preview scope's unleased records and uniquely reclaimable payload bytes without deleting them.
    pub fn preview_clear(&self, scope: &ClearScope) -> Result<Cleanup> {
        let _activity = self.activity.lock().map_err(|_| Error::Unavailable)?;
        let connection = self.lock()?;
        self.select_clear(&connection, scope, None)
    }

    /// Delete the already user-authorized scope, reclaim unreferenced content and compact storage.
    pub fn clear(&self, scope: &ClearScope) -> Result<Cleanup> {
        self.clear_inner(scope, None)
    }

    /// Execute a user confirmation only if its preview still describes the current history state.
    pub fn clear_confirmed(&self, scope: &ClearScope, preview: Cleanup) -> Result<Cleanup> {
        self.clear_inner(scope, Some(preview))
    }

    /// Serialize preview validation and deletion so capture cannot broaden a confirmed selection.
    fn clear_inner(&self, scope: &ClearScope, expected: Option<Cleanup>) -> Result<Cleanup> {
        let _activity = self.activity.lock().map_err(|_| Error::Unavailable)?;
        self.drain_legacy_gc()?;
        let mut connection = self.lock()?;
        let report = self.select_clear(&connection, scope, None)?;
        if expected.is_some_and(|preview| preview != report) {
            return Err(Error::Stale);
        }
        self.erase_selected(&mut connection)?;
        drop(connection);
        self.drain_legacy_gc()?;
        Self::compact(&*self.lock()?)?;
        Ok(report)
    }

    /// Expire observations strictly older than now_ms minus the configured calendar-day interval.
    pub fn maintain(&self, now_ms: i64) -> Result<Cleanup> {
        let _activity = self.activity.lock().map_err(|_| Error::Unavailable)?;
        self.drain_legacy_gc()?;
        let cutoff = now_ms.saturating_sub(i64::from(self.policy()?.retention_days) * 86_400_000);
        let mut connection = self.lock()?;
        self.select_clear(&connection, &ClearScope::All, Some(cutoff))?;
        let mut report = self.selected_usage(&connection)?;
        self.erase_selected(&mut connection)?;
        let tx = connection.transaction()?;
        let capacity = self.trim_budget(&tx, None, None)?;
        tx.commit()?;
        report.versions += capacity.versions;
        report.legacy += capacity.legacy;
        report.reclaimable_bytes += capacity.reclaimable_bytes;
        drop(connection);
        self.drain_legacy_gc()?;
        let connection = self.lock()?;
        let dirty: bool =
            connection.query_row("SELECT dirty FROM store_state WHERE id=1", [], |row| {
                row.get(0)
            })?;
        if dirty {
            Self::compact(&connection)?;
        } else {
            connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        }
        Ok(report)
    }

    /// Remove the prepared selection atomically, then make reclaimed database pages observable on disk.
    fn erase_selected(&self, connection: &mut Connection) -> Result<()> {
        let tx = connection.transaction()?;
        tx.execute("INSERT OR IGNORE INTO legacy_gc SELECT id,raw_size,modified_ns FROM legacy WHERE id IN(SELECT id FROM selected_legacy)", [])?;
        tx.execute(
            "DELETE FROM legacy WHERE id IN(SELECT id FROM selected_legacy)",
            [],
        )?;
        tx.execute(
            "DELETE FROM versions WHERE id IN(SELECT id FROM selected_versions)",
            [],
        )?;
        tx.execute("DELETE FROM objects WHERE NOT EXISTS(SELECT 1 FROM versions WHERE object=objects.hash)", [])?;
        tx.execute(
            "DELETE FROM files WHERE NOT EXISTS(SELECT 1 FROM versions WHERE file_id=files.id)",
            [],
        )?;
        tx.execute("DELETE FROM imports WHERE version IS NULL AND NOT EXISTS(SELECT 1 FROM legacy WHERE legacy.id=imports.id)", [])?;
        tx.commit()?;
        Ok(())
    }

    /// Evict oldest unleased rows until live SQLite pages fit, optionally preserving a new capture.
    pub(crate) fn trim_budget(
        &self,
        connection: &Connection,
        keep: Option<VersionId>,
        migrating: Option<&str>,
    ) -> Result<Cleanup> {
        let limit = self.policy()?.max_total_bytes;
        let mut report = Cleanup {
            versions: 0,
            legacy: 0,
            reclaimable_bytes: 0,
            epoch: connection.query_row("SELECT epoch FROM store_state WHERE id=1", [], |row| {
                row.get(0)
            })?,
        };
        while used_bytes(connection)? + legacy_bytes(connection, migrating)? > limit {
            let oldest: Option<(i64, Option<Vec<u8>>)> = connection
                .query_row(
                    "SELECT id,object FROM versions v WHERE (?1 IS NULL OR id<>?1)
                 AND NOT EXISTS(SELECT 1 FROM active_leases p WHERE p.id=v.id)
                 ORDER BY at_ms,id LIMIT 1",
                    [keep.map(|id| id.0)],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            let legacy: Option<(String, i64, i64)> = connection.query_row(
                "SELECT id,coalesce(at_ms,modified_ns/1000000),raw_size FROM legacy WHERE (?1 IS NULL OR id<>?1)
                 ORDER BY coalesce(at_ms,modified_ns/1000000),id LIMIT 1",
                [migrating], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?)),
            ).optional()?;
            let version_time: Option<i64> = oldest
                .as_ref()
                .map(|(id, _)| {
                    connection.query_row("SELECT at_ms FROM versions WHERE id=?1", [id], |row| {
                        row.get(0)
                    })
                })
                .transpose()?;
            if let Some((id, _, size)) =
                legacy.filter(|(_, time, _)| version_time.is_none_or(|version| *time <= version))
            {
                connection.execute("INSERT OR IGNORE INTO legacy_gc SELECT id,raw_size,modified_ns FROM legacy WHERE id=?1", [&id])?;
                connection.execute("DELETE FROM legacy WHERE id=?1", [&id])?;
                report.legacy += 1;
                report.reclaimable_bytes += size as u64;
                continue;
            }
            let Some((id, object)) = oldest else {
                return Err(Error::Quota);
            };
            connection.execute("DELETE FROM versions WHERE id=?1", [id])?;
            if let Some(object) = object {
                let bytes: i64 = connection.query_row(
                    "SELECT length(payload) FROM objects WHERE hash=?1",
                    [&object],
                    |r| r.get(0),
                )?;
                let removed = connection.execute(
                    "DELETE FROM objects WHERE hash=?1 AND NOT EXISTS(SELECT 1 FROM versions WHERE object=?1)", [&object])?;
                if removed != 0 {
                    report.reclaimable_bytes += bytes as u64;
                }
            }
            report.versions += 1;
        }
        connection.execute(
            "DELETE FROM files WHERE NOT EXISTS(SELECT 1 FROM versions WHERE file_id=files.id)",
            [],
        )?;
        Ok(report)
    }

    /// Reclaim persistent free pages; this blocking operation belongs on the host's worker.
    fn compact(connection: &Connection) -> Result<()> {
        connection.execute_batch(
            "PRAGMA wal_checkpoint(TRUNCATE); VACUUM; UPDATE store_state SET dirty=0; PRAGMA wal_checkpoint(TRUNCATE);",
        )?;
        Ok(())
    }

    /// Build connection-private selections using bound scope values and current operation leases.
    fn select_clear(
        &self,
        connection: &Connection,
        scope: &ClearScope,
        expires: Option<i64>,
    ) -> Result<Cleanup> {
        connection.execute_batch(
            "CREATE TEMP TABLE IF NOT EXISTS selected_versions(id INTEGER PRIMARY KEY);
             CREATE TEMP TABLE IF NOT EXISTS selected_legacy(id TEXT PRIMARY KEY);
             DELETE FROM selected_versions; DELETE FROM selected_legacy;",
        )?;
        self.sync_leases(connection)?;
        let (version, file, workspace) = match scope {
            ClearScope::All => (None, None, None),
            ClearScope::Version(id) => (Some(id.0), None, None),
            ClearScope::File(id) => (None, Some(id.0), None),
            ClearScope::Workspace(path) => (None, None, Some(path_text(path)?)),
            ClearScope::Legacy(_) => (None, None, None),
        };
        connection.execute(
            "INSERT INTO selected_versions SELECT id FROM versions v
             WHERE (?1 IS NULL OR id=?1) AND (?2 IS NULL OR file_id=?2)
             AND (?3 IS NULL OR workspace=?3) AND (?4 IS NULL OR at_ms<?4)
             AND ?5=1 AND NOT EXISTS(SELECT 1 FROM active_leases p WHERE p.id=v.id)",
            params![
                version,
                file,
                workspace,
                expires,
                !matches!(scope, ClearScope::Legacy(_))
            ],
        )?;
        if matches!(scope, ClearScope::All | ClearScope::Legacy(_)) {
            let id = match scope {
                ClearScope::Legacy(id) => Some(id),
                _ => None,
            };
            connection.execute(
                "INSERT INTO selected_legacy SELECT id FROM legacy WHERE (?1 IS NULL OR id=?1)
                AND (?2 IS NULL OR coalesce(at_ms,modified_ns/1000000)<?2)",
                params![id, expires],
            )?;
        }
        self.selected_usage(connection)
    }

    /// Snapshot the current in-flight roots while activity prevents a new lease from racing cleanup.
    pub(crate) fn sync_leases(&self, connection: &Connection) -> Result<()> {
        connection.execute_batch("CREATE TEMP TABLE IF NOT EXISTS active_leases(id INTEGER PRIMARY KEY); DELETE FROM active_leases;")?;
        for id in self.pins.lock().map_err(|_| Error::Unavailable)?.keys() {
            connection.execute("INSERT INTO active_leases(id) VALUES(?1)", [id])?;
        }
        Ok(())
    }

    /// Count the current temporary selection and objects with no references outside it.
    fn selected_usage(&self, connection: &Connection) -> Result<Cleanup> {
        let versions: i64 =
            connection.query_row("SELECT count(*) FROM selected_versions", [], |row| {
                row.get(0)
            })?;
        let bytes: i64 = connection.query_row(
            "SELECT coalesce(sum(length(payload)),0) FROM objects o
             WHERE EXISTS(SELECT 1 FROM versions v JOIN selected_versions s ON s.id=v.id WHERE v.object=o.hash)
             AND NOT EXISTS(SELECT 1 FROM versions v WHERE v.object=o.hash
                 AND NOT EXISTS(SELECT 1 FROM selected_versions s WHERE s.id=v.id))", [], |row| row.get(0),
        )?;
        let (legacy, raw): (i64, i64) = connection.query_row(
            "SELECT count(*),coalesce(sum(raw_size),0) FROM legacy WHERE id IN(SELECT id FROM selected_legacy)",
            [], |row| Ok((row.get(0)?,row.get(1)?)),
        )?;
        Ok(Cleanup {
            versions: versions as u64,
            legacy: legacy as u64,
            reclaimable_bytes: (bytes + raw) as u64,
            epoch: connection.query_row("SELECT epoch FROM store_state WHERE id=1", [], |row| {
                row.get(0)
            })?,
        })
    }
}

/// Live page bytes include index overhead while allowing a following VACUUM to release free pages.
fn used_bytes(connection: &Connection) -> Result<u64> {
    let pages: i64 = connection.pragma_query_value(None, "page_count", |row| row.get(0))?;
    let free: i64 = connection.pragma_query_value(None, "freelist_count", |row| row.get(0))?;
    let size: i64 = connection.pragma_query_value(None, "page_size", |row| row.get(0))?;
    if pages < free || free < 0 || size <= 0 {
        return Err(Error::Corrupt);
    }
    Ok((pages - free) as u64 * size as u64)
}

/// A migration's source copy is temporary until the validated target is committed and reclaimed.
fn legacy_bytes(connection: &Connection, migrating: Option<&str>) -> Result<u64> {
    let bytes: i64 = connection.query_row(
        "SELECT coalesce(sum(raw_size),0) FROM legacy WHERE (?1 IS NULL OR id<>?1)",
        [migrating],
        |row| row.get(0),
    )?;
    Ok(bytes as u64)
}
