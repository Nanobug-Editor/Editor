//! Actual SDK guests prove that market updates retain the public manager's data transaction semantics.
use super::*;

/// A package-controlled migration policy drives faults without introducing a host-side test API.
fn migration_package(source: &Package, version: u32, policy: &str) -> (Package, Vec<u8>) {
    let mut files = source.files.clone();
    let mut manifest: serde_json::Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    manifest["version"] = serde_json::json!(format!("{version}.0.0"));
    manifest["data_format"] = serde_json::json!({"version": version, "migration_hook": true});
    manifest["api"]["required"]["storage.migration"] = "^1".into();
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    files.insert("migration-policy.txt".into(), policy.as_bytes().to_vec());
    let bytes = super::updates::zip_bytes(&files, 10);
    (Package::from_bytes(&bytes).unwrap(), bytes)
}

/// Real HTTP, SHA verification and the installation actor must preserve guest state on migration or activation failure.
#[test]
#[ignore = "build capability-example with --plugin-package and export target/marketplace-wasm via marketplace tests.export_wasm_fixture"]
fn marketplace_wasm_updates_restore_old_data_after_guest_failures() {
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/marketplace-wasm");
    let source = Package::read(&fixture.join("notes.zip")).expect("export current SDK guest");
    let template = std::fs::read(fixture.join("catalog.json")).unwrap();
    let (old, _) = migration_package(&source, 1, "ok");
    for policy in ["fail", "activate-fail", "ok"] {
        eprintln!("market WASM update {policy}: installing original guest");
        let (candidate, bytes) = migration_package(&source, 2, policy);
        let mut catalog: serde_json::Value = serde_json::from_slice(&template).unwrap();
        let release = &mut catalog["plugins"][0]["versions"][0];
        release["version"] = "2.0.0".into();
        release["manifest"] = serde_json::to_value(&candidate.manifest).unwrap();
        release["sha256"] = candidate.digest.clone().into();
        let server = Server::from_bytes(&serde_json::to_vec(&catalog).unwrap(), bytes);
        let home = tempfile::tempdir().unwrap();
        std::fs::write(home.path().join("source.txt"), "workspace").unwrap();
        let root = home.path().join("runtime");
        let environment = Environment {
            workspace: home.path().display().to_string(),
            ..Default::default()
        };
        let id = &old.manifest.id;
        let mut manager = Manager::open(root.clone(), environment.clone()).unwrap();
        manager
            .install(&old, old.manifest.permissions.clone())
            .unwrap();
        manager
            .invoke_command(id, "scope-write", serde_json::json!({"text": "original"}))
            .unwrap();
        manager.shutdown();
        drop(manager);
        eprintln!("market WASM update {policy}: starting installation worker");
        let worker = Worker::start_background(root.clone(), environment.clone(), true);
        wait(&worker, |state| state.ready);
        worker.refresh_market(
            root.join("marketplace/catalog.json"),
            format!("{}/catalog.json", server.url),
        );
        wait(&worker, |state| state.market.fresh());
        let (reviewed, generation) = {
            let state = worker.state.lock().unwrap();
            (
                state.market.catalog.as_ref().unwrap().plugins[0].versions[0].clone(),
                state.market.generation(),
            )
        };
        assert!(worker.install_market(reviewed, generation, Default::default()));
        eprintln!("market WASM update {policy}: awaiting candidate transaction");
        wait(&worker, |state| state.progress.is_none());
        assert!(
            worker
                .state
                .lock()
                .unwrap()
                .instance_epochs
                .contains_key(id),
            "{policy}: a guest must remain live"
        );
        let (tx, rx) = futures::channel::oneshot::channel();
        worker.tx.send(Work::Shutdown(Some(tx))).unwrap();
        let _ = futures::executor::block_on(rx);
        eprintln!("market WASM update {policy}: reopening committed registry");

        // Reopen through the public manager and observe content through the guest's public command.
        let mut restored = Manager::open(root, environment).unwrap();
        assert_eq!(
            restored.installed[id].manifest.version,
            if policy == "ok" { "2.0.0" } else { "1.0.0" }
        );
        restored
            .invoke_command(id, "scope-read", serde_json::Value::Null)
            .unwrap();
        let plugin_runtime::plugin_protocol::ui::Kind::Text { text } =
            &restored.live[id].views.values().next().unwrap().root.kind
        else {
            panic!("expected the guest's native text view");
        };
        assert_eq!(
            text,
            if policy == "ok" {
                "workspace|v2:original"
            } else {
                "workspace|original"
            },
            "{policy}"
        );
        assert_eq!(
            std::fs::read_to_string(home.path().join("source.txt")).unwrap(),
            "workspace"
        );
        restored.shutdown();
    }
}
