//! User-scoped history eligibility and retention settings, separate from project ignore rules.

use crate::{Error, Result, SkipReason};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// User-owned capture and retention settings supplied by the application profile.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Policy {
    /// Whether new automatic observations may be captured.
    pub enabled: bool,
    /// Calendar days after which a retained observation expires.
    pub retention_days: u32,
    /// Profile-wide persistent byte budget, including metadata and legacy snapshots.
    pub max_total_bytes: u64,
    /// Uncompressed byte limit for one text capture; never above the native 32 MiB recovery limit.
    pub max_file_bytes: u64,
    /// Native-independent slash-separated glob patterns, unrelated to Git or explorer ignores.
    pub exclude: Vec<String>,
}

impl Default for Policy {
    /// Apply the confirmed thirty-day, decimal-gigabyte and eight-mebibyte defaults.
    fn default() -> Self {
        Self {
            enabled: true,
            retention_days: 30,
            max_total_bytes: 1_000_000_000,
            max_file_bytes: 8 * 1024 * 1024,
            exclude: vec![
                "**/.git/**".into(),
                "**/target/**".into(),
                "**/node_modules/**".into(),
            ],
        }
    }
}

impl Policy {
    /// Validate all limits and patterns before an application replaces its current policy.
    pub fn validate(&self) -> Result<()> {
        if self.retention_days == 0
            || self.max_total_bytes == 0
            || self.max_file_bytes == 0
            || self.max_file_bytes > 32 * 1024 * 1024
        {
            return Err(Error::InvalidPolicy(
                "positive limits and at most 32 MiB per file are required".into(),
            ));
        }
        for pattern in &self.exclude {
            glob::Pattern::new(pattern).map_err(|error| Error::InvalidPolicy(error.to_string()))?;
        }
        Ok(())
    }

    /// Explain why bytes at path cannot be captured under this policy, if applicable.
    pub fn skip(&self, path: &Path, bytes: &[u8]) -> Option<SkipReason> {
        if let Some(reason) = self.skip_path(path) {
            return Some(reason);
        }
        if bytes.len() as u64 > self.max_file_bytes {
            return Some(SkipReason::TooLarge);
        }
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if matches!(
            extension.as_str(),
            "png"
                | "jpg"
                | "jpeg"
                | "gif"
                | "bmp"
                | "webp"
                | "ico"
                | "avif"
                | "svg"
                | "pdf"
                | "zip"
                | "exe"
                | "dll"
                | "wasm"
                | "sqlite"
                | "db"
        ) || bytes.contains(&0)
            || std::str::from_utf8(bytes).is_err()
        {
            return Some(SkipReason::Unsupported);
        }
        None
    }

    /// Check automatic observation of a path before reading bytes; binary paths may retain metadata.
    pub fn skip_path(&self, path: &Path) -> Option<SkipReason> {
        if !self.enabled {
            return Some(SkipReason::Disabled);
        }
        let name = path.to_string_lossy().replace('\\', "/");
        let options = glob::MatchOptions {
            case_sensitive: !cfg!(windows),
            require_literal_separator: false,
            require_literal_leading_dot: false,
        };
        if self.exclude.iter().any(|pattern| {
            glob::Pattern::new(pattern).is_ok_and(|pattern| pattern.matches_with(&name, options))
        }) {
            return Some(SkipReason::Excluded);
        }
        None
    }
}
