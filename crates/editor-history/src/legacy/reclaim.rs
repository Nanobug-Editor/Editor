//! Filesystem reclamation follows durable metadata decisions and can be retried after interruption.

use super::LegacyFingerprint;
use crate::{History, Result};
use rusqlite::OptionalExtension;

impl History {
    /// Drain committed old-file deletion intents while the caller holds activity, never before commit.
    pub(crate) fn drain_legacy_gc(&self) -> Result<()> {
        loop {
            let next: Option<(String, i64, i64)> = self
                .lock()?
                .query_row(
                    "SELECT id,raw_size,modified_ns FROM legacy_gc ORDER BY id LIMIT 1",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()?;
            let Some((id, size, modified)) = next else {
                return Ok(());
            };
            self.legacy_path(&id)?;
            self.legacy_io
                .as_ref()
                .as_ref()
                .ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::Unsupported,
                        "Native legacy history reclamation is unavailable",
                    )
                })?
                .remove(
                    &self.root,
                    std::path::Path::new(&id),
                    LegacyFingerprint {
                        size: size as u64,
                        modified_ns: modified,
                    },
                )?;
            self.lock()?
                .execute("DELETE FROM legacy_gc WHERE id=?1", [&id])?;
        }
    }
}
