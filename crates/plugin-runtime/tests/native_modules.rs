//! Native adapters enter through ordinary package admission, with an exact immutable file closure.
use plugin_runtime::Package;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
#[path = "native_modules/consumers.rs"]
mod consumers;

/// A module declaration grants neither private-directory sharing nor a host-loaded native plugin.
#[test]
fn admits_a_closed_module_and_rejects_changed_or_unlisted_files() {
    let (mut manifest, mut files) = input();
    package(&manifest, &files).unwrap();
    files.insert(
        "modules/adapter/hidden.cjs".into(),
        b"module.exports = {}".to_vec(),
    );
    assert!(
        package(&manifest, &files).is_err(),
        "unlisted runtime neighbors are not a sealed closure"
    );
    files.remove("modules/adapter/hidden.cjs");
    files.insert("modules/adapter/index.cjs".into(), b"changed".to_vec());
    assert!(
        package(&manifest, &files).is_err(),
        "entry bytes must match the approved digest"
    );
    manifest["module_exports"]["adapter"]["root"] = json!("../outside");
    assert!(package(&manifest, &files).is_err());
}

/// Exporting executable bytes requires a dedicated capability and an explicit installation grant.
#[test]
fn module_exports_cannot_borrow_asset_read_or_process_permissions() {
    let (mut manifest, files) = input();
    manifest["permissions"] = json!(["assets.read"]);
    assert!(package(&manifest, &files).is_err());
    manifest["permissions"] = json!(["native.modules"]);
    manifest["api"]["required"] = json!({});
    assert!(package(&manifest, &files).is_err());
}

/// Build a minimal independent resource package; no fake WASM store or test-only host API is used.
fn input() -> (Value, BTreeMap<String, Vec<u8>>) {
    let code = b"module.exports = { init() {} };";
    let hash = format!("{:x}", Sha256::digest(code));
    let manifest = json!({
        "id":"novel-adapter", "name":"Novel adapter", "version":"0.1.0", "protocol":7,
        "api":{"base":"^1", "required":{"language.modules":"^1"}},
        "permissions":["native.modules"], "storage_limit":1024,
        "module_exports":{"adapter":{
            "root":"modules/adapter", "entry":"index.cjs", "sha256":hash,
            "files":{"index.cjs":hash}, "abi":"novel.engine/1", "engine":{"novel":"=1.0.0"}
        }}
    });
    (
        manifest,
        BTreeMap::from([("modules/adapter/index.cjs".into(), code.to_vec())]),
    )
}

fn package(manifest: &Value, files: &BTreeMap<String, Vec<u8>>) -> anyhow::Result<Package> {
    let mut files = files.clone();
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(manifest).unwrap(),
    );
    Package::from_files(files)
}

