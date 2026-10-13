//! A profile lease elects one GUI owner; authenticated loopback requests receive native-dispatch receipts.
use super::Admission;
use anyhow::Context as _;
use rust_i18n::t;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions, TryLockError},
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc as reply,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const FRAME_LIMIT: usize = 256 * 1024;
const DELIVERY_LIMIT: Duration = Duration::from_secs(12);
const IO_LIMIT: Duration = Duration::from_secs(2);
const IO_POLL: Duration = Duration::from_millis(100);
const RECEIPT_LIMIT: Duration = Duration::from_secs(10);

/// Private endpoint metadata is replaced only by the process holding the profile's live OS lease.
#[derive(Serialize, Deserialize)]
struct Endpoint {
    version: u32,
    port: u16,
    process: u32,
    token: [u8; 32],
}

/// Native path code units preserve legal Windows names without a lossy UTF-8 round trip.
#[derive(Serialize, Deserialize)]
struct NativePath {
    #[cfg(windows)]
    wide: Vec<u16>,
    #[cfg(unix)]
    bytes: Vec<u8>,
}
impl NativePath {
    fn encode(path: PathBuf) -> Self {
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt as _;
            Self {
                wide: path.as_os_str().encode_wide().collect(),
            }
        }
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt as _;
            Self {
                bytes: path.as_os_str().as_bytes().to_vec(),
            }
        }
    }
    fn decode(self) -> anyhow::Result<PathBuf> {
        #[cfg(windows)]
        let value = {
            use std::os::windows::ffi::OsStringExt as _;
            anyhow::ensure!(
                self.wide.len() <= 32767 && !self.wide.contains(&0),
                t!("projects.external_invalid_path")
            );
            std::ffi::OsString::from_wide(&self.wide)
        };
        #[cfg(unix)]
        let value = {
            use std::os::unix::ffi::OsStringExt as _;
            anyhow::ensure!(
                self.bytes.len() <= 65536 && !self.bytes.contains(&0),
                t!("projects.external_invalid_path")
            );
            std::ffi::OsString::from_vec(self.bytes)
        };
        let path = PathBuf::from(value);
        anyhow::ensure!(path.is_absolute(), t!("projects.external_absolute_path"));
        Ok(path)
    }
}

#[derive(Serialize, Deserialize)]
struct Request {
    token: [u8; 32],
    id: String,
    target: Option<NativePath>,
}

/// Accepted means the real application processed the request; trust or close decisions remain user's work.
#[derive(Clone, Serialize, Deserialize)]
pub(super) enum Response {
    Accepted,
    Rejected(String),
}

/// The socket worker cannot report success before this request reaches the application thread.
pub(super) struct Pending {
    pub(super) id: String,
    pub(super) target: Option<PathBuf>,
    expires: Instant,
    reply: reply::Sender<Response>,
}
impl Pending {
    /// The application must not open a delayed request after its dispatch receipt has already expired.
    pub(super) fn is_current(&self) -> bool {
        Instant::now() < self.expires
    }
    pub(super) fn complete(self, response: Response) {
        let _ = self.reply.send(response);
    }
}

/// The only GUI data owner retains the file lease and transport through the last-window shutdown.
pub(crate) struct Server {
    _lease: File,
    endpoint: PathBuf,
    stopped: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    requests: reply::Receiver<Pending>,
}
impl Server {
    /// Take one immutable incoming request without blocking the UI or waking it from a native thread.
    pub(super) fn next_request(&self) -> Option<Pending> {
        self.requests.try_recv().ok()
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        // Keep the lease while stopping the listener and removing its stale announcement.
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        let _ = fs::remove_file(&self.endpoint);
    }
}

/// Elect by a real OS lock, never by a PID file or an installer mutex; a crash releases the lease.
pub(super) fn enter(profile: &Path, target: Option<PathBuf>) -> anyhow::Result<Admission> {
    fs::create_dir_all(profile)?;
    let directory = profile.canonicalize()?.join("gui");
    fs::create_dir_all(&directory)?;
    let endpoint_path = directory.join("endpoint.json");
    let lease = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(directory.join("owner.lock"))?;
    let id = random_id()?;
    let deadline = Instant::now() + DELIVERY_LIMIT;
    let mut last_error = String::new();
    loop {
        match lease.try_lock() {
            Ok(()) => return start(lease, endpoint_path),
            Err(TryLockError::Error(error)) => return Err(error.into()),
            Err(TryLockError::WouldBlock) => {}
        }
        // During startup the owner may hold its lease before endpoint publication. Retry only within this limit.
        if let Ok(endpoint) = read_endpoint(&endpoint_path) {
            match deliver(&endpoint, id.clone(), target.clone(), deadline) {
                Ok(Response::Accepted) => return Ok(Admission::Delivered),
                Ok(Response::Rejected(reason)) => return Err(anyhow::anyhow!(reason)),
                Err(error) => last_error = error.to_string(),
            }
        }
        if Instant::now() >= deadline {
            anyhow::bail!(t!(
                "projects.external_delivery_failed",
                details = last_error
            ));
        }
        thread::sleep(Duration::from_millis(40));
    }
}

