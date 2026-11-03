//! Bounded TLS-only GitHub reads and verified private staging never write into the running app.

use super::release::{API, GithubRelease, Identity, Release};
use anyhow::{Context, Result, bail, ensure};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

/// One request chain permits GitHub's asset CDN, but never an arbitrary or plaintext redirect.
fn allowed_url(url: &url::Url) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port_or_known_default() == Some(443)
        && matches!(
            url.host_str(),
            Some(
                "github.com"
                    | "api.github.com"
                    | "release-assets.githubusercontent.com"
                    | "objects.githubusercontent.com"
            )
        )
}

/// Use bounded timeouts and inspect every redirect before following it with no credentials.
fn get(address: &str, timeout: Duration) -> Result<ureq::http::Response<ureq::Body>> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .https_only(true)
        .max_redirects(0)
        .max_redirects_will_error(false)
        .http_status_as_error(false)
        .timeout_global(Some(timeout))
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_recv_body(Some(Duration::from_secs(30)))
        .build()
        .into();
    let mut address = url::Url::parse(address)?;
    for _ in 0..6 {
        ensure!(allowed_url(&address), "Untrusted host update URL");
        let response = agent
            .get(address.as_str())
            .header("User-Agent", "Nanobug-Host-Updater")
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .call()?;
        if response.status().is_redirection() {
            let next = response
                .headers()
                .get("location")
                .context("Update redirect has no location")?
                .to_str()?;
            address = address.join(next)?;
            continue;
        }
        return Ok(response);
    }
    bail!("Too many update redirects")
}

/// Read at most `limit` bytes from `reader`; overflow is rejected instead of silently truncated.
fn read_bounded(reader: impl Read, limit: u64) -> Result<Vec<u8>> {
    let mut data = Vec::new();
    reader.take(limit + 1).read_to_end(&mut data)?;
    ensure!(
        data.len() as u64 <= limit,
        "Update metadata exceeds size limit"
    );
    Ok(data)
}

/// Read one small application-owned file while enforcing the same limit as network metadata.
pub(super) fn read_small_file(path: &Path, limit: u64) -> Result<Vec<u8>> {
    read_bounded(File::open(path)?, limit)
}

/// Public stable discovery uses GitHub's latest release; preview discovery includes prereleases.
pub(super) fn check(identity: &Identity, preview: bool) -> Result<Option<Release>> {
    let address = if preview {
        format!("{API}?per_page=100")
    } else {
        format!("{API}/latest")
    };
    let mut response = get(&address, Duration::from_secs(30))?;
    if response.status().as_u16() == 404 && !preview {
        return Ok(None);
    }
    ensure!(
        response.status().is_success(),
        "GitHub update check returned HTTP {}",
        response.status()
    );
    let data = read_bounded(response.body_mut().as_reader(), 4 * 1024 * 1024)?;
    let releases = if preview {
        serde_json::from_slice::<Vec<GithubRelease>>(&data)?
    } else {
        vec![serde_json::from_slice::<GithubRelease>(&data)?]
    };
    Ok(super::release::select(
        releases,
        &identity.version,
        preview,
        identity.edition,
    ))
}

/// SHA-256 uses a fixed buffer so verification does not retain another copy of an executable.
pub(super) fn file_digest(path: &Path) -> Result<String> {
    let mut reader = File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

/// Transfer an exact `size` and `digest` into `writer`; cancellation never publishes a ready file.
pub(super) fn transfer(
    mut reader: impl Read,
    mut writer: impl Write,
    size: u64,
    digest: &str,
    cancel: &AtomicBool,
    progress: &AtomicU64,
) -> Result<()> {
    let mut hash = Sha256::new();
    let mut received = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        ensure!(!cancel.load(Ordering::Relaxed), "Download cancelled");
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        received = received
            .checked_add(count as u64)
            .context("Download size overflow")?;
        ensure!(
            received <= size,
            "Update download exceeds the published size"
        );
        writer.write_all(&buffer[..count])?;
        hash.update(&buffer[..count]);
        progress.store(received, Ordering::Relaxed);
    }
    ensure!(received == size, "Update download is incomplete");
    ensure!(
        format!("{:x}", hash.finalize()).eq_ignore_ascii_case(digest),
        "Update SHA-256 mismatch"
    );
    ensure!(!cancel.load(Ordering::Relaxed), "Download cancelled");
    Ok(())
}

/// A durable ready pointer references one verified package in a randomly named private job directory.
#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub(super) struct Prepared {
    pub(super) directory: PathBuf,
    pub(super) release: Release,
    pub(super) edition: super::release::Edition,
}

impl Prepared {
    /// The package filename is derived from validated publisher metadata, not a CLI argument.
    pub(super) fn package(&self) -> Result<PathBuf> {
        Ok(self.directory.join(
            &self
                .release
                .asset
                .as_ref()
                .context("Release has no package")?
                .name,
        ))
    }

