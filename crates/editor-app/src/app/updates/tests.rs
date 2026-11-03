//! Release, durable staging and native navigation regressions use isolated application profiles.

use super::*;
use release::{Asset, GithubRelease, REPOSITORY};
use semver::Version;
use std::fs;

/// Construct a publisher response with exact platform assets and no networking.
fn github(version: &str, preview: bool) -> GithubRelease {
    let name = Edition::Portable.asset_name(version);
    GithubRelease {
        tag_name: format!("v{version}"),
        html_url: format!("{REPOSITORY}/releases/tag/v{version}"),
        name: Some(format!("Nanobug {version}")),
        body: Some("发行说明 / Release notes".into()),
        published_at: Some("2026-11-02T00:00:00Z".into()),
        draft: false,
        prerelease: preview,
        assets: vec![Asset {
            name: name.clone(),
            browser_download_url: format!("{REPOSITORY}/releases/download/v{version}/{name}"),
            size: 3,
            digest: Some(format!("sha256:{}", "0".repeat(64))),
        }],
    }
}

/// Full SemVer ordering handles internal build revisions, stable promotion and unordered API results.
#[test]
fn host_update_release_selection_compares_full_versions_and_channels() {
    let current = Version::parse("0.1.0-internal.20261102").unwrap();
    let candidates = vec![
        github("0.1.0-internal.20261102.2", true),
        github("0.2.0-beta.1", true),
        github("0.1.0", false),
    ];
    let preview = release::select(candidates.clone(), &current, true, Edition::Portable).unwrap();
    assert_eq!(preview.version, "0.2.0-beta.1");
    let stable = release::select(candidates, &current, false, Edition::Portable).unwrap();
    assert_eq!(stable.version, "0.1.0");
    assert!(
        release::select(
            vec![github("0.1.0-internal.20261102.2", true)],
            &Version::parse("0.1.0").unwrap(),
            true,
            Edition::Portable
        )
        .is_none()
    );
    assert!(
        release::select(
            vec![github("0.1.0", false)],
            &Version::parse("0.1.0").unwrap(),
            false,
            Edition::Portable
        )
        .is_none()
    );
}

/// Drafts, forged publishers, unexpected asset URLs and missing hashes never become executable updates.
#[test]
fn host_update_release_validation_rejects_untrusted_candidates() {
    let current = Version::parse("0.0.1").unwrap();
    let mut draft = github("0.1.0", false);
    draft.draft = true;
    let mut publisher = github("0.1.0", false);
    publisher.html_url = "https://github.com/other/Editor/releases/tag/v0.1.0".into();
    let mut download = github("0.1.0", false);
    download.assets[0].browser_download_url = "https://evil.example/Nanobug.zip".into();
    let mut digest = github("0.1.0", false);
    digest.assets[0].digest = None;
    for candidate in [draft, publisher, download, digest] {
        assert!(release::select(vec![candidate], &current, false, Edition::Portable).is_none());
    }
    let mut missing = github("0.1.0", false);
    missing.assets.clear();
    let release = release::select(vec![missing], &current, false, Edition::Portable).unwrap();
    assert!(release.asset.is_none());
}

/// Persist a real tiny verified package and reconstruct its cached descriptor on disk.
fn prepared(profile: &Path) -> Prepared {
    let directory = profile.join("updates/job-fixture");
    fs::create_dir_all(&directory).unwrap();
    let mut selected = release::select(
        vec![github("0.2.0", false)],
        &Version::parse("0.1.0").unwrap(),
        false,
        Edition::Portable,
    )
    .unwrap();
    let asset = selected.asset.as_mut().unwrap();
    let path = directory.join(&asset.name);
    fs::write(&path, b"zip").unwrap();
    asset.digest = Some(format!("sha256:{}", download::file_digest(&path).unwrap()));
    let prepared = Prepared {
        directory,
        release: selected,
        edition: Edition::Portable,
    };
    download::save_json(&profile.join("updates/ready.json"), &prepared).unwrap();
    prepared
}

