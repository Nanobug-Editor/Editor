//! One current UI intent owns one request; replacing it never replays or rolls back effects.
use super::*;

/// Starting a new intent failed; cleanup remains independently observable and retryable.
#[derive(Debug)]
pub struct IntentFailure {
    /// Parameter, allocation or cancellation failure from the original operation.
    pub cause: Failure,
    /// Cleanup receipts; unresolved resources remain in the original EditorIntent.
    pub cleanup: CleanupReport,
}

/// A locally ordered UI intent whose current result is still authenticated by the host.
///
/// Feed request notifications through `update`, and explicitly close when the feature ends.
/// Target/revision and side-effect admission remain host responsibilities. This type neither
/// clones nor serializes a live request, and dropping it performs no imported host calls.
#[derive(Default)]
#[must_use = "retain the intent to filter notifications and explicitly release pending work"]
pub struct EditorIntent {
    /// Never reused across replacement, even when a new request could not be started.
    generation: u64,
    /// Failed cleanup remains here; accepting=false prevents those results reaching the UI.
    task: Option<EditorTask>,
    /// Local ownership may outlive interest in a request after cancellation or cleanup failure.
    accepting: bool,
}
impl EditorIntent {
    /// Replace the current intent with `operation` and its original `timeout_ms` deadline.
    /// `mode` explicitly chooses cancellation semantics; failure never restores the old intent.
    /// Returns a monotonically increasing local generation, not a host authority token.
    pub fn replace(
        &mut self,
        operation: EditorOperation,
        timeout_ms: u32,
        mode: CancelMode,
    ) -> Result<u64, IntentFailure> {
        let invalid = |cause| IntentFailure {
            cause,
            cleanup: CleanupReport::default(),
        };
        if !(1..=300_000).contains(&timeout_ms) {
            return Err(invalid(Failure::new(
                ErrorCode::InvalidRequest,
                "Invalid editor deadline",
            )));
        }
        let bytes = serde_json::to_vec(&operation).map_err(|_| {
            invalid(Failure::new(
                ErrorCode::InvalidRequest,
                "Cannot encode editor operation",
            ))
        })?;
        check_request_size(bytes.len()).map_err(invalid)?;
        let generation = self.generation.checked_add(1).ok_or_else(|| {
            invalid(Failure::new(
                ErrorCode::LimitExceeded,
                "Intent generations exhausted",
            ))
        })?;
        self.accepting = false;
        self.generation = generation;
        let cancelled = self.task.as_ref().map(|task| task.cancel(mode));
        let cleanup = self.close();
        if !cleanup.is_complete() {
            return Err(IntentFailure {
                cause: Failure::new(
                    ErrorCode::OperationFailed,
                    "Previous intent cleanup is incomplete",
                ),
                cleanup,
            });
        }
        if let Some(Err(error)) = cancelled
            // A completed request can still own its handle until the terminal notification is sent.
            // Only after successful owned cleanup may either terminal race permit replacement.
            && !matches!(error.code, ErrorCode::InvalidHandle | ErrorCode::InvalidState)
        {
            return Err(IntentFailure {
                cause: error,
                cleanup,
            });
        }
        self.task = Some(
            EditorTask::start(operation, timeout_ms)
                .map_err(|cause| IntentFailure { cause, cleanup })?,
        );
        self.accepting = true;
        Ok(generation)
    }

    /// Accept only the current matching host receipt; terminal requests were already host-released.
    pub fn update(&mut self, notification: &Notification) -> Option<RequestUpdate> {
        if !self.accepting {
            return None;
        }
        let update = self.task.as_mut()?.update(notification)?;
        if update.is_terminal() {
            self.task = None;
            self.accepting = false;
        }
        Some(update)
    }

    /// Stop accepting results and request `mode`; the returned effect never claims rollback.
    /// Call `close` to release the retained request identity after observing this receipt.
    pub fn cancel(&mut self, mode: CancelMode) -> Result<CancellationEffect, Failure> {
        self.accepting = false;
        self.task
            .as_ref()
            .ok_or_else(|| Failure::new(ErrorCode::InvalidState, "No current intent"))?
            .cancel(mode)
    }

    /// Explicitly release a still-pending identity; completed requests need no second host call.
    pub fn close(&mut self) -> CleanupReport {
        self.accepting = false;
        let mut slot = self.task.as_ref().map(|task| task.handle.clone());
        let report = resources::release(&mut slot);
        if slot.is_none() {
            self.task = None;
        }
        report
    }
}
