//! Real initialization replies must acknowledge both the control version and declared methods.
use super::*;
use plugin_runtime::{
    DemandReason, LanguageRole, Manager, Package,
    plugin_protocol::{Environment, api::Failure, language::DocumentContext},
};
use std::collections::BTreeMap;

/// Missing, malformed or partial server support fails before any custom request is dispatched.
#[test]
#[ignore = "build cargo build -p plugin-runtime --example lsp_fixture first"]
fn native_control_requires_the_servers_version_and_complete_method_range() {
    for marker in [
        Value::Null,
        json!({"version":2,"methods":["fixture/barrier"]}),
        json!({"version":1,"methods":[]}),
        json!({"version":1,"methods":["fixture/barrier","fixture/barrier"]}),
        json!({"version":1,"methods":["fixture/barrier"],"authority":true}),
    ] {
        let workspace = tempfile::tempdir().unwrap();
        let profile = tempfile::tempdir().unwrap();
        let executable = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("examples/lsp_fixture.exe")
            .canonicalize()
            .expect("build the native fixture in the same Cargo output directory");
        let log = workspace.path().join("wire.jsonl");
        let manifest = json!({
            "id":"unknown-handshake","name":"Unknown handshake","version":"0.1.0","protocol":7,
            "api":{"base":"^1","required":{"language.lsp":"^1.5","language.control":"^1","process":"^1"}},
            "contributions":"plugin.toml","permissions":["process.service.analysis"],"storage_limit":1024,
            "services":{"analysis":{"program":executable,"args":[log]}},
            "language_servers":[{"id":"analysis","language":"novel","service":"analysis",
                "initialization_options":{"fixtureControl":marker},
                "control_methods":{"fixture/barrier":{"parameters":{"type":"null"},"result":{"type":"null"}}}}]
        });
        let package = Package::from_files(BTreeMap::from([
            ("manifest.json".into(), serde_json::to_vec(&manifest).unwrap()),
            ("plugin.toml".into(), b"[plugin]\nid='unknown-handshake'\nname='Unknown handshake'\nversion='0.1.0'\nhost_version='^0.1'\n".to_vec()),
        ])).unwrap();
        let mut manager = Manager::open(
            profile.path().into(),
            Environment {
                workspace: workspace.path().display().to_string(),
                ..Default::default()
            },
        )
        .unwrap();
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
        let token = manager.availability("unknown-handshake").unwrap();
        let document = DocumentContext {
            path: "one.novel".into(),
            generation: token.configuration_generation(),
        };
        let wait = manager
            .begin_demand(
                &token,
                DemandReason::LanguageDocument {
                    provider: "unknown-handshake/analysis".into(),
                    role: LanguageRole::Lsp,
                    document: document.clone(),
                },
                10_000,
            )
            .unwrap();
        let prepared = manager.take_demand_preparation().unwrap().run();
        manager.commit_demand(prepared).unwrap();
        assert!(matches!(
            wait.status(),
            plugin_runtime::RequestUpdate::Completed { result: Ok(_) }
        ));
        let service = manager
            .language_document_service("unknown-handshake/analysis", &document)
            .unwrap()
            .unwrap();
        let server = LanguageServer::from_service(service).unwrap();
        let result = server.control_request(
            "fixture/barrier",
            json!({"arguments":null}),
            &RequestCancellation::default(),
        );
        let error = result.expect_err(&format!("server handshake must reject {marker}"));
        assert_eq!(
            error.downcast_ref::<Failure>().map(|failure| failure.code),
            Some(plugin_runtime::plugin_protocol::api::ErrorCode::CapabilityUnavailable),
            "{error:#}"
        );
        let wire = std::fs::read_to_string(&log).unwrap();
        assert_eq!(
            wire.lines()
                .filter_map(|line| serde_json::from_str::<Value>(line).ok())
                .filter(|frame| frame["method"] == "initialize")
                .count(),
            1,
            "a permanent protocol mismatch must not trigger repeated startup attempts"
        );
        assert!(
            !wire
                .lines()
                .filter_map(|line| serde_json::from_str::<Value>(line).ok())
                .any(|frame| frame["method"] == "fixture/barrier"),
            "custom requests cannot precede a valid ACK"
        );
        manager.disable("unknown-handshake").unwrap();
    }
}
