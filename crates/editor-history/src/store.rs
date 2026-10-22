//! Durable whole-content objects and their directly indexed versions.

use crate::{Capture, CaptureOutcome, Error, Policy, Result, SkipReason, VersionId};
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard, RwLock},
    time::Duration,
};

/// Cloneable profile-owned history; callers run blocking operations on their background executor.
#[derive(Clone)]
pub struct History {
    /// One serialized writer also coordinates readers with future reclamation.
    pub(crate) connection: Arc<Mutex<Connection>>,
    /// Canonical private directory; it can never capture itself as a project file.
    pub(crate) root: Arc<PathBuf>,
    /// Replacing a validated profile policy affects subsequent observations, not existing versions.
    pub(crate) policy: Arc<RwLock<Policy>>,
    /// Serializes multi-step capture/pin and cleanup so no protection can be reclaimed in between.
    pub(crate) activity: Arc<Mutex<()>>,
    /// In-memory leases end on process exit and therefore never become permanent retention roots.
    pub(crate) pins: Arc<Mutex<HashMap<i64, usize>>>,
    /// Other processes cannot maintain this profile without sharing its in-flight roots.
    pub(crate) writer_lock: Arc<std::fs::File>,
    /// Native legacy access is fixed for this profile's lifetime; new-format storage needs none.
    pub(crate) legacy_io: Arc<Option<Arc<dyn crate::LegacyIo>>>,
}

impl History {
    /// Open or create root's history database using policy, preserving any unsupported existing data.
    pub fn open(root: impl AsRef<Path>, policy: Policy) -> Result<Self> {
        Self::open_with_legacy_io(root, policy, None)
    }

    /// Open with optional native authority for retired raw snapshots. Without an adapter, legacy
    /// metadata remains visible but reads/reclamation fail explicitly and preserve the original.
    pub fn open_with_legacy_io(
        root: impl AsRef<Path>,
        policy: Policy,
        legacy_io: Option<Arc<dyn crate::LegacyIo>>,
    ) -> Result<Self> {
        policy.validate()?;
        std::fs::create_dir_all(root.as_ref())?;
        let root = std::fs::canonicalize(root.as_ref())?;
        let adapter_type = legacy_io.as_ref().map(|adapter| adapter.as_ref().type_id());
        crate::profiles::open(root, policy, adapter_type, |root, policy| {
            let writer_lock = std::fs::OpenOptions::new()
                .create(true)
                .read(true)
                .write(true)
                .truncate(false)
                .open(root.join("writer.lock"))?;
            writer_lock.try_lock().map_err(|_| Error::Unavailable)?;
            let mut connection = Connection::open(root.join("history.sqlite3"))?;
            connection.busy_timeout(Duration::from_secs(2))?;
            crate::schema::initialize(&mut connection)?;
            Ok(Self {
                connection: Arc::new(Mutex::new(connection)),
                root: Arc::new(root),
                policy: Arc::new(RwLock::new(policy)),
                activity: Arc::new(Mutex::new(())),
                pins: Arc::new(Mutex::new(HashMap::new())),
                writer_lock: Arc::new(writer_lock),
                legacy_io: Arc::new(legacy_io),
            })
        })
        .and_then(|history| {
            history.refresh_legacy()?;
            Ok(history)
        })
    }

    /// Replace policy after validating all of it; invalid input leaves the previous settings intact.
    pub fn set_policy(&self, policy: Policy) -> Result<()> {
        policy.validate()?;
        *self.policy.write().map_err(|_| Error::Unavailable)? = policy;
        Ok(())
    }

    /// Return the active user policy without exposing mutable shared configuration.
    pub fn policy(&self) -> Result<Policy> {
        self.policy
            .read()
            .map(|policy| policy.clone())
            .map_err(|_| Error::Unavailable)
    }

    /// Persist capture's immutable bytes and metadata atomically, returning the committed version.
    pub fn capture(&self, capture: Capture) -> Result<CaptureOutcome> {
        let _activity = self.activity.lock().map_err(|_| Error::Unavailable)?;
        self.drain_legacy_gc()?;
        let outcome = self.capture_inner(capture, None, None)?;
        self.drain_legacy_gc()?;
        Ok(outcome)
    }

