//! Releasing the last pending peer must stop code whose previously committed consumer retired.
use super::*;

/// A real native refusal releases only B's pending use. Once A has retired there is no remaining
/// authority for their shared bytes, so even the still-held original pipe must physically close.
#[test]
#[ignore = "build native-consumers through host --plugin-package, set NANOBUG_NATIVE_CONSUMERS_PACKAGE, and build lsp_fixture"]
fn releasing_the_last_pending_peer_reaps_code_from_a_retired_committed_consumer() {
    let workspace = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    std::fs::create_dir(workspace.path().join("first")).unwrap();
    let source = workspace.path().join("first/source.novel");
    std::fs::write(&source, "saved source").unwrap();
    let mut manager = Manager::open(
        profile.path().into(),
        Environment {
            workspace: workspace.path().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    let component = Package::read(&input("NANOBUG_NATIVE_CONSUMERS_PACKAGE"))
        .unwrap()
        .files["plugin.wasm"]
        .clone();
    for package in packages(&component, &profile.path().join("wire.jsonl")) {
        // This separate scenario grants the existing third consumer all declared permissions.
        // Its fresh lease must not make quota cleanup forget A's already loaded code.
        let package = with_release_receipt(package);
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
    }
    demand::ready(
        &mut manager,
        SOURCE,
        DemandReason::Service("novel.module".into()),
    );
    let original = prepare(&mut manager, "first/source.novel");
    for client in [CLIENT_A, CLIENT_B, DENIED] {
        demand::ready(&mut manager, client, DemandReason::Command("run".into()));
    }
    let mut workers = Vec::new();
    assert_eq!(
        query(
            &mut manager,
            CLIENT_A,
            &original,
            &mut workers,
            &mut BTreeMap::new(),
        )["state"],
        "complete"
    );
    let (request, pending) = pending_peer(&mut manager, &original, CLIENT_B);
    worker(&mut workers, &original).barrier();
    manager.disable(CLIENT_A).unwrap();
    assert!(
        original.is_active(),
        "the independent pending peer still authorizes the shared code"
    );
    worker(&mut workers, &original).barrier();
    let (third_request, third_pending) = pending_peer(&mut manager, &original, DENIED);
    worker(&mut workers, &original).barrier();
    released_refusal(worker(&mut workers, &original));
    third_pending.release();
    third_request.finish(Err(api::Failure::new(
        api::ErrorCode::OperationFailed,
        "fixture request refused",
    )));
    assert!(original.is_active());
    worker(&mut workers, &original).barrier();
    released_refusal(worker(&mut workers, &original));
    pending.release();
    request.finish(Err(api::Failure::new(
        api::ErrorCode::OperationFailed,
        "fixture request refused",
    )));
    assert!(
        !original.is_active(),
        "released peers must not erase the retired consumer's previously committed code"
    );
    assert_eq!(
        original.process_count(),
        0,
        "releasing the last pending peer must reap code with no remaining live consumer"
    );
    worker(&mut workers, &original).assert_closed();
    assert!(manager.instance_id(CLIENT_B).is_some());
    assert!(manager.instance_id(SOURCE).is_some());
    assert_eq!(std::fs::read_to_string(source).unwrap(), "saved source");
    manager.shutdown();
}

/// Keep the original independently compiled SDK guest and only enable a definite native receipt.
fn with_release_receipt(package: Package) -> Package {
    if package.manifest.id != ENGINE {
        return package;
    }
    let mut files = package.files;
    let mut manifest: Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    manifest["services"]["analysis"]["args"]
        .as_array_mut()
        .unwrap()
        .push(json!("--released-on-refuse"));
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    Package::from_files(files).unwrap()
}

/// A still-pending SDK request supplies the sealed original authority. The trusted host keeps
/// this dispatch ticket until the actual native refusal, instead of prematurely committing it.
fn pending_peer(
    manager: &mut Manager,
    plan: &Arc<LanguageService>,
    client: &str,
) -> (
    plugin_runtime::EditorRequest,
    plugin_runtime::NativeModuleAttachment,
) {
    manager
        .invoke_command(
            client,
            "run",
            json!({"document":{"path":"first/source.novel","generation":0}}),
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        manager.poll();
        let requests = manager.take_editor_requests();
        if !requests.is_empty() {
            assert_eq!(requests.len(), 1);
            let (_, request) = requests.into_iter().next().unwrap();
            assert!(request.begin());
            let metadata = request.language_control_metadata().unwrap();
            assert_eq!(metadata.consumer, manager.instance_id(client).unwrap());
            assert_eq!(metadata.owner, manager.instance_id(ENGINE).unwrap());
            assert_eq!(metadata.imports.len(), 1);
            request.register_language_consumer(plan).unwrap();
            let mut pending = metadata.imports[0].reserve(plan).unwrap();
            pending.dispatch().unwrap();
            return (request, pending);
        }
        assert!(
            Instant::now() < deadline,
            "{client} never issued its SDK import"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Match the exact framed JSON-RPC refusal before interpreting its strict public release receipt.
fn released_refusal(worker: &mut Worker) {
    worker.sequence += 1;
    let body = serde_json::to_vec(&json!({"jsonrpc":"2.0","id":worker.sequence,
        "method":"fixture/refuse","params":{"arguments":null}}))
    .unwrap();
    write!(worker.input, "Content-Length: {}\r\n\r\n", body.len()).unwrap();
    worker.input.write_all(&body).unwrap();
    worker.input.flush().unwrap();
    let response = worker.read(false);
    assert_eq!(response["id"], worker.sequence);
    assert_eq!(response["error"]["code"], -32042);
    let receipt: plugin_runtime::plugin_protocol::language_control::NativeControlFailureData =
        serde_json::from_value(response["error"]["data"].clone()).unwrap();
    assert!(receipt.valid());
}
