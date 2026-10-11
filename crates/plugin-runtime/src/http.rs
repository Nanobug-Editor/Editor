//! General HTTP uses the existing instance resource table; workers never receive guest filesystem access.
use plugin_protocol::{
    api::{ErrorCode, Failure},
    http::{Event, Header, Request},
};
use std::{
    collections::BTreeSet,
    net::TcpStream,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
#[cfg(test)]
mod tests;
mod transport;

/// Host-only proxy and trust configuration. Plugins cannot change proxy routing or TLS verification.
#[derive(Clone, Default)]
pub struct HttpConfiguration {
    /// Explicit HTTP(S) CONNECT proxy. None disables environment proxy inheritance.
    pub proxy: Option<String>,
    /// Explicit DER roots for a managed host; an empty list retains the standard WebPKI roots.
    pub trusted_roots: Vec<Vec<u8>>,
    workers: Arc<AtomicUsize>,
}
impl std::fmt::Debug for HttpConfiguration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpConfiguration")
            .field("proxy_configured", &self.proxy.is_some())
            .field("active_workers", &self.active_workers())
            .finish_non_exhaustive()
    }
}
impl HttpConfiguration {
    /// Configure an explicit host proxy while retaining the common producer budget.
    pub fn with_proxy(mut self, proxy: Option<String>) -> Self {
        self.proxy = proxy;
        self
    }
    /// Supply host-managed DER roots without disabling certificate chain or hostname verification.
    pub fn with_trusted_roots(mut self, roots: Vec<Vec<u8>>) -> Self {
        self.trusted_roots = roots;
        self
    }
    /// Number of live producers, including bounded DNS/connect work finishing after cancellation.
    pub fn active_workers(&self) -> usize {
        self.workers.load(Ordering::Acquire)
    }
}

/// Cancellation shuts down the actual socket through TLS/CONNECT wrappers and wakes queue waits.
#[derive(Debug)]
pub(super) struct Control {
    cancelled: AtomicBool,
    deadline: Instant,
    socket: Mutex<Option<Arc<TcpStream>>>,
}
impl Control {
    pub(super) fn check(&self) -> Result<(), Failure> {
        if self.cancelled.load(Ordering::Acquire) {
            Err(failure(ErrorCode::Cancelled, "HTTP request cancelled"))
        } else if Instant::now() >= self.deadline {
            Err(failure(
                ErrorCode::TimedOut,
                "HTTP request deadline exceeded",
            ))
        } else {
            Ok(())
        }
    }
    pub(super) fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
        if let Some(socket) = self.socket.lock().unwrap().as_ref() {
            let _ = socket.shutdown(std::net::Shutdown::Both);
        }
    }
}

/// Eight queued events bound resident body bytes to 256 KiB per stream; cancellation remains observable.
pub(super) struct Stream {
    pub(super) control: Arc<Control>,
    receiver: mpsc::Receiver<Event>,
    terminal: bool,
}
impl Stream {
    pub(super) fn read(&mut self) -> Result<Option<Event>, Failure> {
        if self.terminal {
            return Err(failure(ErrorCode::InvalidState, "HTTP stream is exhausted"));
        }
        if let Err(error) = self.control.check() {
            self.control.cancel();
            self.terminal = true;
            return Ok(Some(Event::Failed { error }));
        }
        match self.receiver.try_recv() {
            Ok(event) => {
                self.terminal = event.is_terminal();
                Ok(Some(event))
            }
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => {
                self.terminal = true;
                Ok(Some(Event::Failed {
                    error: failure(ErrorCode::OperationFailed, "HTTP producer stopped"),
                }))
            }
        }
    }
}
impl Drop for Stream {
    fn drop(&mut self) {
        self.control.cancel();
    }
}

/// Canonical exact origins are the target grants, independent from the base network permission.
pub(crate) fn origin(value: &str) -> Result<String, Failure> {
    let url = url::Url::parse(value)
        .map_err(|_| failure(ErrorCode::InvalidRequest, "Invalid HTTP target"))?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || url.host_str().is_none()
    {
        return Err(failure(ErrorCode::InvalidRequest, "Invalid HTTP target"));
    }
    Ok(url.origin().ascii_serialization())
}
pub(crate) fn grant(value: &str) -> Result<String, Failure> {
    origin(value).map(|value| format!("network.http:{value}"))
}
fn authorize(url: &str, permissions: &BTreeSet<String>) -> Result<(), Failure> {
    if !permissions.contains("network.http") || !permissions.contains(&grant(url)?) {
        return Err(failure(
            ErrorCode::PermissionDenied,
            "HTTP target origin is not granted",
        ));
    }
    Ok(())
}