/// Damaged metadata remains bounded even if a file grows between opening and reading it.
fn read_endpoint(path: &Path) -> anyhow::Result<Endpoint> {
    let mut bytes = Vec::new();
    File::open(path)?.take(4097).read_to_end(&mut bytes)?;
    anyhow::ensure!(bytes.len() <= 4096, t!("projects.external_frame_quota"));
    Ok(serde_json::from_slice(&bytes)?)
}

/// Bind only loopback, publish a random session token inside the selected private profile, then admit requests.
fn start(lease: File, endpoint_path: PathBuf) -> anyhow::Result<Admission> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    listener.set_nonblocking(true)?;
    let mut token = [0u8; 32];
    getrandom::fill(&mut token)
        .map_err(|error| anyhow::anyhow!(t!("projects.external_randomness", details = error)))?;
    let endpoint = Endpoint {
        version: 1,
        port: listener.local_addr()?.port(),
        process: std::process::id(),
        token,
    };
    let mut announcement = tempfile::NamedTempFile::new_in(endpoint_path.parent().unwrap())?;
    announcement.write_all(&serde_json::to_vec(&endpoint)?)?;
    announcement.as_file().sync_all()?;
    announcement.persist(&endpoint_path)?;
    let (sender, requests) = reply::sync_channel(16);
    let stopped = Arc::new(AtomicBool::new(false));
    let stop = stopped.clone();
    let worker = thread::Builder::new()
        .name("nanobug-open-requests".into())
        .spawn(move || {
            while !stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((mut stream, address)) if address.ip().is_loopback() => {
                        let result = receive(&mut stream, &token, &sender, &stop);
                        if let Err(error) = result
                            && !stop.load(Ordering::Acquire)
                            && let Ok(mut frames) = FramedStream::new(
                                &mut stream,
                                Instant::now() + IO_LIMIT,
                                Some(&stop),
                            )
                        {
                            // Shutdown never waits for an error reply to an unresponsive peer.
                            let _ = frames.write(&Response::Rejected(error.to_string()));
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10))
                    }
                    Err(_) => break,
                    _ => {}
                }
            }
        })
        .context(t!("projects.external_worker").to_string())?;
    Ok(Admission::Primary(Server {
        _lease: lease,
        endpoint: endpoint_path,
        stopped,
        worker: Some(worker),
        requests,
    }))
}

/// Authenticate and bound the envelope before publishing an immutable request to GPUI.
fn receive(
    stream: &mut TcpStream,
    token: &[u8; 32],
    sender: &reply::SyncSender<Pending>,
    stopped: &AtomicBool,
) -> anyhow::Result<()> {
    let mut frames = FramedStream::new(stream, Instant::now() + IO_LIMIT, Some(stopped))?;
    let request: Request = frames.read()?;
    let difference = request
        .token
        .iter()
        .zip(token)
        .fold(0u8, |difference, (a, b)| difference | (a ^ b));
    anyhow::ensure!(
        difference == 0
            && request.id.len() == 32
            && request.id.bytes().all(|byte| byte.is_ascii_hexdigit()),
        t!("projects.external_invalid_session")
    );
    let target = request.target.map(NativePath::decode).transpose()?;
    let (reply, receipt) = reply::channel();
    let deadline = Instant::now() + RECEIPT_LIMIT;
    sender
        .try_send(Pending {
            id: request.id,
            target,
            expires: deadline,
            reply,
        })
        .map_err(|_| anyhow::anyhow!(t!("projects.external_queue_busy")))?;
    loop {
        match receipt.recv_timeout(Duration::from_millis(20)) {
            Ok(response) => {
                frames.deadline = Instant::now() + IO_LIMIT;
                return frames.write(&response);
            }
            Err(reply::RecvTimeoutError::Disconnected) => {
                anyhow::bail!(t!("projects.external_dispatch_exited"))
            }
            Err(reply::RecvTimeoutError::Timeout) => {}
        }
        anyhow::ensure!(
            !stopped.load(Ordering::Acquire) && Instant::now() < deadline,
            t!("projects.external_dispatch_expired")
        );
    }
}

