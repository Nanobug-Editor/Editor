//! Public SDK collection discovery must not reinterpret ordinary single-provider service opens.
use plugin_runtime::{
    Manager, Package,
    plugin_protocol::{Environment, api, settings::Scope},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
#[path = "support/demand.rs"]
mod demand;

/// Independent SDK/WASM input; no broker constructor or fabricated native owner enters this test.
fn package(id: &str, provider: bool, collection: bool, capability: &str) -> Package {
    let component = std::fs::read(
        std::env::var("NANOBUG_SERVICE_SET_COMPONENT")
            .expect("build service-set fixture with the host --plugin-cargo SDK"),
    )
    .unwrap();
    let method = json!({"parameters":{"type":"string","max_bytes":8192},"result":{"type":"string","max_bytes":32768}});
    let declaration = if provider {
        json!({"provides":{"unfamiliar.extension":{"version":"1.0.0","collection":collection,"methods":{"echo":method}}}})
    } else {
        json!({"requires":{"unfamiliar.extension":{"version":"^1","optional":true,"collection":collection,"methods":{"echo":method}}}})
    };
    let manifest = json!({"id":id,"name":id,"version":"0.1.0","protocol":7,"component":"plugin.wasm","storage_limit":1024,
        "api":{"base":"^1","required":{"plugin.services":capability,"plugin.commands":"^1"}},"permissions":["services.call"],
        "commands":[{"id":"probe","title":"Probe public collection contract","signature":method}],"plugin_services":declaration});
    Package::from_files(BTreeMap::from([
        (
            "manifest.json".into(),
            serde_json::to_vec(&manifest).unwrap(),
        ),
        ("plugin.wasm".into(), component),
    ]))
    .unwrap()
}

/// Execute a real public guest request and return its transport result, including expected denials.
fn request(manager: &mut Manager, owner: &str, operation: Value) -> Value {
    if manager.instance_id(owner).is_none() {
        demand::ready(
            manager,
            owner,
            plugin_runtime::DemandReason::Command("probe".into()),
        );
    }
    let pending = manager
        .invoke_typed_command(owner, "probe", json!(operation.to_string()), 30_000)
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        manager.poll();
        match pending.status() {
            api::RequestUpdate::Completed { result } => {
                return serde_json::from_str(result.unwrap().as_str().unwrap()).unwrap();
            }
            api::RequestUpdate::Cancelled { reason, .. } => panic!("Probe cancelled: {reason:?}"),
            _ => assert!(
                std::time::Instant::now() < deadline,
                "Public SDK probe timed out"
            ),
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}
fn service(operation: Value) -> Value {
    json!({"method":"service","operation":operation})
}

/// Red first: the previous public SDK cannot deserialize OpenSet and cannot preserve a collection epoch.
#[test]
#[ignore = "build tests/fixtures/service-set via host --plugin-cargo; set NANOBUG_SERVICE_SET_COMPONENT to its wasm"]
fn public_open_set_requires_opt_in_honors_selection_and_retires_the_complete_old_set() {
    let profile = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(
        profile.path().into(),
        Environment {
            workspace: workspace.path().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    let mut old = package("old-consumer", false, true, "^1.2").files;
    let mut manifest: Value = serde_json::from_slice(&old["manifest.json"]).unwrap();
    manifest["api"]["required"]["plugin.services"] = json!("^1.1");
    old.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    assert!(
        Package::from_files(old).is_err(),
        "collection declaration must require services >=1.2"
    );
    for (id, provider, collection, capability) in [
        ("set-consumer", false, true, "^1.2"),
        ("single-consumer", false, false, "^1.1"),
        ("extension-a", true, true, "^1.2"),
        ("extension-b", true, true, "^1.2"),
    ] {
        let package = package(id, provider, collection, capability);
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
    }
    let open = service(json!({"kind":"open_set","contract":"unfamiliar.extension"}));
    assert_eq!(
        request(&mut manager, "single-consumer", open.clone())["Err"]["code"],
        json!("permission_denied")
    );
    let ordinary = request(
        &mut manager,
        "set-consumer",
        service(json!({"kind":"open","contract":"unfamiliar.extension"})),
    );
    assert_eq!(ordinary["Err"]["code"], json!("conflict"));
    let opened = request(&mut manager, "set-consumer", open.clone());
    let set = &opened["Ok"]["ServiceSet"];
    assert_eq!(set["members"].as_array().unwrap().len(), 2);
    let revision = set["revision"].clone();
    let retained = set["members"][0]["reference"].clone();
    let call = request(
        &mut manager,
        "set-consumer",
        service(
            json!({"kind":"call","reference":retained,"method":"echo","arguments":"first activation","timeout_ms":30_000}),
        ),
    );
    assert!(
        call["Ok"] == "Unit",
        "a member must be a usable public reference: {call}"
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while manager.instance_id("extension-a").is_none() {
        manager.poll();
        assert!(
            std::time::Instant::now() < deadline,
            "member must activate through its actual service demand"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let current = request(
        &mut manager,
        "set-consumer",
        service(
            json!({"kind":"validate_set","contract":"unfamiliar.extension","revision":revision}),
        ),
    );
    assert_eq!(
        current["Ok"], "Unit",
        "first activation must preserve the discovered epoch"
    );
    manager
        .set_service_provider(
            api::InstanceScope::Workspace,
            Scope::User,
            "unfamiliar.extension",
            Some("extension-b"),
        )
        .unwrap();
    let selected = request(&mut manager, "set-consumer", open.clone());
    assert_eq!(
        selected["Ok"]["ServiceSet"]["members"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        selected["Ok"]["ServiceSet"]["members"][0]["provider"],
        "extension-b"
    );
    let stale = request(
        &mut manager,
        "set-consumer",
        service(
            json!({"kind":"validate_set","contract":"unfamiliar.extension","revision":revision}),
        ),
    );
    assert_eq!(stale["Err"]["code"], "invalid_handle");
    let stale = request(
        &mut manager,
        "set-consumer",
        service(
            json!({"kind":"call","reference":retained,"method":"echo","arguments":"late","timeout_ms":1000}),
        ),
    );
    assert_eq!(stale["Err"]["code"], "invalid_handle");
    manager.disable("extension-b").unwrap();
    assert_eq!(
        request(&mut manager, "set-consumer", open)["Err"]["code"],
        "capability_unavailable"
    );
    manager.shutdown();
    assert_eq!(manager.resource_count(), 0);
}

/// A collection reference used for code lending must keep its selection epoch after RPC completion.
/// Before the authority fix, both the old lease and its already loaded process survived this change.
#[test]
#[ignore = "build service-set SDK fixture and plugin-runtime lsp_fixture example; set NANOBUG_SERVICE_SET_COMPONENT"]
fn public_collection_selection_retires_borrowed_native_code_without_stopping_exporter() {
    native_collection_case(2, true);
}

/// Empty discovery still owns a composition epoch: adding the first contributor retires primary code.
#[test]
#[ignore = "build current service-set release SDK fixture and lsp_fixture; set NANOBUG_SERVICE_SET_COMPONENT"]
fn public_empty_collection_retires_primary_module_when_first_member_is_installed() {
    native_collection_case(0, false);
}

/// A live peer returning no contributed modules must still be a dependency of the primary worker.
#[test]
#[ignore = "build current service-set release SDK fixture and lsp_fixture; set NANOBUG_SERVICE_SET_COMPONENT"]
fn public_declined_contribution_retires_primary_module_when_membership_changes() {
    native_collection_case(1, false);
}

/// Shared public setup only; each case starts actual packages, captures native metadata and spawns
/// the managed stdio fixture. No lease, authority, broker or service object is fabricated by a test.
fn native_collection_case(member_count: usize, borrow_member: bool) {
    use plugin_runtime::{DemandReason, LanguageRole, plugin_protocol::language::DocumentContext};
    use sha2::{Digest, Sha256};
    let native = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/debug/examples/lsp_fixture.exe")
        .canonicalize()
        .unwrap();
    let profile = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    std::fs::write(workspace.path().join("source.novel"), "unchanged").unwrap();
    let mut manager = Manager::open(
        profile.path().into(),
        Environment {
            workspace: workspace.path().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    let native_package = |id: &str, provider: bool| {
        let mut files = package(id, provider, true, "^1.2").files;
        let mut manifest: Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
        manifest["api"]["required"]["language.modules"] = json!("^1");
        manifest["permissions"] = json!([
            "native.modules",
            "editor.read",
            "workspace.read",
            "services.call"
        ]);
        manifest["commands"][0]["signature"]["permissions"] = json!([
            "native.modules",
            "editor.read",
            "workspace.read",
            "services.call"
        ]);
        {
            let code = b"module.exports = () => ({create(info) {return info.languageService;}});";
            let hash = format!("{:x}", Sha256::digest(code));
            manifest["module_exports"] = json!({"extension":{
                "root":"modules/extension","entry":"index.cjs","sha256":hash,
                "files":{"index.cjs":hash},"abi":"unfamiliar.extension/1","engine":{"novel":"=1.0.0"}
            }});
            files.insert("modules/extension/index.cjs".into(), code.to_vec());
        }
        if !provider {
            manifest["api"]["required"]["language.control"] = json!("^1");
            manifest["api"]["required"]["language.lsp"] = json!("^1.5");
            manifest["api"]["required"]["process"] = json!("^1");
            manifest["permissions"]
                .as_array_mut()
                .unwrap()
                .push(json!("process.service.analysis"));
            manifest["services"] = json!({"analysis":{"program":native,"args":[]}});
            manifest["language_servers"] = json!([{"id":"analysis","language":"unfamiliar","service":"analysis",
                "control_methods":{"novel/attach":{"parameters":{"type":"null"},"result":{"type":"null"},
                    "permissions":["native.modules","editor.read","workspace.read"]}}}]);
        }
        files.insert(
            "manifest.json".into(),
            serde_json::to_vec(&manifest).unwrap(),
        );
        Package::from_files(files).unwrap()
    };
    for (id, provider) in [
        ("set-engine", false),
        ("extension-a", true),
        ("extension-b", true),
    ]
    .into_iter()
    .take(member_count + 1)
    {
        let package = native_package(id, provider);
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
    }
    let document = DocumentContext {
        path: "source.novel".into(),
        generation: manager
            .availability("set-engine")
            .unwrap()
            .configuration_generation(),
    };
    demand::ready(
        &mut manager,
        "set-engine",
        DemandReason::LanguageDocument {
            provider: "set-engine/analysis".into(),
            role: LanguageRole::Lsp,
            document: document.clone(),
        },
    );
    let server = manager
        .language_document_service("set-engine/analysis", &document)
        .unwrap()
        .unwrap();
    let opened = request(
        &mut manager,
        "set-engine",
        service(json!({"kind":"open_set","contract":"unfamiliar.extension"})),
    );
    let reference = opened["Ok"]["ServiceSet"]["members"][0]["reference"].clone();
    assert_eq!(
        opened["Ok"]["ServiceSet"]["members"]
            .as_array()
            .unwrap()
            .len(),
        member_count
    );
    if member_count > 0 {
        let call = request(
            &mut manager,
            "set-engine",
            service(
                json!({"kind":"call","reference":reference,"method":"echo","arguments":"[]","timeout_ms":30_000}),
            ),
        );
        assert_eq!(call["Ok"], "Unit", "{call}");
        demand::ready(
            &mut manager,
            "extension-a",
            DemandReason::Command("probe".into()),
        );
    }
    let exporter = manager.instance_id("extension-a").map(str::to_owned);
    let mut native_request = json!({"provider":"analysis","document":{"path":"source.novel","generation":0},
        "method":"novel/attach","arguments":null,"imports":[{"export":"extension","reference":if borrow_member {reference.clone()} else {Value::Null}}]});
    if !borrow_member {
        native_request["service_sets"] = json!([{"contract":"unfamiliar.extension","revision":opened["Ok"]["ServiceSet"]["revision"]}]);
    }
    let accepted = request(
        &mut manager,
        "set-engine",
        json!({"method":"editor","timeout_ms":30_000,
        "operation":{"kind":"language_request","request":native_request}}),
    );
    assert!(accepted["Ok"]["Accepted"].is_object(), "{accepted}");
    let requests = manager.take_editor_requests();
    assert_eq!(requests.len(), 1);
    let (_, control) = requests.into_iter().next().unwrap();
    assert!(control.begin());
    let lease = control.language_control_metadata().unwrap().imports[0].clone();
    lease.attach(&server).unwrap();
    let child = server.spawn().unwrap();
    assert_eq!(server.process_count(), 1);
    control.finish(Ok(api::EditorValue::Language(Value::Null)));
    let pending_metadata = if !borrow_member {
        native_request["imports"] = json!([]);
        let accepted = request(
            &mut manager,
            "set-engine",
            json!({"method":"editor","timeout_ms":30_000,
            "operation":{"kind":"language_request","request":native_request}}),
        );
        assert!(accepted["Ok"]["Accepted"].is_object(), "{accepted}");
        let (_, pending) = manager.take_editor_requests().pop().unwrap();
        assert!(pending.begin());
        Some(pending)
    } else {
        None
    };
    if borrow_member {
        manager
            .set_service_provider(
                api::InstanceScope::Workspace,
                Scope::User,
                "unfamiliar.extension",
                Some("extension-b"),
            )
            .unwrap();
    } else {
        let package = native_package(
            if member_count == 0 {
                "extension-a"
            } else {
                "extension-b"
            },
            true,
        );
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
    }
    manager.poll();
    if let Some(pending) = pending_metadata {
        pending.finish(Ok(api::EditorValue::Language(Value::Null)));
        assert!(
            matches!(
                pending.status(),
                api::RequestUpdate::Cancelled { .. }
                    | api::RequestUpdate::Completed { result: Err(_) }
            ),
            "late metadata cannot outlive empty/non-contributing membership"
        );
    }
    if let Some(exporter) = exporter {
        assert_eq!(
            manager.instance_id("extension-a"),
            Some(exporter.as_str()),
            "selection does not disable the publisher"
        );
    }
    assert!(
        !lease.is_live(),
        "a module lease must inherit the collection epoch"
    );
    assert!(
        lease.attach(&server).is_err(),
        "an old lease must not authorize another load"
    );
    assert!(
        !server.is_active(),
        "loaded code must retire with its composition"
    );
    assert_eq!(server.process_count(), 0, "retirement reaps the process");
    drop(child);
    manager.shutdown();
}
