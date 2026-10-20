//! Renewed native source intents cannot inherit an abandoned deferred guest's old control result.
use super::*;
use plugin_runtime::{
    DemandReason, EditorRequest, LanguageRole, Manager,
    plugin_protocol::{Environment, api, language::DocumentContext},
};
use std::time::{Duration, Instant};

/// The source's first real native control is in flight when its original demand is cancelled.
/// A fresh demand for the same configuration must run independently of the stale control result.
/// Manager owns no text model: actual dirty EditorState synchronization is covered by native UI tests.
#[test]
#[ignore = "build fixtures/language-delegate through the current host --plugin-package; set NANOBUG_LANGUAGE_DELEGATE_PACKAGE"]
fn renewed_source_does_not_join_an_abandoned_deferred_control() {
    let fixture = std::env::var_os("NANOBUG_LANGUAGE_DELEGATE_PACKAGE")
        .map(std::path::PathBuf::from)
        .expect("select the independently built current SDK fixture");
    let compiled = Package::read(&fixture).unwrap();
    let (engine, source) = packages(compiled.files["plugin.wasm"].clone());
    let workspace = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    std::fs::create_dir(workspace.path().join("first")).unwrap();
    std::fs::write(workspace.path().join("first/source.novel"), "original").unwrap();
    let mut manager = Manager::open(
        profile.path().into(),
        Environment {
            workspace: workspace.path().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    for package in [&engine, &source] {
        manager
            .install(package, package.manifest.permissions.clone())
            .unwrap();
    }
    let availability = manager.availability("renewed-source").unwrap();
    let document = DocumentContext {
        path: "first/source.novel".into(),
        generation: availability.configuration_generation(),
    };
    let reason = || DemandReason::LanguageDocument {
        provider: "renewed-source/analysis".into(),
        role: LanguageRole::Lsp,
        document: document.clone(),
    };
    let original = manager
        .begin_demand(&availability, reason(), 120_000)
        .unwrap();
    let abandoned = wait_native(&mut manager);
    original.cancel().unwrap();
    let renewed = manager
        .begin_demand(&availability, reason(), 120_000)
        .unwrap();
    abandoned.finish(Err(api::Failure::new(
        api::ErrorCode::StaleRevision,
        "Original source changed during the abandoned control",
    )));

    let deadline = Instant::now() + Duration::from_secs(120);
    let fresh = loop {
        manager.poll();
        if let Some((_, request)) = manager.take_editor_requests().pop() {
            assert!(request.begin());
            break request;
        }
        assert!(
            !renewed.status().is_terminal(),
            "the renewed source inherited an abandoned control result: {:?}",
            renewed.status()
        );
        assert!(
            Instant::now() < deadline,
            "renewed preparation never reached native dispatch"
        );
        std::thread::sleep(Duration::from_millis(5));
    };
    let token = manager.availability("renewed-engine").unwrap();
    let engine_document = DocumentContext {
        generation: token.configuration_generation(),
        ..document.clone()
    };
    let preparing = manager
        .begin_demand(
            &token,
            DemandReason::LanguageDocument {
                provider: "renewed-engine/analysis".into(),
                role: LanguageRole::Lsp,
                document: engine_document.clone(),
            },
            120_000,
        )
        .unwrap();
    loop {
        manager.poll();
        if let api::RequestUpdate::Completed { result } = preparing.status() {
            result.unwrap();
            break;
        }
        assert!(
            Instant::now() < deadline,
            "renewed engine preparation stalled"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    let service = manager
        .language_document_service("renewed-engine/analysis", &engine_document)
        .unwrap()
        .unwrap();
    fresh.language_control_metadata().unwrap().imports[0]
        .attach(&service)
        .unwrap();
    fresh.finish_language_binding(Value::Null, service.clone());
    loop {
        manager.poll();
        if let api::RequestUpdate::Completed { result } = renewed.status() {
            let receipt = result.unwrap();
            manager.admit_receipt(&availability, &receipt).unwrap();
            break;
        }
        assert!(
            Instant::now() < deadline,
            "renewed binding did not complete"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(matches!(
        original.status(),
        api::RequestUpdate::Cancelled { .. }
    ));
    let borrowed = manager
        .language_document_service("renewed-source/analysis", &document)
        .unwrap()
        .unwrap();
    assert!(std::sync::Arc::ptr_eq(&borrowed, &service));
    assert!(borrowed.is_active());
    // This fixed deferred guest has no Snapshot implementation. Explicit public removal verifies
    // source/module authority retirement before checked empty-scope shutdown; it is not a state roundtrip.
    manager.disable("renewed-source").unwrap();
    assert!(!borrowed.is_active());
    assert_eq!(borrowed.process_count(), 0);
    assert!(manager.instance_id("renewed-source").is_none());
    manager.disable("renewed-engine").unwrap();
    assert!(!service.is_active());
    assert_eq!(service.process_count(), 0);
    assert!(manager.instance_id("renewed-engine").is_none());
    manager.shutdown_checked().unwrap();
}

/// Wait for a real guest-issued host request, never injecting a reply or calling a private broker.
fn wait_native(manager: &mut Manager) -> EditorRequest {
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        manager.poll();
        if let Some((_, request)) = manager.take_editor_requests().pop() {
            assert!(request.begin());
            assert!(matches!(
                request.operation(),
                api::EditorOperation::LanguageRequest { .. }
            ));
            return request;
        }
        assert!(
            Instant::now() < deadline,
            "first deferred control did not arrive"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Both unknown-language packages use the actual SDK guest and ordinary checked module contracts.
/// The native plan is not spawned here: this regression isolates deferred authority/result routing.
fn packages(wasm: Vec<u8>) -> (Package, Package) {
    let method = plugin_runtime::plugin_protocol::language_control::binding_prepare_method();
    let (mut source, mut files) = input();
    source["id"] = json!("renewed-source");
    source["component"] = json!("plugin.wasm");
    source["api"]["required"] =
        json!({"language.modules":"^1","language.lsp":"^1.5","plugin.services":"^1.1"});
    source["permissions"] = json!([
        "native.modules",
        "editor.read",
        "workspace.read",
        "services.call"
    ]);
    source["plugin_services"] = json!({
        "provides":{"novel.prepare":{"version":"1.0.0","methods":{"prepare":method}}},
        "requires":{"novel.engine":{"version":"^1","methods":{"attach":method}}}
    });
    source["language_servers"] = json!([{"id":"analysis","language":"unfamiliar","service":"",
        "delegate":{"contract":"novel.prepare","method":"prepare"}}]);
    files.insert("plugin.wasm".into(), wasm.clone());
    let source = package(&source, &files).unwrap();
    let engine = json!({
        "id":"renewed-engine","name":"Renewed engine","version":"0.1.0","protocol":7,
        "component":"plugin.wasm","storage_limit":1024,
        "api":{"base":"^1","required":{"language.modules":"^1","language.control":"^1","language.lsp":"^1.5","plugin.services":"^1.1","process":"^1"}},
        "permissions":["native.modules","editor.read","workspace.read","process.service.analysis"],
        "services":{"analysis":{"program":std::env::current_exe().unwrap(),"args":[]}},
        "plugin_services":{"provides":{"novel.engine":{"version":"1.0.0","methods":{"attach":method}}}},
        "language_servers":[{"id":"analysis","language":"unfamiliar-engine","service":"analysis","hook":true,
            "control_methods":{"novel/attach":{"parameters":{"type":"null"},"result":{"type":"null"},
                "permissions":["native.modules","editor.read","workspace.read"]}}}]
    });
    let engine = package(&engine, &BTreeMap::from([("plugin.wasm".into(), wasm)])).unwrap();
    (engine, source)
}
