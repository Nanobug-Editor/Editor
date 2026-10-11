//! Deterministic loopback service used by actual SDK consumers; no plugin receives a network helper.
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

#[derive(Clone, Debug)]
pub struct Capture {
    pub headers: String,
    pub body: Vec<u8>,
}
pub trait Connection: Read + Write + Send {}
impl<T: Read + Write + Send> Connection for T {}
pub struct Server {
    pub origin: String,
    pub captured: Arc<Mutex<Vec<Capture>>>,
    /// Safe request-framing receipts are available even when a body intentionally stalls.
    pub framing: Arc<Mutex<Vec<(String, usize, bool)>>>,
    stop: Arc<AtomicBool>,
    sockets: Arc<Mutex<Vec<TcpStream>>>,
    thread: Option<JoinHandle<()>>,
}
impl Server {
    /// The handler can fragment bodies, block a response or issue a redirect through a real socket.
    pub fn new(handler: impl Fn(&mut dyn Connection, &Capture) + Send + Sync + 'static) -> Self {
        Self::start("http", |stream| Box::new(stream), handler)
    }
    pub fn start(
        scheme: &str,
        connection: impl Fn(TcpStream) -> Box<dyn Connection> + Send + Sync + 'static,
        handler: impl Fn(&mut dyn Connection, &Capture) + Send + Sync + 'static,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        listener.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let sockets = Arc::new(Mutex::new(vec![]));
        let captured = Arc::new(Mutex::new(vec![]));
        let framing = Arc::new(Mutex::new(vec![]));
        let (stopped, owned, output) = (stop.clone(), sockets.clone(), captured.clone());
        let handler = Arc::new(handler);
        let connection = Arc::new(connection);
        let frames = framing.clone();
        let thread = thread::spawn(move || {
            let mut clients = vec![];
            while !stopped.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        // Windows accepted sockets inherit a nonblocking listener's mode; handlers own blocking IO.
                        stream.set_nonblocking(false).unwrap();
                        stream
                            .set_read_timeout(Some(Duration::from_secs(3)))
                            .unwrap();
                        stream
                            .set_write_timeout(Some(Duration::from_secs(3)))
                            .unwrap();
                        owned.lock().unwrap().push(stream.try_clone().unwrap());
                        let (handler, connection, output, frames) = (
                            handler.clone(),
                            connection.clone(),
                            output.clone(),
                            frames.clone(),
                        );
                        clients.push(thread::spawn(move || {
                            let mut stream = connection(stream);
                            let mut header = vec![];
                            let mut byte = [0];
                            while header.len() < 32768 && !header.ends_with(b"\r\n\r\n") {
                                if stream.read(&mut byte).unwrap_or(0) == 0 {
                                    return;
                                }
                                header.push(byte[0]);
                            }
                            let headers = String::from_utf8_lossy(&header).into_owned();
                            let len = headers
                                .lines()
                                .find_map(|line| {
                                    line.to_ascii_lowercase()
                                        .strip_prefix("content-length:")
                                        .and_then(|value| value.trim().parse::<usize>().ok())
                                })
                                .unwrap_or(0);
                            frames.lock().unwrap().push((
                                headers.split_whitespace().next().unwrap_or("").into(),
                                len,
                                headers
                                    .to_ascii_lowercase()
                                    .contains("transfer-encoding: chunked"),
                            ));
                            let mut body = vec![0; len.min(256 * 1024)];
                            if stream.read_exact(&mut body).is_err() {
                                return;
                            }
                            let capture = Capture { headers, body };
                            output.lock().unwrap().push(capture.clone());
                            handler(&mut *stream, &capture);
                        }));
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2))
                    }
                    Err(_) => break,
                }
            }
            for client in clients {
                client.join().unwrap();
            }
        });
        Self {
            origin: format!("{scheme}://localhost:{port}"),
            captured,
            framing,
            stop,
            sockets,
            thread: Some(thread),
        }
    }
    pub fn url(&self, path: &str) -> String {
        format!("{}{path}", self.origin)
    }
    pub fn count(&self) -> usize {
        self.captured.lock().unwrap().len()
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        for stream in self.sockets.lock().unwrap().iter() {
            let _ = stream.shutdown(std::net::Shutdown::Both);
        }
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}

pub fn response(stream: &mut dyn Connection, status: u16, body: &[u8]) {
    let _ = write!(
        stream,
        "HTTP/1.1 {status} Fixture\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(body);
    let _ = stream.flush();
}