/// The compiled guest composes source -> engine -> native host, then borrows the same live plan.
/// Build the SDK fixture and native stdio example before explicitly running this ignored acceptance.
#[test]
#[ignore = "build fixtures/language-delegate through host --plugin-package and plugin-runtime example lsp_fixture first"]
fn independent_guest_delegates_a_native_plan_and_retires_loaded_modules() {
    use plugin_runtime::{
        DemandReason, LanguageRole, Manager,
        plugin_protocol::{Environment, api, language::DocumentContext},
    };
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };
    // A current host SDK build may be supplied without overwriting the retained historical input.
    // Both paths still enter the same public Package verifier and use the guest's declared contracts.
    let fixture = std::env::var_os("NANOBUG_LANGUAGE_DELEGATE_PACKAGE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/language-delegate/language-delegate-fixture-0.1.0.zip")
        });
    let compiled = Package::read(&fixture).expect("Build the independent SDK fixture first");
    let wasm = compiled.files["plugin.wasm"].clone();
    let workspace = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    let native = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/debug/examples/lsp_fixture.exe")
        .canonicalize()
        .expect("Build the native stdio example first");
    let method = plugin_runtime::plugin_protocol::language_control::binding_prepare_method();
    let (source_manifest, mut source_files) = input();
    let mut source = source_manifest;
    source["id"] = json!("module-source");
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
    source_files.insert("plugin.wasm".into(), wasm.clone());
    let source = package(&source, &source_files).unwrap();
    let engine = json!({
        "id":"module-engine","name":"Unfamiliar engine","version":"0.1.0","protocol":7,
        "component":"plugin.wasm","storage_limit":1024,
        "api":{"base":"^1","required":{"language.modules":"^1","language.control":"^1","language.lsp":"^1.5","plugin.services":"^1.1","process":"^1"}},
        "permissions":["native.modules","editor.read","workspace.read","process.service.analysis"],
        // The fixture requires a host-owned log path to stay alive. An empty argv could make a
        // retained process lease look healthy even though the child exited before these checks.
        "services":{"analysis":{"program":native,"args":[profile.path().join("module-wire.jsonl")]}},
        "plugin_services":{"provides":{"novel.engine":{"version":"1.0.0","methods":{"attach":method}}}},
        "language_servers":[{"id":"analysis","language":"unfamiliar-engine","service":"analysis","hook":true,
            "control_methods":{"novel/attach":{"parameters":{"type":"null"},"result":{"type":"null"},
                "permissions":["native.modules","editor.read","workspace.read"]}}}]
    });
    let engine = package(&engine, &BTreeMap::from([("plugin.wasm".into(), wasm)])).unwrap();
    for root in ["first", "second"] {
        std::fs::create_dir(workspace.path().join(root)).unwrap();
        std::fs::write(
            workspace.path().join(root).join("source.novel"),
            "original source",
        )
        .unwrap();
    }
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
    let availability = manager.availability("module-source").unwrap();
    let document = DocumentContext {
        path: "first/source.novel".into(),
        generation: availability.configuration_generation(),
    };
    let demand = manager
        .begin_demand(
            &availability,
            DemandReason::LanguageDocument {
                provider: "module-source/analysis".into(),
                role: LanguageRole::Lsp,
                document: document.clone(),
            },
            120_000,
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(180);
    let mut transport = None;
    let mut child = None;
    let admitted = loop {
        manager.poll();
        for (_, request) in manager.take_editor_requests() {
            assert!(request.begin());
            let api::EditorOperation::LanguageRequest { request: control } = request.operation()
            else {
                panic!("Only a native language operation is expected");
            };
            let metadata = request.language_control_metadata().unwrap();
            assert_ne!(
                metadata.consumer, metadata.borrower,
                "The original host consumer is distinct from the immediate source plugin"
            );
            assert_eq!(metadata.imports.len(), 1);
            assert_eq!(metadata.imports[0].descriptor().abi, "novel.engine/1");
            assert_eq!(metadata.binding_document.as_ref(), Some(&document));
            assert_eq!(control.document.generation, 0);
            let original = manager.availability("module-engine").unwrap();
            let native_document = DocumentContext {
                path: document.path.clone(),
                generation: original.configuration_generation(),
            };
            let ready = manager
                .begin_demand(
                    &original,
                    DemandReason::LanguageDocument {
                        provider: "module-engine/analysis".into(),
                        role: LanguageRole::Lsp,
                        document: native_document.clone(),
                    },
                    120_000,
                )
                .unwrap();
            loop {
                manager.poll();
                match ready.status() {
                    api::RequestUpdate::Completed { result } => {
                        result.unwrap();
                        break;
                    }
                    api::RequestUpdate::Cancelled { reason, .. } => {
                        panic!("Engine preparation canceled: {reason:?}")
                    }
                    _ => {}
                }
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(5));
            }
            let service = manager
                .language_document_service("module-engine/analysis", &native_document)
                .unwrap()
                .unwrap();
            metadata.imports[0].attach(&service).unwrap();
            let process = service.spawn().unwrap();
            assert_eq!(service.process_count(), 1);
            child = Some(process);
            request.finish_language_binding(Value::Null, service.clone());
            let status = request.status();
            assert!(
                matches!(
                    status,
                    api::RequestUpdate::Completed {
                        result: Ok(api::EditorValue::LanguageBinding { .. })
                    }
                ),
                "Native attachment must issue a binding: {status:?}"
            );
            transport = Some(service);
        }
        match demand.status() {
            api::RequestUpdate::Completed { result } => break result.unwrap(),
            api::RequestUpdate::Cancelled { reason, .. } => {
                panic!("Delegation canceled: {reason:?}")
            }
            _ => {}
        }
        assert!(
            Instant::now() < deadline,
            "Deferred public preparation must terminate"
        );
        std::thread::sleep(Duration::from_millis(5));
    };
    manager.admit_receipt(&availability, &admitted).unwrap();
    let borrowed = manager
        .language_document_service("module-source/analysis", &document)
        .unwrap()
        .unwrap();
    let transport = transport.unwrap();
    assert!(
        Arc::ptr_eq(&borrowed, &transport),
        "A delegated document must reuse the existing engine plan"
    );
    assert_eq!(
        std::fs::read_to_string(workspace.path().join("first/source.novel")).unwrap(),
        "original source"
    );
    let engine_availability = manager.availability("module-engine").unwrap();
    let sibling = DocumentContext {
        path: "second/source.novel".into(),
        generation: engine_availability.configuration_generation(),
    };
    demand_ready(&mut manager, &sibling);
    let sibling_plan = manager
        .language_document_service("module-engine/analysis", &sibling)
        .unwrap()
        .unwrap();
    assert!(!Arc::ptr_eq(&sibling_plan, &transport));
    // A matching provider name is insufficient: both attachment and binding issuance must reject
    // the other project's exact plan, even though the same live guest owns both native contexts.
    let rejected_document = DocumentContext {
        path: "first/rejected.novel".into(),
        generation: availability.configuration_generation(),
    };
    std::fs::write(workspace.path().join(&rejected_document.path), "untouched").unwrap();
    let rejected = manager
        .begin_demand(
            &availability,
            DemandReason::LanguageDocument {
                provider: "module-source/analysis".into(),
                role: LanguageRole::Lsp,
                document: rejected_document.clone(),
            },
            120_000,
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut observed_rejection = false;
    loop {
        manager.poll();
        for (_, request) in manager.take_editor_requests() {
            assert!(request.begin());
            let metadata = request.language_control_metadata().unwrap();
            assert!(metadata.imports[0].attach(&sibling_plan).is_err());
            request.finish_language_binding(Value::Null, sibling_plan.clone());
            assert!(matches!(
                request.status(),
                api::RequestUpdate::Completed {
                    result: Err(api::Failure {
                        code: api::ErrorCode::InvalidHandle,
                        ..
                    })
                }
            ));
            observed_rejection = true;
        }
        if let api::RequestUpdate::Completed { result } = rejected.status() {
            assert!(result.is_err());
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        observed_rejection,
        "wrong-context result must reach the public completion gate"
    );
    assert!(transport.is_active() && sibling_plan.is_active());
    manager.disable("module-source").unwrap();
    assert!(
        !transport.is_active(),
        "Unloading the source invalidates native code already loaded into the engine"
    );
    assert_eq!(
        transport.process_count(),
        0,
        "Retirement reaps the managed process before releasing executable bytes"
    );
    assert!(
        manager
            .language_document_service("module-source/analysis", &document)
            .is_none()
    );
    assert!(
        sibling_plan.is_active(),
        "An unrelated project keeps its active native plan"
    );
    let lender_document = DocumentContext {
        path: document.path.clone(),
        generation: engine_availability.configuration_generation(),
    };
    demand_ready(&mut manager, &lender_document);
    let recovered = manager
        .language_document_service("module-engine/analysis", &lender_document)
        .unwrap()
        .unwrap();
    assert!(
        recovered.is_active(),
        "The still-installed engine can recover after borrowed code was retired"
    );
    assert!(!Arc::ptr_eq(&transport, &recovered));
    assert!(Arc::ptr_eq(
        &sibling_plan,
        &manager
            .language_document_service("module-engine/analysis", &sibling)
            .unwrap()
            .unwrap()
    ));
    drop(child);
}

/// Drive the public preparation queue without touching a WASM store or private broker state.
fn demand_ready(
    manager: &mut plugin_runtime::Manager,
    document: &plugin_runtime::plugin_protocol::language::DocumentContext,
) {
    let token = manager.availability("module-engine").unwrap();
    let demand = manager
        .begin_demand(
            &token,
            plugin_runtime::DemandReason::LanguageDocument {
                provider: "module-engine/analysis".into(),
                role: plugin_runtime::LanguageRole::Lsp,
                document: document.clone(),
            },
            120_000,
        )
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    loop {
        manager.poll();
        match demand.status() {
            plugin_runtime::RequestUpdate::Completed { result } => {
                result.unwrap();
                return;
            }
            plugin_runtime::RequestUpdate::Cancelled { reason, .. } => {
                panic!("Native demand canceled: {reason:?}")
            }
            _ => {}
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}
