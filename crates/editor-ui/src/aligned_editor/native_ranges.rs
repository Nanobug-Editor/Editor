//! Retain only caret geometry produced by the original native paints for visible source pieces.
use super::*;
use gpui_base::input::{DisplayPoint, RopeExt as _};
use gpui_kit::{Point, Window, point};
use std::{cell::RefCell, collections::HashMap};

/// A single painted frame's source-relative native carets and immutable buffer identity.
#[derive(Clone)]
pub(super) struct NativeRanges(Rc<RefCell<PaintedRanges>>);

/// Reject same-length edits before reusing geometry; the snapshot never becomes a text owner.
#[derive(Default)]
struct PaintedRanges {
    /// Shares immutable COW leaves only, using the existing native-buffer identity check.
    source: Option<content::NativeContent>,
    /// Last painted native selection; a new selection must settle native caret scrolling first.
    selection: Range<usize>,
    /// Marked UTF-16 range from the same native input delegate, including an unmark transition.
    marked: Option<Range<usize>>,
    /// Original native caret rectangles, keyed by their UTF-8 source offsets.
    carets: HashMap<usize, Bounds<Pixels>>,
}

impl NativeRanges {
    /// Bind this frame's geometry to the current immutable native buffer, without copying bytes.
    pub fn new(editor: &EditorState, marked: Option<Range<usize>>) -> Self {
        Self(Rc::new(RefCell::new(PaintedRanges {
            source: Some(content::NativeContent::new(editor.text())),
            selection: editor.selected_range(),
            marked,
            ..Default::default()
        })))
    }

    /// Whether `editor` still holds the buffer from which this frame's coordinates were painted.
    pub fn matches(&self, editor: &EditorState) -> bool {
        self.0
            .borrow()
            .source
            .as_ref()
            .is_some_and(|source| source.matches(editor.text()))
    }

    /// A horizontal boundary probe must not compete with native selection/caret auto-reveal.
    pub fn stable_input(&self, editor: &EditorState, marked: &Option<Range<usize>>) -> bool {
        let painted = self.0.borrow();
        painted
            .source
            .as_ref()
            .is_some_and(|source| source.matches(editor.text()))
            && painted.selection == editor.selected_range()
            && painted.marked == *marked
    }

    /// Read `editor` after a clipped native paint in `window`. Public display-map boundaries
    /// restrict queries to visible visual rows, including when one logical line wraps extensively.
    pub fn capture(&self, editor: &EditorState, window: &Window) {
        let (Some(origin), Some(line), Some(visible)) = (
            editor.text_bounds(),
            editor.line_height(),
            editor.visible_row_range(),
        ) else {
            return;
        };
        let clip = window
            .content_mask()
            .bounds
            .intersect(&editor.input_bounds());
        if clip.size.height <= px(0.) || line <= px(0.) {
            return;
        }
        let map = editor.display_map();
        let count = map.display_row_count();
        if count == 0 {
            return;
        }
        let first = (((clip.top() - origin.top()) / line).floor().max(0.) as usize).min(count - 1);
        let end = (((clip.bottom() - origin.top()) / line).ceil().max(0.) as usize).min(count);
        let text = editor.text();
        let byte_at = |row| {
            let point = map.display_pos_to_buffer_pos(DisplayPoint::new(row, 0));
            (text.line_start_offset(point.line) + point.col).min(text.len())
        };
        let first_byte = byte_at(first);
        let end_byte = if end < count {
            byte_at(end)
        } else {
            text.len()
        };
        let mut painted = self.0.borrow_mut();
        if painted
            .source
            .as_ref()
            .is_none_or(|source| !source.matches(text))
        {
            painted.source = Some(content::NativeContent::new(text));
            painted.carets.clear();
        }
        for character in text.byte_to_char_idx(first_byte)..=text.byte_to_char_idx(end_byte) {
            let byte = text.char_to_byte_idx(character);
            if painted.carets.contains_key(&byte)
                || !visible.contains(&text.offset_to_point(byte).row)
            {
                continue;
            }
            if let Some(mut caret) = editor.range_to_bounds(&(byte..byte)) {
                // Gutter copies have different physical origins. Normalize both axes before
                // retaining geometry so queries can use the final input authority's coordinates.
                caret.origin -= Point::new(editor.input_bounds().left(), origin.top());
                painted.carets.insert(byte, caret);
            }
        }
    }

    /// Reconstitute UTF-8 `range` in `measured` only while `editor` retains the painted buffer.
    /// Missing endpoints return None; queries never prepare hidden glyphs or approximate positions.
    pub fn bounds(
        &self,
        range: &Range<usize>,
        measured: &Measurement,
        editor: &EditorState,
    ) -> Option<Bounds<Pixels>> {
        let painted = self.0.borrow();
        if !painted.source.as_ref()?.matches(editor.text()) {
            return None;
        }
        let start = painted.carets.get(&range.start)?;
        let end = painted.carets.get(&range.end)?;
        let origin = point(measured.native.left(), measured.source_origin.y);
        Some(Bounds::from_corners(
            origin + start.origin,
            origin + end.origin + point(px(0.), end.size.height),
        ))
    }

    /// Find the native caret nearest `position` on its visual row, relative to `measured`.
    /// Only native rectangles painted from the current `editor` can answer platform hit queries.
    pub fn byte_for_point(
        &self,
        position: Point<Pixels>,
        measured: &Measurement,
        editor: &EditorState,
    ) -> Option<usize> {
        let painted = self.0.borrow();
        if !painted.source.as_ref()?.matches(editor.text()) {
            return None;
        }
        let line = measured.line_height;
        let row = ((position.y - measured.source_origin.y) / line).floor() * line;
        painted
            .carets
            .iter()
            .filter(|(_, caret)| (caret.top() - row).abs() < px(0.5))
            .min_by(|(a, left), (b, right)| {
                let distance = |caret: &Bounds<Pixels>| {
                    f32::from(position.x - measured.native.left() - caret.left()).abs()
                };
                distance(left).total_cmp(&distance(right)).then(a.cmp(b))
            })
            .map(|(byte, _)| *byte)
    }
}