/// A secondary grants foreground activation to the announced owner, and waits for its actual dispatch receipt.
fn deliver(
    endpoint: &Endpoint,
    id: String,
    target: Option<PathBuf>,
    deadline: Instant,
) -> anyhow::Result<Response> {
    anyhow::ensure!(
        endpoint.version == 1 && endpoint.port != 0,
        t!("projects.external_endpoint_unsupported")
    );
    let remaining = deadline.saturating_duration_since(Instant::now());
    anyhow::ensure!(
        !remaining.is_zero(),
        t!("projects.external_dispatch_expired")
    );
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, endpoint.port));
    let mut stream = TcpStream::connect_timeout(&address, IO_LIMIT.min(remaining))?;
    permit_foreground(endpoint.process);
    let mut frames =
        FramedStream::new(&mut stream, (Instant::now() + IO_LIMIT).min(deadline), None)?;
    frames.write(&Request {
        token: endpoint.token,
        id,
        target: target.map(NativePath::encode),
    })?;
    frames.deadline = (Instant::now() + RECEIPT_LIMIT).min(deadline);
    frames.read()
}

/// Each launch retry retains the same unpredictable request identity within its bounded delivery attempt.
fn random_id() -> anyhow::Result<String> {
    let mut value = [0u8; 16];
    getrandom::fill(&mut value)
        .map_err(|error| anyhow::anyhow!(t!("projects.external_randomness", details = error)))?;
    Ok(value.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Complete frames share an absolute deadline; trickle traffic cannot extend it or delay final shutdown.
struct FramedStream<'a> {
    stream: &'a mut TcpStream,
    deadline: Instant,
    stopped: Option<&'a AtomicBool>,
}
impl<'a> FramedStream<'a> {
    fn new(
        stream: &'a mut TcpStream,
        deadline: Instant,
        stopped: Option<&'a AtomicBool>,
    ) -> anyhow::Result<Self> {
        // Windows accepted sockets inherit nonblocking mode. Short blocking intervals allow cooperative cancellation.
        stream.set_nonblocking(false)?;
        stream.set_read_timeout(Some(IO_POLL))?;
        stream.set_write_timeout(Some(IO_POLL))?;
        Ok(Self {
            stream,
            deadline,
            stopped,
        })
    }

    fn check_active(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            Instant::now() < self.deadline
                && !self
                    .stopped
                    .is_some_and(|stop| stop.load(Ordering::Acquire)),
            t!("projects.external_dispatch_expired")
        );
        Ok(())
    }

    /// Reject oversized input before allocating and preserve one deadline across both header and body.
    fn read<T: serde::de::DeserializeOwned>(&mut self) -> anyhow::Result<T> {
        let mut length = [0u8; 4];
        self.read_bytes(&mut length)?;
        let length = u32::from_le_bytes(length) as usize;
        anyhow::ensure!(
            length > 0 && length <= FRAME_LIMIT,
            t!("projects.external_frame_quota")
        );
        let mut bytes = vec![0u8; length];
        self.read_bytes(&mut bytes)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    /// Encode bounded frames without interpreting a path as a command or trusting repeated socket timeouts.
    fn write(&mut self, value: &impl Serialize) -> anyhow::Result<()> {
        let bytes = serde_json::to_vec(value)?;
        anyhow::ensure!(
            bytes.len() <= FRAME_LIMIT,
            t!("projects.external_frame_quota")
        );
        self.write_bytes(&(bytes.len() as u32).to_le_bytes())?;
        self.write_bytes(&bytes)
    }

    fn read_bytes(&mut self, mut bytes: &mut [u8]) -> anyhow::Result<()> {
        while !bytes.is_empty() {
            self.check_active()?;
            match self.stream.read(bytes) {
                Ok(0) => return Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof).into()),
                Ok(count) => bytes = &mut bytes[count..],
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::TimedOut
                            | std::io::ErrorKind::WouldBlock
                            | std::io::ErrorKind::Interrupted
                    ) => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }

    fn write_bytes(&mut self, mut bytes: &[u8]) -> anyhow::Result<()> {
        while !bytes.is_empty() {
            self.check_active()?;
            match self.stream.write(bytes) {
                Ok(0) => return Err(std::io::Error::from(std::io::ErrorKind::WriteZero).into()),
                Ok(count) => bytes = &bytes[count..],
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::TimedOut
                            | std::io::ErrorKind::WouldBlock
                            | std::io::ErrorKind::Interrupted
                    ) => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }
}

#[cfg(windows)]
fn permit_foreground(process: u32) {
    #[link(name = "user32")]
    unsafe extern "system" {
        fn AllowSetForegroundWindow(process: u32) -> i32;
    }
    // Explorer may authorize its child to pass activation onward; failure never bypasses project identity or trust.
    let _ = unsafe { AllowSetForegroundWindow(process) };
}
#[cfg(not(windows))]
fn permit_foreground(_process: u32) {}
