//! Version-pinned SDK consumers and disposable OS credentials for lifecycle integration tests.
use plugin_runtime::Package;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::{Cursor, Write},
    path::{Path, PathBuf},
};

/// Read an actual SDK-built ZIP before changing only the declaration under test.
/// Old 0.1.1 bytes are pinned to the delivered SDK output; newer outputs must be built by the host.
pub fn package(id: &str, version: &str, edit: impl FnOnce(&mut Value)) -> Package {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../target/cloud-api-test/{id}-{version}.zip"));
    let bytes = std::fs::read(&path).expect(
        "Build new cloud guests through host --plugin-package; retain delivered 0.1.1 ZIPs",
    );
    if version == "0.1.1" {
        let expected = match id {
            "cloud-translation" => {
                "C1D6D01DD5D725F1686E71BEBFFCE8F1CDBD8CFFB8CAED64665314D096A0C67C"
            }
            "code-host-query" => "8E245135A2EC3745B8D5195E3C4F48378DC0746B3156D91B6FEB7E438776A6C4",
            _ => panic!("Unknown delivered SDK fixture"),
        };
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)).to_ascii_uppercase(),
            expected,
            "An old manifest over newly rebuilt WASM is not an old SDK consumer"
        );
    }
    let original = Package::from_bytes(&bytes).unwrap();
    assert_eq!(original.manifest.protocol, 7);
    assert_eq!(original.manifest.version.to_string(), version);
    let mut files = original.files;
    let mut manifest: Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    edit(&mut manifest);
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    inspect(files).unwrap()
}

/// Reinspect the complete real archive, including negative host-floor/protocol declarations.
/// The WASM and every other asset remain identical; no host test API or SDK copy is added.
pub fn inspect(files: BTreeMap<String, Vec<u8>>) -> anyhow::Result<Package> {
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in files {
        archive.start_file(name, zip::write::SimpleFileOptions::default())?;
        archive.write_all(&bytes)?;
    }
    Package::from_bytes(&archive.finish()?.into_inner())
}

/// Own exactly the isolated profile and explicitly registered test credential targets.
/// Credentials are written through the guest SDK; Drop removes only these known OS keys.
pub struct Credentials {
    /// Disposable profile root; its canonical path contributes to the own-secret namespace.
    pub root: tempfile::TempDir,
    targets: Vec<String>,
}
impl Credentials {
    /// Allocate an isolated profile without reading or importing the user's credential targets.
    pub fn new() -> Self {
        Self {
            root: tempfile::tempdir().unwrap(),
            targets: vec![],
        }
    }

    /// Register a plugin-private key before writing it, ensuring cleanup also on a failed assertion.
    pub fn register(&mut self, id: &str, key: &str) {
        let canonical: PathBuf = self.root.path().canonicalize().unwrap();
        let target = format!(
            "Nanobug/plugin/{:x}/{:x}/{:x}",
            Sha256::digest(canonical.to_string_lossy().as_bytes()),
            Sha256::digest(id.as_bytes()),
            Sha256::digest(key.as_bytes())
        );
        if !self.targets.contains(&target) {
            self.targets.push(target);
        }
    }
}
impl Drop for Credentials {
    fn drop(&mut self) {
        for target in &self.targets {
            let _ = platform_windows::credentials::CredentialStore::new(target)
                .and_then(|store| store.delete());
        }
    }
}
