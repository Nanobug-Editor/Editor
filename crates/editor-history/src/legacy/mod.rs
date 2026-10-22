//! Indexed access to retired path-hash snapshots without inventing their original file paths.

mod import;
mod io;
mod reclaim;
use crate::{Error, History, Result};
pub use io::{LegacyFingerprint, LegacyIo};
use rusqlite::params;
use std::{
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    time::UNIX_EPOCH,
};

/// An old raw snapshot with only the metadata the retired format actually preserved.
#[derive(Clone, Debug)]
pub struct LegacyEntry {
    /// Validated relative bucket/name, never an original project path.
    pub id: String,
    /// Timestamp-prefixed, sanitized filename available for display and filtering.
    pub filename: String,
    /// Original milliseconds, or None when the legacy name cannot be parsed.
    pub at_ms: Option<i64>,
    /// Original file byte length, including unsupported binary snapshots.
    pub size: u64,
    /// Detect replacement between listing, migration and deferred deletion.
    pub(crate) modified_ns: i64,
}

/// Exclusive legacy-page position; unknown timestamps sort as zero.
#[derive(Clone, Debug)]
pub struct LegacyCursor {
    /// Original timestamp or zero for an unparseable name.
    pub at_ms: i64,
    /// Relative name disambiguating equal timestamps.
    pub id: String,
}

/// Legacy lists use indexed metadata rather than reading every payload to render a page.
#[derive(Clone, Debug)]
pub struct LegacyQuery {
    /// Literal filename or old relative-id filter, without claiming a recovered original path.
    pub filter: String,
    /// Exclusive position at the previous page's last row.
    pub before: Option<LegacyCursor>,
    /// Bounded page size, clamped to 1 through 200.
    pub limit: usize,
}

impl Default for LegacyQuery {
    /// Query the newest fifty old entries.
    fn default() -> Self {
        Self {
            filter: String::new(),
            before: None,
            limit: 50,
        }
    }
}

impl LegacyEntry {
    /// Continue from this exact old entry without relying on timestamp uniqueness.
    pub fn cursor(&self) -> LegacyCursor {
        LegacyCursor {
            at_ms: self.at_ms.unwrap_or(0),
            id: self.id.clone(),
        }
    }
}

