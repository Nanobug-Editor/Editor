//! Owned loopback endpoints exercise real HTTP polling/deadlines without external credentials.
use std::{
    io::{Read, Write},
    net::{Shutdown, TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

/// Captures remain in memory and never print authorization values in failed diagnostics.
#[derive(Clone)]
pub struct Capture {
    /// Actual HTTP method emitted by the independent guest.
    pub method: String,
    /// Request target observed by the controlled endpoint.
    pub path: String,
    /// Original domain payload; no token is included in the test payload.
    pub body: Vec<u8>,
    authorization: Option<String>,
}
impl Capture {
    /// Compare a fake test token without returning credential bytes to assertion formatting.
    pub fn uses_token(&self, token: &str) -> bool {
        self.authorization.as_deref() == Some(format!("Bearer {token}").as_str())
    }

    /// An empty configured reference must omit authentication, including any default key.
    pub fn unauthenticated(&self) -> bool {
        self.authorization.is_none()
    }
}

/// Fixed declared ports keep the package's actual origin grants unchanged.
/// /held responds only after release; /stall waits for cancellation/deadline; all others reply in UTF-8.
pub struct Server {
    captures: Arc<Mutex<Vec<Capture>>>,
    released: Arc<AtomicBool>,
    stopped: Arc<AtomicBool>,
    sockets: Arc<Mutex<Vec<TcpStream>>>,
    worker: Option<JoinHandle<()>>,
}
impl Server {
    /// Bind an owned test endpoint and retain every accepted socket for deterministic Drop cleanup.
    pub fn new(port: u16) -> Self {
        let listener = TcpListener::bind(("127.0.0.1", port)).expect(
            "Lifecycle tests own loopback ports 18906/18907; stop earlier QA servers first",
        );
        listener.set_nonblocking(true).unwrap();
        let captures = Arc::new(Mutex::new(vec![]));
        let released = Arc::new(AtomicBool::new(false));
        let stopped = Arc::new(AtomicBool::new(false));
        let sockets = Arc::new(Mutex::new(vec![]));
        let (output, gate, stop, owned) = (
            captures.clone(),
            released.clone(),
            stopped.clone(),
            sockets.clone(),
        );
        let worker = thread::spawn(move || {
            let mut clients = vec![];
            while !stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((socket, _)) => {
                        // Accepted Windows sockets must explicitly leave the listener's nonblocking mode.
                        socket.set_nonblocking(false).unwrap();
                        socket
                            .set_read_timeout(Some(Duration::from_secs(3)))
                            .unwrap();
                        socket
                            .set_write_timeout(Some(Duration::from_secs(3)))
                            .unwrap();
                        owned.lock().unwrap().push(socket.try_clone().unwrap());
                        let (output, gate, stop) = (output.clone(), gate.clone(), stop.clone());
                        clients.push(thread::spawn(move || serve(socket, &output, &gate, &stop)));
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2));
                    }
                    Err(_) => break,
                }
            }
            for client in clients {
                client.join().unwrap();
            }
        });
        Self {
            captures,
            released,
            stopped,
            sockets,
            worker: Some(worker),
        }
    }

    /// Count completed request headers/bodies, independently of the guest's displayed status.
    pub fn count(&self) -> usize {
        self.captures.lock().unwrap().len()
    }

    /// Copy one bounded wire observation without exposing the server's synchronization state.
    pub fn capture(&self, index: usize) -> Capture {
        self.captures.lock().unwrap()[index].clone()
    }

    /// Release the held response only after the test has applied the new live polling interval.
    pub fn release(&self) {
        self.released.store(true, Ordering::Release);
    }
}

/// Parse only a bounded real HTTP request; credential bytes are retained solely in this memory record.
fn read_request(socket: &mut TcpStream) -> Option<Capture> {
    let mut header = vec![];
    let mut byte = [0];
    while !header.ends_with(b"\r\n\r\n") && header.len() < 32 * 1024 {
        if socket.read(&mut byte).unwrap_or(0) == 0 {
            return None;
        }
        header.push(byte[0]);
    }
    assert!(
        header.ends_with(b"\r\n\r\n"),
        "Unexpected unbounded request header"
    );
    let header = String::from_utf8_lossy(&header);
    let mut first = header.lines().next().unwrap_or("").split_whitespace();
    let method = first.next().unwrap_or("").to_string();
    let path = first.next().unwrap_or("").to_string();
    let mut length = 0;
    let mut authorization = None;
    for line in header.lines().skip(1) {
        if let Some((name, value)) = line.split_once(':') {
            match name.to_ascii_lowercase().as_str() {
                "content-length" => length = value.trim().parse().unwrap(),
                "authorization" => authorization = Some(value.trim().to_string()),
                _ => {}
            }
        }
    }
    assert!(length <= 256 * 1024, "Unexpected unbounded request body");
    let mut body = vec![0; length];
    socket.read_exact(&mut body).ok()?;
    Some(Capture {
        method,
        path,
        body,
        authorization,
    })
}

/// Hold, stall or answer on the actual transport; no host result or timing state is simulated.
fn serve(
    mut socket: TcpStream,
    output: &Mutex<Vec<Capture>>,
    gate: &AtomicBool,
    stop: &AtomicBool,
) {
    let Some(capture) = read_request(&mut socket) else {
        return;
    };
    let stall = capture.path == "/stall";
    let held = capture.path == "/held";
    {
        let mut captures = output.lock().unwrap();
        assert!(
            captures.len() < 64,
            "Unexpected repeated network side effects"
        );
        captures.push(capture);
    }
    if stall {
        let _ = socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\n");
        let _ = socket.flush();
        let _ = socket.read(&mut [0]);
        return;
    }
    while held && !gate.load(Ordering::Acquire) && !stop.load(Ordering::Acquire) {
        thread::sleep(Duration::from_millis(2));
    }
    if stop.load(Ordering::Acquire) {
        return;
    }
    let body = "hot 世界😀".as_bytes();
    let _ = write!(
        socket,
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = socket.write_all(body);
    let _ = socket.flush();
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        self.released.store(true, Ordering::Release);
        for socket in self.sockets.lock().unwrap().iter() {
            let _ = socket.shutdown(Shutdown::Both);
        }
        if let Some(worker) = self.worker.take() {
            worker.join().unwrap();
        }
    }
}
