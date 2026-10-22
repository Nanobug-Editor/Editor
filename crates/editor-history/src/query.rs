//! Metadata-only, cursor-based history queries and observable storage usage.

use crate::store::path_text;
use crate::{FileId, History, Query, Reason, Result, Scope, Stats, Version, VersionId};
use rusqlite::params;
use std::path::PathBuf;

impl History {
    /// Read query's metadata page without touching compressed payloads or replaying older versions.
    pub fn list(&self, query: &Query) -> Result<Vec<Version>> {
        let workspace = match &query.scope {
            Scope::Workspace(path) => Some(path_text(path)?),
            _ => None,
        };
        let standalone = matches!(query.scope, Scope::Standalone);
        let connection = self.lock()?;
        let mut statement = connection.prepare(
            "SELECT v.id,v.file_id,v.path,v.workspace,v.at_ms,v.reason,v.present,o.raw_size,v.previous_path,v.origin FROM versions v
             JOIN files f ON f.id=v.file_id LEFT JOIN objects o ON o.hash=v.object
             WHERE (?1 IS NULL OR v.file_id=?1) AND (?2 IS NULL OR v.workspace=?2)
             AND (?3=0 OR v.workspace IS NULL)
             AND (?4='' OR instr(v.path,?4)>0 OR instr(f.path,?4)>0 OR instr(v.previous_path,?4)>0)
             AND (?8=0 OR f.alive=0)
             AND (?5 IS NULL OR v.at_ms<?5 OR (v.at_ms=?5 AND v.id<?6))
             ORDER BY v.at_ms DESC,v.id DESC LIMIT ?7",
        )?;
        let rows = statement.query_map(
            params![
                query.file.map(|file| file.0),
                workspace,
                standalone,
                query.path,
                query.before.map(|cursor| cursor.at_ms),
                query.before.map(|cursor| cursor.id.0),
                query.limit.clamp(1, 200) as i64,
                query.deleted_only
            ],
            |row| {
                Ok(Version {
                    id: VersionId(row.get(0)?),
                    file: FileId(row.get(1)?),
                    path: PathBuf::from(row.get::<_, String>(2)?),
                    previous_path: row.get::<_, Option<String>>(8)?.map(Into::into),
                    origin: row.get::<_, Option<i64>>(9)?.map(VersionId),
                    workspace: row.get::<_, Option<String>>(3)?.map(PathBuf::from),
                    at_ms: row.get(4)?,
                    reason: Reason::parse(&row.get::<_, String>(5)?).map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            5,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?,
                    present: row.get(6)?,
                    size: row.get::<_, Option<i64>>(7)?.map(|value| value as u64),
                })
            },
        )?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// Report retained version and object counts and compressed versus original byte usage.
    pub fn stats(&self) -> Result<Stats> {
        let connection = self.lock()?;
        let versions: i64 =
            connection.query_row("SELECT count(*) FROM versions", [], |r| r.get(0))?;
        let (objects, compressed, raw): (i64, i64, i64) = connection.query_row(
            "SELECT count(*),coalesce(sum(length(payload)),0),coalesce(sum(raw_size),0) FROM objects",
            [], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?)),
        )?;
        Ok(Stats {
            versions: versions as u64,
            objects: objects as u64,
            compressed_bytes: compressed as u64,
            raw_bytes: raw as u64,
            disk_bytes: file_size(&self.root.join("history.sqlite3"))?,
            wal_bytes: file_size(&self.root.join("history.sqlite3-wal"))?,
            legacy_bytes: connection.query_row(
                "SELECT coalesce(sum(raw_size),0) FROM legacy",
                [],
                |row| row.get::<_, i64>(0),
            )? as u64,
            pending_gc_bytes: connection.query_row(
                "SELECT coalesce(sum(raw_size),0) FROM legacy_gc",
                [],
                |row| row.get::<_, i64>(0),
            )? as u64,
        })
    }
}

/// Missing transient files contribute no bytes; other filesystem errors remain visible.
fn file_size(path: &std::path::Path) -> Result<u64> {
    match std::fs::metadata(path) {
        Ok(metadata) => Ok(metadata.len()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(error) => Err(error.into()),
    }
}