    /// Recheck the private job boundary, delivery style, size and digest before any installation.
    pub(super) fn verify(&self, profile: &Path, edition: super::release::Edition) -> Result<()> {
        ensure!(self.edition == edition, "Update installation style changed");
        self.release.validate(edition)?;
        let root = profile.join("updates").canonicalize()?;
        let directory = self.directory.canonicalize()?;
        ensure!(
            directory.parent() == Some(root.as_path()),
            "Update job escaped private storage"
        );
        let package = self.package()?;
        ensure!(
            package.canonicalize()?.parent() == Some(directory.as_path()),
            "Update package escaped its job"
        );
        let asset = self
            .release
            .asset
            .as_ref()
            .context("Release has no package")?;
        ensure!(
            fs::metadata(&package)?.len() == asset.size,
            "Cached update size changed"
        );
        ensure!(
            file_digest(&package)?.eq_ignore_ascii_case(asset.sha256()?),
            "Cached update SHA-256 changed"
        );
        Ok(())
    }

    /// Restore a previously downloaded package only for a still-newer release of this running host.
    pub(super) fn load(profile: &Path, identity: &Identity) -> Result<Option<Self>> {
        let path = profile.join("updates/ready.json");
        if !path.exists() {
            return Ok(None);
        }
        let prepared: Self = serde_json::from_slice(&read_small_file(&path, 128 * 1024)?)?;
        if !semver::Version::parse(&prepared.release.version)?
            .cmp_precedence(&identity.version)
            .is_gt()
        {
            fs::remove_file(path)?;
            return Ok(None);
        }
        prepared.verify(profile, identity.edition)?;
        Ok(Some(prepared))
    }
}

/// Atomically publish application-owned JSON after its complete bytes have reached the disk.
pub(super) fn save_json(path: &Path, value: &impl serde::Serialize) -> Result<()> {
    let parent = path.parent().context("JSON destination has no parent")?;
    fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer_pretty(&mut file, value)?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|error| error.error)?;
    Ok(())
}

/// Download into private staging; only an exact verified package becomes the persisted ready update.
pub(super) fn prepare(
    profile: &Path,
    identity: &Identity,
    release: Release,
    cancel: Arc<AtomicBool>,
    progress: Arc<AtomicU64>,
) -> Result<Prepared> {
    release.validate(identity.edition)?;
    let asset = release
        .asset
        .as_ref()
        .context("No update package for this platform")?;
    let root = profile.join("updates");
    fs::create_dir_all(&root)?;
    let directory = tempfile::Builder::new().prefix("job-").tempdir_in(&root)?;
    let prepared = Prepared {
        directory: directory.path().to_owned(),
        release: release.clone(),
        edition: identity.edition,
    };
    let mut response = get(&asset.browser_download_url, Duration::from_secs(15 * 60))?;
    ensure!(
        response.status().is_success(),
        "GitHub update download returned HTTP {}",
        response.status()
    );
    let mut file = File::create(prepared.package()?)?;
    transfer(
        response.body_mut().as_reader(),
        &mut file,
        asset.size,
        asset.sha256()?,
        &cancel,
        &progress,
    )?;
    file.sync_all()?;
    drop(file);
    // Copy the current executable outside the install tree before it will be replaced.
    fs::copy(
        &identity.executable,
        directory.path().join("update-helper.exe"),
    )?;
    ensure!(!cancel.load(Ordering::Relaxed), "Download cancelled");
    save_json(&root.join("ready.json"), &prepared)?;
    let _ = directory.keep();
    Ok(prepared)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exact verified bytes succeed; corruption, truncation, overflow and cancellation fail closed.
    #[test]
    fn host_update_transfer_checks_integrity_and_cancellation() {
        let data = b"verified update";
        let digest = format!("{:x}", Sha256::digest(data));
        let cancel = AtomicBool::new(false);
        let progress = AtomicU64::new(0);
        let mut out = Vec::new();
        transfer(
            &data[..],
            &mut out,
            data.len() as u64,
            &digest,
            &cancel,
            &progress,
        )
        .unwrap();
        assert_eq!(out, data);
        assert_eq!(progress.load(Ordering::Relaxed), data.len() as u64);
        for (size, checksum) in [
            (data.len() as u64 + 1, digest.clone()),
            (1, digest.clone()),
            (data.len() as u64, "0".repeat(64)),
        ] {
            assert!(transfer(&data[..], Vec::new(), size, &checksum, &cancel, &progress).is_err());
        }
        cancel.store(true, Ordering::Relaxed);
        assert!(
            transfer(
                &data[..],
                Vec::new(),
                data.len() as u64,
                &digest,
                &cancel,
                &progress
            )
            .is_err()
        );
    }

    /// Redirect policy rejects downgrade, foreign publisher hosts, credentials and nonstandard ports.
    #[test]
    fn host_update_redirects_stay_on_github_https() {
        for address in [
            "http://github.com/a",
            "https://evil.example/a",
            "https://user@github.com/a",
            "https://github.com:444/a",
        ] {
            assert!(
                !allowed_url(&url::Url::parse(address).unwrap()),
                "{address}"
            );
        }
        assert!(allowed_url(
            &url::Url::parse("https://release-assets.githubusercontent.com/file").unwrap()
        ));
        assert!(read_bounded(&b"abcd"[..], 3).is_err());
    }
}
