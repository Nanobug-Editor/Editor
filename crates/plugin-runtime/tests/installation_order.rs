//! Public package installation retains durable ordering independently of package names and availability.

use plugin_runtime::{Manager, Package, plugin_protocol::Environment};
use std::collections::BTreeMap;

/// Build an inert current package with the requested identity and version through normal validation.
fn package(id: &str, version: &str) -> Package {
    let manifest = serde_json::json!({
        "id":id, "name":id, "version":version, "protocol":7,
        "api":{"base":"^1"}, "scope":"workspace", "contributions":"plugin.toml", "storage_limit":1024
    });
    Package::from_files(BTreeMap::from([
        (
            "manifest.json".into(),
            serde_json::to_vec(&manifest).unwrap(),
        ),
        (
            "plugin.toml".into(),
            format!("[plugin]\nid='{id}'\nname='{id}'\nversion='{version}'\nhost_version='^0.1'\n")
                .into_bytes(),
        ),
    ]))
    .unwrap()
}

/// Install via the public manager so metadata follows the same transaction as ordinary resource packages.
fn install(manager: &mut Manager, id: &str, version: &str) {
    manager
        .install(&package(id, version), Default::default())
        .unwrap();
}

/// Updates, enablement and reopening retain original positions; a removed package rejoins at the end.
#[test]
fn installation_order_survives_updates_enablement_and_reopen() {
    let profile = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let environment = Environment {
        workspace: project.path().display().to_string(),
        ..Default::default()
    };
    let mut manager = Manager::open(profile.path().into(), environment.clone()).unwrap();
    install(&mut manager, "z-first", "1.0.0");
    install(&mut manager, "a-second", "1.0.0");
    assert_eq!(manager.installed["z-first"].install_sequence, 1);
    assert_eq!(manager.installed["a-second"].install_sequence, 2);

    manager.disable("z-first").unwrap();
    manager.enable("z-first").unwrap();
    install(&mut manager, "z-first", "2.0.0");
    drop(manager);

    let mut manager = Manager::open(profile.path().into(), environment).unwrap();
    let published = manager.published_entries();
    let sequences: BTreeMap<_, _> = published
        .iter()
        .map(|entry| (entry.manifest.id.as_str(), entry.install_sequence))
        .collect();
    assert_eq!(sequences, BTreeMap::from([("z-first", 1), ("a-second", 2)]));
    assert_eq!(manager.installed["z-first"].manifest.version, "2.0.0");
    install(&mut manager, "m-third", "1.0.0");
    assert_eq!(manager.installed["m-third"].install_sequence, 3);

    manager.uninstall("z-first", false).unwrap();
    install(&mut manager, "z-first", "2.0.0");
    let saved = Manager::read_registry(profile.path()).unwrap();
    assert_eq!(saved["z-first"].install_sequence, 4);
    assert_eq!(saved["a-second"].install_sequence, 2);
    assert_eq!(saved["m-third"].install_sequence, 3);
}

/// Installing before a project exists uses the same order and repeating admission does not move a package.
#[test]
fn installation_order_is_recorded_for_dormant_workspace_packages() {
    let profile = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(profile.path().into(), Environment::default()).unwrap();
    install(&mut manager, "z-first", "1.0.0");
    install(&mut manager, "a-second", "1.0.0");
    install(&mut manager, "z-first", "1.0.0");
    let saved = Manager::read_registry(profile.path()).unwrap();
    assert_eq!(saved["z-first"].install_sequence, 1);
    assert_eq!(saved["a-second"].install_sequence, 2);
    assert!(manager.workspace().is_empty());
    assert_eq!(manager.resource_count(), 0);
}

/// Older registries retain an explicit unknown position and a deterministic ID fallback without fabricated history.
#[test]
fn installation_order_legacy_records_keep_unknown_positions() {
    let profile = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let environment = Environment {
        workspace: project.path().display().to_string(),
        ..Default::default()
    };
    let mut manager = Manager::open(profile.path().into(), environment.clone()).unwrap();
    install(&mut manager, "z-legacy", "1.0.0");
    install(&mut manager, "a-legacy", "1.0.0");
    drop(manager);

    let path = profile.path().join("registry.json");
    let mut legacy: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    for entry in legacy.as_object_mut().unwrap().values_mut() {
        entry.as_object_mut().unwrap().remove("install_sequence");
    }
    std::fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
    let mut manager = Manager::open(profile.path().into(), environment).unwrap();
    assert_eq!(
        manager
            .published_entries()
            .iter()
            .map(|entry| (entry.manifest.id.as_str(), entry.install_sequence))
            .collect::<Vec<_>>(),
        vec![("a-legacy", 0), ("z-legacy", 0)]
    );
    install(&mut manager, "z-legacy", "2.0.0");
    install(&mut manager, "new-install", "1.0.0");
    let saved = Manager::read_registry(profile.path()).unwrap();
    assert_eq!(saved["a-legacy"].install_sequence, 0);
    assert_eq!(saved["z-legacy"].install_sequence, 0);
    assert_eq!(saved["new-install"].install_sequence, 1);
}
