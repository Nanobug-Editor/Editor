//! Independent SDK requests must retain a source's sidecar context between bounded calls.
//! Withheld replies and deadlines use worker::native_language_control_tests so production OS
//! scheduling can advance the UI while a dedicated thread waits on the real native pipe.
use super::*;
use protocol::api::{self, EditorOperation, EditorValue, RequestUpdate};
use serde_json::{Value, json};

/// A source without a primary LSP still keeps its approved auxiliary transport across resolve-like
/// operations. A real SDK guest issues both calls; wire initialization counts expose lost lifetimes.
#[gpui::test]
#[ignore = "build current SDK capability-example into NANOBUG_CONTROL_FIXTURE_PACKAGE and native lsp_fixture first"]
fn sequential_native_controls_keep_the_open_source_transport(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("source.txt");
    std::fs::write(&path, "source = 1\n").unwrap();
    let path = path.canonicalize().unwrap();
    let log = directory.path().join("wire.jsonl");
    let package = control_package(&log);
    let (app, visual, mut manager) = shell(cx, directory.path(), vec![package]);
    open(&app, &path, visual);
    visual.simulate_input("// 未保存😀\n");
    crate::extensions::demand_plugin_fixture(
        &mut manager,
        "capability-example",
        plugin_runtime::DemandReason::Command("scope-probe".into()),
    )
    .unwrap();

    let started = std::time::Instant::now();
    for _ in 0..2 {
        let request = begin_control(&app, &mut manager, visual, "fixture/barrier", 15_000);
        finish_control(&app, &mut manager, visual, &request);
        assert!(
            matches!(
                request.status(),
                RequestUpdate::Completed {
                    result: Ok(EditorValue::Language(Value::Null))
                }
            ),
            "native control failed: {:?}",
            request.status()
        );
        // The first completion must fully release transient waits before the second request begins.
        // Persistent native authority belongs to the open source and caller, never to a task deadline.
        pump(visual, 30);
        crate::extensions::lsp_tests::publish(&app, &mut manager, visual);
    }
    let frames = wire_frames(&log);
    assert_eq!(
        frames
            .iter()
            .filter(|frame| frame["method"] == "fixture/barrier")
            .count(),
        2
    );
    eprintln!("two warm native controls: {:?}", started.elapsed());
    assert_eq!(
        frames
            .iter()
            .filter(|frame| frame["method"] == "initialize")
            .count(),
        1,
        "sequential source controls must not restart the same authorized context"
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "source = 1\n");
    manager.shutdown();
}

/// The original SDK guest and ordinary package validation retain all request admission checks.
/// Only the fixture's declared native methods and executable are changed for these scenarios.
fn control_package(log: &Path) -> Package {
    let fixture = PathBuf::from(std::env::var_os("NANOBUG_CONTROL_FIXTURE_PACKAGE").unwrap());
    let mut files = Package::read(&fixture).unwrap().files;
    let mut manifest: Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    manifest["api"]["required"]["language.control"] = json!("^1");
    manifest["api"]["required"]["language.lsp"] = json!("^1.5");
    manifest["api"]["required"]["process"] = json!("^1");
    manifest["permissions"]
        .as_array_mut()
        .unwrap()
        .push(json!("process.service.analysis"));
    // Match the active test profile rather than an unrelated workspace's default target.
    let executable = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/lsp_fixture.exe")
        .canonicalize()
        .unwrap();
    manifest["services"] = json!({"analysis":{"program":executable,"args":[log]}});
    manifest["language_servers"] = json!([{"id":"analysis","language":"unfamiliar-sidecar","service":"analysis",
    "control_methods":{
        "fixture/barrier":{"parameters":{"type":"null"},"result":{"type":"null"}},
        "fixture/pending":{"parameters":{"type":"null"},"result":{"type":"null"}}
    }}]);
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    Package::from_files(files).unwrap()
}

/// Submit one real SDK editor operation and dispatch its sealed request at the native Host seam.
fn begin_control(
    app: &Entity<EditorApp>,
    manager: &mut Manager,
    visual: &mut VisualTestContext,
    method: &str,
    timeout_ms: u32,
) -> plugin_runtime::EditorRequest {
    manager
        .invoke_command(
            "capability-example",
            "scope-probe",
            serde_json::to_value(api::Operation::Editor {
                operation: EditorOperation::LanguageRequest {
                    request: protocol::language_control::Request {
                        provider: "analysis".into(),
                        document: protocol::language::DocumentContext {
                            path: "source.txt".into(),
                            generation: 0,
                        },
                        method: method.into(),
                        arguments: Value::Null,
                        context_snapshot: false,
                        service_sets: Vec::new(),
                        imports: Vec::new(),
                        publish_binding: None,
                    },
                },
                timeout_ms,
            })
            .unwrap(),
        )
        .unwrap();
    let requests = manager.take_editor_requests();
    assert_eq!(requests.len(), 1, "the SDK must issue exactly one request");
    let request = requests[0].1.clone();
    visual.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.perform_editor_request("capability-example", request.clone(), window, cx)
        })
    });
    request
}

/// Drive real IO, Manager publications and native timers to an observed terminal SDK outcome.
fn finish_control(
    app: &Entity<EditorApp>,
    manager: &mut Manager,
    visual: &mut VisualTestContext,
    request: &plugin_runtime::EditorRequest,
) {
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    while !request.status().is_terminal() {
        crate::extensions::lsp_tests::publish(app, manager, visual);
        pump(visual, 10);
        // A real guest completion can become terminal during this publication turn. Its own
        // timeout/result remains authoritative even if the observer's turn crossed its deadline.
        assert!(
            request.status().is_terminal() || std::time::Instant::now() < deadline,
            "native control did not complete: {:?}",
            request.status()
        );
    }
    pump(visual, 30);
    crate::extensions::lsp_tests::publish(app, manager, visual);
}

/// Read only complete fixture frames so cancellation and initialization assertions follow real IO.
fn wire_frames(log: &Path) -> Vec<Value> {
    std::fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
