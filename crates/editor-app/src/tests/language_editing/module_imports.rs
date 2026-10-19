//! Failed module imports must not lend a rejected exporter lifetime over an existing native peer.
use super::*;
use protocol::{api, language::DocumentContext, language_control::Import, service, ui::Kind};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::PathBuf, time::Instant};
mod error_budget;
mod uncertain;

/// Two independently built unknown guests enter the real dispatcher and framed connection. The
/// fixture reports released imports, without claiming that arbitrary external effects roll back.
#[gpui::test]
#[ignore = "set current SDK NANOBUG_CONTROL_FIXTURE_PACKAGE and NANOBUG_NATIVE_CONSUMERS_PACKAGE; build lsp_fixture"]
fn released_native_imports_do_not_retire_an_existing_healthy_context(cx: &mut TestAppContext) {
    let workspace = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    let path = workspace.path().join("source.txt");
    std::fs::write(&path, "saved source\n").unwrap();
    let path = path.canonicalize().unwrap();
    let wire = profile.path().join("wire.jsonl");
    let (app, visual, mut manager) =
        editor(cx, workspace.path(), profile.path(), packages(&wire, true));
    open(&app, &path, visual);
    for (plugin, reason) in [
        (
            "capability-example",
            plugin_runtime::DemandReason::Command("scope-probe".into()),
        ),
        (
            "rejected-module-source",
            plugin_runtime::DemandReason::Service("novel.module".into()),
        ),
    ] {
        crate::extensions::demand_plugin_fixture(&mut manager, plugin, reason).unwrap();
    }
    let accepted = begin(
        &app,
        &mut manager,
        visual,
        "fixture/barrier",
        vec![Import {
            export: "healthy".into(),
            reference: None,
        }],
        15_000,
    );
    finish(&app, &mut manager, visual, &accepted);
    assert_completed(&accepted);
    let document = DocumentContext {
        path: "source.txt".into(),
        generation: manager
            .availability("capability-example")
            .unwrap()
            .configuration_generation(),
    };
    let original = manager
        .language_document_service("capability-example/analysis", &document)
        .unwrap()
        .unwrap();
    let reference = open_module(&mut manager);
    let rejected = begin(
        &app,
        &mut manager,
        visual,
        "fixture/refuse",
        vec![Import {
            export: "rejected".into(),
            reference: Some(reference),
        }],
        15_000,
    );
    finish(&app, &mut manager, visual, &rejected);
    assert!(
        matches!(rejected.status(), api::RequestUpdate::Completed { result:Err(ref error) }
        if error.code == api::ErrorCode::OperationFailed && error.message.contains("fixture request refused")),
        "the actual native refusal must reach its original request: {:?}",
        rejected.status()
    );
    assert!(original.is_active());
    manager.disable("rejected-module-source").unwrap();
    crate::extensions::lsp_tests::publish(&app, &mut manager, visual);
    assert!(
        original.is_active(),
        "a definitively released foreign module must not retain authority over the healthy shared process"
    );
    let barrier = begin(
        &app,
        &mut manager,
        visual,
        "fixture/barrier",
        Vec::new(),
        15_000,
    );
    finish(&app, &mut manager, visual, &barrier);
    assert_completed(&barrier);
    assert!(Arc::ptr_eq(
        &original,
        &manager
            .language_document_service("capability-example/analysis", &document)
            .unwrap()
            .unwrap()
    ));
    assert_eq!(
        frames(&wire)
            .iter()
            .filter(|message| message["method"] == "initialize")
            .count(),
        1,
        "the later success must come from the original worker, not a replacement process"
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "saved source\n");
    manager.shutdown();
}

/// Preserve the real shell and manager publication seam with all runtime data outside the project.
fn editor<'a>(
    cx: &'a mut TestAppContext,
    root: &Path,
    profile: &Path,
    packages: Vec<Package>,
) -> (Entity<EditorApp>, &'a mut VisualTestContext, Manager) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::bind_editor_shell_keys(cx);
        typography::init(cx);
        apply_theme(builtin_theme(false), cx);
        cx.set_reduce_motion(true);
    });
    let mut manager = Manager::open(
        profile.join("plugins"),
        protocol::Environment {
            workspace: root.display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    for package in packages {
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
    }
    let workspace = Workspace::open(root).unwrap();
    let capture = Rc::new(RefCell::new(None));
    let slot = capture.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let app = cx.new(|cx| EditorApp::new(workspace, None, window, cx));
        *slot.borrow_mut() = Some(app.clone());
        Root::new(app, window, cx)
    });
    let app = capture.borrow_mut().take().unwrap();
    visual.simulate_resize(size(px(1200.), px(800.)));
    crate::extensions::lsp_tests::publish(&app, &mut manager, visual);
    (app, visual, manager)
}

