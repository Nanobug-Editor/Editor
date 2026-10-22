//! Independent SDK requests must retain a source's sidecar context between bounded calls.
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
    let executable = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/debug/examples/lsp_fixture.exe")
        .canonicalize()
        .unwrap();
    manifest["services"] = json!({"analysis":{"program":executable,"args":[log]}});
    manifest["language_servers"] = json!([{"id":"analysis","language":"unfamiliar-sidecar","service":"analysis",
        "control_methods":{"fixture/barrier":{"parameters":{"type":"null"},"result":{"type":"null"}}}}]);
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    let package = Package::from_files(files).unwrap();
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
                            method: "fixture/barrier".into(),
                            arguments: Value::Null,
                            context_snapshot: false,
                            service_sets: Vec::new(),
                            imports: Vec::new(),
                            publish_binding: None,
                        },
                    },
                    timeout_ms: 15_000,
                })
                .unwrap(),
            )
            .unwrap();
        let requests = manager.take_editor_requests();
        assert_eq!(
            requests.len(),
            1,
            "the real SDK must issue exactly one native request"
        );
        let request = requests[0].1.clone();
        visual.update(|window, cx| {
            app.update(cx, |app, cx| {
                app.perform_editor_request("capability-example", request.clone(), window, cx)
            })
        });
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while !request.status().is_terminal() {
            crate::extensions::lsp_tests::publish(&app, &mut manager, visual);
            pump(visual, 10);
            assert!(
                std::time::Instant::now() < deadline,
                "native control did not complete: {:?}",
                request.status()
            );
        }
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
    let frames = std::fs::read_to_string(&log)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
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
