//! Frozen Package inputs for host/runtime tests; these files never enter SDK or application payloads.
// Individual integration-test binaries use different facets of this shared test-only fixture loader.
#![allow(dead_code)]
use plugin_runtime::Package;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

#[derive(Deserialize)]
struct Inputs {
    packages: Vec<Input>,
    official_catalog: CatalogInput,
    official_statistics: CatalogInput,
}

/// The admitted snapshot is separate from packages and never controls a production source.
#[derive(Deserialize)]
struct CatalogInput {
    archive: String,
    sha256: String,
}

#[derive(Deserialize)]
struct Input {
    id: String,
    version: String,
    archive: String,
    sha256: String,
}

/// Find the fixed ledger beside the runtime integration tests, independently of the executable's location.
fn directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("plugin-runtime/tests/fixtures/packages")
}

/// Each identifier resolves only to the explicit version/hash in the checked-in input ledger.
fn inputs() -> &'static Inputs {
    static INPUTS: OnceLock<Inputs> = OnceLock::new();
    INPUTS.get_or_init(|| {
        serde_json::from_slice(&std::fs::read(directory().join("inputs.json")).unwrap()).unwrap()
    })
}

/// Exact archive names select historical inputs; bare IDs select the current frozen package.
fn input(id: &str) -> &'static Input {
    inputs()
        .packages
        .iter()
        .find(|input| input.id == id || input.archive.strip_suffix(".zip") == Some(id))
        .unwrap_or_else(|| panic!("No frozen package input for {id}"))
}

/// Verify the registry-built admission snapshot before tests use its source and download contracts.
pub(crate) fn official_catalog() -> Vec<u8> {
    let catalog = &inputs().official_catalog;
    let bytes = std::fs::read(directory().join(&catalog.archive)).unwrap();
    assert_eq!(format!("{:x}", Sha256::digest(&bytes)), catalog.sha256);
    bytes
}

/// Statistics retain their measured snapshot and digest; tests never replace its unavailable values with zero.
pub(crate) fn official_statistics() -> Vec<u8> {
    let statistics = &inputs().official_statistics;
    let bytes = std::fs::read(directory().join(&statistics.archive)).unwrap();
    assert_eq!(format!("{:x}", Sha256::digest(&bytes)), statistics.sha256);
    bytes
}

/// Return the exact archive after checking its digest; missing or changed bytes fail, with no network fallback.
pub(crate) fn archive(id: &str) -> PathBuf {
    let input = input(id);
    let path = directory().join(&input.archive);
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(format!("{:x}", Sha256::digest(&bytes)), input.sha256);
    path
}

/// Inspect through the production public Package boundary without executing WASM or preparing native dependencies.
pub(crate) fn package(id: &str) -> Package {
    let package = Package::read(&archive(id)).unwrap();
    let input = input(id);
    assert_eq!(package.manifest.id, input.id);
    assert_eq!(package.manifest.version, input.version);
    package
}

/// Resource-only consumers get validated Package files in a process-owned temporary directory.
/// Extraction preserves their original grammar/query/declaration assertions, without shipping built-in resources.
pub(crate) fn root(id: &str) -> PathBuf {
    static ROOTS: OnceLock<Mutex<BTreeMap<String, tempfile::TempDir>>> = OnceLock::new();
    let roots = ROOTS.get_or_init(Default::default);
    if let Some(directory) = roots.lock().unwrap().get(id) {
        return directory.path().to_owned();
    }
    let package = package(id);
    let temporary = tempfile::tempdir().unwrap();
    // Package validation rejects traversal and symlink entries before any file reaches this temporary root.
    for (name, bytes) in package.files {
        let path = temporary.path().join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
    let mut roots = roots.lock().unwrap();
    roots
        .entry(id.to_owned())
        .or_insert(temporary)
        .path()
        .to_owned()
}

/// Read a declaration/resource from the same inspected immutable fixture used by installation tests.
pub(crate) fn text(id: &str, name: &str) -> String {
    std::fs::read_to_string(root(id).join(name)).unwrap()
}
