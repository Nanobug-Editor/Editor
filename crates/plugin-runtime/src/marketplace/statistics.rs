//! Advisory download measurements have separate availability and never grant installation authority.
use super::receive;
use crate::InstallControl;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

const MAX_STATISTICS: usize = 4 * 1024 * 1024;

/// Optional GitHub Release attachment counts; these are advisory and never install authority.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Statistics {
    pub schema: u32,
    pub measured_at: Option<String>,
    pub plugins: Vec<PluginStatistics>,
}

/// A total includes exactly the reviewed version attachments in this measurement.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginStatistics {
    pub id: String,
    pub total: Option<u64>,
    pub versions: Vec<VersionStatistics>,
}

/// A missing count is unavailable, never zero.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VersionStatistics {
    pub version: String,
    pub downloads: Option<u64>,
}

impl Statistics {
    /// Parse bounded public data and reject duplicate or internally inconsistent measurements.
    pub fn parse(bytes: &[u8]) -> anyhow::Result<Self> {
        anyhow::ensure!(bytes.len() <= MAX_STATISTICS, "Statistics exceed limit");
        let statistics: Self = serde_json::from_slice(bytes)?;
        anyhow::ensure!(statistics.schema == 1, "Unsupported statistics schema");
        let mut identities = BTreeSet::new();
        for plugin in &statistics.plugins {
            anyhow::ensure!(
                identities.insert(&plugin.id),
                "Duplicate statistics identity"
            );
            let mut versions = BTreeSet::new();
            let mut sum = 0_u64;
            for version in &plugin.versions {
                anyhow::ensure!(
                    versions.insert(&version.version),
                    "Duplicate statistics version"
                );
                if let Some(count) = version.downloads {
                    sum = sum
                        .checked_add(count)
                        .context("Statistics total overflow")?;
                }
            }
            if let Some(total) = plugin.total {
                anyhow::ensure!(
                    plugin.versions.iter().all(|v| v.downloads.is_some()) && total == sum,
                    "Statistics total mismatch"
                );
            }
        }
        Ok(statistics)
    }

    /// Fetch a separate optional snapshot; a failure must never affect catalog freshness.
    pub fn fetch(url: &str) -> anyhow::Result<Self> {
        Self::parse(&receive(
            url,
            MAX_STATISTICS,
            &InstallControl::default(),
            |_| {},
        )?)
    }

    /// Return the aggregate for an identity, or None when no measurement is available.
    pub fn total(&self, id: &str) -> Option<u64> {
        self.plugins.iter().find(|p| p.id == id)?.total
    }

    /// Return a reviewed version's count, or None when that count has not been measured.
    pub fn version(&self, id: &str, version: &str) -> Option<u64> {
        self.plugins
            .iter()
            .find(|p| p.id == id)?
            .versions
            .iter()
            .find(|v| v.version == version)?
            .downloads
    }
}
