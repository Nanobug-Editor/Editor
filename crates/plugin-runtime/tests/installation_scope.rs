//! Installation consent persists one global default without leaking project availability to other workspaces.

use plugin_runtime::{InstallControl, Manager, Package, plugin_protocol::Environment};
use std::collections::BTreeMap;

#[path = "support/packages.rs"]
mod test_packages;

/// Build a validated inert package for `scope` and `version`; optional `component` tests cold admission.
fn resource_package(scope: &str, version: &str, component: bool) -> Package {
    let mut manifest = serde_json::json!({
        "id":"install-scope", "name":"Install scope", "version":version, "protocol":7,
        "api":{"base":"^1"}, "scope":scope, "contributions":"plugin.toml", "storage_limit":1024
    });
    let mut files = BTreeMap::from([
        ("manifest.json".into(), Vec::new()),
        (
            "plugin.toml".into(),
            format!("[plugin]\nid='install-scope'\nname='Install scope'\nversion='{version}'\nhost_version='^0.1'\n").into_bytes(),
        ),
    ]);
    if component {
        manifest["component"] = "guest.wasm".into();
        files.insert("guest.wasm".into(), b"\0asm\x0d\0\x01\0".to_vec());
    }
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    Package::from_files(files).unwrap()
}

/// An unchecked first install works in its project, survives updates and stays unavailable elsewhere.
#[test]
fn unchecked_install_is_project_only_and_updates_preserve_scope() {
    let profile = tempfile::tempdir().unwrap();
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let environment = Environment {
        workspace: first.path().display().to_string(),
        ..Default::default()
    };
    let mut manager = Manager::open(profile.path().into(), environment.clone()).unwrap();
    manager
        .install_with_control(
            &resource_package("workspace", "1.0.0", false),
            Default::default(),
            &InstallControl::default().with_default_global_enabled(false),
        )
        .unwrap();
    assert!(manager.availability("install-scope").is_ok());
    // A later installation's checkbox cannot overwrite an already saved project-only choice.
    manager
        .install_with_control(
            &resource_package("workspace", "2.0.0", false),
            Default::default(),
            &InstallControl::default().with_default_global_enabled(true),
        )
        .unwrap();
    let saved = Manager::read_registry(profile.path()).unwrap();
    assert!(!saved["install-scope"].enabled);
    assert_eq!(
        saved["install-scope"].project_enabled,
        [environment.workspace.clone()].into()
    );
    drop(manager);
    let mut other = Manager::open(
        profile.path().into(),
        Environment {
            workspace: second.path().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(other.availability("install-scope").is_err());
    drop(other);
    let mut reopened = Manager::open(profile.path().into(), environment).unwrap();
    assert!(reopened.availability("install-scope").is_ok());
}

/// Opting in persists availability for other projects and updates retain that explicit choice.
#[test]
fn checked_install_is_global_and_updates_preserve_scope() {
    let profile = tempfile::tempdir().unwrap();
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(
        profile.path().into(),
        Environment {
            workspace: first.path().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    for (version, global) in [("1.0.0", true), ("2.0.0", false)] {
        manager
            .install_with_control(
                &resource_package("workspace", version, false),
                Default::default(),
                &InstallControl::default().with_default_global_enabled(global),
            )
            .unwrap();
    }
    let saved = Manager::read_registry(profile.path()).unwrap();
    assert!(saved["install-scope"].enabled);
    assert!(saved["install-scope"].project_enabled.is_empty());
    drop(manager);
    let mut other = Manager::open(
        profile.path().into(),
        Environment {
            workspace: second.path().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(other.availability("install-scope").is_ok());
}

/// With no project, both resource and compiled workspace packages persist the unchecked default cold.
#[test]
fn unchecked_install_without_workspace_stays_disabled() {
    for component in [false, true] {
        let profile = tempfile::tempdir().unwrap();
        let mut manager = Manager::open(profile.path().into(), Environment::default()).unwrap();
        manager
            .install_with_control(
                &resource_package("workspace", "1.0.0", component),
                Default::default(),
                &InstallControl::default().with_default_global_enabled(false),
            )
            .unwrap();
        let saved = Manager::read_registry(profile.path()).unwrap();
        assert!(!saved["install-scope"].enabled);
        assert!(saved["install-scope"].project_enabled.is_empty());
        assert_eq!(manager.resource_count(), 0);
    }
}

/// Application packages have no project fallback and require the global opt-in to become available.
#[test]
fn application_install_respects_global_opt_in() {
    for global in [false, true] {
        let profile = tempfile::tempdir().unwrap();
        let mut manager = Manager::open(profile.path().into(), Environment::default()).unwrap();
        manager
            .install_with_control(
                &resource_package("application", "1.0.0", false),
                Default::default(),
                &InstallControl::default().with_default_global_enabled(global),
            )
            .unwrap();
        let saved = Manager::read_registry(profile.path()).unwrap();
        assert_eq!(saved["install-scope"].enabled, global);
        assert!(saved["install-scope"].project_enabled.is_empty());
    }
}

/// A real guest follows the prepared transaction path and publishes the same scope choices atomically.
#[test]
#[ignore = "uses the frozen example WASM ZIP listed in tests/fixtures/packages/inputs.json"]
fn wasm_install_respects_global_opt_in() {
    let package = test_packages::package("example");
    assert!(package.manifest.component.is_some());
    for global in [false, true] {
        let profile = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        let workspace = project.path().display().to_string();
        let mut manager = Manager::open(
            profile.path().into(),
            Environment {
                workspace: workspace.clone(),
                ..Default::default()
            },
        )
        .unwrap();
        manager
            .install_with_control(
                &package,
                package.manifest.permissions.clone(),
                &InstallControl::default().with_default_global_enabled(global),
            )
            .unwrap();
        let saved = Manager::read_registry(profile.path()).unwrap();
        let entry = &saved[&package.manifest.id];
        assert_eq!(entry.enabled, global);
        assert_eq!(entry.project_enabled.contains(&workspace), !global);
        assert!(manager.availability(&package.manifest.id).is_ok());
    }
}
