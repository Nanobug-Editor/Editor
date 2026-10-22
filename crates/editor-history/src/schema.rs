//! Versioned SQLite ownership is checked before any initialization or journal-mode write.

use crate::{Error, Result};
use rusqlite::Connection;

/// SQLite application identifier for Nanobug history, separate from all project databases.
const APPLICATION_ID: i64 = 0x4e424847;
/// First complete-object and direct-version format; unknown future versions are never downgraded.
const FORMAT_VERSION: i64 = 2;

/// Validate existing ownership or create an empty database's entire schema in one transaction.
pub(crate) fn initialize(connection: &mut Connection) -> Result<()> {
    let application: i64 =
        connection.pragma_query_value(None, "application_id", |row| row.get(0))?;
    let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    let tables: i64 = connection.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
        [],
        |row| row.get(0),
    )?;
    let fresh = application == 0 && version == 0 && tables == 0;
    if !fresh && (application != APPLICATION_ID || !(1..=FORMAT_VERSION).contains(&version)) {
        return Err(Error::UnsupportedFormat);
    }
    if !fresh {
        // Missing tables or columns are damage, not an invitation to quietly create empty replacements.
        connection.prepare("SELECT f.path,v.object,o.codec,l.modified_ns,i.version,g.raw_size,s.epoch,s.dirty FROM files f,versions v,objects o,legacy l,imports i,legacy_gc g,store_state s WHERE 0")?;
    }
    connection.execute_batch(
        "PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;",
    )?;
    if !fresh {
        if version == 1 {
            let tx = connection.transaction()?;
            tx.execute_batch(
                "ALTER TABLE versions ADD COLUMN previous_path TEXT;
                ALTER TABLE versions ADD COLUMN origin INTEGER;",
            )?;
            tx.pragma_update(None, "user_version", FORMAT_VERSION)?;
            tx.commit()?;
        }
        connection.prepare("SELECT previous_path,origin FROM versions WHERE 0")?;
        return Ok(());
    }
    let tx = connection.transaction()?;
    tx.execute_batch(
        "CREATE TABLE files(id INTEGER PRIMARY KEY AUTOINCREMENT,path TEXT NOT NULL,workspace TEXT,
             alive INTEGER NOT NULL DEFAULT 1,disk_object BLOB);
         CREATE UNIQUE INDEX files_live_path ON files(path) WHERE alive=1;
         CREATE TABLE objects(hash BLOB PRIMARY KEY CHECK(length(hash)=32),
             raw_size INTEGER NOT NULL CHECK(raw_size>=0),payload BLOB NOT NULL,codec INTEGER NOT NULL DEFAULT 1);
         CREATE TABLE versions(id INTEGER PRIMARY KEY AUTOINCREMENT,file_id INTEGER NOT NULL REFERENCES files(id),
             path TEXT NOT NULL,workspace TEXT,reason TEXT NOT NULL,at_ms INTEGER NOT NULL,
             object BLOB REFERENCES objects(hash),present INTEGER NOT NULL DEFAULT 1,previous_path TEXT,origin INTEGER);
         CREATE INDEX versions_file ON versions(file_id,at_ms DESC,id DESC);
         CREATE INDEX versions_time ON versions(at_ms,id);
         CREATE INDEX versions_object ON versions(object);
         CREATE INDEX versions_workspace ON versions(workspace,at_ms DESC,id DESC);
         CREATE TABLE legacy(id TEXT PRIMARY KEY,filename TEXT NOT NULL,at_ms INTEGER,
             raw_size INTEGER NOT NULL CHECK(raw_size>=0),modified_ns INTEGER NOT NULL,seen INTEGER NOT NULL);
         CREATE INDEX legacy_time ON legacy(at_ms DESC,id DESC);
         CREATE TABLE imports(id TEXT PRIMARY KEY,source_path TEXT NOT NULL,hash BLOB NOT NULL,
             version INTEGER REFERENCES versions(id) ON DELETE SET NULL);
         CREATE TABLE legacy_gc(id TEXT PRIMARY KEY,raw_size INTEGER NOT NULL,modified_ns INTEGER NOT NULL);
         CREATE TABLE store_state(id INTEGER PRIMARY KEY CHECK(id=1),epoch INTEGER NOT NULL,dirty INTEGER NOT NULL DEFAULT 0);
         INSERT INTO store_state VALUES(1,0,0);
         CREATE TRIGGER version_insert AFTER INSERT ON versions BEGIN UPDATE store_state SET epoch=epoch+1; END;
         CREATE TRIGGER version_delete AFTER DELETE ON versions BEGIN UPDATE store_state SET epoch=epoch+1,dirty=1; END;
         CREATE TRIGGER version_update AFTER UPDATE ON versions BEGIN UPDATE store_state SET epoch=epoch+1; END;
         CREATE TRIGGER legacy_insert AFTER INSERT ON legacy BEGIN UPDATE store_state SET epoch=epoch+1; END;
         CREATE TRIGGER legacy_delete AFTER DELETE ON legacy BEGIN UPDATE store_state SET epoch=epoch+1,dirty=1; END;
         CREATE TRIGGER legacy_update AFTER UPDATE ON legacy
             WHEN old.id IS NOT new.id OR old.filename IS NOT new.filename OR old.at_ms IS NOT new.at_ms
             OR old.raw_size IS NOT new.raw_size OR old.modified_ns IS NOT new.modified_ns
             BEGIN UPDATE store_state SET epoch=epoch+1; END;",
    )?;
    tx.pragma_update(None, "application_id", APPLICATION_ID)?;
    tx.pragma_update(None, "user_version", FORMAT_VERSION)?;
    tx.commit()?;
    Ok(())
}
