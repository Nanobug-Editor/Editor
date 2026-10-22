//! Source-free startup and explicit Package imports preserve old choices, authority and actor failure publication.
use super::*;
use crate::extensions::protocol;
use std::io::{Cursor, Write};
use std::sync::atomic::{AtomicBool, Ordering};

mod native_failure;

/// A real independent resource package contributes one arbitrary recognition selector and no executable guest.
fn archive(id: &str, version: &str, extension: &str) -> Vec<u8> {
    let manifest = serde_json::json!({
        "id":id,"name":"Generic prose provider","version":version,"protocol":7,
        "api":{"base":"^1"},"contributions":"plugin.toml","storage_limit":1024
    });
    let contribution = format!(
        "[plugin]\nid='{id}'\nname='Generic prose provider'\nversion='{version}'\nhost_version='^0.1'\n\n[[language_definitions]]\nid='prose'\nname='Prose'\nextensions=['{extension}']\n"
    );
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in [
        ("manifest.json", serde_json::to_vec(&manifest).unwrap()),
        ("plugin.toml", contribution.into_bytes()),
    ] {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(&bytes).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

/// Canonical fixture roots contain retired bundle remnants that startup must ignore.
struct Fixture {
    _directory: tempfile::TempDir,
    workspace: PathBuf,
    root: PathBuf,
    file: PathBuf,
    bytes: Vec<u8>,
}

impl Fixture {
    /// Keep the installed profile separate from project files, matching the production manager's ownership.
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let workspace = directory.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let workspace = workspace.canonicalize().unwrap();
        let root = directory.path().join("private-runtime");
        let file = workspace.join("notes.prose");
        std::fs::write(&file, "unmodified source").unwrap();
        let bytes = archive("bundled-prose", "1.0.0", "prose");
        let fixture = Self {
            _directory: directory,
            workspace,
            root,
            file,
            bytes,
        };
        fixture.catalog("prose", "generic.zip", None);
        fixture
    }

    /// Write valid or corrupt historical catalogs to prove neither can trigger automatic installation.
    fn catalog(&self, extension: &str, filename: &str, digest: Option<&str>) {
        let directory = self.workspace.join(".bundled-plugin-test");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("generic.zip"), &self.bytes).unwrap();
        let package = Package::from_bytes(&self.bytes).unwrap();
        let value = serde_json::json!({"version":1,"packages":[{
            "file":filename,"sha256":digest.unwrap_or(&package.digest),"file_extensions":[extension]
        }]});
        std::fs::write(
            directory.join("bundle-defaults.json"),
            serde_json::to_vec(&value).unwrap(),
        )
        .unwrap();
    }

    /// Public manager restoration is the only readiness authority in these fixtures.
    fn manager(&self) -> Manager {
        Manager::open(self.root.clone(), self.environment()).unwrap()
    }

    /// The actor and helper use the same workspace environment as ordinary public installations.
    fn environment(&self) -> protocol::Environment {
        protocol::Environment {
            workspace: self.workspace.display().to_string(),
            ..Default::default()
        }
    }
}

/// Historical explicit choices remain readable after restart; opening the actor never installs a retired bundle.
#[test]
fn legacy_decline_survives_restart_without_automatic_installation() {
    let fixture = Fixture::new();
    let mut manager = fixture.manager();
    manager.record_bundle_decline("bundled-prose").unwrap();
    drop(manager);
    let reopened = fixture.manager();
    assert!(reopened.has_bundle_choice("bundled-prose").unwrap());
    assert!(reopened.installed.is_empty() && reopened.live.is_empty());
    drop(reopened);
    let choices = std::fs::read(fixture.root.join("bundle-choices.json")).unwrap();
    let worker = Worker::start_background(fixture.root.clone(), fixture.environment(), true);
    wait_for(&worker, |state| state.ready);
    assert!(worker.state.lock().unwrap().entries.is_empty());
    assert_eq!(
        std::fs::read(fixture.root.join("bundle-choices.json")).unwrap(),
        choices
    );
}

/// A corrupt legacy ZIP is inert until an explicit user import; startup does not inspect or rewrite its bytes.
#[test]
fn startup_does_not_read_legacy_zip_or_record_a_choice() {
    let fixture = Fixture::new();
    let path = fixture.workspace.join(".bundled-plugin-test/generic.zip");
    std::fs::write(&path, "not a ZIP").unwrap();
    let worker = Worker::start_background(fixture.root.clone(), fixture.environment(), true);
    wait_for(&worker, |state| state.ready);
    let snapshot = {
        let state = worker.state.lock().unwrap();
        (
            state.entries.len(),
            state.pending.is_some(),
            state.status.is_some(),
        )
    };
    assert_eq!(snapshot, (0, false, false));
    assert!(!fixture.root.join("bundle-choices.json").exists());
    assert_eq!(std::fs::read_to_string(path).unwrap(), "not a ZIP");
}