/// The healthy owner uses the current public diagnostic guest; a separate SDK guest owns only the
/// foreign export. This native stdio fixture does not execute either adapter's code.
fn packages(wire: &Path, released: bool) -> Vec<Package> {
    let fixture = PathBuf::from(std::env::var_os("NANOBUG_CONTROL_FIXTURE_PACKAGE").unwrap());
    let mut files = Package::read(&fixture).unwrap().files;
    let mut manifest: Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    for (capability, version) in [
        ("language.control", "^1"),
        ("language.modules", "^1"),
        ("language.lsp", "^1.5"),
        ("plugin.services", "^1.1"),
        ("process", "^1"),
    ] {
        manifest["api"]["required"][capability] = json!(version);
    }
    let permissions = manifest["permissions"].as_array_mut().unwrap();
    for permission in [
        "native.modules",
        "services.call",
        "process.service.analysis",
    ] {
        if !permissions.contains(&json!(permission)) {
            permissions.push(json!(permission));
        }
    }
    let executable = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/lsp_fixture.exe")
        .canonicalize()
        .unwrap();
    let args = if released {
        json!([wire, "--released-on-refuse"])
    } else {
        json!([wire])
    };
    manifest["services"] = json!({"analysis":{"program":executable,"args":args}});
    let control = json!({"parameters":{"type":"null"},"result":{"type":"null"},"permissions":["editor.read","workspace.read","native.modules"]});
    manifest["language_servers"] = json!([{"id":"analysis","language":"unknown-module-sidecar","service":"analysis",
        "control_methods":{"fixture/barrier":control,"fixture/refuse":control,"fixture/pending":control}}]);
    let method = json!({"parameters":{"type":"null"},"result":{"type":"null"}});
    manifest["plugin_services"] =
        json!({"requires":{"novel.module":{"version":"^1","methods":{"identity":method}}}});
    let healthy = b"module.exports = { kind: 'healthy' };\n";
    manifest["module_exports"] =
        json!({"healthy":export("modules/healthy",healthy,"novel.engine/1")});
    files.insert("modules/healthy/index.cjs".into(), healthy.to_vec());
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    let owner = Package::from_files(files).unwrap();
    let input = PathBuf::from(std::env::var_os("NANOBUG_NATIVE_CONSUMERS_PACKAGE").unwrap());
    let guest = Package::read(&input).unwrap().files["plugin.wasm"].clone();
    let code = b"module.exports = { kind: 'rejected' };\n";
    let publisher = json!({"id":"rejected-module-source","name":"Unknown foreign export","version":"0.1.0","protocol":7,
        "component":"plugin.wasm","storage_limit":16384,"permissions":["native.modules"],
        "api":{"base":"^1","required":{"language.modules":"^1","plugin.services":"^1.1"}},
        "module_exports":{"rejected":export("modules/rejected",code,"novel.engine/2")},
        "plugin_services":{"provides":{"novel.module":{"version":"1.0.0","methods":{"identity":method}}}}});
    vec![
        owner,
        Package::from_files(BTreeMap::from([
            (
                "manifest.json".into(),
                serde_json::to_vec(&publisher).unwrap(),
            ),
            ("plugin.wasm".into(), guest),
            ("modules/rejected/index.cjs".into(), code.to_vec()),
        ]))
        .unwrap(),
    ]
}

/// Exact package closure admission precedes every native descriptor delivery.
fn export(root: &str, code: &[u8], abi: &str) -> Value {
    let hash = format!("{:x}", Sha256::digest(code));
    json!({"root":root,"entry":"index.cjs","sha256":hash,"files":{"index.cjs":hash},"abi":abi,"engine":{"novel":"^1"}})
}

/// Open and observe the caller's actual SDK-owned service reference rather than forging a handle.
fn open_module(manager: &mut Manager) -> api::ResourceHandle {
    manager
        .invoke_command(
            "capability-example",
            "scope-probe",
            serde_json::to_value(api::Operation::Service {
                operation: service::Operation::Open {
                    contract: "novel.module".into(),
                },
            })
            .unwrap(),
        )
        .unwrap();
    let Kind::Text { text } = &manager.live["capability-example"].views["welcome"]
        .as_ref()
        .root
        .kind
    else {
        panic!("the SDK diagnostic view must expose its public operation result");
    };
    let result: Result<api::Value, api::Failure> = serde_json::from_str(text).unwrap();
    let api::Value::Resource(reference) = result.unwrap() else {
        panic!("expected a real service reference")
    };
    reference
}

/// Issue the actual public request, retaining all runtime import seals and permission checks.
fn begin(
    app: &Entity<EditorApp>,
    manager: &mut Manager,
    visual: &mut VisualTestContext,
    method: &str,
    imports: Vec<Import>,
    timeout_ms: u32,
) -> plugin_runtime::EditorRequest {
    manager
        .invoke_command(
            "capability-example",
            "scope-probe",
            serde_json::to_value(api::Operation::Editor {
                operation: api::EditorOperation::LanguageRequest {
                    request: protocol::language_control::Request {
                        provider: "analysis".into(),
                        document: DocumentContext {
                            path: "source.txt".into(),
                            generation: 0,
                        },
                        method: method.into(),
                        arguments: Value::Null,
                        context_snapshot: false,
                        service_sets: Vec::new(),
                        imports,
                        publish_binding: None,
                    },
                },
                timeout_ms,
            })
            .unwrap(),
        )
        .unwrap();
    let requests = manager.take_editor_requests();
    assert_eq!(
        requests.len(),
        1,
        "the SDK must issue its original control request"
    );
    let request = requests[0].1.clone();
    visual.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.perform_editor_request("capability-example", request.clone(), window, cx)
        })
    });
    request
}

/// Drive only production native IO and public manager publications until the original request ends.
fn finish(
    app: &Entity<EditorApp>,
    manager: &mut Manager,
    visual: &mut VisualTestContext,
    request: &plugin_runtime::EditorRequest,
) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !request.status().is_terminal() {
        crate::extensions::lsp_tests::publish(app, manager, visual);
        pump(visual, 10);
        assert!(
            Instant::now() < deadline,
            "native request did not finish: {:?}",
            request.status()
        );
    }
    pump(visual, 30);
    crate::extensions::lsp_tests::publish(app, manager, visual);
}

/// A matched null barrier proves that the existing native pipe remains usable after refusal.
fn assert_completed(request: &plugin_runtime::EditorRequest) {
    assert!(
        matches!(
            request.status(),
            api::RequestUpdate::Completed {
                result: Ok(api::EditorValue::Language(Value::Null))
            }
        ),
        "native request failed: {:?}",
        request.status()
    );
}

/// Only complete real frames contribute to the worker incarnation assertion.
fn frames(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
