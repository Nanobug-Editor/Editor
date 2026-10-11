//! Shared, bounded loopback transport for actual marketplace installation and native consent tests.
use std::{
    io::{Read, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

/// Bounded fixture server lets tests observe attempted package downloads and catalog outages.
pub(crate) struct Server {
    pub(crate) url: String,
    pub(crate) downloads: Arc<AtomicUsize>,
    pub(crate) requests: Arc<AtomicUsize>,
    pub(crate) offline: Arc<AtomicBool>,
    pub(crate) pause: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    task: Option<std::thread::JoinHandle<()>>,
}
impl Server {
    /// Serve the frozen declarative catalog and ZIP through the same bounded HTTP adapter.
    pub(crate) fn new() -> Self {
        Self::from_bytes(
            include_bytes!("../../../plugin-runtime/tests/fixtures/marketplace/catalog.json"),
            include_bytes!("../../../plugin-runtime/tests/fixtures/marketplace/notes.zip").to_vec(),
        )
    }
    /// Serve caller-owned controlled catalog/ZIP bytes, returning a loopback base URL and request counters.
    /// Only the first listing's download URL changes; no guest code is executed by this transport.
    pub(crate) fn from_bytes(catalog_bytes: &[u8], package: Vec<u8>) -> Self {
        Self::with_statistics(catalog_bytes, package, None)
    }
    /// Serve an optional measured statistics snapshot independently of the reviewed download catalog.
    /// This keeps real catalog/outage/ZIP transport while exercising the native statistics display.
    pub(crate) fn with_statistics(
        catalog_bytes: &[u8],
        package: Vec<u8>,
        statistics: Option<Vec<u8>>,
    ) -> Self {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let mut catalog: serde_json::Value = serde_json::from_slice(catalog_bytes).unwrap();
        catalog["plugins"][0]["versions"][0]["url"] = format!("{url}/notes.zip").into();
        let catalog = serde_json::to_vec(&catalog).unwrap();
        let downloads = Arc::new(AtomicUsize::new(0));
        let requests = Arc::new(AtomicUsize::new(0));
        let request_count = requests.clone();
        let offline = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let pause = Arc::new(AtomicBool::new(false));
        let paused = pause.clone();
        let (count, failed, stopped) = (downloads.clone(), offline.clone(), stop.clone());
        let task = std::thread::spawn(move || {
            while !stopped.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((mut socket, _)) => {
                        // Windows accepts may inherit the listener's nonblocking flag; requests arrive later.
                        socket.set_nonblocking(false).unwrap();
                        request_count.fetch_add(1, Ordering::AcqRel);
                        socket
                            .set_read_timeout(Some(Duration::from_secs(2)))
                            .unwrap();
                        let mut request = [0; 4096];
                        let count_read = socket.read(&mut request).unwrap();
                        let request = String::from_utf8_lossy(&request[..count_read]);
                        let zip = request.contains("/notes.zip");
                        let stats = request.contains("/stats.json");
                        let bytes: &[u8] = if zip {
                            count.fetch_add(1, Ordering::AcqRel);
                            &package
                        } else if stats && statistics.is_some() {
                            statistics.as_ref().unwrap()
                        } else {
                            &catalog
                        };
                        let status = if !zip && failed.load(Ordering::Acquire) {
                            "503 Unavailable"
                        } else {
                            "200 OK"
                        };
                        let _ = write!(
                            socket,
                            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            bytes.len()
                        );
                        while zip
                            && paused.load(Ordering::Acquire)
                            && !stopped.load(Ordering::Acquire)
                        {
                            std::thread::sleep(Duration::from_millis(5));
                        }
                        let _ = socket.write_all(bytes);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5))
                    }
                    Err(error) => panic!("fixture server: {error}"),
                }
            }
        });
        Self {
            url,
            downloads,
            requests,
            offline,
            pause,
            stop,
            task: Some(task),
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.task.take().unwrap().join().unwrap();
    }
}
