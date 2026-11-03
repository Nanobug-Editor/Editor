//! Inert GitHub release metadata is validated before it can describe an executable download.

use anyhow::{Context, Result, ensure};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The only publisher trusted by the built-in host updater.
pub(crate) const REPOSITORY: &str = "https://github.com/Nanobug-Editor/Editor";
/// Public release discovery never reads credentials or project configuration.
pub(crate) const API: &str = "https://api.github.com/repos/Nanobug-Editor/Editor/releases";
/// Bound both disk consumption and untrusted download metadata.
pub(crate) const MAX_PACKAGE: u64 = 512 * 1024 * 1024;

/// A packaged Windows host can retain its installation style; source builds only browse releases.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) enum Edition {
    /// An Inno Setup installation retains its existing directory and integration choices.
    Installed,
    /// A portable ZIP is replaced in place without registering an installation.
    Portable,
    /// Other platforms and development executables offer the publisher's download page.
    Browse,
}

impl Edition {
    /// Detect the current executable's delivery layout without consulting a workspace.
    pub(crate) fn detect(executable: &Path) -> Self {
        if !cfg!(all(target_os = "windows", target_arch = "x86_64"))
            || executable
                .file_name()
                .and_then(|name| name.to_str())
                .is_none_or(|name| !name.eq_ignore_ascii_case("Nanobug.exe"))
        {
            return Self::Browse;
        }
        if executable
            .parent()
            .is_some_and(|path| path.join("unins000.dat").is_file())
        {
            Self::Installed
        } else {
            Self::Portable
        }
    }

    /// Choose the exact publisher-controlled asset name for `version`, never a fuzzy suffix match.
    pub(crate) fn asset_name(self, version: &str) -> String {
        match self {
            Self::Installed => format!("Nanobug-Setup-{version}-x64.exe"),
            Self::Portable | Self::Browse => format!("Nanobug-{version}-windows-x64.zip"),
        }
    }
}

/// The current program's identity is distinct from the installer's numeric Windows version.
#[derive(Clone, Debug)]
pub(crate) struct Identity {
    /// Full SemVer, including the prerelease build label when the publisher supplies one.
    pub(crate) version: Version,
    /// Absolute running executable used for installation and restart.
    pub(crate) executable: PathBuf,
    /// Delivery style is preserved by updates.
    pub(crate) edition: Edition,
}

impl Identity {
    /// Read a matching adjacent build identity off-thread; invalid metadata falls back to Cargo.
    pub(crate) fn load(executable: PathBuf) -> Self {
        let base =
            Version::parse(env!("CARGO_PKG_VERSION")).expect("Cargo version is valid SemVer");
        let explicit =
            option_env!("NANOBUG_RELEASE_VERSION").and_then(|value| Version::parse(value).ok());
        let adjacent = (|| {
            let file = executable.parent()?.join("build-info.json");
            let data = super::download::read_small_file(&file, 32 * 1024).ok()?;
            let info: serde_json::Value = serde_json::from_slice(&data).ok()?;
            if info["product"] != "Nanobug"
                || info["application_version"] != env!("CARGO_PKG_VERSION")
            {
                return None;
            }
            // A stale build-info file must never change the running binary's update channel/version.
            let digest = super::download::file_digest(&executable).ok()?;
            if info["executable_sha256"].as_str()? != digest {
                return None;
            }
            Version::parse(info["build_version"].as_str()?).ok()
        })();
        let version = explicit
            .or(adjacent)
            .filter(|version| {
                (version.major, version.minor, version.patch)
                    == (base.major, base.minor, base.patch)
            })
            .unwrap_or(base);
        Self {
            edition: Edition::detect(&executable),
            executable,
            version,
        }
    }
}

/// GitHub's response is inert until publisher URL, channel, version and assets pass validation.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct GithubRelease {
    pub(crate) tag_name: String,
    pub(crate) html_url: String,
    #[serde(default)]
    pub(crate) name: Option<String>,
    #[serde(default)]
    pub(crate) body: Option<String>,
    #[serde(default)]
    pub(crate) published_at: Option<String>,
    #[serde(default)]
    pub(crate) draft: bool,
    #[serde(default)]
    pub(crate) prerelease: bool,
    #[serde(default)]
    pub(crate) assets: Vec<Asset>,
}

