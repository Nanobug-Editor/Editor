//! Stable `lifecycle.timers` 1.0: disposable intervals and resettable delays, without queued tick backlogs.
use crate::api::ResourceHandle;
#[cfg(feature = "guest")]
use crate::api::{ErrorCode, Failure};
use serde::{Deserialize, Serialize};

/// Intervals coalesce missed ticks; a reset invalidates the old pending delay rather than scheduling twice.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    Start {
        delay_ms: u32,
        repeat: bool,
    },
    Reset {
        handle: ResourceHandle,
        delay_ms: u32,
    },
}

#[cfg(feature = "guest")]
/// Allocate one of at most 32 timers; durations are 10 ms to one day, and ticks arrive in Notification::Timer.
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
pub fn reset(handle: &ResourceHandle, delay_ms: u32) -> Result<(), Failure> {
    crate::api::guest::request(crate::api::Operation::Timer {
        operation: Operation::Reset {
            handle: handle.clone(),
            delay_ms,
        },
    })
    .map(|_| ())
}
