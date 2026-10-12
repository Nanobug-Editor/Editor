//! Stable `lifecycle.timers` 1.0: disposable intervals and resettable delays, without queued tick backlogs.
//!
//! Minimum host: wire protocol 7, `api.base = ^1`, and negotiated `lifecycle.timers = ^1`.
//! No separate timer permission grants network/process authority. A required missing capability rejects
//! the package; optional absence must disable the feature or use a plugin fallback.
//!
//! With the `guest` feature, `start` returns an instance/scope-owned handle with a delay of 10 ms–one
//! day. At most 32 timers
//! are live per instance. Due callbacks receive `api::Notification::Timer { handle, sequence }`;
//! sequences increase once per delivered tick. Repeating timers coalesce missed ticks and schedule
//! from actual delivery. At most eight oldest due timers run per host poll, so all due timers get a
//! turn. Host polling provides no hard real-time guarantee. A callback's reset of another due timer
//! replaces that timer's current deadline; it cannot still deliver its superseded tick in the same poll.
//!
//! A one-shot handle is released before its callback and cannot be reset after firing. For debounce,
//! retain one pending one-shot handle and `reset` it for each new intent. Closing that handle cancels
//! the intent before it fires. For HTTP polling, retain a repeating timer and release it together with
//! the stream on completion/cancellation. Disable, trust revocation, failed activation and retirement
//! release timers and delegated origin authority; recovery never replays their external effects.
//!
//! Missing negotiation returns `CapabilityUnavailable`; preactivation returns `InvalidState`; out-of-
//! range delays return `InvalidRequest`; exhausted capacity returns `LimitExceeded`; released/foreign/
//! retired handles return `InvalidHandle`. Reset completes synchronously and retains ownership. Use
//! `api::guest::close_resource` to cancel and release a still-live timer; it does not undo prior ticks.
use crate::api::ResourceHandle;
#[cfg(feature = "guest")]
use crate::api::{ErrorCode, Failure};
use serde::{Deserialize, Serialize};

/// Intervals coalesce missed ticks; a reset invalidates the old pending delay rather than scheduling twice.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    /// Return api::Value::Resource after accepting one instance-owned pending timer.
    Start {
        /// From 10 through 86,400,000 milliseconds.
        delay_ms: u32,
        /// True coalesces intervals; false releases the handle before its single callback.
        repeat: bool,
    },
    /// Replace the current deadline and delay, returning Unit without scheduling a second timer.
    Reset {
        /// A live timer in this exact instance/scope; a fired one-shot is already invalid.
        handle: ResourceHandle,
        /// From 10 through 86,400,000 milliseconds, measured from this reset.
        delay_ms: u32,
    },
}

#[cfg(feature = "guest")]
/// Allocate one of at most 32 timers; durations are 10 ms to one day, and ticks arrive in Notification::Timer.
/// Returns a disposable current-incarnation handle; one-shot timers release it before callback delivery.
pub fn start(delay_ms: u32, repeat: bool) -> Result<ResourceHandle, Failure> {
    match crate::api::guest::request(crate::api::Operation::Timer {
        operation: Operation::Start { delay_ms, repeat },
    })? {
        crate::api::Value::Resource(handle) => Ok(handle),
        _ => Err(Failure::new(
            ErrorCode::InvalidRequest,
            "Unexpected timer acceptance",
        )),
    }
}
#[cfg(feature = "guest")]
/// Debounce one live timer, keeping its instance ownership. Release it through close_resource.
/// Replaces the next deadline even during another callback; invalid/retired handles cannot be restarted.
pub fn reset(handle: &ResourceHandle, delay_ms: u32) -> Result<(), Failure> {
    crate::api::guest::request(crate::api::Operation::Timer {
        operation: Operation::Reset {
            handle: handle.clone(),
            delay_ms,
        },
    })
    .map(|_| ())
}
