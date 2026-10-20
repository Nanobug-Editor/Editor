//! Real LSP bursts verify bounded notification admission and lossless connection control state.

use super::*;

/// Closed-file diagnostics stress the ordinary queue without creating a document or a plugin fault.
fn closed_document_notifications(root: &Path) -> Vec<Value> {
    let uri = file_uri(&root.join("closed.unfamiliar")).unwrap();
    (0..257)
        .map(|index| {
            json!({"jsonrpc":"2.0", "method":"textDocument/publishDiagnostics", "params":{
                "uri":uri, "version":1, "diagnostics":[{
                    "range":{"start":{"line":0,"character":0},"end":{"line":0,"character":1}},
                    "severity":1,"message":format!("document-only-error-{index}")}]}})
        })
        .collect()
}

/// Emit unsolicited messages through the real server without consuming its output queue.
fn notify_fixture(server: &LanguageServer, notifications: Vec<Value>) {
    server
        .connection
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .notify(
            "fixture/notifications",
            json!({"notifications":notifications}),
        )
        .unwrap();
}

/// A receipt barrier makes this a snapshot of the actual idle queue, rather than a racing count.
fn assert_bounded_closed_notifications(server: &LanguageServer, root: &Path) {
    let mut connection = server.connection.lock().unwrap();
    let connection = connection.as_mut().unwrap();
    assert!(
        connection.output.is_ready(),
        "a full queue must preserve declared control state"
    );
    let mut received = 0;
    let deadline = Instant::now() + Duration::from_secs(5);
    while let Ok(message) = connection.output.try_recv() {
        let message = message.unwrap();
        assert_eq!(message["method"], "textDocument/publishDiagnostics");
        connection
            .handle_server_message_until(&message, deadline)
            .unwrap();
        received += 1;
    }
    assert_eq!(received, 256, "only the bounded ordinary queue is retained");
    assert!(
        !connection
            .diagnostics
            .is_open(file_uri(&root.join("closed.unfamiliar")).unwrap().as_str())
    );
}

/// An idle ordinary-notification burst cannot hide later logs, readiness, requests, or responses.
#[test]
#[ignore = "build cargo build -p plugin-runtime --example lsp_fixture first"]
fn native_lsp_document_queue_overflow_preserves_idle_logs_and_control_messages() {
    let directory = tempfile::tempdir().unwrap();
    let (_manager, server) = installed_fixture(directory.path(), None);
    let logs = server.service.runtime_logs();
    server.prepare_until_ready().unwrap();
    {
        // A reply is a wire barrier: startup messages and this earlier false state are consumed first.
        let mut connection = server.connection.lock().unwrap();
        let connection = connection.as_mut().unwrap();
        connection
            .request(
                "fixture/notifications",
                json!({"notifications":[
                    {"jsonrpc":"2.0","method":"fixture/status","params":{"state":"busy"}}
                ]}),
            )
            .unwrap();
        assert!(!connection.output.is_ready());
    }
    let mut notifications = closed_document_notifications(directory.path());
    notifications.extend([
        json!({"jsonrpc":"2.0","method":"fixture/status","params":{"state":"ready"}}),
        json!({"jsonrpc":"2.0","method":"window/logMessage","params":{"type":3,"message":"idle-after-document-overflow"}}),
    ]);
    notify_fixture(&server, notifications);
    // No request, readiness wait, or document poll can rescue the reader before this receipt assertion.
    let records = wait_for_log(&logs, "idle-after-document-overflow");
    let warnings: Vec<_> = records
        .iter()
        .filter(|record| record.level == LogLevel::Warning)
        .collect();
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].source, "lsp/analysis");
    assert_eq!(warnings[0].plugin, OWNER);
    assert_eq!(
        warnings[0].message,
        rust_i18n::t!("plugins.logs.notification_overflow").to_string()
    );
    assert!(records.iter().all(|record| record.level != LogLevel::Error
        && !record.message.starts_with("document-only-error-")));
    assert_bounded_closed_notifications(&server, directory.path());
    // The dropped ready notification still completes the actual service preparation path.
    server.prepare_until_ready().unwrap();
    let mut notifications = closed_document_notifications(directory.path());
    notifications.extend([
        json!({"jsonrpc":"2.0","method":"window/logMessage","params":{"type":3,"message":"idle-after-second-overflow"}}),
        json!({"jsonrpc":"2.0","id":"server-after-overflow","method":"workspace/workspaceFolders","params":{}}),
    ]);
    notify_fixture(&server, notifications);
    let records = wait_for_log(&logs, "idle-after-second-overflow");
    assert_eq!(
        records
            .iter()
            .filter(|record| record.level == LogLevel::Warning)
            .count(),
        1
    );
    {
        let mut connection = server.connection.lock().unwrap();
        let connection = connection.as_mut().unwrap();
        connection.request("fixture/barrier", json!({})).unwrap();
        // A second response confirms the fixture received our answer to its non-discardable request.
        connection.request("fixture/barrier", json!({})).unwrap();
    }
    let wire = std::fs::read_to_string(directory.path().join("wire.jsonl")).unwrap();
    assert!(wire.lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .any(|message| message["id"] == "server-after-overflow" && message["result"].is_array()));
}

