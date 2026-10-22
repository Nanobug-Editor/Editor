//! Actual independently built SDK consumers exercise production Package/Manager and ownership gates.
#[path = "support/demand.rs"]
mod demand;
use plugin_runtime::{
    DemandReason, Manager, Package,
    plugin_protocol::{Environment, api, ui},
};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    time::{Duration, Instant},
};

/// Candidate archives are explicitly selected and hash-checked; there is no target or version fallback.
fn package(name: &str) -> Package {
    let ledger: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/lifecycle-inputs.json")).unwrap();
    let item = ledger["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["archive"] == name)
        .unwrap();
    let directory = std::env::var_os("NANOBUG_TOOLING_FIXTURES")
        .expect("Set NANOBUG_TOOLING_FIXTURES to the explicitly built archive directory");
    let bytes = std::fs::read(std::path::PathBuf::from(directory).join(name)).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        item["sha256"].as_str().unwrap(),
        "Rebuilt candidates need a new verified ledger, never silent substitution"
    );
    Package::from_bytes(&bytes).unwrap()
}

/// Observe consumer business metadata through its normal public UI document.
fn receipt(manager: &Manager, id: &str) -> serde_json::Value {
    let ui::Kind::Text { text } = &manager.live[id].views["result"].root.kind else {
        panic!("Expected receipt text")
    };
    serde_json::from_str(text).unwrap()
}

