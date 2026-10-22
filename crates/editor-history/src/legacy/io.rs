//! The host supplies native path authority for retired snapshot reads and physical reclamation.

use std::{any::Any, fs::File, io, path::Path};

/// The indexed source fingerprint must still match the opened native file.
#[derive(Clone, Copy)]
pub struct LegacyFingerprint {
    /// Exact original byte count, including binary or oversized legacy files.
    pub size: u64,
    /// Modification time in nanoseconds since the Unix epoch.
    pub modified_ns: i64,
}

/// A narrow native seam; SQLite decisions and migration policy remain in editor-history.
/// Implementations must bind operations to the verified root and reject redirected ancestors.
pub trait LegacyIo: Any + Send + Sync {
    /// Open the exact regular source with matching metadata, or return an I/O error without guessing.
    fn open(&self, root: &Path, relative: &Path, expected: LegacyFingerprint) -> io::Result<File>;
    /// Remove only that verified source; absence is success and any mismatch preserves the file.
    fn remove(&self, root: &Path, relative: &Path, expected: LegacyFingerprint) -> io::Result<()>;
}
