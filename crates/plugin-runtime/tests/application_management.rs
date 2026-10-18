//! Profile management installs packages without fabricating a workspace or admitting plugin demands.

use plugin_runtime::{Manager, Package, plugin_protocol::Environment};
use std::collections::BTreeMap;
#[path = "support/packages.rs"]
mod test_packages;

/// The public manager can install before opening a project, but cannot consume workspace capabilities.
#[test]
fn application_management_keeps_workspace_plugins_dormant() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("plugins");
    let package = Package::from_files(BTreeMap::from([
        ("manifest.json".into(), br#"{"id":"empty-startup","name":"Empty startup","version":"1.0.0","protocol":7,"api":{"base":"^1"},"scope":"workspace","contributions":"plugin.toml","storage_limit":1024}"#.to_vec()),
        ("plugin.toml".into(), b"[plugin]\nid='empty-startup'\nname='Empty startup'\nversion='1.0.0'\nhost_version='^0.1'\n".to_vec()),
    ])).unwrap();
    let mut manager = Manager::open(root.clone(), Environment::default()).unwrap();
    manager.install(&package, Default::default()).unwrap();
    assert!(manager.workspace().is_empty());
    assert!(manager.availability("empty-startup").is_err());
    assert_eq!(manager.resource_count(), 0);
    assert!(!root.join("data/empty-startup/workspaces").exists());
    assert!(Manager::read_registry(&root).unwrap()["empty-startup"].enabled);
}

/// Saving user preferences for a dormant component must not allocate or prepare an empty workspace owner.
#[test]
fn application_management_persists_user_settings_without_private_workspace_data() {
    use plugin_runtime::plugin_protocol::settings::Scope;
    for apply in ["restart_instance", "hot_apply"] {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("plugins");
        let api = if apply == "hot_apply" { "^1.1" } else { "^1" };
        let manifest = serde_json::json!({
            "id":"global-setting", "name":"Global setting", "version":"1.0.0", "protocol":7,
            "api":{"base":"^1", "required":{"configuration":api}}, "scope":"workspace",
            "component":"guest.wasm", "settings_hook":apply == "hot_apply", "storage_limit":1024,
            "settings":{"flag":{"title":"Flag", "value_type":{"kind":"boolean"}, "default":false, "scope":"project", "apply":apply}}
        });
        let package = Package::from_files(BTreeMap::from([
            (
                "manifest.json".into(),
                serde_json::to_vec(&manifest).unwrap(),
            ),
            // A valid empty component cannot run configuration hooks, proving this path remains metadata-only.
            ("guest.wasm".into(), b"\0asm\x0d\0\x01\0".to_vec()),
        ]))
        .unwrap();
        let mut manager = Manager::open(root.clone(), Environment::default()).unwrap();
        manager.install(&package, Default::default()).unwrap();
        manager
            .update_setting("global-setting", Scope::User, "flag", Some(true.into()))
            .unwrap();
        assert!(
            manager
                .update_setting("global-setting", Scope::Project, "flag", Some(false.into()))
                .is_err()
        );
        assert!(manager.workspace().is_empty());
        assert_eq!(manager.resource_count(), 0);
        assert!(!root.join("data/global-setting/workspaces").exists());
        let saved: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.join("settings/global-setting.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(saved["user"]["flag"], true);
    }
}

/// An empty management context cannot replace the only fallback before a real scope can validate migration.
#[test]
fn application_management_preserves_old_package_when_update_needs_workspace() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("plugins");
    let make = |version: &str| {
        Package::from_files(BTreeMap::from([
            ("manifest.json".into(), serde_json::to_vec(&serde_json::json!({"id":"upgrade","name":"Upgrade","version":version,"protocol":7,"scope":"workspace","api":{"base":"^1"},"contributions":"plugin.toml","storage_limit":1024})).unwrap()),
            ("plugin.toml".into(), format!("[plugin]\nid='upgrade'\nname='Upgrade'\nversion='{version}'\nhost_version='^0.1'\n").into_bytes()),
        ])).unwrap()
    };
    let old = make("1.0.0");
    let mut manager = Manager::open(root.clone(), Environment::default()).unwrap();
    manager.install(&old, Default::default()).unwrap();
    assert!(manager.install(&make("2.0.0"), Default::default()).is_err());
    assert_eq!(
        Manager::read_registry(&root).unwrap()["upgrade"].digest,
        old.digest
    );
}