/// Neither invalid hashes nor unsafe paths in obsolete recommendation metadata become installation authority.
#[test]
fn invalid_legacy_catalog_hash_selector_and_path_are_ignored() {
    for (extension, filename, digest) in [
        (
            "prose",
            "generic.zip",
            Some("0000000000000000000000000000000000000000000000000000000000000000"),
        ),
        ("different", "generic.zip", None),
        ("prose", "../generic.zip", None),
        ("prose", "CON.zip", None),
    ] {
        let fixture = Fixture::new();
        fixture.catalog(extension, filename, digest);
        let worker = Worker::start_background(fixture.root.clone(), fixture.environment(), true);
        wait_for(&worker, |state| state.ready);
        let state = worker.state.lock().unwrap();
        assert!(state.entries.is_empty() && state.pending.is_none() && state.status.is_none());
        assert!(!fixture.root.join("bundle-choices.json").exists());
    }
}

/// Restoring an alternative disabled provider keeps its explicit state instead of acquiring a former default.
#[test]
fn alternative_provider_and_disabled_restart_are_never_overridden() {
    let fixture = Fixture::new();
    let mut manager = fixture.manager();
    let alternative = Package::from_bytes(&archive("alternative-prose", "1.0.0", "prose")).unwrap();
    manager
        .install(&alternative, alternative.manifest.permissions.clone())
        .unwrap();
    manager.disable("alternative-prose").unwrap();
    drop(manager);
    let worker = Worker::start_background(fixture.root.clone(), fixture.environment(), true);
    wait_for(&worker, |state| state.ready);
    let entries = worker.state.lock().unwrap().entries.clone();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].manifest.id, "alternative-prose");
    assert!(!entries[0].enabled);
    assert!(
        !Manager::read_registry(&fixture.root)
            .unwrap()
            .contains_key("bundled-prose")
    );
}

/// Wait on the actor's public publication with a bounded deadline, without constructing an installed snapshot.
fn wait_for(worker: &Worker, predicate: impl Fn(&Published) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if predicate(&worker.state.lock().unwrap()) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "source-free actor publication timed out"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Inspection of an explicit local import grants nothing; normal lifecycle installation publishes the actual registry.
#[test]
fn production_actor_requires_explicit_import_and_installs_through_normal_lifecycle() {
    let fixture = Fixture::new();
    let worker = Worker::start_background(fixture.root.clone(), fixture.environment(), true);
    wait_for(&worker, |state| state.ready);
    assert!(worker.state.lock().unwrap().entries.is_empty());
    worker
        .tx
        .send(Work::Inspect(
            fixture.workspace.join(".bundled-plugin-test/generic.zip"),
        ))
        .unwrap();
    wait_for(&worker, |state| state.pending.is_some());
    // Native polling consumes this inspected package before the explicit permission dialog.
    let package = worker.state.lock().unwrap().pending.take().unwrap();
    assert!(
        worker.state.lock().unwrap().entries.is_empty(),
        "inspection cannot install or grant permissions"
    );
    assert!(worker.queue_lifecycle(Work::Install(package, true)));
    wait_for(&worker, |state| {
        state.progress.is_none() && !state.entries.is_empty()
    });
    let entries = worker.state.lock().unwrap().entries.clone();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].manifest.id, "bundled-prose");
    assert!(entries[0].enabled);
    assert!(worker.state.lock().unwrap().pending.is_none());
    assert_eq!(
        Manager::read_registry(&fixture.root).unwrap()["bundled-prose"].digest,
        Package::from_bytes(&fixture.bytes).unwrap().digest
    );
}

/// Private-store recovery failures cannot be mistaken for a healthy empty source-free installation.
#[test]
fn actor_recovery_error_keeps_installation_unavailable() {
    let fixture = Fixture::new();
    std::fs::create_dir_all(&fixture.root).unwrap();
    std::fs::write(fixture.root.join("registry.json"), "{invalid").unwrap();
    let worker = Worker::start_background(fixture.root.clone(), fixture.environment(), true);
    wait_for(&worker, |state| state.status.is_some());
    let state = worker.state.lock().unwrap();
    assert!(!state.ready && state.entries.is_empty() && state.pending.is_none());
    assert!(!fixture.root.join("bundle-choices.json").exists());
}

/// Revocation at actual resource preparation still reaches the public InstallControl cutover barrier.
#[test]
fn resource_only_explicit_import_revoked_during_preparation_cannot_commit() {
    let fixture = Fixture::new();
    let worker = Worker::start_background(fixture.root.clone(), fixture.environment(), true);
    wait_for(&worker, |state| state.ready);
    let active = Arc::new(AtomicBool::new(true));
    let revoked = Arc::new(AtomicBool::new(false));
    let preparing = active.clone();
    let reached = revoked.clone();
    let control = plugin_runtime::InstallControl::new(move |stage| {
        if matches!(stage, plugin_runtime::InstallStage::Preparing) {
            preparing.store(false, Ordering::Release);
            reached.store(true, Ordering::Release);
        }
    })
    .with_guard(move || active.load(Ordering::Acquire));
    worker.state.lock().unwrap().install_control = Some(control);
    worker
        .tx
        .send(Work::Install(
            Package::from_bytes(&fixture.bytes).unwrap(),
            true,
        ))
        .unwrap();
    wait_for(&worker, |state| {
        state.status.is_some() || !state.entries.is_empty()
    });
    assert!(
        revoked.load(Ordering::Acquire),
        "the real resource preparation stage ran"
    );
    assert!(worker.state.lock().unwrap().entries.is_empty());
    assert!(
        !Manager::read_registry(&fixture.root)
            .unwrap()
            .contains_key("bundled-prose")
    );
}
