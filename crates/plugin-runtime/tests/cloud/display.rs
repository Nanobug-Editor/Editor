//! Legal response byte limits must preserve complete Unicode-safe views, raw caches and menu actions.
use super::*;
use plugin_runtime::plugin_protocol::ui;

/// Both real SDK consumers remain active at the body limit, including replacement expansion of invalid UTF-8.
#[test]
#[ignore = "opt-in fixed SDK/WASM packages; rebuild sources: crates/plugin-runtime/tests/fixtures/community; see fixtures/packages/README.md"]
fn t25_response_limit_and_invalid_utf8_preserve_full_display_cache_and_cancel() {
    let near = "世界😀".repeat(6553).into_bytes();
    let mut maximum = "世".repeat(21845).into_bytes();
    maximum.push(b'\n');
    let invalid = vec![0xff; 65536];
    let bodies = std::sync::Arc::new([near, maximum, invalid]);
    let responses = bodies.clone();
    let server = endpoint::Server::new(move |stream, request| {
        if request.headers.contains("/stall ") {
            let _ = stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\nx");
            let _ = stream.flush();
            let mut byte = [0];
            let _ = stream.read(&mut byte);
        } else {
            let index = if request.headers.contains("/near ") {
                0
            } else if request.headers.contains("/maximum ") {
                1
            } else {
                2
            };
            endpoint::response(stream, 200, &responses[index]);
        }
    });
    let credentials = SecretCleanup::new("cloud-translation", "api-token");
    let root = credentials.root.path();
    let mut manager = Manager::open(root.into(), environment(root)).unwrap();
    for id in ["cloud-translation", "code-host-query"] {
        install(
            &mut manager,
            &package(id, |manifest| grant(manifest, &server.origin)),
        );
        // The subsequent large-body command is the actual entry; catalog installation stays cold.
        demand(&mut manager, id, DemandReason::Command("query".into()));
    }
    let baseline = manager.resource_count();
    for id in ["cloud-translation", "code-host-query"] {
        for (path, body) in ["near", "maximum", "invalid"]
            .into_iter()
            .zip(bodies.iter())
        {
            manager
                .invoke_command(id, "query", json!({"url":server.url(&format!("/{path}"))}))
                .unwrap();
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                manager.poll();
                let instance = manager.live.get(id);
                assert!(
                    instance
                        .is_some_and(|instance| instance.views.contains_key("cloud")
                            && instance.error.is_none()),
                    "{id} retired for a legal {}-byte body: installed={:?}; instance={:?}",
                    body.len(),
                    manager.installed[id].error,
                    instance.and_then(|instance| instance.error.as_ref())
                );
                if text(&manager, id).contains("Complete") {
                    break;
                }
                assert!(Instant::now() < deadline, "Response did not finish");
                std::thread::sleep(Duration::from_millis(10));
            }
            assert_eq!(
                text(&manager, id),
                format!(
                    "HTTP 200 · {} bytes · Complete\n{}",
                    body.len(),
                    String::from_utf8_lossy(body)
                )
            );
            // A complete result above the viewport must expose the public scrolling contract,
            // so native users can reach every fragment rather than only the first visible block.
            assert!(
                matches!(
                    manager.live[id].views["cloud"].root.kind,
                    ui::Kind::Scroll { .. }
                ),
                "{id} does not expose a scrollable large response"
            );
            assert_eq!(manager.resource_count(), baseline);
            manager
                .invoke_command(id, "read-cache", Json::Null)
                .unwrap();
            assert_eq!(text(&manager, id), String::from_utf8_lossy(body));
            assert!(
                matches!(
                    manager.live[id].views["cloud"].root.kind,
                    ui::Kind::Scroll { .. }
                ),
                "{id} does not expose a scrollable complete cache"
            );
            // Compare raw bytes through the public SDK's owned cache handle, rather than native file inspection.
            let cache = resource(
                probe(
                    &mut manager,
                    id,
                    private_op(storage::Operation::Open {
                        area: storage::Area::Cache,
                    }),
                )
                .unwrap(),
            );
            let Value::Bytes(cached) = probe(
                &mut manager,
                id,
                Op::ReadFile {
                    handle: cache.clone(),
                    path: "last-query.txt".into(),
                },
            )
            .unwrap() else {
                panic!("Expected complete cached bytes")
            };
            assert_eq!(&cached, body);
            probe(&mut manager, id, Op::CloseResource { handle: cache }).unwrap();
        }
        let count = server.count();
        manager
            .invoke_command(id, "query", json!({"url":server.url("/stall")}))
            .unwrap();
        wait(&mut manager, |_| server.count() > count);
        manager.invoke_command(id, "cancel", Json::Null).unwrap();
        assert_eq!(text(&manager, id), "Cancelled");
        assert_eq!(manager.resource_count(), baseline);
    }
}
