//! Native TLS, service-source authority and credential profile aliases use the same real cloud guests.
use super::*;
use plugin_runtime::plugin_protocol::{api::RequestUpdate, service};

/// Only a declared method's caller grants reach the provider; its Token and private roots never do.
#[test]
#[ignore = "build both cloud packages using current host --plugin-package first"]
fn t28_services_cannot_borrow_network_targets_secrets_or_private_roots() {
    let server = endpoint::Server::new(|stream, _| {
        let _ = stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\n");
        let _ = stream.flush();
        let mut byte = [0];
        let _ = stream.read(&mut byte);
    });
    let other = endpoint::Server::new(|stream, _| endpoint::response(stream, 200, b"other"));
    let credentials = SecretCleanup::new("cloud-translation", "api-token");
    let configuration = HttpConfiguration::default();
    let observer = configuration.clone();
    let mut manager = Manager::open_with_resources(
        credentials.root.path().into(),
        environment(credentials.root.path()),
        true,
        HostResources {
            http: configuration,
            ..Default::default()
        },
    )
    .unwrap();
    let shape = |permissions: Vec<String>| json!({"parameters":{"type":"string","max_bytes":4096},"result":{"type":"string","max_bytes":8192},"permissions":permissions});
    let methods = json!({
        "private":shape(vec![]), "borrow-base":shape(vec![]), "borrow-target":shape(vec!["network.http".into()]),
        "allowed":shape(vec!["network.http".into(),format!("network.http:{}",server.origin)])
    });
    install(
        &mut manager,
        &package("cloud-translation", |manifest| {
            grant(manifest, &server.origin);
            grant(manifest, &other.origin);
            manifest["api"]["required"]["plugin.services"] = json!("^1");
            manifest["permissions"]
                .as_array_mut()
                .unwrap()
                .push(json!("services.call"));
            manifest["plugin_services"] =
                json!({"provides":{"example.cloud":{"version":"1.0.0","methods":methods}}});
        }),
    );
    install(
        &mut manager,
        &package("code-host-query", |manifest| {
            grant(manifest, &server.origin);
            // Ordinary network access does not require secrets consent; the caller cannot acquire a Token.
            manifest["permissions"]
                .as_array_mut()
                .unwrap()
                .retain(|permission| permission != "secrets");
            manifest["api"]["required"]["plugin.services"] = json!("^1");
            manifest["permissions"]
                .as_array_mut()
                .unwrap()
                .push(json!("services.call"));
            manifest["plugin_services"] =
                json!({"requires":{"example.cloud":{"version":"^1","methods":methods}}});
        }),
    );
    assert_eq!(
        probe(
            &mut manager,
            "code-host-query",
            secret_op(secrets::Operation::Read {
                key: "api-token".into()
            })
        )
        .unwrap_err()
        .code,
        ErrorCode::PermissionDenied
    );
    probe(
        &mut manager,
        "cloud-translation",
        secret_op(secrets::Operation::Write {
            key: "api-token".into(),
            bytes: b"fake-service-private-token".to_vec(),
        }),
    )
    .unwrap();
    let reference = resource(
        probe(
            &mut manager,
            "code-host-query",
            Op::Service {
                operation: service::Operation::Open {
                    contract: "example.cloud".into(),
                },
            },
        )
        .unwrap(),
    );
    for (method, operation) in [
        (
            "borrow-base",
            http_op(http::Operation::Start {
                request: http::Request::get(server.url("/base")),
            }),
        ),
        (
            "borrow-target",
            http_op(http::Operation::Start {
                request: http::Request::get(server.url("/target")),
            }),
        ),
        (
            "allowed",
            http_op(http::Operation::Start {
                request: http::Request::get(other.url("/provider")),
            }),
        ),
        (
            "private",
            secret_op(secrets::Operation::Read {
                key: "api-token".into(),
            }),
        ),
        ("private", Op::OpenData),
    ] {
        assert_eq!(
            call(&mut manager, &reference, method, operation)
                .unwrap_err()
                .code,
            ErrorCode::PermissionDenied
        );
    }
    assert_eq!(server.count(), 0);
    assert_eq!(other.count(), 0);
    let stream = resource(
        call(
            &mut manager,
            &reference,
            "allowed",
            http_op(http::Operation::Start {
                request: http::Request::get(server.url("/allowed")),
            }),
        )
        .unwrap(),
    );
    assert_eq!(
        probe(
            &mut manager,
            "code-host-query",
            http_op(http::Operation::Read { handle: stream })
        )
        .unwrap_err()
        .code,
        ErrorCode::InvalidHandle
    );
    wait(&mut manager, |_| {
        observer.active_workers() == 1 && server.count() == 1
    });
    assert!(
        !server.captured.lock().unwrap()[0]
            .headers
            .contains("fake-service-private-token")
    );
    manager.disable("code-host-query").unwrap();
    manager.poll();
    let until = Instant::now() + Duration::from_secs(1);
    while observer.active_workers() != 0 {
        assert!(
            Instant::now() < until,
            "Retired service producer count={}",
            observer.active_workers()
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(manager.resource_count(), 1);
}

/// Service output is a bounded declared string containing the guest's ordinary API result.
fn call(
    manager: &mut Manager,
    reference: &api::ResourceHandle,
    method: &str,
    operation: Op,
) -> Result<Value, api::Failure> {
    let accepted = probe(
        manager,
        "code-host-query",
        Op::Service {
            operation: service::Operation::Call {
                reference: reference.clone(),
                method: method.into(),
                arguments: Json::String(serde_json::to_string(&operation).unwrap()),
                timeout_ms: 3000,
            },
        },
    )?;
    let Value::Accepted(handle) = accepted else {
        panic!("Expected service acceptance")
    };
    wait(manager, |manager| {
        serde_json::from_str::<RequestUpdate<Json>>(&text(manager, "code-host-query"))
            .is_ok_and(|update| update.is_terminal())
    });
    let RequestUpdate::Completed { result } =
        serde_json::from_str::<RequestUpdate<Json>>(&text(manager, "code-host-query")).unwrap()
    else {
        panic!("Expected service completion")
    };
    let result = result.and_then(|value| serde_json::from_str(value.as_str().unwrap()).unwrap());
    // Existing service requests release their handle before delivering the terminal notification.
    assert_eq!(
        probe(manager, "code-host-query", Op::CloseResource { handle })
            .unwrap_err()
            .code,
        ErrorCode::InvalidHandle
    );
    result
}

/// Real TLS validates both trust and hostname; a host-selected CONNECT proxy never sees origin credentials.
#[test]
#[ignore = "build cloud packages first; requires existing PowerShell 7 for ephemeral TLS certificates"]
fn t26_tls_connect_proxy_verification_downgrade_and_stalled_tls_cancellation() {
    let certificates = tls::Certificates::new();
    let plain = endpoint::Server::new(|stream, _| endpoint::response(stream, 200, b"plain"));
    let downgrade = plain.url("/downgrade");
    let configuration = certificates.server.clone();
    let secure = endpoint::Server::start(
        "https",
        move |stream| {
            Box::new(rustls::StreamOwned::new(
                rustls::ServerConnection::new(configuration.clone()).unwrap(),
                stream,
            ))
        },
        move |stream, request| {
            if request.headers.contains("/stall ") {
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\n",
                );
                let _ = stream.flush();
                let mut byte = [0];
                let _ = stream.read(&mut byte);
            } else if request.headers.contains("/downgrade ") {
                let _ = write!(
                    stream,
                    "HTTP/1.1 302 Found\r\nLocation: {downgrade}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
                let _ = stream.flush();
            } else {
                endpoint::response(stream, 200, b"verified TLS");
            }
        },
    );
    let proxy = tls::Proxy::new(&secure.origin);
    let configuration = HttpConfiguration::default()
        .with_trusted_roots(vec![certificates.root])
        .with_proxy(Some(proxy.url.clone()));
    let observer = configuration.clone();
    let root = tempfile::tempdir().unwrap();
    let mut manager = Manager::open_with_resources(
        root.path().into(),
        environment(root.path()),
        true,
        HostResources {
            http: configuration,
            ..Default::default()
        },
    )
    .unwrap();
    let wrong = secure.origin.replace("localhost", "127.0.0.1");
    install(
        &mut manager,
        &package("cloud-translation", |manifest| {
            for origin in [&secure.origin, &wrong, &plain.origin] {
                grant(manifest, origin);
            }
        }),
    );
    let mut request = http::Request::get(secure.url("/verified"));
    request.headers.push(http::Header {
        name: "Authorization".into(),
        value: "Bearer fake-tls-token".into(),
    });
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
        Some(http::Event::Finished { bytes: 12 })
    ));
    probe(
        &mut manager,
        "cloud-translation",
        Op::CloseResource { handle },
    )
    .unwrap();
    assert!(
        secure.captured.lock().unwrap()[0]
            .headers
            .contains("fake-tls-token")
    );
    assert!(
        proxy
            .requests
            .lock()
            .unwrap()
            .iter()
            .all(|headers| headers.starts_with("CONNECT ") && !headers.contains("fake-tls-token"))
    );
    for (url, expected) in [
        (format!("{wrong}/wrong-host"), ErrorCode::OperationFailed),
        (secure.url("/downgrade"), ErrorCode::PermissionDenied),
    ] {
        let handle = resource(
            probe(
                &mut manager,
                "cloud-translation",
                http_op(http::Operation::Start {
                    request: http::Request::get(url),
                }),
            )
            .unwrap(),
        );
        assert!(
            matches!(finish(&mut manager,"cloud-translation",&handle).last(),Some(http::Event::Failed{error}) if error.code==expected)
        );
        probe(
            &mut manager,
            "cloud-translation",
            Op::CloseResource { handle },
        )
        .unwrap();
    }
    assert_eq!(plain.count(), 0);
    let handle = resource(
        probe(
            &mut manager,
            "cloud-translation",
            http_op(http::Operation::Start {
                request: http::Request::get(secure.url("/stall")),
            }),
        )
        .unwrap(),
    );
    wait(&mut manager, |_| {
        secure
            .captured
            .lock()
            .unwrap()
            .iter()
            .any(|request| request.headers.contains("/stall "))
    });
    probe(
        &mut manager,
        "cloud-translation",
        Op::CloseResource { handle },
    )
    .unwrap();
    let until = Instant::now() + Duration::from_secs(1);
    while observer.active_workers() != 0 {
        assert!(
            Instant::now() < until,
            "Cancelled TLS producer count={}",
            observer.active_workers()
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    drop(manager);
    // An ordinary host without the explicit QA root rejects this ephemeral chain.
    let root = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(root.path().into(), environment(root.path())).unwrap();
    install(
        &mut manager,
        &package("cloud-translation", |manifest| {
            grant(manifest, &secure.origin)
        }),
    );
    let handle = resource(
        probe(
            &mut manager,
            "cloud-translation",
            http_op(http::Operation::Start {
                request: http::Request::get(secure.url("/untrusted")),
            }),
        )
        .unwrap(),
    );
    assert!(
        matches!(finish(&mut manager,"cloud-translation",&handle).last(),Some(http::Event::Failed{error}) if error.code==ErrorCode::OperationFailed)
    );
    assert!(
        !secure
            .captured
            .lock()
            .unwrap()
            .iter()
            .any(|request| request.headers.contains("/untrusted "))
    );
}

/// Relative, absolute and Windows extended spellings share a system key; another profile has its own namespace.
#[test]
#[ignore = "build cloud packages first; uses an independent real Windows Credential test namespace"]
fn t27_profile_aliases_preserve_credentials_and_other_profiles_are_isolated() {
    let credentials = SecretCleanup::new("cloud-translation", "api-token");
    let profile = credentials.root.path();
    let mut manager = Manager::open(profile.into(), environment(profile)).unwrap();
    install(&mut manager, &package("cloud-translation", |_| {}));
    probe(
        &mut manager,
        "cloud-translation",
        secret_op(secrets::Operation::Write {
            key: "api-token".into(),
            bytes: b"fake-profile-alias-token".to_vec(),
        }),
    )
    .unwrap();
    drop(manager);
    let current = std::env::current_dir().unwrap();
    let current = current.components().collect::<Vec<_>>();
    let target = profile.components().collect::<Vec<_>>();
    let common = current
        .iter()
        .zip(&target)
        .take_while(|(left, right)| left == right)
        .count();
    let relative = std::iter::repeat_n(std::path::PathBuf::from(".."), current.len() - common)
        .chain(
            target[common..]
                .iter()
                .map(|component| component.as_os_str().into()),
        )
        .collect::<std::path::PathBuf>();
    for spelling in [
        relative,
        profile.to_path_buf(),
        profile.canonicalize().unwrap(),
    ] {
        let mut manager = Manager::open(spelling, environment(profile)).unwrap();
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
    }
    let other = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(other.path().into(), environment(other.path())).unwrap();
    install(&mut manager, &package("cloud-translation", |_| {}));
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
    assert_eq!(text(&manager, "cloud-translation"), "secret-missing");
}
