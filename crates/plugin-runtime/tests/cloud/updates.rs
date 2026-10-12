//! Resource retirement and private recovery enter through independently SDK-built cloud guests.
use super::*;

/// Activation failure retires new cloud resources and staged bytes while preserving committed data and Token.
#[test]
#[ignore = "opt-in fixed SDK/WASM packages; rebuild sources: crates/plugin-runtime/tests/fixtures/community; see fixtures/packages/README.md"]
fn t30_failed_cloud_update_retires_candidate_and_recovers_data_without_replaying_http() {
    let server = endpoint::Server::new(|stream, _| {
        let _ = stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\n");
        let _ = stream.flush();
        let mut byte = [0];
        let _ = stream.read(&mut byte);
    });
    let credentials = SecretCleanup::new("cloud-translation", "api-token");
    let workspace = tempfile::tempdir().unwrap();
    let configuration = HttpConfiguration::default();
    let observer = configuration.clone();
    let mut manager = Manager::open_with_resources(
        credentials.root.path().into(),
        environment(workspace.path()),
        true,
        HostResources {
            http: configuration,
            ..Default::default()
        },
    )
    .unwrap();
    let old = package("cloud-translation", |manifest| {
        grant(manifest, &server.origin)
    });
    install(&mut manager, &old);
    probe(
        &mut manager,
        "cloud-translation",
        secret_op(secrets::Operation::Write {
            key: "api-token".into(),
            bytes: b"fake-update-private-token".to_vec(),
        }),
    )
    .unwrap();
    let data = resource(probe(&mut manager, "cloud-translation", Op::OpenData).unwrap());
    probe(
        &mut manager,
        "cloud-translation",
        Op::WriteFile {
            handle: data.clone(),
            path: "startup-state.txt".into(),
            bytes: b"committed old bytes".to_vec(),
        },
    )
    .unwrap();
    let stream = resource(
        probe(
            &mut manager,
            "cloud-translation",
            http_op(http::Operation::Start {
                request: http::Request::get(server.url("/old")),
            }),
        )
        .unwrap(),
    );
    let timer = resource(
        probe(
            &mut manager,
            "cloud-translation",
            timer_op(timers::Operation::Start {
                delay_ms: 1000,
                repeat: true,
            }),
        )
        .unwrap(),
    );
    let mut files = package("cloud-translation", |manifest| {
        grant(manifest, &server.origin);
        // The delivered consumer is 0.1.1; retain a newer candidate for the failed upgrade boundary.
        manifest["version"] = json!("0.1.2");
    })
    .files;
    files.insert(
        "activation-policy.json".into(),
        serde_json::to_vec(
            &json!({"request":http::Request::get(server.url("/candidate")),"fail":true}),
        )
        .unwrap(),
    );
    let candidate = archive_files(files);
    assert!(
        manager
            .install(&candidate, candidate.manifest.permissions.clone())
            .is_err()
    );
    assert_eq!(
        manager.installed["cloud-translation"].manifest.version,
        old.manifest.version
    );
    assert_eq!(manager.resource_count(), 1);
    let until = Instant::now() + Duration::from_secs(1);
    while observer.active_workers() != 0 {
        assert!(
            Instant::now() < until,
            "Recovered producer count={}",
            observer.active_workers()
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    for handle in [data, stream, timer] {
        assert_eq!(
            probe(
                &mut manager,
                "cloud-translation",
                Op::CloseResource { handle }
            )
            .unwrap_err()
            .code,
            ErrorCode::InvalidHandle
        );
    }
    let data = resource(probe(&mut manager, "cloud-translation", Op::OpenData).unwrap());
    let Value::Bytes(bytes) = probe(
        &mut manager,
        "cloud-translation",
        Op::ReadFile {
            handle: data,
            path: "startup-state.txt".into(),
        },
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(bytes, b"committed old bytes");
    manager
        .invoke_command(
            "cloud-translation",
            "probe",
            serde_json::to_value(secret_op(secrets::Operation::Read {
                key: "api-token".into(),
            }))
            .unwrap(),
        )
        .unwrap();
    assert_eq!(text(&manager, "cloud-translation"), "secret-present");
    let count = server.count();
    manager.poll();
    std::thread::sleep(Duration::from_millis(50));
    manager.poll();
    assert_eq!(server.count(), count);
    assert_eq!(std::fs::read_dir(workspace.path()).unwrap().count(), 0);
    assert!(
        manager
            .runtime_logs()
            .records("cloud-translation")
            .iter()
            .all(|record| !record.message.contains("fake-update-private-token"))
    );
}

/// Cross-origin authorization clears custom credentials and rejects preserved bodies; offline transport is explicit.
#[test]
#[ignore = "opt-in fixed SDK/WASM packages; rebuild sources: crates/plugin-runtime/tests/fixtures/community; see fixtures/packages/README.md"]
fn t26_allowed_cross_origin_redirects_strip_credentials_and_refuse_body_forwarding() {
    let other = endpoint::Server::new(|stream, _| endpoint::response(stream, 200, b"other"));
    let destination = other.url("/target");
    let server = endpoint::Server::new(move |stream, request| {
        let status = if request.headers.contains("/preserve ") {
            307
        } else {
            303
        };
        let _ = write!(
            stream,
            "HTTP/1.1 {status} Redirect\r\nLocation: {destination}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
        let _ = stream.flush();
    });
    let offline = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let unavailable = format!("http://localhost:{}", offline.local_addr().unwrap().port());
    let root = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(root.path().into(), environment(root.path())).unwrap();
    install(
        &mut manager,
        &package("cloud-translation", |manifest| {
            for origin in [&server.origin, &other.origin, &unavailable] {
                grant(manifest, origin);
            }
        }),
    );
    let mut request = http::Request::get(server.url("/preserve"));
    request.method = "POST".into();
    request.body = b"private request body".to_vec();
    let handle = resource(
        probe(
            &mut manager,
            "cloud-translation",
            http_op(http::Operation::Start { request }),
        )
        .unwrap(),
    );
    assert!(
        matches!(finish(&mut manager,"cloud-translation",&handle).last(),Some(http::Event::Failed{error}) if error.code==ErrorCode::PermissionDenied)
    );
    probe(
        &mut manager,
        "cloud-translation",
        Op::CloseResource { handle },
    )
    .unwrap();
    assert_eq!(other.count(), 0);
    for method in ["GET", "POST"] {
        let mut request = http::Request::get(server.url("/cross"));
        request.method = method.into();
        if method == "POST" {
            request.body = b"private post body".to_vec();
        }
        for name in ["Authorization", "Cookie", "X-Api-Key", "X-Private-Query"] {
            request.headers.push(http::Header {
                name: name.into(),
                value: "fake-cross-origin-credential".into(),
            });
        }
        let handle = resource(
            probe(
                &mut manager,
                "cloud-translation",
                http_op(http::Operation::Start { request }),
            )
            .unwrap(),
        );
        assert!(matches!(
            finish(&mut manager, "cloud-translation", &handle).last(),
            Some(http::Event::Finished { bytes: 5 })
        ));
        probe(
            &mut manager,
            "cloud-translation",
            Op::CloseResource { handle },
        )
        .unwrap();
    }
    assert!(other.captured.lock().unwrap().iter().all(
        |request| request.headers.starts_with("GET ")
            && !request.headers.contains("fake-cross-origin-credential")
            && request.body.is_empty()
    ));
    drop(offline);
    let handle = resource(
        probe(
            &mut manager,
            "cloud-translation",
            http_op(http::Operation::Start {
                request: http::Request::get(unavailable),
            }),
        )
        .unwrap(),
    );
    assert!(
        matches!(finish(&mut manager,"cloud-translation",&handle).last(),Some(http::Event::Failed{error}) if error.code==ErrorCode::OperationFailed)
    );
    probe(
        &mut manager,
        "cloud-translation",
        Op::CloseResource { handle },
    )
    .unwrap();
}
