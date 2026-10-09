//! Independent SDK-built cloud packages enter the production Package/Manager boundary.
use plugin_runtime::{
    Manager, Package,
    plugin_protocol::{Environment, api, ui::Kind},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::{Cursor, Write},
    path::Path,
    time::{Duration, Instant},
};

pub fn package(id: &str, edit: impl FnOnce(&mut Value)) -> Package {
    let archive = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../target/cloud-api-test/{id}-0.1.0.zip"));
    let mut files: BTreeMap<_, _> = Package::read(&archive)
        .expect("Build both cloud packages through --plugin-package")
        .files;
    let mut manifest: Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    edit(&mut manifest);
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    archive_files(files)
}
pub fn archive_files(files: BTreeMap<String, Vec<u8>>) -> Package {
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in files {
        archive
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        archive.write_all(&bytes).unwrap();
    }
    Package::from_bytes(&archive.finish().unwrap().into_inner()).unwrap()
}
pub fn environment(path: &Path) -> Environment {
    Environment {
        workspace: path.display().to_string(),
        locale: "en".into(),
        ..Default::default()
    }
}
pub fn install(manager: &mut Manager, package: &Package) {
    manager
        .install(package, package.manifest.permissions.clone())
        .unwrap();
}
pub fn text(manager: &Manager, id: &str) -> String {
    let Kind::Text { text } = &manager.live[id].views["cloud"].root.kind else {
        panic!("Expected public native result text")
    };
    text.clone()
}
pub fn probe(
    manager: &mut Manager,
    id: &str,
    operation: api::Operation,
) -> Result<api::Value, api::Failure> {
    manager
        .invoke_command(id, "probe", serde_json::to_value(operation).unwrap())
        .unwrap();
    serde_json::from_str(&text(manager, id)).unwrap()
}
pub fn resource(value: api::Value) -> api::ResourceHandle {
    let api::Value::Resource(handle) = value else {
        panic!("Expected owned resource")
    };
    handle
}
pub fn grant(manifest: &mut Value, origin: &str) {
    manifest["permissions"]
        .as_array_mut()
        .unwrap()
        .push(json!(format!("network.http:{origin}")));
}
pub fn wait(manager: &mut Manager, condition: impl Fn(&Manager) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !condition(manager) {
        assert!(
            Instant::now() < deadline,
            "Timed out waiting for public cloud output: resources={} instances={:?}",
            manager.resource_count(),
            manager
                .live
                .iter()
                .map(|(id, instance)| (
                    id,
                    manager.installed[id].error.as_ref(),
                    instance.views.get("cloud").map(|view| &view.root.kind)
                ))
                .collect::<Vec<_>>()
        );
        manager.poll();
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Credentials and all owned private files are removed when the temporary host profile is dropped.
pub struct SecretCleanup {
    pub root: tempfile::TempDir,
    pub target: String,
}
impl SecretCleanup {
    pub fn new(plugin: &str, key: &str) -> Self {
        use sha2::{Digest, Sha256};
        let root = tempfile::Builder::new()
            .prefix(&format!("cloud-qa-{}-", uuid::Uuid::new_v4()))
            .tempdir()
            .unwrap();
        let canonical = root.path().canonicalize().unwrap();
        let profile = format!(
            "{:x}",
            Sha256::digest(canonical.to_string_lossy().as_bytes())
        );
        let target = format!(
            "Nanobug/plugin/{profile}/{:x}/{:x}",
            Sha256::digest(plugin.as_bytes()),
            Sha256::digest(key.as_bytes())
        );
        Self { root, target }
    }
}
impl Drop for SecretCleanup {
    fn drop(&mut self) {
        let _ = platform_windows::credentials::CredentialStore::new(&self.target)
            .and_then(|store| store.delete());
    }
}
