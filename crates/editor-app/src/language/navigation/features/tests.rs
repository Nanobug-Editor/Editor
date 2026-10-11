//! Real installed packages and controlled stdio requests exercise the native language feature seam.

use super::super::*;
use plugin_runtime::plugin_protocol::{api::DocumentVersion, direct, language::SourceSnapshot};

/// Cancelled and successful features share one real service and one dirty Unicode synchronization.
#[test]
#[ignore = "build current SDK capability-example and lsp_fixture before running"]
fn native_lsp_shared_features_preserve_unsaved_utf16_and_single_connection() {
    let directory = tempfile::tempdir().unwrap();
    let exe = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/debug/examples/lsp_fixture.exe")
        .canonicalize()
        .unwrap();
    let log = directory.path().join("wire.jsonl");
    let package =
        crate::extensions::lsp_tests::package_with_args(&exe, &log, &["--ignore-references-once"]);
    let mut manager = plugin_runtime::Manager::open(
        directory.path().join("installed"),
        plugin_runtime::plugin_protocol::Environment {
            workspace: directory.path().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    let service = manager.language_services()["capability-example/analysis"]
        .as_ref()
        .unwrap()
        .clone();
    let server = LanguageServer::from_service(service.clone()).unwrap();
    server.prepare_until_ready().unwrap();
    let lease = server.open_document(file_uri(&directory.path().join("dirty.novel")).unwrap());
    let source = SourceSnapshot {
        document: DocumentVersion {
            id: "native-dirty".into(),
            path: "dirty.novel".into(),
            revision: 7,
        },
        text: "中文🙂\r\ncall(参数)".into(),
    };
    // The fixture withholds one actual reply. Cancel only after the request reaches its wire,
    // then verify successful requests reuse the same process rather than consuming retry budget.
    let cancelled_id = cancel_first_reference(&server, &lease, &source, directory.path(), &log);
    assert_eq!(service.process_count(), 1);
    assert!(server.is_active());
    for _ in 0..2 {
        let result = server
            .feature_for(
                lease.clone(),
                source.clone(),
                10,
                direct::Feature::References,
                &plugin_runtime::RequestCancellation::default(),
                directory.path(),
            )
            .unwrap();
        let direct::Result::References { locations } = result else {
            panic!("Expected references")
        };
        assert_eq!(locations.len(), 1);
        assert!(matches!(
            &locations[0].resource,
            plugin_runtime::plugin_protocol::api::ResourceIdentity::Local { path }
                if path == "dirty.novel"
        ));
        assert_eq!(locations[0].position.line, 0);
        assert_eq!(locations[0].position.character, 2);
    }
    let result = server
        .feature_for(
            lease.clone(),
            source.clone(),
            10,
            direct::Feature::SignatureHelp,
            &plugin_runtime::RequestCancellation::default(),
            directory.path(),
        )
        .unwrap();
    let direct::Result::SignatureHelp { help: Some(help) } = result else {
        panic!("Expected signature overloads")
    };
    assert_eq!(help.signatures.len(), 2);
    assert_eq!(help.active_signature, Some(1));
    assert_eq!(help.active_parameter, Some(1));
    assert_eq!(help.signatures[1].active_parameter, Some(0));
    assert_eq!(help.signatures[1].parameters[0].label, "🙂");
    assert_eq!(help.signatures[1].parameters[1].label, "参数");
    // The standard edit is a readonly proposal for the same native revision, not an immediate write.
    let result = server
        .feature_for(
            lease.clone(),
            source.clone(),
            10,
            direct::Feature::CodeActions,
            &plugin_runtime::RequestCancellation::default(),
            directory.path(),
        )
        .unwrap();
    let direct::Result::CodeActions { actions } = result else {
        panic!("Expected a native code-action proposal")
    };
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].title, "Fix Unicode");
    assert_eq!(actions[0].plan.origin.as_ref(), Some(&source.document));
    let [
        plugin_runtime::plugin_protocol::workspace::Change::EditDocument {
            document,
            edits,
            selection,
        },
    ] = actions[0].plan.changes.as_slice()
    else {
        panic!("A standard current-source fix must retain one complete document operation")
    };
    assert_eq!(document, &source.document);
    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].range.start, 6);
    assert_eq!(edits[0].range.end, 10);
    assert_eq!(edits[0].text, "新🙂");
    assert_eq!((selection.start, selection.end), (13, 13));
    let wire = std::fs::read_to_string(log).unwrap();
    let messages: Vec<Value> = wire
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(
        messages
            .iter()
            .filter(|message| message["method"] == "initialize")
            .count(),
        1
    );
    assert_eq!(
        messages
            .iter()
            .filter(|message| message["method"] == "textDocument/didOpen")
            .count(),
        1
    );
    let request = messages
        .iter()
        .find(|message| message["method"] == "textDocument/references")
        .unwrap();
    assert_eq!(
        request["params"]["position"],
        json!({"line":0,"character":4})
    );
    assert!(
        messages.iter().any(|message| {
            message["method"] == "$/cancelRequest" && message["params"]["id"] == cancelled_id
        }),
        "Cancellation must name the actual abandoned request"
    );
    assert_eq!(
        messages
            .iter()
            .filter(|message| message["method"] == "textDocument/references")
            .count(),
        3,
        "The cancelled request and both successful requests use this one connection"
    );
    assert_eq!(service.process_count(), 1);
    assert!(
        messages
            .iter()
            .any(|message| { message["params"]["textDocument"]["text"] == source.text })
    );
    assert!(
        !directory.path().join("dirty.novel").exists(),
        "No saved file substitutes for the native source"
    );
    server.retire();
    assert_eq!(service.process_count(), 0);
}

/// Observe the standard request before cancelling its public wait token. Bounded wire polling
/// tolerates a concurrently written final JSON line; no timer alone stands in for request arrival.
fn cancel_first_reference(
    server: &LanguageServer,
    lease: &DocumentLease,
    source: &SourceSnapshot,
    workspace: &Path,
    log: &Path,
) -> Value {
    let cancellation = plugin_runtime::RequestCancellation::default();
    std::thread::scope(|scope| {
        let task = scope.spawn(|| {
            server.feature_for(
                lease.clone(),
                source.clone(),
                10,
                direct::Feature::References,
                &cancellation,
                workspace,
            )
        });
        let started = Instant::now();
        let id = loop {
            let wire = std::fs::read_to_string(log).unwrap_or_default();
            if let Some(message) = wire
                .lines()
                .filter_map(|line| serde_json::from_str::<Value>(line).ok())
                .find(|message| message["method"] == "textDocument/references")
            {
                break message["id"].clone();
            }
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "The controlled service must receive the request"
            );
            std::thread::sleep(Duration::from_millis(10));
        };
        cancellation.cancel();
        let error = task.join().unwrap().unwrap_err();
        assert_eq!(
            error
                .downcast_ref::<plugin_runtime::plugin_protocol::api::Failure>()
                .map(|failure| failure.code.clone()),
            Some(plugin_runtime::plugin_protocol::api::ErrorCode::Cancelled)
        );
        id
    })
}