/// Seed only inert cached bytes for native UI consent tests; no executable helper is supplied.
pub(crate) fn ready_fixture(profile: &Path, cx: &mut App) -> Entity<Updater> {
    let cached = prepared(profile);
    let updater = ensure(profile, cx);
    updater.update(cx, |updater, cx| {
        updater.preferences.automatic = false;
        updater.available = Some(cached.release.clone());
        updater.prepared = Some(cached);
        updater.phase = Phase::Ready;
        updater.error = None;
        cx.notify();
        cx.refresh_windows();
    });
    updater
}

/// Restart persistence never trusts changed bytes, a different delivery style or an escaped job path.
#[test]
fn host_update_cached_package_is_reverified() {
    let profile = tempfile::tempdir().unwrap();
    let cached = prepared(profile.path());
    let identity = Identity {
        version: Version::parse("0.1.0").unwrap(),
        executable: profile.path().join("Nanobug.exe"),
        edition: Edition::Portable,
    };
    assert!(Prepared::load(profile.path(), &identity).unwrap().is_some());
    assert!(cached.verify(profile.path(), Edition::Installed).is_err());
    fs::write(cached.package().unwrap(), b"bad").unwrap();
    assert!(Prepared::load(profile.path(), &identity).is_err());
    let mut escaped = cached;
    escaped.directory = profile.path().to_owned();
    assert!(escaped.verify(profile.path(), Edition::Portable).is_err());
}

/// Applying a user preference does not modify workspace configuration or grant automatic installation.
#[test]
fn host_update_preferences_preserve_the_user_channel() {
    let profile = tempfile::tempdir().unwrap();
    assert!(!Preferences::load(profile.path(), false).unwrap().preview);
    assert!(Preferences::load(profile.path(), true).unwrap().preview);
    download::save_json(
        &profile.path().join("updates/preferences.json"),
        &Preferences {
            automatic: false,
            preview: false,
        },
    )
    .unwrap();
    let restored = Preferences::load(profile.path(), true).unwrap();
    assert!(!restored.automatic && !restored.preview);
}

/// A cancelled all-window quit disarms installation while retaining the verified package for retry.
#[gpui_kit::gpui::test]
fn host_update_cancelled_exit_preserves_ready_package(cx: &mut gpui_kit::TestAppContext) {
    let profile = tempfile::tempdir().unwrap();
    let cached = prepared(profile.path());
    cx.update(|cx| {
        let updater = ensure(profile.path(), cx);
        updater.update(cx, |updater, _| {
            updater.prepared = Some(cached.clone());
            updater.available = Some(cached.release.clone());
            updater.phase = Phase::Installing;
        });
        cx.set_global(PendingInstall {
            updater: updater.clone(),
        });
        cancel_install(cx);
        assert!(!cx.has_global::<PendingInstall>());
        assert!(updater.read(cx).phase == Phase::Ready);
        assert!(updater.read(cx).prepared.is_some());
        assert!(cached.package().unwrap().exists());
        assert!(finish_exit(cx).is_ok());
    });
}

/// Real GitHub metadata and package verification can be requested explicitly without installing anything.
#[test]
#[ignore = "downloads a public GitHub host release into an isolated temporary profile"]
fn host_update_github_release_download_smoke() {
    let profile = tempfile::tempdir().unwrap();
    let identity = Identity {
        version: Version::parse("0.0.1").unwrap(),
        executable: std::env::current_exe().unwrap(),
        edition: Edition::Portable,
    };
    let release = download::check(&identity, true)
        .unwrap()
        .expect("The publisher must have a newer release");
    let prepared = download::prepare(
        profile.path(),
        &identity,
        release,
        Arc::new(AtomicBool::new(false)),
        Arc::new(AtomicU64::new(0)),
    )
    .unwrap();
    prepared.verify(profile.path(), Edition::Portable).unwrap();
    assert!(Prepared::load(profile.path(), &identity).unwrap().is_some());
    let extracted = profile.path().join("extracted-app");
    fs::create_dir(&extracted).unwrap();
    portable::apply(
        &prepared.package().unwrap(),
        &extracted,
        &prepared.directory,
    )
    .unwrap();
    assert!(extracted.join("Nanobug.exe").is_file());
}
