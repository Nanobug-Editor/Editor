//! Query behavior uses two actual independent cloud packages and bounded socket fixtures.
use super::*;
/// Two unrelated cloud consumers perform POST/GET, use a securely entered Token and incrementally render bytes.
#[test]
#[ignore = "build both cloud packages using current host --plugin-package first"]
fn t25_two_cloud_consumers_stream_use_system_tokens_and_cache_without_leaking_secrets() {
    let server = endpoint::Server::new(|stream, _| {
        let _ = stream.write_all(
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n",
        );
        for chunk in ["first 世界😀\n", "second 分块\n"] {
            let _ = write!(stream, "{:x}\r\n{chunk}\r\n", chunk.len());
            let _ = stream.flush();
            std::thread::sleep(Duration::from_millis(120));
        }
        let _ = stream.write_all(b"0\r\n\r\n");
        let _ = stream.flush();
    });
    let credentials = SecretCleanup::new("cloud-translation", "api-token");
    let workspace = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(
        credentials.root.path().into(),
        environment(workspace.path()),
    )
    .unwrap();
    for id in ["cloud-translation", "code-host-query"] {
        let package = package(id, |manifest| grant(manifest, &server.origin));
        install(&mut manager, &package);
    }
    manager
        .invoke_command("cloud-translation", "set-token", Json::Null)
        .unwrap();
    let input = manager
        .live
        .get_mut("cloud-translation")
        .unwrap()
        .take_editor_requests()
        .pop()
        .unwrap();
    assert!(input.begin());
    input.finish(Ok(api::EditorValue::Interaction(
        plugin_runtime::plugin_protocol::interaction::Value::Input(
            "fake-native-token-sdk-test".into(),
        ),
    )));
    manager.poll();
    assert!(text(&manager, "cloud-translation").contains("Token stored securely"));
    let baseline = manager.resource_count();
    for (id, query) in [
        ("cloud-translation", "hello 世界😀"),
        ("code-host-query", "Nanobug"),
    ] {
        manager
            .invoke_command(
                id,
                "query",
                json!({"url":server.url("/query"),"text":query}),
            )
            .unwrap();
    }
    let until = Instant::now() + Duration::from_secs(10);
    while !text(&manager, "cloud-translation").contains("first 世界😀") {
        assert!(
            Instant::now() < until,
            "Missing incremental result: framing={:?} captures={} text={}",
            server.framing.lock().unwrap(),
            server.count(),
            text(&manager, "cloud-translation")
        );
        manager.poll();
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!text(&manager, "cloud-translation").contains("second 分块"));
    wait(&mut manager, |manager| {
        ["cloud-translation", "code-host-query"]
            .iter()
            .all(|id| text(manager, id).contains("Complete"))
    });
    assert_eq!(manager.resource_count(), baseline);
    let captured = server.captured.lock().unwrap();
    assert!(
        captured
            .iter()
            .any(|request| request.headers.starts_with("POST ")
                && request.body == "hello 世界😀".as_bytes())
    );
    assert!(
        captured
            .iter()
            .any(|request| request.headers.starts_with("GET ") && request.body.is_empty())
    );
    assert!(
        captured
            .iter()
            .any(|request| request.headers.contains("Bearer fake-native-token-"))
    );
    drop(captured);
    manager
        .invoke_command("cloud-translation", "read-cache", Json::Null)
        .unwrap();
    assert_eq!(
        text(&manager, "cloud-translation"),
        "first 世界😀\nsecond 分块\n"
    );
    manager.checkpoint().unwrap();
    for id in ["cloud-translation", "code-host-query"] {
        assert!(!text(&manager, id).contains("fake-native-token"));
        assert!(
            manager
                .runtime_logs()
                .records(id)
                .iter()
                .all(|record| !record.message.contains("fake-native-token"))
        );
        let snapshot = manager.live.get_mut(id).unwrap().snapshot().unwrap();
        assert!(!snapshot.data.contains("fake-native-token"));
    }
    fn scan(path: &std::path::Path) {
        for entry in std::fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                scan(&path);
            } else {
                assert!(
                    !std::fs::read(path)
                        .unwrap()
                        .windows(b"fake-native-token".len())
                        .any(|bytes| bytes == b"fake-native-token")
                );
            }
        }
    }
    // Release the live runtime's lock before scanning all ordinary settings, registry and checkpoint files.
    drop(manager);
    scan(credentials.root.path());
    assert_eq!(std::fs::read_dir(workspace.path()).unwrap().count(), 0);
}