/// Losing the final push must invalidate the old watermark and request a complete live snapshot.
/// The server only publishes on source synchronization; unchanged text therefore proves recovery.
#[test]
#[ignore = "build plugin-runtime --example lsp_fixture in the same Cargo target first"]
fn notification_overflow_resynchronizes_unchanged_live_sources_before_diagnostics() {
    let directory = tempfile::tempdir().unwrap();
    let (_manager, server) = installed_fixture(directory.path(), None);
    let uri = file_uri(&directory.path().join("live.novel")).unwrap();
    let source = "unsaved 中文😀e\u{301}\r\n";
    let lease = server.open_document(uri.clone());
    server
        .definitions(uri.clone(), source.into(), Position::new(0, 0))
        .unwrap();
    let initial = server
        .diagnostics_for(lease.clone(), source)
        .unwrap()
        .unwrap();
    assert_eq!(initial[0].message, format!("unsaved:{source}"));

    let mut notifications = closed_document_notifications(directory.path());
    notifications.push(
        json!({"jsonrpc":"2.0", "method":"textDocument/publishDiagnostics",
        "params":{"uri":uri,"version":1,"diagnostics":[]}}),
    );
    notifications.push(json!({"jsonrpc":"2.0", "method":"window/logMessage",
        "params":{"type":3,"message":"after-lost-live-publication"}}));
    notify_fixture(&server, notifications);
    wait_for_log(
        &server.service.runtime_logs(),
        "after-lost-live-publication",
    );

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if server
            .diagnostics_for(lease.clone(), source)
            .unwrap()
            .is_some_and(|items| {
                items
                    .iter()
                    .any(|item| item.message == format!("unsaved:{source}"))
            })
        {
            let frames = std::fs::read_to_string(directory.path().join("wire.jsonl")).unwrap();
            let synchronized = frames
                .lines()
                .map(|line| serde_json::from_str::<Value>(line).unwrap())
                .filter(|frame| {
                    matches!(
                        frame["method"].as_str(),
                        Some("textDocument/didOpen" | "textDocument/didChange")
                    )
                })
                .collect::<Vec<_>>();
            assert!(
                synchronized.len() >= 2,
                "a lost publication must not leave the prior source watermark marked complete"
            );
            assert!(
                synchronized.last().unwrap()["params"]["textDocument"]["version"]
                    .as_i64()
                    .unwrap()
                    > synchronized[0]["params"]["textDocument"]["version"]
                        .as_i64()
                        .unwrap()
            );
            break;
        }
        assert!(
            Instant::now() < deadline,
            "live diagnostics must recover without another user edit"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        !directory.path().join("live.novel").exists(),
        "synchronization must never save the source"
    );
}

/// Readiness declared on a standard log message remains observable after that message bypasses the queue.
#[test]
#[ignore = "build cargo build -p plugin-runtime --example lsp_fixture first"]
fn native_lsp_log_only_readiness_does_not_require_a_document_queue_wakeup() {
    let directory = tempfile::tempdir().unwrap();
    let package = fixture_package_with_readiness(
        directory.path(),
        None,
        json!({
            "notification":"window/logMessage", "pointer":"/message", "expected":"ready-in-log", "timeout_ms":5000
        }),
    );
    let (_manager, server) = installed_package(directory.path(), &package);
    let logs = server.service.runtime_logs();
    server.prepare().unwrap();
    notify_fixture(
        &server,
        vec![json!({"jsonrpc":"2.0", "method":"window/logMessage",
        "params":{"type":3,"message":"ready-in-log"}})],
    );
    // There is no queued readiness message to wake recv_timeout; the reader's control state is sufficient.
    let started = Instant::now();
    server.prepare_until_ready().unwrap();
    assert!(started.elapsed() < Duration::from_secs(2));
    let records = wait_for_log(&logs, "ready-in-log");
    assert!(records.iter().all(|record| record.level == LogLevel::Info));
}

/// A server that announces ready but stops reading replies cannot renew the declared startup budget.
#[test]
#[ignore = "build cargo build -p plugin-runtime --example lsp_fixture first"]
fn native_lsp_blocked_server_reply_respects_readiness_deadline() {
    let directory = tempfile::tempdir().unwrap();
    let package = fixture_package_with_readiness(
        directory.path(),
        None,
        json!({
            "notification":"fixture/status", "pointer":"/state", "expected":"ready", "timeout_ms":500
        }),
    );
    let (_manager, server) = installed_package(directory.path(), &package);
    let logs = server.service.runtime_logs();
    server.prepare_until_ready().unwrap();
    server
        .connection
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .notify("fixture/block-with-ready", json!({}))
        .unwrap();
    // Receipt follows the actual server request and ready frames, so its large reply is pending in the queue.
    wait_for_log(&logs, "fixture stdin blocked");
    let started = Instant::now();
    // Measure one existing preparation attempt: the public recovery loop may restart a healthy fixture.
    let result = server.prepare_ready_once();
    let elapsed = started.elapsed();
    // This direct attempt does not own recovery teardown, so explicitly stop the blocked native child.
    server.retire();
    let error = result.unwrap_err();
    assert!(elapsed < Duration::from_secs(3));
    assert!(
        format!("{error:#}").contains("wait for LSP pipe write"),
        "unexpected failure: {error:#}"
    );
}
