//! One completion gate and deadline policy is shared by editor work and plugin service calls.
use plugin_protocol::api::{CancelMode, CancellationEffect, ErrorCode, Failure, RequestUpdate};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct Completion<T> {
    /// All clones share one observation receipt, independent from business result ownership.
    observation: Arc<Mutex<Option<crate::observability::Observation>>>,
    state: Arc<Mutex<(u64, RequestUpdate<T>, bool)>>,
    deadline: Instant,
    /// Downstream admissions observe cancellation immediately, before the manager's next tick.
    wait_alive: Arc<AtomicBool>,
    /// Instance retirement revokes queued native work even before the manager's next polling tick.
    pub(crate) lifetimes: Vec<Arc<AtomicBool>>,
    /// Ancestor waits observe their real status, including deadlines that elapsed without a poll.
    /// Completed sources stay valid; only cancellation seals their downstream admissions.
    wait_sources: Vec<Arc<dyn Fn() -> bool + Send + Sync>>,
}

#[cfg(test)]
mod observation_tests {
    use super::*;
    use crate::observability::{Owner, RuntimeObservations, Stage};

    /// Cancelling and completing the same actual gate cannot duplicate its terminal observation.
    #[test]
    fn completion_observation_records_one_terminal_during_race() {
        let sink = RuntimeObservations::default();
        sink.start_capture();
        let completion = Completion::<u8>::new(1000);
        completion.observe(sink.begin(
            Owner::new("fixture", "instance", "scope"),
            Stage::Request,
            None,
        ));
        let other = completion.clone();
        let thread = std::thread::spawn(move || other.finish(Ok(7)));
        let _ = completion.cancel(CancelMode::StopWaiting, ErrorCode::Cancelled);
        thread.join().unwrap();
        let snapshot = sink.snapshot(None);
        assert_eq!(snapshot.records.len(), 1);
        assert_eq!(snapshot.owners[0].calls["request"], 1);
        assert!(matches!(
            snapshot.records[0].outcome,
            crate::observability::Outcome::Completed | crate::observability::Outcome::Cancelled
        ));
    }
}
impl<T: Clone> Completion<T> {
    pub fn new(timeout_ms: u32) -> Self {
        Self {
            observation: Arc::new(Mutex::new(None)),
            state: Arc::new(Mutex::new((0, RequestUpdate::Accepted, false))),
            deadline: Instant::now() + Duration::from_millis(timeout_ms.into()),
            wait_alive: Arc::new(AtomicBool::new(true)),
            lifetimes: Vec::new(),
            wait_sources: Vec::new(),
        }
    }
    /// Begin and finish are idempotent even if a publication or completion arrives twice.
    pub fn begin(&self) -> bool {
        self.expire(Instant::now());
        let mut state = self.state.lock().unwrap();
        if !matches!(state.1, RequestUpdate::Accepted) {
            return false;
        }
        state.0 += 1;
        state.1 = RequestUpdate::Progress {
            message: "Executing".into(),
        };
        true
    }
    pub fn finish(&self, result: Result<T, Failure>) {
        self.expire(Instant::now());
        let mut state = self.state.lock().unwrap();
        if !state.1.is_terminal() {
            let outcome = if result.is_ok() {
                crate::observability::Outcome::Completed
            } else {
                crate::observability::Outcome::Failed
            };
            state.0 += 1;
            state.1 = RequestUpdate::Completed { result };
            drop(state);
            self.finish_observation(outcome);
        }
    }
    /// Background service preparation may report progress while retaining the original deadline.
    pub(crate) fn progress(&self, message: String) {
        self.expire(Instant::now());
        let mut state = self.state.lock().unwrap();
        if matches!(state.1, RequestUpdate::Progress { .. }) {
            state.0 += 1;
            state.1 = RequestUpdate::Progress { message };
        }
    }
    pub fn status(&self) -> RequestUpdate<T> {
        self.expire(Instant::now());
        self.state.lock().unwrap().1.clone()
    }
    pub fn update(&self) -> (u64, RequestUpdate<T>) {
        self.expire(Instant::now());
        let state = self.state.lock().unwrap();
        (state.0, state.1.clone())
    }
    /// Side effects cannot be rolled back by cancelling the wait for their result.
    pub fn enter_side_effect(&self) -> bool {
        self.expire(Instant::now());
        let mut state = self.state.lock().unwrap();
        if !matches!(state.1, RequestUpdate::Progress { .. }) {
            return false;
        }
        state.2 = true;
        true
    }
    pub fn expire(&self, now: Instant) {
        if self
            .lifetimes
            .iter()
            .any(|alive| !alive.load(Ordering::Acquire))
            || self.wait_sources.iter().any(|source_alive| !source_alive())
        {
            let _ = self.cancel(CancelMode::TryTerminate, ErrorCode::InvalidHandle);
            return;
        }
        if now >= self.deadline {
            let _ = self.cancel(CancelMode::TryTerminate, ErrorCode::TimedOut);
        }
    }
    pub fn cancel(
        &self,
        _mode: CancelMode,
        reason: ErrorCode,
    ) -> Result<CancellationEffect, Failure> {
        let mut state = self.state.lock().unwrap();
        if state.1.is_terminal() {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "Request already completed",
            ));
        }
        let effect = if state.2 {
            CancellationEffect::WaitingStopped
        } else {
            CancellationEffect::NotExecuted
        };
        state.0 += 1;
        state.1 = RequestUpdate::Cancelled { reason, effect };
        self.wait_alive.store(false, Ordering::Release);
        drop(state);
        self.finish_observation(if reason == ErrorCode::TimedOut {
            crate::observability::Outcome::TimedOut
        } else {
            crate::observability::Outcome::Cancelled
        });
        Ok(effect)
    }
    /// Bind a forwarded invocation to this wait without changing the launch's resource lifetime.
    pub(crate) fn wait_lifetime(&self) -> Arc<AtomicBool> {
        self.wait_alive.clone()
    }
    /// Observe an ancestor's original completion at worker and final-publication admission.
    /// Service ancestry is acyclic and bounded; observing a completed source does not reuse its
    /// expired transient deadline or clone its potentially large successful result.
    pub(crate) fn bind_wait_source<U: Clone + Send + 'static>(&mut self, source: &Completion<U>) {
        let source = source.clone();
        self.wait_sources.push(Arc::new(move || {
            source.expire(Instant::now());
            !matches!(
                &source.state.lock().unwrap().1,
                RequestUpdate::Cancelled { .. }
            )
        }));
    }
    /// Native preparation may consume its conditions after real effects. Reuse this completion's
    /// existing expiry gate instead of adding another request, deadline or cancellation state.
    pub(crate) fn bind_condition(&mut self, is_live: impl Fn() -> bool + Send + Sync + 'static) {
        self.wait_sources.push(Arc::new(is_live));
    }
    /// Forwarded activation keeps the original absolute deadline; ceil avoids rounding a live wait to zero.
    pub(crate) fn remaining_timeout_ms(&self) -> u32 {
        let remaining = self.deadline.saturating_duration_since(Instant::now());
        remaining
            .as_millis()
            .saturating_add(u128::from(remaining.subsec_nanos() % 1_000_000 != 0))
            .min(u32::MAX.into()) as u32
    }
    /// A forwarded operation cannot admit effects after the original invocation's deadline,
    /// even when activation and the outer service call return different payload types.
    pub(crate) fn constrain_deadline<U>(&mut self, parent: &Completion<U>) {
        self.deadline = self.deadline.min(parent.deadline);
    }
    /// Detach a caller's wait while an entered service continues to report owned native work.
    /// Before execution, cancellation seals the real gate; after entry, its receipt is still needed
    /// for cleanup and must not be discarded. The caller publishes its own cancelled projection.
    pub(crate) fn detach_wait(&self) -> Result<CancellationEffect, Failure> {
        self.expire(Instant::now());
        let mut state = self.state.lock().unwrap();
        if state.1.is_terminal() {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "Request already completed",
            ));
        }
        if state.2 {
            return Ok(CancellationEffect::WaitingStopped);
        }
        state.0 += 1;
        state.1 = RequestUpdate::Cancelled {
            reason: ErrorCode::Cancelled,
            effect: CancellationEffect::NotExecuted,
        };
        self.wait_alive.store(false, Ordering::Release);
        drop(state);
        self.finish_observation(crate::observability::Outcome::Cancelled);
        Ok(CancellationEffect::NotExecuted)
    }
    pub fn retire(&self) {
        let _ = self.cancel(CancelMode::TryTerminate, ErrorCode::Cancelled);
    }
    /// Attach a host-created observation before publishing this completion to other threads.
    pub(crate) fn observe(&self, observation: crate::observability::Observation) {
        *self
            .observation
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(observation);
    }
    /// Taking the shared receipt ensures cancellation/completion races record only one terminal result.
    fn finish_observation(&self, outcome: crate::observability::Outcome) {
        let observation = self
            .observation
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
        if let Some(observation) = observation {
            observation.finish(outcome);
        }
    }
}