    /// Save a recovered copy with its origin metadata while retaining an independent file identity.
    pub fn capture_copy(&self, capture: Capture, origin: VersionId) -> Result<CaptureOutcome> {
        let _activity = self.activity.lock().map_err(|_| Error::Unavailable)?;
        self.drain_legacy_gc()?;
        let outcome = self.capture_inner(capture, None, Some(origin))?;
        self.drain_legacy_gc()?;
        Ok(outcome)
    }

    /// Capture while the caller owns activity, allowing protection to add a lease before cleanup.
    pub(crate) fn capture_inner(
        &self,
        capture: Capture,
        migrating: Option<&str>,
        origin: Option<VersionId>,
    ) -> Result<CaptureOutcome> {
        if capture
            .path
            .canonicalize()
            .unwrap_or_else(|_| capture.path.clone())
            .starts_with(&*self.root)
        {
            return Ok(CaptureOutcome::Skipped(SkipReason::Excluded));
        }
        let policy = self.policy()?;
        if let Some(reason) = policy.skip(&capture.path, &capture.contents) {
            return Ok(CaptureOutcome::Skipped(reason));
        }
        let path = path_text(&capture.path)?;
        let workspace = capture.workspace.as_deref().map(path_text).transpose()?;
        let hash = Sha256::digest(&capture.contents).to_vec();
        let mut connection = self.lock()?;
        self.sync_leases(&connection)?;
        let tx = connection.transaction()?;
        let explicit = match capture.file {
            Some(file)
                if tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM files WHERE id=?1)",
                    [file.0],
                    |row| row.get::<_, bool>(0),
                )? =>
            {
                Some(file.0)
            }
            _ => None,
        };
        // A known retired bucket has one identity even when its original file is absent.
        // Ordinary captures deliberately ignore tombstones so independent recreation stays separate.
        let imported = if capture.reason == crate::Reason::Imported {
            tx.query_row("SELECT v.file_id FROM versions v JOIN files f ON f.id=v.file_id
                WHERE v.path=?1 AND f.path=?1 AND v.reason='imported' AND f.alive=0 ORDER BY v.id DESC LIMIT 1",
                [&path], |row| row.get::<_, i64>(0)).optional()?
        } else {
            None
        };
        let bound = explicit
            .or(tx
                .query_row(
                    "SELECT id FROM files WHERE path=?1 AND alive=1",
                    [&path],
                    |row| row.get(0),
                )
                .optional()?)
            .or(imported);
        let file = if let Some(file) = bound {
            file
        } else {
            tx.execute(
                "INSERT INTO files(path,workspace,alive) VALUES(?1,?2,?3)",
                params![
                    path,
                    workspace,
                    !matches!(
                        capture.reason,
                        crate::Reason::Imported | crate::Reason::BeforeRestore
                    ) || capture.path.exists()
                ],
            )?;
            tx.last_insert_rowid()
        };
        // A content key is only trustworthy after validating its stored object. In particular,
        // deduplication cannot acknowledge unreadable bytes as the sole restore/delete preimage.
        let existing = tx
            .query_row(
                "SELECT raw_size,CASE WHEN length(payload)<=35651584 THEN payload END,codec
            FROM objects WHERE hash=?1",
                [&hash],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, Option<Vec<u8>>>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()?;
        let object_exists = existing.is_some();
        if let Some((size, payload, codec)) = existing {
            if decode_object(
                &hash,
                size,
                payload.as_deref().ok_or(Error::Corrupt)?,
                codec,
            )? != capture.contents
            {
                return Err(Error::Corrupt);
            }
        }
        let previous: Option<i64> = tx.query_row(
            "SELECT v.id FROM files f JOIN versions v ON v.file_id=f.id AND v.object=f.disk_object
             WHERE f.id=?1 AND f.disk_object=?2 AND f.alive=1 ORDER BY v.id DESC LIMIT 1",
            params![file, hash], |row| row.get(0),
        ).optional()?;
        if let Some(previous) = previous.filter(|_| {
            capture.reason != crate::Reason::BeforeRestore
                && capture.reason != crate::Reason::Imported
        }) {
            return Ok(CaptureOutcome::Unchanged(VersionId(previous)));
        }
        if !object_exists {
            let payload = zstd::stream::encode_all(capture.contents.as_slice(), 1)?;
            tx.execute(
                "INSERT INTO objects(hash,raw_size,payload) VALUES(?1,?2,?3)",
                params![hash, capture.contents.len() as i64, payload],
            )?;
        }
        tx.execute(
            "INSERT INTO versions(file_id,path,workspace,reason,at_ms,object,origin) VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![file, path, workspace, capture.reason.as_str(), capture.at_ms, hash, origin.map(|version| version.0)],
        )?;
        let version = VersionId(tx.last_insert_rowid());
        if capture.reason != crate::Reason::BeforeRestore
            && capture.reason != crate::Reason::Imported
        {
            tx.execute(
                "UPDATE files SET disk_object=?1,path=?2,workspace=?3,alive=1 WHERE id=?4",
                params![hash, path, workspace, file],
            )?;
        }
        // Keep this proposed point while evicting older unleased data. Any failure rolls the
        // entire transaction back, including evictions, so an impossible capture loses no history.
        self.trim_budget(&tx, Some(version), migrating)?;
        tx.commit()?;
        Ok(CaptureOutcome::Recorded(version))
    }

    /// Decode version's one complete object and validate its exact length and digest.
    pub fn read(&self, version: VersionId) -> Result<Vec<u8>> {
        let (hash, size, payload, codec): (Vec<u8>, i64, Option<Vec<u8>>, i64) = self.lock()?.query_row(
            "SELECT o.hash,o.raw_size,CASE WHEN length(o.payload)<=35651584 THEN o.payload END,o.codec FROM versions v JOIN objects o ON o.hash=v.object WHERE v.id=?1",
            [version.0],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;
        decode_object(
            &hash,
            size,
            payload.as_deref().ok_or(Error::Corrupt)?,
            codec,
        )
    }

    /// Hold the profile writer while database state is read or committed.
    pub(crate) fn lock(&self) -> Result<MutexGuard<'_, Connection>> {
        self.connection.lock().map_err(|_| Error::Unavailable)
    }
}

