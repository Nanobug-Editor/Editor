//! Validate complete native row metadata with wrapping or an exact native horizontal boundary.
use super::*;
use gpui_kit::{Point, TextAlign};

/// A planning hint before paint; admission must repeat this check after native layout finishes.
/// Actual wrapping proves Base uses a bounded horizontal content width. Without that proof,
/// unwrapped sources additionally need a native boundary probe or complete glyph measurement.
pub(super) fn supported(editor: &EditorState, source_rows: &[Range<usize>]) -> bool {
    supported_rows(editor, source_rows)
        && editor.display_map().wrap_row_count() > editor.display_map().buffer_line_count()
}

/// Complete native rows are also sufficient without wrapping when a native x clamp supplies width.
pub(super) fn supported_rows(editor: &EditorState, source_rows: &[Range<usize>]) -> bool {
    let map = editor.display_map();
    !editor.presentation().is_masked()
        && editor.presentation().text_align() == TextAlign::Left
        && map.folded_ranges().is_empty()
        && map.buffer_line_count() == source_rows.len()
        && map.text().len() == editor.text().len()
        && map.display_row_count() == map.wrap_row_count()
}

#[cfg(test)]
thread_local! {
    /// Count complete metadata validation separately from native glyph-range queries.
    pub(super) static VALIDATED_ROWS: Cell<usize> = const { Cell::new(0) };
}

/// Consume the completed native map for `source_rows`, retaining only immutable pixel rows.
/// `origin`, `viewport` and `native_scroll` belong to that same final authority paint; `minimum_x`
/// is an optional current-frame native clamp, already restored before that paint. No text,
/// cursor, selection, composition or native scrolling is changed. Unsupported maps return None.
pub(super) fn read(
    editor: &EditorState,
    source_rows: &[Range<usize>],
    origin: Point<Pixels>,
    viewport: Bounds<Pixels>,
    native_scroll: Pixels,
    minimum_x: Option<Pixels>,
) -> Option<Measurement> {
    if !supported_rows(editor, source_rows)
        || (minimum_x.is_none() && !supported(editor, source_rows))
    {
        return None;
    }
    let line_height = editor.line_height()?;
    let line = f32::from(line_height);
    let map = editor.display_map();
    let count = map.display_row_count();
    let height = count as f32 * line;
    if !line.is_finite() || line <= 0. || !height.is_finite() || height > MAX_LAYOUT_HEIGHT {
        return None;
    }
    let mut logical_rows = Vec::with_capacity(source_rows.len());
    let mut end = 0;
    for row in 0..source_rows.len() {
        let range = map.buffer_line_to_display_row_range(row)?;
        // Reject hidden, partial or discontinuous projections; the native fallback can measure
        // those cases without inventing source coordinates or a second wrapping algorithm.
        if range.start != end || range.end <= range.start || range.end > count {
            return None;
        }
        logical_rows.push(range.start as f32 * line..range.end as f32 * line);
        end = range.end;
    }
    if end != count {
        return None;
    }
    #[cfg(test)]
    VALIDATED_ROWS.with(|rows| rows.set(rows.get() + source_rows.len()));
    Some(Measurement {
        native: editor.input_bounds(),
        viewport,
        source_origin: origin,
        native_inset_y: origin.y - (viewport.top() - native_scroll),
        logical_rows: logical_rows.into(),
        visual_rows: (0..count).map(|row| row as f32 * line).collect(),
        line_height,
        // Wrapping bounds width; otherwise the original native clamp supplies its exact scroll
        // range, including native caret clearance. No glyph or longest-line estimate is used.
        source_width: viewport.size.width - minimum_x.unwrap_or(px(0.)),
        cursor: editor.cursor(),
        caret: geometry::painted_caret(editor),
        text_len: editor.text().len(),
    })
}