/// A deferred language result may publish to its captured real scope while profile management stays empty.
#[test]
fn application_management_restores_its_projection_after_project_demand_completion() {
    use plugin_runtime::plugin_protocol::{api, diagnostics::Fix, workspace};
    use plugin_runtime::{DemandReason, LanguageFixOrigin, LanguageRole};
    let directory = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let environment = Environment {
        workspace: project.path().canonicalize().unwrap().display().to_string(),
        ..Default::default()
    };
    let manifest = serde_json::json!({
        "id":"routed-language", "name":"Routed language", "version":"1.0.0", "protocol":7,
        "api":{"base":"^1", "required":{"language.lsp":"^1.4", "language.formatting":"^1", "process":"^1", "editor.edit":"^1.1", "workspace.files":"^1.3"}},
        "contributions":"plugin.toml", "scope":"workspace", "permissions":["process.service.analysis", "editor.read", "editor.write"], "storage_limit":1024,
        "services":{"analysis":{"program":std::env::current_exe().unwrap(), "args":[]}},
        "language_servers":[{"id":"analysis","language":"independent","service":"analysis"}, {"id":"formatter","language":"independent","service":"analysis","primary":false,"formatting":true}]
    });
    let package = Package::from_files(BTreeMap::from([
        ("manifest.json".into(), serde_json::to_vec(&manifest).unwrap()),
        ("plugin.toml".into(), b"[plugin]\nid='routed-language'\nname='Routed language'\nversion='1.0.0'\nhost_version='^0.1'\n".to_vec()),
    ])).unwrap();
    let mut manager = Manager::open(directory.path().into(), environment.clone()).unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    let availability = manager.availability("routed-language").unwrap();
    let first = manager
        .begin_demand(
            &availability,
            DemandReason::Language {
                provider: "routed-language/analysis".into(),
                role: LanguageRole::Lsp,
            },
            10_000,
        )
        .unwrap();
    let preparation = manager.take_demand_preparation().unwrap().run();
    manager.commit_demand(preparation).unwrap();
    assert!(matches!(
        first.status(),
        api::RequestUpdate::Completed { result: Ok(_) }
    ));
    let service = manager.language_services()["routed-language/analysis"]
        .as_ref()
        .unwrap()
        .clone();
    let document = api::DocumentVersion {
        id: "source".into(),
        path: "source.independent".into(),
        revision: 1,
    };
    manager
        .submit_language_fix(
            &LanguageFixOrigin::Lsp(service),
            document.clone(),
            Fix {
                title: "Edit".into(),
                plan: workspace::Plan {
                    title: "Edit".into(),
                    origin: Some(document.clone()),
                    changes: vec![workspace::Change::EditDocument {
                        document: document.clone(),
                        edits: vec![api::TextEdit {
                            range: api::TextRange { start: 0, end: 0 },
                            text: "new".into(),
                        }],
                        selection: api::TextRange { start: 3, end: 3 },
                    }],
                },
            },
        )
        .unwrap();
    let second = manager
        .begin_demand(
            &availability,
            DemandReason::Language {
                provider: "routed-language/formatter".into(),
                role: LanguageRole::Formatter,
            },
            10_000,
        )
        .unwrap();
    let preparation = manager.take_demand_preparation().unwrap().run();
    manager.select_application(Environment::default());
    manager.commit_demand(preparation).unwrap();
    manager.poll();
    assert!(matches!(
        second.status(),
        api::RequestUpdate::Completed { result: Ok(_) }
    ));
    assert!(manager.workspace().is_empty());
    assert!(manager.take_editor_requests().is_empty());
    manager.select_workspace(environment, true).unwrap();
    let requests = manager.take_editor_requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].0, "routed-language");
}

