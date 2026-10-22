//! Stable `network.http` 1.0: owned, pull-based HTTP streams with explicit backpressure and cancellation.
//!
//! Minimum host: wire protocol 7, `api.base = ^1`, and negotiated `network.http = ^1`.
//! Installation must grant both `network.http` and the exact canonical `network.http:<origin>`,
//! including a nondefault port. Wildcard targets are unavailable; secret consent grants no network
//! authority. A required missing capability rejects the package; optional absence must disable the
//! feature or use a plugin fallback. These operations do not provide browser OAuth or AI-specific APIs.
//!
//! With the `guest` feature, `start` returns an instance/scope-owned stream. Poll `read` to release
//! capacity: `None` means
//! pending; Headers precede Chunks; one Finished/Failed event seals the stream, then reads return
//! `InvalidState`. Non-2xx responses remain ordinary status metadata. Close the handle using
//! `api::guest::close_resource` after the terminal event; terminal observation does not release it.
//! Retain a disposable timer to poll a pending stream rather than blocking a guest invocation.
//!
//! Four producer workers are shared by the host HTTP configuration. Each queue has eight events;
//! chunks are at most 32 KiB. Request bodies are at most 256 KiB; request headers have 64 entries and
//! 16 KiB total name/value bytes. Response headers have 128 entries and a 32 KiB complete wire block.
//! The requested response bound is 1 byte–16 MiB. Acceptance starts the 100–120,000 ms deadline,
//! including slow-consumer queue backpressure. Bounds apply without a trustworthy Content-Length.
//!
//! URLs are absolute HTTP(S), at most 4,096 bytes, without user-info, `@`, fragments or controls.
//! Host framing/connection/proxy headers are not caller-controlled. Every redirect rechecks the next
//! origin before connecting; at most five hops are allowed. HTTPS-to-HTTP downgrade is denied.
//! Cross-origin redirects strip all supplied headers and reject forwarding nonempty bodies. A 303,
//! or POST with 301/302, changes to GET with an empty body before this check. Delegated public services
//! intersect provider and original caller target permissions; they never borrow stronger grants.
//!
//! The host chooses TLS roots and proxy policy; guests cannot disable certificate/hostname checks or
//! inherit proxy environment variables. The Windows host accepts explicit `NANOBUG_HTTP_PROXY` for
//! HTTP(S) CONNECT, otherwise connects directly. WebPKI roots apply unless the embedding host supplies
//! explicit roots. Transport failures expose safe categories without URLs or credential payloads.
//!
//! `api::guest::cancel_request` revokes a stream and shuts down its socket; reading returns terminal
//! `Failed(Cancelled)` while the handle remains owned. CloseResource cancels and releases without a
//! readable terminal event. Disable, trust revocation and incarnation retirement revoke producers.
//! Socket IO checks revocation at most every 50 ms, including stalled TLS/CONNECT/body IO. OS DNS
//! is synchronous: cancellation/deadline is visible immediately, but an already-running DNS call may
//! occupy one of four workers until the OS returns. It cannot create a late connection or unbounded
//! helper threads. Cancellation never rolls back HTTP effects already observed by the remote system.
//!
//! Missing negotiation returns `CapabilityUnavailable`; missing origin/base consent returns
//! `PermissionDenied`; malformed bounds return `InvalidRequest`; quota/oversize returns `LimitExceeded`;
//! retired/foreign handles return `InvalidHandle`. Stream failures carry `TimedOut`, `Cancelled` or
//! `OperationFailed` as appropriate. Request/Header/Event Debug omit sensitive content.
use crate::api::{ErrorCode, Failure, ResourceHandle};
use serde::{Deserialize, Serialize};

#[cfg(feature = "guest")]
mod task;
#[cfg(feature = "guest")]
pub use task::{HttpBatch, HttpCancellation, HttpStartError, HttpTask};

/// One HTTP header. Duplicate response headers retain their order; request framing is host-owned.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Header {
    /// Case-insensitive ASCII header name; framing and proxy authentication remain host-owned.
    pub name: String,
    /// Printable ASCII plus horizontal tab; encode Unicode application content in the body or URL.
    pub value: String,
}
impl std::fmt::Debug for Header {
    /// Header values may be arbitrary API keys or response cookies; diagnostics show only their names.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Header")
            .field("name", &self.name)
            .field("value", &"<redacted>")
            .finish()
    }
}

