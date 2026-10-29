//! Observe session persistence separately from native file-navigation latency.
use std::time::Duration;

/// The durable result of one save; failures remain in the caller's original error channel.
#[derive(Clone, Copy)]
pub(super) enum SaveOutcome {
    /// The atomic replacement and disk synchronization completed successfully.
    Written,
    /// The stored snapshot already matches the current logical session.
    Unchanged,
}

/// Emit an opt-in tracing event for `elapsed` and `result`, without logging session contents.
pub(super) fn record(elapsed: Duration, result: &anyhow::Result<SaveOutcome>) {
    let outcome = match result {
        Ok(SaveOutcome::Written) => "written",
        Ok(SaveOutcome::Unchanged) => "unchanged",
        Err(_) => "failed",
    };
    tracing::debug!(
        target: "nanobug::performance",
        operation = "session.save",
        outcome,
        elapsed_ms = elapsed.as_secs_f64() * 1000.,
        "session persistence completed"
    );
    #[cfg(test)]
    TOTALS.with(|totals| {
        let mut current = totals.get();
        current.attempts += 1;
        current.elapsed_us += elapsed.as_micros() as u64;
        match result {
            Ok(SaveOutcome::Written) => current.writes += 1,
            Ok(SaveOutcome::Unchanged) => current.unchanged += 1,
            Err(_) => current.failures += 1,
        }
        totals.set(current);
    });
}

#[cfg(test)]
thread_local! {
    /// Native acceptance runs saves on its real UI thread; each child starts from zero.
    static TOTALS: std::cell::Cell<SaveTimings> = const { std::cell::Cell::new(SaveTimings {
        attempts: 0, writes: 0, unchanged: 0, failures: 0, elapsed_us: 0,
    }) };
}

/// Cumulative observations, used only to describe saves during an actual native interaction.
#[cfg(test)]
#[derive(Clone, Copy, Default, serde::Serialize)]
pub(crate) struct SaveTimings {
    /// Save requests received, including identical states and failures.
    attempts: u64,
    /// Successful atomic replacements of the profile file.
    writes: u64,
    /// Requests that found identical durable contents.
    unchanged: u64,
    /// Save errors returned through the unchanged caller interface.
    failures: u64,
    /// Total wall time in the save entry, including serialization and filesystem work.
    elapsed_us: u64,
}

#[cfg(test)]
impl SaveTimings {
    /// Return observations made after `earlier`, sampled on the same native UI thread.
    pub(crate) fn since(self, earlier: Self) -> Self {
        Self {
            attempts: self.attempts - earlier.attempts,
            writes: self.writes - earlier.writes,
            unchanged: self.unchanged - earlier.unchanged,
            failures: self.failures - earlier.failures,
            elapsed_us: self.elapsed_us - earlier.elapsed_us,
        }
    }
}

/// Snapshot this test thread's cumulative persistence observations without changing saved state.
#[cfg(test)]
pub(crate) fn save_timings() -> SaveTimings {
    TOTALS.with(std::cell::Cell::get)
}
