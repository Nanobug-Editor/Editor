//! Reserve bounded transient Canvas draft bytes before cloning or replacement allocation.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
/// All native Canvas drafts share this final process bound, including pending acknowledgement copies.
static RESIDENT: AtomicUsize = AtomicUsize::new(0);
const LIMIT: usize = 64 * 1024 * 1024;

/// An exact-sized native draft allocation releases its reservation on every retirement/error path.
pub(super) struct Buffer(String);
impl Buffer {
    /// Reserve before allocating, without retaining an external source or giving it mutable ownership.
    pub(super) fn new(text: &str) -> Result<Self, Failure> {
        Self::parts(text, "", "")
    }
    /// Construct an entire bounded replacement before changing the currently visible draft.
    pub(super) fn replaced(&self, range: CanvasTextRange, text: &str) -> Result<Self, Failure> {
        Self::parts(&self.0[..range.start], text, &self.0[range.end..])
    }
    /// Fallible snapshot duplication keeps acknowledgement failures atomic.
    pub(super) fn snapshot(&self) -> Result<Self, Failure> {
        Self::new(&self.0)
    }
    /// Source is read-only to callers; all mutations reserve an independent candidate first.
    pub(super) fn text(&self) -> &str {
        &self.0
    }
    /// Charge actual UTF-8 payload length in the per-session pending budget too.
    pub(super) fn len(&self) -> usize {
        self.0.len()
    }
    /// Share the same checked path for insertion, deletion and initial source materialization.
    fn parts(before: &str, text: &str, after: &str) -> Result<Self, Failure> {
        let size = before
            .len()
            .checked_add(text.len())
            .and_then(|size| size.checked_add(after.len()))
            .filter(|size| *size <= 65536)
            .ok_or_else(|| limit("Canvas native text exceeds 64 KiB"))?;
        RESIDENT
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |used| {
                used.checked_add(size).filter(|sum| *sum <= LIMIT)
            })
            .map_err(|_| limit("Canvas native draft residency exceeds 64 MiB"))?;
        let mut value = String::with_capacity(size);
        value.push_str(before);
        value.push_str(text);
        value.push_str(after);
        Ok(Self(value))
    }
}
impl Drop for Buffer {
    fn drop(&mut self) {
        RESIDENT.fetch_sub(self.0.len(), Ordering::AcqRel);
    }
}
