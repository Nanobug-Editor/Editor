//! A real SDK consumer cannot obtain another instance's roots or borrow absent native grants.
use plugin_runtime::{
    Manager, Package,
    plugin_protocol::{Environment, ui},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// An opted-in readonly selector can choose only a unique declared subset, without creating effects.
#[test]
#[ignore = "Build fixtures/native-context with --plugin-cargo and set NANOBUG_NATIVE_CONTEXT_COMPONENT"]
fn readonly_native_selection_refuses_foreign_duplicate_process_and_ui_effects() {
    use plugin_runtime::plugin_protocol::api::RequestUpdate;
    let component =
        std::fs::read(std::env::var_os("NANOBUG_NATIVE_CONTEXT_COMPONENT").unwrap()).unwrap();
    let root = tempfile::tempdir().unwrap();
    let mut declaration = plugin_runtime::plugin_protocol::targets::declaration();
    declaration
        .preparation
        .insert("prepare".into(), vec!["native".into()]);
    let manifest = json!({"id":"preparation-boundary","name":"Preparation boundary","version":"1.0.0","protocol":7,"component":"fixture.wasm","storage_limit":1024,"service_preparation_hook":true,
        "api":{"base":"^1","required":{"process":">=1.9,<2","dependencies":">=1.4,<2","plugin.services":">=1.1,<2","ui.native":"^1","ui.panels":"^1"}},
        "permissions":["workspace.read","process.exec","process.service.native","dependencies.prepare","ui.panels"],"services":{"native":{"program":"deliberately-absent-native-context-fixture","on_demand":true,"installation":{
            "executable":"inert/tool.exe","artifacts":[{"id":"inert","version":"1","platform":format!("{}-{}",std::env::consts::OS,std::env::consts::ARCH),"sha256":format!("{:x}",Sha256::digest(b"never acquired")),"source":{"kind":"package","path":"inert.bin"},"format":{"kind":"file","path":"tool.exe"}}]
        }}},
        "plugin_services":{"provides":{"run.targets":declaration}},"panels":[{"id":"result","title":"Result","position":"right"}]
    });
    let package = Package::from_files(BTreeMap::from([
        (
            "manifest.json".into(),
            serde_json::to_vec(&manifest).unwrap(),
        ),
        ("fixture.wasm".into(), component),
        ("inert.bin".into(), b"never acquired".to_vec()),
    ]))
    .unwrap();
    let mut manager = Manager::open(
        root.path().join("profile"),
        Environment {
            workspace: root.path().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    for mode in [
        "valid",
        "foreign",
        "duplicate",
        "execute",
        "native-context",
        "ui",
    ] {
        let request = manager
            .begin_target_call(
                "preparation-boundary",
                "prepare",
                json!({"workspace":root.path().display().to_string(),"binding":mode,"env":[]}),
            )
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
        let result = loop {
            manager.poll();
            if let RequestUpdate::Completed { result } = request.status() {
                break result;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "Readonly selector timed out: {mode}"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        assert_eq!(result.is_ok(), mode == "valid", "{mode}: {result:?}");
        assert_eq!(manager.live["preparation-boundary"].process_count(), 0);
        assert!(
            manager.live["preparation-boundary"].views.is_empty(),
            "Preparation must not publish UI"
        );
    }
    manager.set_workspace_trust(false).unwrap();
    assert!(
        manager
            .begin_target_call(
                "preparation-boundary",
                "prepare",
                json!({"workspace":root.path().display().to_string(),"binding":"valid","env":[]})
            )
            .is_err()
    );
    assert_eq!(manager.resource_count(), 0);
}
#[path = "support/demand.rs"]
mod demand_support;

/// Paths remain descriptive metadata: missing grants fail and successful inspection creates no job.
#[test]
#[ignore = "Build fixtures/native-context with --plugin-cargo and set NANOBUG_NATIVE_CONTEXT_COMPONENT"]
fn native_service_context_is_owned_granted_and_side_effect_free() {
    let component =
        std::fs::read(std::env::var_os("NANOBUG_NATIVE_CONTEXT_COMPONENT").unwrap()).unwrap();
    let root = tempfile::tempdir().unwrap();
    for (id, grants, permitted) in [
        (
            "context-a",
            vec!["process.exec", "process.service.native", "ui.panels"],
            true,
        ),
        (
            "context-b",
            vec!["process.exec", "process.service.native", "ui.panels"],
            true,
        ),
        (
            "context-no-exec",
            vec!["process.service.native", "ui.panels"],
            false,
        ),
        (
            "context-no-service",
            vec!["process.exec", "ui.panels"],
            false,
        ),
    ] {
        let manifest = json!({"id":id,"name":id,"version":"1.0.0","protocol":7,"component":"fixture.wasm","storage_limit":1024,
            "api":{"base":"^1","required":{"process":">=1.9,<2","dependencies":">=1.2,<2","ui.native":"^1","ui.panels":"^1"}},
            "permissions":grants,"services":{"native":{"program":"deliberately-absent-native-context-fixture"}},
            "commands":[{"id":"inspect","title":"Inspect owned native context"}],"panels":[{"id":"result","title":"Result","position":"right"}]
        });
        let package = Package::from_files(BTreeMap::from([
            (
                "manifest.json".into(),
                serde_json::to_vec(&manifest).unwrap(),
            ),
            ("fixture.wasm".into(), component.clone()),
        ]))
        .unwrap();
        let profile = root.path().join(id);
        let mut manager = Manager::open(
            profile.clone(),
            Environment {
                workspace: root.path().display().to_string(),
                ..Default::default()
            },
        )
        .unwrap();
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
        demand_support::ready(
            &mut manager,
            id,
            plugin_runtime::DemandReason::Command("inspect".into()),
        );
        manager.invoke_command(id, "inspect", Value::Null).unwrap();
        let document = manager.live[id].views.values().next().unwrap();
        let ui::Kind::Text { text } = &document.root.kind else {
            panic!("Expected receipt");
        };
        let receipt: Value = serde_json::from_str(text).unwrap();
        if permitted {
            for field in ["assets", "data"] {
                let path = std::path::Path::new(receipt["Ok"][field].as_str().unwrap());
                assert!(
                    path.is_dir() && path.starts_with(profile.canonicalize().unwrap()),
                    "{receipt}"
                );
            }
        } else {
            assert_eq!(receipt["Err"]["code"], "permission_denied", "{receipt}");
        }
        assert_eq!(manager.live[id].process_count(), 0);
        manager
            .invoke_command(id, "inspect", json!("foreign"))
            .unwrap();
        let document = manager.live[id].views.values().next().unwrap();
        let ui::Kind::Text { text } = &document.root.kind else {
            panic!("Expected refusal");
        };
        assert!(text.contains("permission_denied"), "{text}");
        manager.disable(id).unwrap();
        assert!(manager.invoke_command(id, "inspect", Value::Null).is_err());
        assert_eq!(manager.resource_count(), 0);
    }
}
