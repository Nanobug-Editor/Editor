//! Native UTF-16 ranges refer to the same original extended graphemes as Canvas UTF-8 layouts.
use super::{CanvasTextRange, grapheme_boundaries, is_grapheme_boundary};
use std::ops::Range;

/// Convert native UTF-16 `range` in original `text` to legal Canvas UTF-8 grapheme boundaries.
/// Nonempty replacements expand outward; a collapsed/reversed caret snaps to the preceding edge.
/// Out-of-range endpoints clamp to end-of-text. No text, native font or editable state is retained.
pub fn canvas_text_range_from_utf16(text: &str, range: Range<usize>) -> CanvasTextRange {
    let mut units = 0;
    let mut previous = 0;
    let mut first = 0;
    let mut last = None;
    for byte in grapheme_boundaries(text) {
        units += text[previous..byte].encode_utf16().count();
        previous = byte;
        if units <= range.start {
            first = byte;
        }
        if units >= range.end.max(range.start) && last.is_none() {
            last = Some(byte);
        }
    }
    CanvasTextRange {
        start: first,
        end: if range.start >= range.end {
            first
        } else {
            last.unwrap_or(text.len())
        },
    }
}
impl CanvasTextRange {
    /// Convert this original UTF-8 grapheme range to native UTF-16 units in `text`.
    /// Reject split scalars/clusters, reversed and out-of-range input without changing text or selection.
    pub fn to_utf16(&self, text: &str) -> Result<Range<usize>, String> {
        if self.start > self.end
            || !is_grapheme_boundary(text, self.start)
            || !is_grapheme_boundary(text, self.end)
        {
            return Err("Native text range requires original grapheme boundaries".into());
        }
        let start = text[..self.start].encode_utf16().count();
        Ok(start..start + text[self.start..self.end].encode_utf16().count())
    }
}