/// Use a real SDK guest with an unavailable optional service, so package preparation remains observable.
fn optional_workspace_package(directory: &std::path::Path) -> Package {
    let source = test_packages::package("sdk-styles-fixture");
    let mut files = source.files.clone();
    let mut manifest: serde_json::Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    manifest["api"]["required"]["language.lsp"] = "^1.4".into();
    manifest["api"]["required"]["process"] = "^1".into();
    manifest["api"]["required"]["dependencies"] = ">=1.1,<2".into();
    manifest["permissions"].as_array_mut().unwrap().extend([
        "process.service.analysis".into(),
        "dependencies.prepare".into(),
    ]);
    manifest["services"] = serde_json::json!({"analysis":{"program":"missing-optional", "installation":{
        "artifacts":[{"id":"native", "version":"1", "platform":format!("{}-{}",std::env::consts::OS,std::env::consts::ARCH), "sha256":"0000000000000000000000000000000000000000000000000000000000000000", "source":{"kind":"local","path":directory.join("missing.exe")}, "format":{"kind":"file","path":"server.exe"}}], "executable":"native/server.exe"}}});
    manifest["language_servers"] = serde_json::json!([{"id":"analysis","language":"independent","service":"analysis", "optional_installation":true}]);
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    Package::from_files(files).unwrap()
}

/// The frozen SDK guest validates cold installation and reinstall with both private-data retention options.
#[test]
#[ignore = "requires the hash-pinned sdk-styles-fixture package built through the host --plugin-cargo entry"]
fn application_management_cold_wasm_install_prepares_each_real_scope_and_reinstall() {
    let directory = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let package = optional_workspace_package(directory.path());
    let root = directory.path().join("plugins");
    let environment = Environment {
        workspace: project.path().canonicalize().unwrap().display().to_string(),
        ..Default::default()
    };
    let mut manager = Manager::open(root.clone(), Environment::default()).unwrap();
    let data = root.join("data").join(&package.manifest.id);
    let mut retained = false;
    for delete_data in [true, false, true] {
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
        assert_eq!(
            data.exists(),
            retained,
            "empty management must leave retained data untouched"
        );
        manager.select_workspace(environment.clone(), true).unwrap();
        let entry = &manager.installed[&package.manifest.id];
        assert!(
            entry.error.is_none(),
            "first optional preparation must preserve resources: {:?}",
            entry.error
        );
        assert!(
            root.join("data")
                .join(&package.manifest.id)
                .join("workspaces")
                .is_dir()
        );
        assert!(
            manager.instance_id(&package.manifest.id).is_none(),
            "preparation cannot activate business"
        );
        let sentinel = manager
            .data_directory(&package.manifest.id)
            .join("retained.txt");
        if retained {
            assert_eq!(std::fs::read(&sentinel).unwrap(), b"retained private state");
        }
        std::fs::write(&sentinel, b"retained private state").unwrap();
        manager
            .uninstall(&package.manifest.id, delete_data)
            .unwrap();
        retained = !delete_data;
        assert_eq!(data.exists(), retained);
        assert!(
            !root
                .join("pending-workspace-installations")
                .join(&package.manifest.id)
                .exists()
        );
        manager.select_application(Environment::default());
    }
}

/// Preparing a resumed project cannot turn its local override into a global preference.
#[test]
#[ignore = "requires the hash-pinned sdk-styles-fixture package built through the host --plugin-cargo entry"]
fn application_management_reinstall_preserves_project_only_enablement() {
    let directory = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let package = test_packages::package("sdk-styles-fixture");
    let root = directory.path().join("plugins");
    let environment = Environment {
        workspace: project.path().canonicalize().unwrap().display().to_string(),
        ..Default::default()
    };
    let mut manager = Manager::open(root.clone(), environment.clone()).unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    manager.disable(&package.manifest.id).unwrap();
    manager
        .set_project_enabled(&package.manifest.id, true)
        .unwrap();
    manager.select_application(Environment::default());
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    manager.select_workspace(environment.clone(), true).unwrap();
    let entry = &manager.installed[&package.manifest.id];
    assert!(entry.error.is_none(), "{:?}", entry.error);
    assert!(
        !entry.enabled,
        "preparing a project must preserve the global default"
    );
    assert!(entry.project_enabled_in(&environment.workspace));
    assert!(!Manager::read_registry(&root).unwrap()[&package.manifest.id].enabled);
    assert!(manager.availability(&package.manifest.id).is_ok());
}
