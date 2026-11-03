//! A migrated resource package remains usable through ordinary Manager installation and loading.

use super::*;
use platform_windows::portable_data::{LegacyLocations, prepare};

/// Relocating the complete private store keeps installed metadata, package bytes and private data.
#[test]
fn portable_profile_keeps_installed_package_and_private_data() {
    let temporary = tempfile::tempdir().unwrap();
    let legacy = temporary.path().join("legacy");
    let portable = temporary.path().join("installation");
    let package = Package::from_files(BTreeMap::from([
        ("manifest.json".into(), br#"{"id":"portable-fixture","name":"Portable fixture","version":"1.0.0","protocol":7,"api":{"base":"^1"},"contributions":"plugin.toml","storage_limit":4096}"#.to_vec()),
        ("plugin.toml".into(), b"[plugin]\nid='portable-fixture'\nname='Portable fixture'\nversion='1.0.0'\nhost_version='^0.1'\n".to_vec()),
    ])).unwrap();
    let environment = Environment {
        workspace: temporary.path().display().to_string(),
        ..Default::default()
    };
    let mut manager = Manager::open(legacy.join("runtime-plugins"), environment.clone()).unwrap();
    manager.install(&package, BTreeSet::new()).unwrap();
    let data = manager.data_directory("portable-fixture");
    std::fs::create_dir_all(&data).unwrap();
    std::fs::write(data.join("saved.bin"), b"preserved private data").unwrap();
    drop(manager);
    prepare(
        &portable,
        Some(LegacyLocations {
            profile: Some(legacy.clone()),
            history: None,
        }),
    )
    .unwrap();
    let mut migrated = Manager::open(portable.join("runtime-plugins"), environment).unwrap();
    assert_eq!(
        std::fs::read(
            migrated
                .data_directory("portable-fixture")
                .join("saved.bin")
        )
        .unwrap(),
        b"preserved private data"
    );
    assert!(
        portable
            .join("runtime-plugins/packages/portable-fixture")
            .join(&package.digest)
            .join("plugin.toml")
            .is_file()
    );
    migrated.disable("portable-fixture").unwrap();
    migrated.enable("portable-fixture").unwrap();
    assert_eq!(
        std::fs::read(data.join("saved.bin")).unwrap(),
        b"preserved private data"
    );
}
