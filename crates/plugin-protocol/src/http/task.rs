//! A pull-based HTTP task owns its stream and polling timer without buffering business content.
use super::*;
use crate::api::guest::{self, CleanupReport, resources::release};
use crate::api::{CancelMode, CancellationEffect, Negotiated, Notification};

/// Ordered data from one bounded callback; cleanup failures do not rewrite the transfer outcome.
#[derive(Debug, Default)]
#[must_use = "consume transfer events and inspect any cleanup failures"]
pub struct HttpBatch {
    /// At most eight events or 256 KiB of chunk bytes; UTF-8 may cross chunk boundaries.
    pub events: Vec<Event>,
    /// Independent release receipts, including resources retained by the HttpTask for retry.
    pub cleanup: CleanupReport,
}

/// Partial startup preserves the original error and, when necessary, the cleanup owner.
pub struct HttpStartError {
    /// Failure before the complete task was ready; never silently replaced by cleanup failure.
    pub cause: Failure,
    /// What was released after that failure.
    pub cleanup: CleanupReport,
    /// Present only when cleanup failed; explicitly close this task again or let host retirement revoke it.
    pub recovery: Option<HttpTask>,
}
impl std::fmt::Debug for HttpStartError {
    /// Do not turn host messages or request data into implicit debug output.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpStartError")
            .field("code", &self.cause.code)
            .field("cleanup_pending", &self.cleanup.pending.len())
            .field("recovery", &self.recovery.is_some())
            .finish()
    }
}

/// Cancellation and resource release are distinct; neither rolls back an external HTTP effect.
#[derive(Debug)]
#[must_use = "inspect both cancellation and cleanup outcomes"]
pub struct HttpCancellation {
    /// Actual host cancellation receipt, or its typed failure.
    pub effect: Result<CancellationEffect, Failure>,
    /// Closing the stream/timer is attempted even when cancellation failed.
    pub cleanup: CleanupReport,
}

/// A bounded stream plus one polling timer. Feed notifications into `on_event`, and explicitly
/// close when the feature ends. Drop does not call the host; instance retirement remains the fallback.
#[must_use = "retain and explicitly close or cancel the HTTP task"]
pub struct HttpTask {
    /// Actual HTTP ownership; a terminal event does not release this slot in the host.
    stream: Option<ResourceHandle>,
    /// A separate disposable periodic timer drives bounded pulls.
    timer: Option<ResourceHandle>,
    /// Closed/terminal tasks ignore future timer receipts even if cleanup must still be retried.
    accepting: bool,
}
impl HttpTask {
    /// Start `request` with `poll_ms` in 10..=86,400,000 after checking negotiated features.
    /// Permission and resource checks remain host-owned. Partial failure returns a recovery owner.
    pub fn start(api: &Negotiated, request: Request, poll_ms: u32) -> Result<Self, HttpStartError> {
        let fail = |cause| HttpStartError {
            cause,
            cleanup: CleanupReport::default(),
            recovery: None,
        };
        for capability in ["network.http", "lifecycle.timers"] {
            if !api.capabilities.contains_key(capability) {
                return Err(fail(Failure::new(
                    ErrorCode::CapabilityUnavailable,
                    "HTTP task capability unavailable",
                )));
            }
        }
        if !(10..=86_400_000).contains(&poll_ms) {
            return Err(fail(Failure::new(
                ErrorCode::InvalidRequest,
                "Invalid polling interval",
            )));
        }
        let stream = start(request).map_err(fail)?;
        let mut task = Self {
            stream: Some(stream),
            timer: None,
            accepting: true,
        };
        match crate::timers::start(poll_ms, true) {
            Ok(timer) => {
                task.timer = Some(timer);
                Ok(task)
            }
            Err(cause) => {
                let cleanup = task.close();
                let recovery = (!cleanup.is_complete()).then_some(task);
                Err(HttpStartError {
                    cause,
                    cleanup,
                    recovery,
                })
            }
        }
    }

    /// Pull only for this task's timer; unmatched or post-terminal receipts do no host work.
    /// Stops at pending, eight events or 256 KiB. Raw bytes and non-2xx status remain ordinary data.
    pub fn on_event(&mut self, notification: &Notification) -> HttpBatch {
        let mut batch = HttpBatch::default();
        if !self.accepting
            || !matches!(notification, Notification::Timer { handle, .. } if self.timer.as_ref() == Some(handle))
        {
            return batch;
        }
        let Some(stream) = self.stream.clone() else {
            return batch;
        };
        let mut bytes = 0;
        while batch.events.len() < 8 && bytes < 256 * 1024 {
            let event = match read(&stream) {
                Ok(Some(event)) => event,
                Ok(None) => break,
                Err(error) => Event::Failed { error },
            };
            if let Event::Chunk { bytes: chunk } = &event {
                bytes += chunk.len();
            }
            let terminal = event.is_terminal();
            batch.events.push(event);
            if terminal {
                batch.cleanup = self.close();
                break;
            }
        }
        batch
    }

    /// Stop waiting with `mode` and close both resources; expose the actual effect separately.
    pub fn cancel(&mut self, mode: CancelMode) -> HttpCancellation {
        self.accepting = false;
        let effect = self
            .stream
            .as_ref()
            .ok_or_else(|| Failure::new(ErrorCode::InvalidState, "HTTP task is closed"))
            .and_then(|stream| guest::cancel_request(stream, mode));
        HttpCancellation {
            effect,
            cleanup: self.close(),
        }
    }

    /// Stop polling before closing the stream; failures remain owned for explicit retry.
    pub fn close(&mut self) -> CleanupReport {
        self.accepting = false;
        let mut report = release(&mut self.timer);
        report.merge(release(&mut self.stream));
        report
    }
}