/// Bound decoding before allocation and verify one independent object before it may be reused.
fn decode_object(hash: &[u8], size: i64, payload: &[u8], codec: i64) -> Result<Vec<u8>> {
    if codec != 1 {
        return Err(Error::UnsupportedFormat);
    }
    if hash.len() != 32 || !(0..=32 * 1024 * 1024).contains(&size) {
        return Err(Error::Corrupt);
    }
    let mut bytes = Vec::with_capacity(size as usize);
    zstd::stream::read::Decoder::new(payload)
        .map_err(|_| Error::Corrupt)?
        .take(size as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::Corrupt)?;
    if bytes.len() as i64 != size || Sha256::digest(&bytes).as_slice() != hash {
        return Err(Error::Corrupt);
    }
    Ok(bytes)
}

/// Serialize path losslessly; the native adapter is responsible for normalization and authority.
pub(crate) fn path_text(path: &Path) -> Result<String> {
    path.to_str().map(str::to_owned).ok_or(Error::InvalidPath)
}

#[cfg(test)]
mod fault_tests {
    use super::*;
    use crate::Reason;

    /// A real SQLite page-allocation failure enters the public capture rollback path, not a fake store.
    #[test]
    fn sqlite_full_preserves_previously_acknowledged_content() {
        let directory = tempfile::tempdir().unwrap();
        let history = History::open(directory.path().join("history"), Policy::default()).unwrap();
        let path = directory.path().join("note.txt");
        let old = history
            .capture(Capture {
                file: None,
                path: path.clone(),
                workspace: None,
                contents: b"acknowledged".to_vec(),
                reason: Reason::Saved,
                at_ms: 1_000,
            })
            .unwrap()
            .version()
            .unwrap();
        let connection = history.lock().unwrap();
        let pages: i64 = connection
            .pragma_query_value(None, "page_count", |row| row.get(0))
            .unwrap();
        connection
            .pragma_update(None, "max_page_count", pages)
            .unwrap();
        drop(connection);
        let mut bytes = Vec::new();
        for index in 0_u32..32_768 {
            bytes.extend_from_slice(
                format!("{index:08x} {:x}\n", Sha256::digest(index.to_le_bytes())).as_bytes(),
            );
        }
        let error = history
            .capture(Capture {
                file: None,
                path,
                workspace: None,
                contents: bytes,
                reason: Reason::Saved,
                at_ms: 1_001,
            })
            .unwrap_err();
        assert!(
            matches!(error, Error::Database(ref error) if error.sqlite_error_code() == Some(rusqlite::ErrorCode::DiskFull))
        );
        assert_eq!(history.read(old).unwrap(), b"acknowledged");
        assert_eq!(history.stats().unwrap().versions, 1);
    }
}