/// The general entry point accepts arbitrary methods/body, returns non-2xx statuses and limits bytes/deadlines.
#[test]
#[ignore = "build both cloud packages using current host --plugin-package first"]
fn t26_http_methods_errors_size_timeout_and_redirect_grants_are_observable() {
    let other = endpoint::Server::new(|stream, _| endpoint::response(stream, 200, b"other"));
    let target = other.url("/target");
    let server = endpoint::Server::new(move |stream, request| {
        let path = request.headers.split_whitespace().nth(1).unwrap_or("/");
        match path {
            "/redirect" => {
                let _ = write!(
                    stream,
                    "HTTP/1.1 302 Found\r\nLocation: {target}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
            }
            "/large" => {
                let _=stream.write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n400\r\n");
                let _ = stream.write_all(&[b'x'; 1024]);
                let _ = stream.write_all(b"\r\n0\r\n\r\n");
                let _ = stream.flush();
            }
            "/stall" => {
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 20\r\nConnection: close\r\n\r\n",
                );
                let _ = stream.flush();
                let mut byte = [0];
                let _ = stream.read(&mut byte);
            }
            _ => endpoint::response(stream, 503, b"service unavailable"),
        }
    });
    let root = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(root.path().into(), environment(root.path())).unwrap();
    install(
        &mut manager,
        &package("cloud-translation", |manifest| {
            grant(manifest, &server.origin)
        }),
    );
    let mut request = http::Request::get(server.url("/status"));
    request.method = "PATCH".into();
    request.body = b"patch body".to_vec();
    let handle = resource(
        probe(
            &mut manager,
            "cloud-translation",
            http_op(http::Operation::Start { request }),
        )
        .unwrap(),
    );
    let events = finish(&mut manager, "cloud-translation", &handle);
    assert!(matches!(
        &events[0],
        http::Event::Headers { status: 503, .. }
    ));
    assert!(matches!(
        events.last(),
        Some(http::Event::Finished { bytes: 19 })
    ));
    assert_eq!(server.captured.lock().unwrap()[0].body, b"patch body");
    probe(
        &mut manager,
        "cloud-translation",
        Op::CloseResource { handle },
    )
    .unwrap();
    for (path, expected) in [
        ("/large", ErrorCode::LimitExceeded),
        ("/stall", ErrorCode::TimedOut),
        ("/redirect", ErrorCode::PermissionDenied),
    ] {
        let mut request = http::Request::get(server.url(path));
        request.max_response_bytes = 32;
        request.timeout_ms = if path == "/stall" { 200 } else { 3000 };
        let handle = resource(
            probe(
                &mut manager,
                "cloud-translation",
                http_op(http::Operation::Start { request }),
            )
            .unwrap(),
        );
        let events = finish(&mut manager, "cloud-translation", &handle);
        assert!(
            matches!(events.last(),Some(http::Event::Failed{error}) if error.code==expected),
            "{events:?}"
        );
        probe(
            &mut manager,
            "cloud-translation",
            Op::CloseResource { handle },
        )
        .unwrap();
    }
    assert_eq!(other.count(), 0);
    // Reading a secret cannot compensate for a missing exact network target grant.
    let request = http::Request::get(other.url("/direct"));
    assert_eq!(
        probe(
            &mut manager,
            "cloud-translation",
            http_op(http::Operation::Start { request })
        )
        .unwrap_err()
        .code,
        ErrorCode::PermissionDenied
    );
}
