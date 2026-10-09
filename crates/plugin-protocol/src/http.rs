//! Stable `network.http` 1.0: owned, pull-based HTTP streams with explicit backpressure and cancellation.
use crate::api::{ErrorCode, Failure, ResourceHandle};
use serde::{Deserialize, Serialize};

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
    Headers { status: u16, headers: Vec<Header> },
    Chunk { bytes: Vec<u8> },
    Finished { bytes: u64 },
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
    Start { request: Request },
    Read { handle: ResourceHandle },
}

#[cfg(feature = "guest")]
/// Start a HTTP stream; errors include missing capability/grant, invalid target and worker quota.
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
