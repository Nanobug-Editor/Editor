//! Map immutable semantic byte anchors; actual display geometry stays in Base's viewport locator.
use std::ops::Range;

/// Unchanged intervals preserve exact byte distance. Inside an edited interval, preserve its
/// relative byte position; the caller clips the result to a real UTF-8/CRLF caret boundary.
pub(super) fn offset(offset: usize, side: usize, changes: &[[Range<usize>; 2]]) -> usize {
    let mut previous = [0usize; 2];
    for pair in changes {
        let (from, to) = (&pair[side], &pair[1 - side]);
        if offset < from.start {
            return previous[1 - side] + offset.saturating_sub(previous[side]);
        }
        if offset <= from.end {
            return if from.is_empty() {
                to.end
            } else {
                to.start
                    + ((offset.saturating_sub(from.start) as u64 * to.len() as u64)
                        / from.len() as u64) as usize
            };
        }
        previous = [pair[0].end, pair[1].end];
    }
    previous[1 - side] + offset.saturating_sub(previous[side])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Insertions, deletions and unequal paired changes preserve the unchanged suffix both ways.
    #[test]
    fn native_code_sync_mapping_preserves_unchanged_intervals_and_bounds_changed_ranges() {
        let changes = [[2..2, 2..6], [8..12, 12..14]];
        assert_eq!(offset(1, 0, &changes), 1);
        assert_eq!(offset(2, 0, &changes), 6);
        assert_eq!(offset(5, 0, &changes), 9);
        assert_eq!(offset(10, 0, &changes), 13);
        assert_eq!(offset(15, 0, &changes), 17);
        assert_eq!(offset(17, 1, &changes), 15);
        assert_eq!(offset(3, 1, &changes), 2);
        assert_eq!(offset(7, 0, &[]), 7);
    }
}