/// A release asset carries both its expected length and GitHub's SHA-256 digest.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct Asset {
    pub(crate) name: String,
    pub(crate) browser_download_url: String,
    pub(crate) size: u64,
    #[serde(default)]
    pub(crate) digest: Option<String>,
}

impl Asset {
    /// Return a validated hex SHA-256, rejecting legacy assets without an authenticated digest.
    pub(crate) fn sha256(&self) -> Result<&str> {
        let digest = self
            .digest
            .as_deref()
            .and_then(|value| value.strip_prefix("sha256:"))
            .context("GitHub release asset has no SHA-256 digest")?;
        ensure!(
            digest.len() == 64 && digest.bytes().all(|b| b.is_ascii_hexdigit()),
            "Invalid release SHA-256"
        );
        Ok(digest)
    }
}

/// Validated display metadata and an optional installable asset share one immutable release identity.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct Release {
    /// Full release version; encoded as text to keep the wire contract independent of semver features.
    pub(crate) version: String,
    pub(crate) url: String,
    pub(crate) title: String,
    pub(crate) notes: String,
    pub(crate) published_at: String,
    /// Absence leaves download-page browsing available for unsupported assets/platforms.
    pub(crate) asset: Option<Asset>,
}

impl Release {
    /// Revalidate persisted metadata before trusting a cached package or helper job.
    pub(crate) fn validate(&self, edition: Edition) -> Result<()> {
        let version = Version::parse(&self.version)?;
        ensure!(
            self.url == format!("{REPOSITORY}/releases/tag/v{version}")
                || self.url == format!("{REPOSITORY}/releases/tag/{version}"),
            "Unexpected release publisher URL"
        );
        if let Some(asset) = &self.asset {
            ensure!(
                asset.name == edition.asset_name(&self.version),
                "Unexpected release asset name"
            );
            ensure!(
                (1..=MAX_PACKAGE).contains(&asset.size),
                "Release asset exceeds size limit"
            );
            asset.sha256()?;
            let tag = self.url.rsplit('/').next().context("Missing release tag")?;
            ensure!(
                asset.browser_download_url
                    == format!("{REPOSITORY}/releases/download/{tag}/{}", asset.name),
                "Unexpected release asset URL"
            );
        }
        Ok(())
    }
}

/// Select only a strictly newer compatible channel; array order and release dates are not versions.
pub(crate) fn select(
    releases: Vec<GithubRelease>,
    current: &Version,
    preview: bool,
    edition: Edition,
) -> Option<Release> {
    releases
        .into_iter()
        .filter_map(|release| {
            let version = Version::parse(
                release
                    .tag_name
                    .strip_prefix('v')
                    .unwrap_or(&release.tag_name),
            )
            .ok()?;
            if release.tag_name.len() > 128
                || release.draft
                || !version.cmp_precedence(current).is_gt()
                || (!preview && (release.prerelease || !version.pre.is_empty()))
            {
                return None;
            }
            let asset_name = edition.asset_name(&version.to_string());
            let asset = release
                .assets
                .into_iter()
                .find(|asset| asset.name == asset_name);
            let selected = Release {
                version: version.to_string(),
                url: release.html_url,
                title: release
                    .name
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| version.to_string())
                    .chars()
                    .take(256)
                    .collect(),
                // Bound retained UI/cache metadata; the full publisher notes remain on the release page.
                notes: release
                    .body
                    .unwrap_or_default()
                    .chars()
                    .take(16 * 1024)
                    .collect(),
                published_at: release.published_at.unwrap_or_default(),
                asset,
            };
            selected.validate(edition).ok()?;
            Some((version, selected))
        })
        .max_by(|(a, _), (b, _)| a.cmp_precedence(b))
        .map(|(_, release)| release)
}