/// Acceptance reserves one of four host-wide producer slots before any DNS or network side effect.
pub(super) fn start(
    request: Request,
    permissions: BTreeSet<String>,
    configuration: HttpConfiguration,
) -> Result<Stream, Failure> {
    request.validate()?;
    authorize(&request.url, &permissions)?;
    if let Some(proxy) = &configuration.proxy {
        origin(proxy)?;
    }
    configuration
        .workers
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
            (count < 4).then_some(count + 1)
        })
        .map_err(|_| failure(ErrorCode::LimitExceeded, "HTTP producer quota exceeded"))?;
    let control = Arc::new(Control {
        cancelled: AtomicBool::new(false),
        deadline: Instant::now() + Duration::from_millis(request.timeout_ms.into()),
        socket: Mutex::new(None),
    });
    let (sender, receiver) = mpsc::sync_channel(8);
    let worker_control = control.clone();
    let count = configuration.workers.clone();
    let reservation = WorkerReservation(count.clone());
    std::thread::Builder::new()
        .name("plugin-http".into())
        .spawn(move || {
            let _reservation = reservation;
            if let Err(error) = transfer(
                request,
                &permissions,
                &configuration,
                &worker_control,
                &sender,
            ) {
                let _ = send(&sender, Event::Failed { error }, &worker_control);
            }
        })
        .map_err(|_| failure(ErrorCode::OperationFailed, "Could not start HTTP worker"))?;
    Ok(Stream {
        control,
        receiver,
        terminal: false,
    })
}
struct WorkerReservation(Arc<AtomicUsize>);
impl Drop for WorkerReservation {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Blocking body reads are cancellable by the retained socket. Queue pressure never allocates another chunk.
fn send(
    sender: &mpsc::SyncSender<Event>,
    mut event: Event,
    control: &Control,
) -> Result<(), Failure> {
    loop {
        control.check()?;
        match sender.try_send(event) {
            Ok(()) => return Ok(()),
            Err(mpsc::TrySendError::Disconnected(_)) => {
                return Err(failure(ErrorCode::Cancelled, "HTTP consumer released"));
            }
            Err(mpsc::TrySendError::Full(value)) => {
                event = value;
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
}
fn transfer(
    mut request: Request,
    permissions: &BTreeSet<String>,
    configuration: &HttpConfiguration,
    control: &Arc<Control>,
    sender: &mpsc::SyncSender<Event>,
) -> Result<(), Failure> {
    use std::io::Read;
    use ureq::unversioned::transport::{ConnectProxyConnector, Connector, RustlsConnector};
    let mut redirects = 0;
    loop {
        control.check()?;
        authorize(&request.url, permissions)?;
        let proxy = configuration
            .proxy
            .as_ref()
            .map(|value| ureq::Proxy::new(value))
            .transpose()
            .map_err(|_| failure(ErrorCode::InvalidRequest, "Invalid host HTTP proxy"))?;
        let mut tls = ureq::tls::TlsConfig::builder();
        if !configuration.trusted_roots.is_empty() {
            tls = tls.root_certs(ureq::tls::RootCerts::Specific(Arc::new(
                configuration
                    .trusted_roots
                    .iter()
                    .map(|bytes| ureq::tls::Certificate::from_der(bytes).to_owned())
                    .collect(),
            )));
        }
        let config = ureq::Agent::config_builder()
            .proxy(proxy)
            .max_redirects(0)
            .max_redirects_will_error(false)
            .http_status_as_error(false)
            .max_response_header_size(32 * 1024)
            .input_buffer_size(32 * 1024)
            .output_buffer_size(16 * 1024)
            .timeout_global(Some(
                control.deadline.saturating_duration_since(Instant::now()),
            ))
            .timeout_connect(Some(Duration::from_secs(1)))
            .tls_config(tls.build())
            .build();
        let connector =
            ().chain(ConnectProxyConnector::default())
                .chain(transport::OwnedConnector(control.clone()))
                .chain(RustlsConnector::default());
        let agent = ureq::Agent::with_parts(
            config,
            connector,
            transport::BoundedResolver(control.clone()),
        );
        let mut wire = ureq::http::Request::builder()
            .method(request.method.as_str())
            .uri(&request.url);
        for header in &request.headers {
            wire = wire.header(&header.name, &header.value);
        }
        let wire = wire
            .body(request.body.as_slice())
            .map_err(|_| failure(ErrorCode::InvalidRequest, "Invalid HTTP request"))?;
        let mut response = agent
            .run(wire)
            .map_err(|error| transport_failure(control, &error))?;
        control.check()?;
        if matches!(response.status().as_u16(), 301 | 302 | 303 | 307 | 308)
            && request.max_redirects > 0
        {
            if redirects >= request.max_redirects {
                return Err(failure(
                    ErrorCode::LimitExceeded,
                    "HTTP redirect limit exceeded",
                ));
            }
            let location = response
                .headers()
                .get("location")
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| {
                    failure(
                        ErrorCode::OperationFailed,
                        "HTTP redirect has no valid Location",
                    )
                })?;
            let current = url::Url::parse(&request.url)
                .map_err(|_| failure(ErrorCode::InvalidRequest, "Invalid HTTP target"))?;
            let target = current
                .join(location)
                .map_err(|_| failure(ErrorCode::InvalidRequest, "Invalid HTTP redirect"))?;
            if current.scheme() == "https" && target.scheme() != "https" {
                return Err(failure(
                    ErrorCode::PermissionDenied,
                    "HTTPS redirect downgrade denied",
                ));
            }
            authorize(target.as_str(), permissions)?;
            if response.status().as_u16() == 303
                || (matches!(response.status().as_u16(), 301 | 302) && request.method == "POST")
            {
                request.method = "GET".into();
                request.body.clear();
            }
            if origin(current.as_str())? != origin(target.as_str())? {
                if !request.body.is_empty() {
                    return Err(failure(
                        ErrorCode::PermissionDenied,
                        "Cross-origin redirect cannot forward a request body",
                    ));
                }
                // Every caller-supplied header can carry a credential, including custom API-key headers.
                request.headers.clear();
            }
            request.url = target.to_string();
            request.validate()?;
            redirects += 1;
            continue;
        }
        let mut header_bytes = 0;
        let mut headers = vec![];
        for (name, value) in response.headers() {
            let value = value.to_str().map_err(|_| {
                failure(
                    ErrorCode::OperationFailed,
                    "Invalid response header encoding",
                )
            })?;
            header_bytes += name.as_str().len() + value.len();
            if header_bytes > 32 * 1024 || headers.len() >= 128 {
                return Err(failure(
                    ErrorCode::LimitExceeded,
                    "HTTP response headers exceeded quota",
                ));
            }
            headers.push(Header {
                name: name.to_string(),
                value: value.into(),
            });
        }
        if response
            .headers()
            .get("content-length")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
            .is_some_and(|bytes| bytes > request.max_response_bytes.into())
        {
            return Err(failure(
                ErrorCode::LimitExceeded,
                "HTTP response exceeded byte quota",
            ));
        }
        send(
            sender,
            Event::Headers {
                status: response.status().as_u16(),
                headers,
            },
            control,
        )?;
        let mut reader = response.body_mut().as_reader();
        let mut buffer = [0; 32768];
        let mut bytes = 0u64;
        loop {
            control.check()?;
            let count = reader
                .read(&mut buffer)
                .map_err(|_| transport_failure(control, &ureq::Error::ConnectionFailed))?;
            control.check()?;
            if count == 0 {
                send(sender, Event::Finished { bytes }, control)?;
                return Ok(());
            }
            bytes += count as u64;
            if bytes > request.max_response_bytes.into() {
                return Err(failure(
                    ErrorCode::LimitExceeded,
                    "HTTP response exceeded byte quota",
                ));
            }
            send(
                sender,
                Event::Chunk {
                    bytes: buffer[..count].to_vec(),
                },
                control,
            )?;
        }
    }
}
fn transport_failure(control: &Control, error: &ureq::Error) -> Failure {
    control.check().err().unwrap_or_else(|| {
        // Error categories are useful diagnostics; upstream payloads may contain a sensitive URI or header.
        let (code, message) = match error {
            ureq::Error::Timeout(_) => (ErrorCode::TimedOut, "HTTP transport deadline exceeded"),
            ureq::Error::Http(_) | ureq::Error::BadUri(_) => {
                (ErrorCode::InvalidRequest, "HTTP request encoding failed")
            }
            ureq::Error::HostNotFound => (ErrorCode::OperationFailed, "HTTP DNS lookup failed"),
            ureq::Error::Protocol(_) => (ErrorCode::OperationFailed, "HTTP protocol failed"),
            ureq::Error::Tls(_) | ureq::Error::Rustls(_) | ureq::Error::TlsRequired => (
                ErrorCode::OperationFailed,
                "HTTP TLS verification or handshake failed",
            ),
            ureq::Error::ConnectProxyFailed(_) => (
                ErrorCode::OperationFailed,
                "HTTP host proxy connection failed",
            ),
            ureq::Error::LargeResponseHeader(_, _) => (
                ErrorCode::LimitExceeded,
                "HTTP response headers exceeded quota",
            ),
            ureq::Error::TooManyRedirects | ureq::Error::RedirectFailed => (
                ErrorCode::OperationFailed,
                "HTTP redirect processing failed",
            ),
            _ => (ErrorCode::OperationFailed, "HTTP transport failed"),
        };
        failure(code, message)
    })
}
fn failure(code: ErrorCode, message: &str) -> Failure {
    Failure::new(code, message)
}
