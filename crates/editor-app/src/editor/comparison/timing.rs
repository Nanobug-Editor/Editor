//! Optional source/admission timing excludes document text, paths and credentials from diagnostics.
use std::{sync::OnceLock, time::Instant};

/// Set NANOBUG_TRACE_DIFF to log synchronous stage durations; normal use reads one cached flag.
pub(in crate::editor) struct Span {
    stage: &'static str,
    started: Option<Instant>,
}
impl Span {
    /// Start a named stage only when diagnostic timing was explicitly enabled for this process.
    pub(in crate::editor) fn new(stage: &'static str) -> Self {
        static ENABLED: OnceLock<bool> = OnceLock::new();
        Self {
            stage,
            started: ENABLED
                .get_or_init(|| std::env::var_os("NANOBUG_TRACE_DIFF").is_some())
                .then(Instant::now),
        }
    }
}
impl Drop for Span {
    fn drop(&mut self) {
        if let Some(started) = self.started {
            eprintln!(
                "diff-stage {} {:.3}ms",
                self.stage,
                started.elapsed().as_secs_f64() * 1000.
            );
        }
    }
}
