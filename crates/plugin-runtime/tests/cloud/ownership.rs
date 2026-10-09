//! Private namespaces, timer fairness and cloud resource retirement share production ownership.
use super::*;
/// Queue pressure and a blocked body both remain cancellable; disable/trust revocation free counted producers.
#[test]
#[ignore = "build both cloud packages using current host --plugin-package first"]
fn t30_backpressure_cancel_disable_and_trust_revoke_stop_workers() {
    let server = endpoint::Server::new(|stream, request| {
        if request.headers.contains("/pressure ") {
            let _ = stream.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Length: 16777216\r\nConnection: close\r\n\r\n",
            );
            for _ in 0..512 {
                if stream.write_all(&[b'x'; 32768]).is_err() {
                    break;
                }
            }
        } else {
            let _ = stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\n");
            let _ = stream.flush();
            let mut byte = [0];
            let _ = stream.read(&mut byte);
        }
    });
    let root = tempfile::tempdir().unwrap();
    let configuration = HttpConfiguration::default();
    let observer = configuration.clone();
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
    install(
        &mut manager,
        &package("cloud-translation", |manifest| {
            grant(manifest, &server.origin)
        }),
    );
    let baseline = manager.resource_count();
    let mut pressure = vec![];
    for _ in 0..4 {
        let mut request = http::Request::get(server.url("/pressure"));
        request.max_response_bytes = 16 * 1024 * 1024;
        pressure.push(resource(
            probe(
                &mut manager,
                "cloud-translation",
                http_op(http::Operation::Start { request }),
            )
            .unwrap(),
        ));
    }
    assert_eq!(observer.active_workers(), 4);
    assert_eq!(
        probe(
            &mut manager,
            "cloud-translation",
            http_op(http::Operation::Start {
                request: http::Request::get(server.url("/blocked"))
            })
        )
        .unwrap_err()
        .code,
        ErrorCode::LimitExceeded
    );
    for handle in pressure {
        probe(
            &mut manager,
            "cloud-translation",
            Op::CloseResource { handle },
        )
        .unwrap();
    }
    let until = Instant::now() + Duration::from_secs(1);
    while observer.active_workers() != 0 {
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(manager.resource_count(), baseline);
    for (index, path) in ["/pressure", "/blocked", "/pressure", "/blocked"]
        .into_iter()
        .enumerate()
    {
        let mut request = http::Request::get(server.url(path));
        request.max_response_bytes = 16 * 1024 * 1024;
        let handle = resource(
            probe(
                &mut manager,
                "cloud-translation",
                http_op(http::Operation::Start { request }),
            )
            .unwrap(),
        );
        std::thread::sleep(Duration::from_millis(120));
        assert_eq!(observer.active_workers(), 1);
        let started = Instant::now();
        match index {
            0 => {
                probe(
                    &mut manager,
                    "cloud-translation",
                    Op::CancelRequest {
                        handle: handle.clone(),
                        mode: api::CancelMode::TryTerminate,
                    },
                )
                .unwrap();
                assert!(
                    matches!(read(&mut manager,"cloud-translation",&handle),Some(http::Event::Failed{error}) if error.code==ErrorCode::Cancelled)
                );
                probe(
                    &mut manager,
                    "cloud-translation",
                    Op::CloseResource { handle },
                )
                .unwrap();
            }
            1 => {
                probe(
                    &mut manager,
                    "cloud-translation",
                    Op::CloseResource { handle },
                )
                .unwrap();
            }
            2 => {
                manager.disable("cloud-translation").unwrap();
            }
            _ => {
                manager.set_workspace_trust(false).unwrap();
            }
        }
        while observer.active_workers() != 0 {
            assert!(
                started.elapsed() < Duration::from_secs(1),
                "Cancelled producer did not release"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        if index < 2 {
            assert_eq!(manager.resource_count(), baseline);
        }
        if index == 2 {
            manager.enable("cloud-translation").unwrap();
        }
    }
    assert_eq!(manager.resource_count(), 0);
}

/// Fair coalesced scheduling reaches all 32 intervals, while resetting a one-shot delay prevents its old tick.
#[test]
#[ignore = "build both cloud packages using current host --plugin-package first"]
fn t29_timers_are_fair_bounded_disposable_and_debounce_resets_delay() {
    let server = endpoint::Server::new(|stream, _| endpoint::response(stream, 200, b"query"));
    let root = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(root.path().into(), environment(root.path())).unwrap();
    let translation = package("cloud-translation", |manifest| {
        grant(manifest, &server.origin);
        manifest["commands"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id":"timer-observations","title":"Timer observations"}))
    });
    install(&mut manager, &translation);
    install(
        &mut manager,
        &package("code-host-query", |manifest| {
            grant(manifest, &server.origin)
        }),
    );
    let baseline = manager.resource_count();
    let mut handles = vec![];
    for _ in 0..32 {
        handles.push(resource(
            probe(
                &mut manager,
                "cloud-translation",
                timer_op(timers::Operation::Start {
                    delay_ms: 10,
                    repeat: true,
                }),
            )
            .unwrap(),
        ));
    }
    assert_eq!(
        probe(
            &mut manager,
            "cloud-translation",
            timer_op(timers::Operation::Start {
                delay_ms: 10,
                repeat: true
            })
        )
        .unwrap_err()
        .code,
        ErrorCode::LimitExceeded
    );
    std::thread::sleep(Duration::from_millis(30));
    for _ in 0..4 {
        manager.poll();
    }
    manager
        .invoke_command("cloud-translation", "timer-observations", Json::Null)
        .unwrap();
    let ticks: std::collections::BTreeMap<String, u64> =
        serde_json::from_str(&text(&manager, "cloud-translation")).unwrap();
    assert_eq!(ticks.len(), 32);
    assert!(
        handles
            .iter()
            .all(|handle| ticks[&handle.resource.to_string()] > 0)
    );
    for handle in handles {
        probe(
            &mut manager,
            "cloud-translation",
            Op::CloseResource { handle },
        )
        .unwrap();
    }
    assert_eq!(manager.resource_count(), baseline);
    let handle = resource(
        probe(
            &mut manager,
            "cloud-translation",
            timer_op(timers::Operation::Start {
                delay_ms: 30,
                repeat: false,
            }),
        )
        .unwrap(),
    );
    probe(
        &mut manager,
        "cloud-translation",
        timer_op(timers::Operation::Reset {
            handle: handle.clone(),
            delay_ms: 200,
        }),
    )
    .unwrap();
    std::thread::sleep(Duration::from_millis(60));
    manager.poll();
    assert_eq!(manager.resource_count(), baseline + 1);
    std::thread::sleep(Duration::from_millis(180));
    manager.poll();
    assert_eq!(manager.resource_count(), baseline);
    assert_eq!(
        probe(
            &mut manager,
            "cloud-translation",
            timer_op(timers::Operation::Reset {
                handle,
                delay_ms: 100
            })
        )
        .unwrap_err()
        .code,
        ErrorCode::InvalidHandle
    );
    // Cancelling the pending intent must retire debounce itself, before any timer callback can start HTTP.
    for id in ["cloud-translation", "code-host-query"] {
        manager.invoke_command(id, "refresh", Json::Null).unwrap();
        manager.invoke_command(id, "cancel", Json::Null).unwrap();
        assert_eq!(manager.resource_count(), baseline);
        std::thread::sleep(Duration::from_millis(360));
        manager.poll();
        manager.poll();
        assert_eq!(server.count(), 0);
        assert_eq!(text(&manager, id), "Cancelled");
        assert_eq!(manager.resource_count(), baseline);
    }
}