impl History {
    /// Index native metadata under old sixteen-hex buckets, preserving unknown-path snapshots.
    pub fn refresh_legacy(&self) -> Result<()> {
        let _activity = self.activity.lock().map_err(|_| Error::Unavailable)?;
        let mut connection = self.lock()?;
        let tx = connection.transaction()?;
        let generation: i64 =
            tx.query_row("SELECT coalesce(max(seen),0)+1 FROM legacy", [], |row| {
                row.get(0)
            })?;
        for directory in std::fs::read_dir(&*self.root)? {
            let directory = directory?;
            let bucket = directory.file_name().to_string_lossy().into_owned();
            if !is_bucket(&bucket)
                || !directory.file_type()?.is_dir()
                || is_link(&std::fs::symlink_metadata(directory.path())?)
            {
                continue;
            }
            for file in std::fs::read_dir(directory.path())? {
                let file = file?;
                let metadata = std::fs::symlink_metadata(file.path())?;
                if !metadata.is_file() || is_link(&metadata) {
                    continue;
                }
                let filename = file
                    .file_name()
                    .into_string()
                    .map_err(|_| Error::InvalidPath)?;
                let id = format!("{bucket}/{filename}");
                let removed: bool = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM legacy_gc WHERE id=?1)",
                    [&id],
                    |row| row.get(0),
                )?;
                if removed {
                    continue;
                }
                let at = filename
                    .split_once('-')
                    .and_then(|(time, _)| time.parse::<i64>().ok());
                tx.execute(
                    "INSERT INTO legacy(id,filename,at_ms,raw_size,modified_ns,seen) VALUES(?1,?2,?3,?4,?5,?6)
                     ON CONFLICT(id) DO UPDATE SET filename=excluded.filename,at_ms=excluded.at_ms,
                     raw_size=excluded.raw_size,modified_ns=excluded.modified_ns,seen=excluded.seen",
                    params![id,filename,at,metadata.len() as i64,modified_ns(&metadata)?,generation],
                )?;
            }
        }
        tx.execute("DELETE FROM legacy WHERE seen<>?1", [generation])?;
        tx.commit()?;
        Ok(())
    }

    /// Read query's indexed legacy metadata page, without loading or associating raw contents.
    pub fn legacy(&self, query: &LegacyQuery) -> Result<Vec<LegacyEntry>> {
        let connection = self.lock()?;
        let mut statement = connection.prepare(
            "SELECT id,filename,at_ms,raw_size,modified_ns FROM legacy
             WHERE (?1='' OR instr(filename,?1)>0 OR instr(id,?1)>0)
             AND (?2 IS NULL OR coalesce(at_ms,0)<?2 OR (coalesce(at_ms,0)=?2 AND id<?3))
             ORDER BY coalesce(at_ms,0) DESC,id DESC LIMIT ?4",
        )?;
        statement
            .query_map(
                params![
                    query.filter,
                    query.before.as_ref().map(|cursor| cursor.at_ms),
                    query.before.as_ref().map(|cursor| &cursor.id),
                    query.limit.clamp(1, 200) as i64
                ],
                |row| {
                    Ok(LegacyEntry {
                        id: row.get(0)?,
                        filename: row.get(1)?,
                        at_ms: row.get(2)?,
                        size: row.get::<_, i64>(3)? as u64,
                        modified_ns: row.get(4)?,
                    })
                },
            )?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// Materialize a bounded old payload for readonly preview; binary bytes remain binary bytes.
    pub fn read_legacy(&self, entry: &LegacyEntry) -> Result<Vec<u8>> {
        if entry.size > 32 * 1024 * 1024 {
            return Err(Error::TooLarge);
        }
        let _activity = self.activity.lock().map_err(|_| Error::Unavailable)?;
        self.read_legacy_inner(entry)
    }

    /// Stream even oversized old content into output; callers discard staging output on any error.
    pub fn copy_legacy(&self, entry: &LegacyEntry, output: &mut impl Write) -> Result<()> {
        let _activity = self.activity.lock().map_err(|_| Error::Unavailable)?;
        let mut file = self.open_legacy(entry)?;
        let copied = std::io::copy(&mut (&mut file).take(entry.size.saturating_add(1)), output)?;
        if copied != entry.size || modified_ns(&file.metadata()?)? != entry.modified_ns {
            return Err(Error::Stale);
        }
        Ok(())
    }

    /// Read a known source while activity already excludes local garbage collection.
    fn read_legacy_inner(&self, entry: &LegacyEntry) -> Result<Vec<u8>> {
        if entry.size > 32 * 1024 * 1024 {
            return Err(Error::TooLarge);
        }
        let mut file = self.open_legacy(entry)?;
        let mut bytes = Vec::with_capacity(entry.size as usize);
        (&mut file)
            .take(entry.size.saturating_add(1))
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 != entry.size || modified_ns(&file.metadata()?)? != entry.modified_ns
        {
            return Err(Error::Stale);
        }
        Ok(bytes)
    }

    /// Open a regular private source only when it still matches the selected metadata.
    fn open_legacy(&self, entry: &LegacyEntry) -> Result<std::fs::File> {
        self.legacy_path(&entry.id)?;
        Ok(self
            .legacy_io
            .as_ref()
            .as_ref()
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "Native legacy history access is unavailable",
                )
            })?
            .open(
                &self.root,
                Path::new(&entry.id),
                LegacyFingerprint {
                    size: entry.size,
                    modified_ns: entry.modified_ns,
                },
            )?)
    }

    /// Validate old layout components before joining any caller-provided snapshot identity.
    pub(crate) fn legacy_path(&self, id: &str) -> Result<PathBuf> {
        let components: Vec<_> = Path::new(id).components().collect();
        if components.len() != 2
            || components
                .iter()
                .any(|part| !matches!(part, Component::Normal(_)))
            || !is_bucket(&components[0].as_os_str().to_string_lossy())
            || components[1]
                .as_os_str()
                .to_string_lossy()
                .contains([':', '\\', '/', '<', '>', '"', '|', '?', '*'])
            || components[1]
                .as_os_str()
                .to_string_lossy()
                .ends_with(['.', ' '])
        {
            return Err(Error::InvalidPath);
        }
        Ok(self.root.join(id))
    }
}

/// The old format created precisely sixteen hexadecimal bucket names.
fn is_bucket(value: &str) -> bool {
    value.len() == 16 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Preserve a platform-independent change fingerprint without reading the complete old payload.
fn modified_ns(metadata: &std::fs::Metadata) -> Result<i64> {
    let duration = metadata
        .modified()?
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Error::Stale)?;
    i64::try_from(duration.as_nanos()).map_err(|_| Error::Stale)
}

/// Junctions and other Windows reparse points are not regular old history sources.
fn is_link(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}