/// Request limits count the complete transfer, including time waiting for a slow consumer.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    /// Absolute HTTP(S) URL without user-info or fragment. Each redirect requires its own origin grant.
    pub url: String,
    /// Uppercase ASCII method of at most 32 bytes; CONNECT and TRACE are forbidden.
    pub method: String,
    /// At most 64 entries and 16 KiB of combined names and values; duplicates retain their order.
    #[serde(default)]
    pub headers: Vec<Header>,
    /// At most 256 KiB. Response bytes arrive in chunks of at most 32 KiB.
    #[serde(default)]
    pub body: Vec<u8>,
    /// Between 100 ms and 120 seconds, measured from acceptance rather than first body byte.
    pub timeout_ms: u32,
    /// At most 16 MiB, independently enforced when Content-Length is absent or dishonest.
    pub max_response_bytes: u32,
    /// At most five. HTTPS-to-HTTP downgrade is always denied; cross-origin credentials are stripped.
    pub max_redirects: u8,
}
impl std::fmt::Debug for Request {
    /// Diagnostics never reveal URLs, headers or body content containing credentials.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpRequest")
            .field("method", &self.method)
            .field("body_bytes", &self.body.len())
            .finish_non_exhaustive()
    }
}
impl Request {
    /// Start a bounded GET; installation must grant `network.http` and the exact target origin.
    pub fn get(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            method: "GET".into(),
            headers: vec![],
            body: vec![],
            timeout_ms: 30000,
            max_response_bytes: 1024 * 1024,
            max_redirects: 3,
        }
    }
    /// Transport-independent bounds reject malformed/injected requests before resource allocation.
    pub fn validate(&self) -> Result<(), Failure> {
        let invalid = || Failure::new(ErrorCode::InvalidRequest, "Invalid HTTP request or bounds");
        if self.url.len() > 4096
            || !(self.url.starts_with("http://") || self.url.starts_with("https://"))
            || self.url.contains(['@', '#'])
            || self.url.chars().any(char::is_control)
            || self.method.is_empty()
            || self.method.len() > 32
            || !self
                .method
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b == b'-')
            || matches!(self.method.as_str(), "CONNECT" | "TRACE")
            || self.body.len() > 256 * 1024
            || !(100..=120000).contains(&self.timeout_ms)
            || !(1..=16 * 1024 * 1024).contains(&self.max_response_bytes)
            || self.max_redirects > 5
            || self.headers.len() > 64
        {
            return Err(invalid());
        }
        let mut bytes = 0;
        for header in &self.headers {
            bytes += header.name.len() + header.value.len();
            if header.name.is_empty()
                || !header
                    .name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
                || header
                    .value
                    .bytes()
                    .any(|b| b >= 127 || (b < 32 && b != b'\t'))
                || matches!(
                    header.name.to_ascii_lowercase().as_str(),
                    "host"
                        | "content-length"
                        | "transfer-encoding"
                        | "connection"
                        | "proxy-authorization"
                        | "upgrade"
                )
            {
                return Err(invalid());
            }
        }
        if bytes > 16 * 1024 {
            return Err(invalid());
        }
        Ok(())
    }
}

/// A stream is exhausted exactly once. Non-2xx statuses are ordinary metadata, not transport errors.
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    /// Response metadata precedes body chunks; non-2xx status is not a transport failure.
    Headers { status: u16, headers: Vec<Header> },
    /// At most 32 KiB of raw bytes; multibyte text may cross chunk boundaries.
    Chunk { bytes: Vec<u8> },
    /// Terminal success with total raw bytes; the owned handle still requires release.
    Finished { bytes: u64 },
    /// Terminal transport, revocation or bounds failure, with a safe typed category.
    Failed { error: Failure },
}
impl std::fmt::Debug for Event {
    /// Stream content is explicit SDK data, never an implicit debug/log payload.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Headers { status, headers } => f
                .debug_struct("Headers")
                .field("status", status)
                .field("count", &headers.len())
                .finish(),
            Self::Chunk { bytes } => f
                .debug_struct("Chunk")
                .field("bytes", &bytes.len())
                .finish(),
            Self::Finished { bytes } => f.debug_struct("Finished").field("bytes", bytes).finish(),
            Self::Failed { error } => f.debug_struct("Failed").field("error", error).finish(),
        }
    }
}
impl Event {
    /// A terminal event seals the stream; close its owned handle after observing it.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Finished { .. } | Self::Failed { .. })
    }
}

/// Reading one event releases queue capacity. `None` means pending, never an empty successful body.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    /// Validate and accept a transfer, returning api::Value::Resource rather than its final response.
    Start { request: Request },
    /// Pull one event as api::Value::Http; None is pending, not successful completion.
    Read { handle: ResourceHandle },
}

#[cfg(feature = "guest")]
/// Start a HTTP stream; errors include missing capability/grant, invalid target and worker quota.
/// The Request fixes the whole-transfer deadline and bounds; the returned handle belongs to this incarnation.
pub fn start(request: Request) -> Result<ResourceHandle, Failure> {
    request.validate()?;
    match crate::api::guest::request(crate::api::Operation::Http {
        operation: Operation::Start { request },
    })? {
        crate::api::Value::Resource(handle) => Ok(handle),
        _ => Err(Failure::new(
            ErrorCode::InvalidRequest,
            "Unexpected HTTP acceptance",
        )),
    }
}
#[cfg(feature = "guest")]
/// Poll one bounded event; use a disposable timer for retries and release the stream when finished.
/// Returns None while pending, one ordered event when available, and InvalidState after its terminal event.
pub fn read(handle: &ResourceHandle) -> Result<Option<Event>, Failure> {
    match crate::api::guest::request(crate::api::Operation::Http {
        operation: Operation::Read {
            handle: handle.clone(),
        },
    })? {
        crate::api::Value::Http(event) => Ok(event),
        _ => Err(Failure::new(
            ErrorCode::InvalidRequest,
            "Unexpected HTTP stream event",
        )),
    }
}
