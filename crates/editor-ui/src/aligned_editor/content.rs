//! Public immutable native-buffer identities detect preedit edits without another text model.
use super::MAX_SOURCE_BYTES;
use gpui_base::input::Rope;

/// An immutable admission snapshot, never edited or used as an alternate document/Undo source.
/// Rope's public clone is O(1) and shares COW leaves. Retaining those leaves prevents both in-place
/// mutation and pointer-address reuse while comparing the current public chunk identities.
pub(super) struct NativeContent {
    snapshot: Option<Rope>,
    bytes: usize,
}

impl NativeContent {
    pub fn new(native: &Rope) -> Self {
        Self {
            // Rejected sources have no row plan to invalidate. Do not retain a huge previous
            // buffer or traverse its chunks on every ordinary native focus/scroll notification.
            snapshot: (native.len() <= MAX_SOURCE_BYTES).then(|| native.clone()),
            bytes: native.len(),
        }
    }

    /// Compare chunk identities, not bytes: caret/scroll/layout notifications do O(chunks) work
    /// without hashing or copying megabytes. Same-byte-length IME replacements still change a
    /// retained COW leaf, whereas an unchanged buffer retains exactly the same chunk sequence.
    pub fn matches(&self, native: &Rope) -> bool {
        if self.bytes != native.len() {
            return false;
        }
        let Some(snapshot) = self.snapshot.as_ref() else {
            return true;
        };
        let mut previous = snapshot.chunks();
        let mut current = native.chunks();
        loop {
            match (previous.next(), current.next()) {
                (None, None) => return true,
                (Some(left), Some(right))
                    if left.len() == right.len() && left.as_ptr() == right.as_ptr() => {}
                _ => return false,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_identity_detects_equal_length_replacement_and_cancellation() {
        let mut native = Rope::from_str("before\n拼😀\nafter\n");
        let previous = NativeContent::new(&native);
        assert!(previous.matches(&native));
        assert!(previous.matches(&native.clone()));
        let start = "before\n".len();
        let end = start + "拼😀".len();
        native.remove(start..end);
        native.insert(start, "漢😃");
        assert_eq!(native.len(), previous.bytes);
        assert!(
            !previous.matches(&native),
            "same length is not the same candidate"
        );
        let replacement = NativeContent::new(&native);
        native.remove(start..end);
        native.insert(start, "拼😀");
        assert!(
            !replacement.matches(&native),
            "cancel/restore must invalidate the replacement"
        );
        assert_eq!(
            previous.snapshot.as_ref().unwrap().to_string(),
            "before\n拼😀\nafter\n"
        );
    }

    #[test]
    fn rejected_native_identity_does_not_retain_or_scan_oversize_buffers() {
        let mut native = Rope::from_str(&"x".repeat(MAX_SOURCE_BYTES + 1));
        let rejected = NativeContent::new(&native);
        assert!(rejected.snapshot.is_none());
        native.remove(0..1);
        native.insert(0, "y");
        assert!(
            rejected.matches(&native),
            "a refused same-size plan needs no byte/chunk scan"
        );
        native.remove(0..1);
        assert!(
            !rejected.matches(&native),
            "returning within admission bounds must retry the plan"
        );
        let admitted = NativeContent::new(&native);
        assert!(admitted.snapshot.is_some());
        assert!(admitted.matches(&native));
    }
}
