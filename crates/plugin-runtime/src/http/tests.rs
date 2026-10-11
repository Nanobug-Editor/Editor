//! Real sockets verify the owned transport independently of WASM compilation, before the SDK integration suite.
use super::*;
#[path = "../../tests/support/cloud_http.rs"]
mod endpoint;
#[path = "../../tests/support/cloud_tls.rs"]
#[cfg(windows)]
mod tls;

/// Both an empty GET and a byte-bodied method finish without waiting for the global deadline.
#[test]
fn owned_http_socket_returns_status_chunks_and_finished() {
    let server = endpoint::Server::new(|stream, _| endpoint::response(stream, 200, b"body"));
    let grants = BTreeSet::from(["network.http".into(), grant(&server.origin).unwrap()]);
    for method in ["GET", "PATCH"] {
        let mut request = Request::get(server.url("/query"));
        request.method = method.into();
        request.timeout_ms = 1000;
        if method == "PATCH" {
            request.body = b"request body".to_vec();
        }
        let configuration = HttpConfiguration::default();
        let observer = configuration.clone();
        let mut stream = start(request, grants.clone(), configuration).unwrap();
        let mut events = vec![];
        loop {
            if let Some(event) = stream.read().unwrap() {
                let terminal = event.is_terminal();
                events.push(event);
                if terminal {
                    break;
                }
            } else {
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        assert!(
            matches!(events.last(), Some(Event::Finished { bytes: 4 })),
            "events={events:?} workers={} captures={:?}",
            observer.active_workers(),
            server.captured.lock().unwrap()
        );
    }
    assert_eq!(server.count(), 2);
    // The boolean describes chunked framing; fixed byte bodies carry their actual Content-Length.
    assert_eq!(
        *server.framing.lock().unwrap(),
        vec![("GET".into(), 0, false), ("PATCH".into(), 12, false)]
    );
    let captures = server.captured.lock().unwrap();
    assert!(captures[0].headers.starts_with("GET /query "));
    assert!(captures[0].body.is_empty());
    assert!(captures[1].headers.starts_with("PATCH /query "));
    assert_eq!(captures[1].body, b"request body");
}

/// The owned TCP socket underneath a TLS CONNECT tunnel interrupts a stalled body read.
#[test]
#[cfg(windows)]
fn owned_tls_connect_socket_cancellation_releases_its_worker() {
    let certificates = tls::Certificates::new();
    let configuration = certificates.server.clone();
    let server = endpoint::Server::start(
        "https",
        move |stream| {
            Box::new(rustls::StreamOwned::new(
                rustls::ServerConnection::new(configuration.clone()).unwrap(),
                stream,
            ))
        },
        |stream, _| {
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\n")
                .unwrap();
            stream.flush().unwrap();
            let mut byte = [0];
            let _ = stream.read(&mut byte);
        },
    );
    let proxy = tls::Proxy::new(&server.origin);
    let configuration = HttpConfiguration::default()
        .with_trusted_roots(vec![certificates.root])
        .with_proxy(Some(proxy.url.clone()));
    let observer = configuration.clone();
    let permissions = BTreeSet::from(["network.http".into(), grant(&server.origin).unwrap()]);
    let mut stream = start(
        Request::get(server.url("/stall")),
        permissions,
        configuration,
    )
    .unwrap();
    let until = Instant::now() + Duration::from_secs(3);
    loop {
        assert!(Instant::now() < until);
        if matches!(stream.read().unwrap(), Some(Event::Headers { .. })) {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(observer.active_workers(), 1);
    drop(stream);
    let until = Instant::now() + Duration::from_secs(1);
    while observer.active_workers() != 0 {
        assert!(
            Instant::now() < until,
            "Cancelled TLS count={}",
            observer.active_workers()
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(proxy.requests.lock().unwrap().len(), 1);
}