/// Real HTTP callbacks preserve non-2xx status and release stream/timer before instance retirement.
#[test]
#[ignore = "Build lifecycle-http/editor with --plugin-package, set NANOBUG_TOOLING_FIXTURES; see docs/plugins/verification/plugin-developer-tooling.md"]
fn lifecycle_http_real_wasm_releases_transfer_and_polling_resources() {
    let server = std::net::TcpListener::bind("127.0.0.1:18927").unwrap();
    server.set_nonblocking(true).unwrap();
    let serving = std::thread::spawn(move || {
        let until = Instant::now() + Duration::from_secs(60);
        loop {
            match server.accept() {
                Ok((mut stream, _)) => {
                    stream
                        .set_read_timeout(Some(Duration::from_secs(5)))
                        .unwrap();
                    let mut request = [0u8; 1024];
                    stream.read(&mut request).unwrap();
                    std::thread::sleep(Duration::from_millis(50));
                    stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 10\r\nConnection: close\r\n\r\n").unwrap();
                    stream.write_all("中文🙂".as_bytes()).unwrap();
                    return;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < until);
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("{error}"),
            }
        }
    });
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
    let package = package("lifecycle-http-0.1.2.zip");
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    assert!(manager.live.is_empty(), "Installation must remain cold");
    let observations = manager.runtime_observations();
    observations.start_capture();
    demand::ready(
        &mut manager,
        "lifecycle-http",
        DemandReason::Command("query".into()),
    );
    manager
        .invoke_command("lifecycle-http", "query", serde_json::Value::Null)
        .unwrap();
    let until = Instant::now() + Duration::from_secs(10);
    while receipt(&manager, "lifecycle-http")["finished"] != true {
        manager.poll();
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(receipt(&manager, "lifecycle-http")["status"], 404);
    assert_eq!(receipt(&manager, "lifecycle-http")["bytes"], 10);
    let counts = manager.live["lifecycle-http"].resource_counts();
    assert_eq!((counts.http, counts.timers), (0, 0));
    serving.join().unwrap();
    // The endpoint is now gone: transport failure remains a terminal business event with clean handles.
    manager
        .invoke_command("lifecycle-http", "query", serde_json::Value::Null)
        .unwrap();
    let until = Instant::now() + Duration::from_secs(10);
    while receipt(&manager, "lifecycle-http")["finished"] != true {
        manager.poll();
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(receipt(&manager, "lifecycle-http")["status"].is_null());
    assert_eq!(manager.live["lifecycle-http"].resource_counts().handles, 0);
    manager
        .invoke_command("lifecycle-http", "full_timers", serde_json::Value::Null)
        .unwrap();
    assert_eq!(
        receipt(&manager, "lifecycle-http")["start_error"],
        "limit_exceeded"
    );
    assert_eq!(receipt(&manager, "lifecycle-http")["pending"], 0);
    assert_eq!(manager.live["lifecycle-http"].resource_counts().handles, 0);
    // Trap before draining a new stream; instance retirement is the final guard, not Drop imports.
    manager
        .invoke_command("lifecycle-http", "query", serde_json::Value::Null)
        .unwrap();
    assert!(
        manager
            .invoke_command("lifecycle-http", "trap", serde_json::Value::Null)
            .is_err()
    );
    manager.disable("lifecycle-http").unwrap();
    let snapshot = observations.snapshot(Some("lifecycle-http"));
    assert!(
        snapshot
            .owners
            .iter()
            .filter(|owner| owner.owner.resource_owner)
            .all(|owner| owner.resources.as_ref().unwrap().retired)
    );
    assert!(
        snapshot
            .records
            .iter()
            .any(|record| record.stage == plugin_runtime::observability::Stage::Http)
    );
    assert!(snapshot.records.iter().any(|record| record.stage
        == plugin_runtime::observability::Stage::Native
        && record.elapsed_us >= 50_000));
}

/// Replacing a real pending editor request suppresses its old completion and preserves one subscription.
#[test]
#[ignore = "Build lifecycle-http/editor with --plugin-package, set NANOBUG_TOOLING_FIXTURES; see docs/plugins/verification/plugin-developer-tooling.md"]
fn lifecycle_editor_real_wasm_replaces_intent_and_closes_observation() {
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
    let package = package("lifecycle-editor-0.1.2.zip");
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    demand::ready(
        &mut manager,
        "lifecycle-editor",
        DemandReason::Command("query".into()),
    );
    manager
        .invoke_command("lifecycle-editor", "query", serde_json::Value::Null)
        .unwrap();
    let old = manager.take_editor_requests().pop().unwrap().1;
    manager
        .invoke_command("lifecycle-editor", "query", serde_json::Value::Null)
        .unwrap();
    let current = manager.take_editor_requests().pop().unwrap().1;
    assert!(
        !old.begin(),
        "The replaced request must not begin its native work"
    );
    old.finish(Ok(api::EditorValue::Documents {
        documents: vec![],
        active: None,
    }));
    assert!(current.begin());
    current.finish(Ok(api::EditorValue::Documents {
        documents: vec![],
        active: None,
    }));
    manager.poll();
    assert_eq!(receipt(&manager, "lifecycle-editor")["results"], 1);
    // Complete before the host has delivered the terminal notification, then replace again.
    manager
        .invoke_command("lifecycle-editor", "query", serde_json::Value::Null)
        .unwrap();
    let completed = manager.take_editor_requests().pop().unwrap().1;
    assert!(completed.begin());
    completed.finish(Ok(api::EditorValue::Documents {
        documents: vec![],
        active: None,
    }));
    manager
        .invoke_command("lifecycle-editor", "query", serde_json::Value::Null)
        .unwrap();
    let replacement = manager.take_editor_requests().pop().unwrap().1;
    assert!(replacement.begin());
    replacement.finish(Ok(api::EditorValue::Documents {
        documents: vec![],
        active: None,
    }));
    manager.poll();
    assert_eq!(receipt(&manager, "lifecycle-editor")["results"], 2);
    assert_eq!(
        manager.live["lifecycle-editor"]
            .resource_counts()
            .subscriptions,
        1
    );
    manager
        .invoke_command("lifecycle-editor", "close", serde_json::Value::Null)
        .unwrap();
    assert_eq!(
        manager.live["lifecycle-editor"].resource_counts().handles,
        0
    );
    manager.shutdown_checked().unwrap();
}
