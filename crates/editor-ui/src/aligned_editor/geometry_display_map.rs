//! Read complete wrapped-row metadata from the original native editor after its authority paint.
use super::*;
use gpui_kit::{Point, TextAlign};

/// A planning hint before paint; admission must repeat this check after native layout finishes.
/// Actual wrapping proves Base uses a bounded horizontal content width. Without that proof,
/// hidden glyph measurements still supply the exact horizontal extent of unwrapped sources.
pub(super) fn supported(editor: &EditorState, source_rows: &[Range<usize>]) -> bool {
    let map = editor.display_map();
    !editor.presentation().is_masked()
        && editor.presentation().text_align() == TextAlign::Left
        && map.folded_ranges().is_empty()
        && map.buffer_line_count() == source_rows.len()
        && map.text().len() == editor.text().len()
        && map.wrap_row_count() > map.buffer_line_count()
        && map.display_row_count() == map.wrap_row_count()
}

/// Consume the completed native map for `source_rows`, retaining only immutable pixel rows.
/// `origin`, `viewport` and `native_scroll` belong to that same final authority paint; no text,
/// cursor, selection, composition or native scrolling is changed. Unsupported maps return None.
pub(super) fn read(
    editor: &EditorState,
    source_rows: &[Range<usize>],
    origin: Point<Pixels>,
    viewport: Bounds<Pixels>,
    native_scroll: Pixels,
) -> Option<Measurement> {
    if !supported(editor, source_rows) {
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
    Some(Measurement {
        native: editor.input_bounds(),
        viewport,
        source_origin: origin,
        native_inset_y: origin.y - (viewport.top() - native_scroll),
        logical_rows: logical_rows.into(),
        visual_rows: (0..count).map(|row| row as f32 * line).collect(),
        line_height,
        // Base's wrapped input uses its available wrap width even with inline tokens. Alignment
        // therefore has no horizontal overflow; right/center caret clearance is excluded above.
        source_width: viewport.size.width,
        cursor: editor.cursor(),
        caret: geometry::painted_caret(editor),
        text_len: editor.text().len(),
    })
}
