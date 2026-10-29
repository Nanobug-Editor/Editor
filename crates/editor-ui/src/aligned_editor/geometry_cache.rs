//! Retain text-free native row geometry across translations of an unchanged aligned viewport.
use super::*;
use gpui_base::input::RopeExt as _;
use gpui_kit::{Point, TextStyle, Window};

/// The epoch covers source/fold membership; external native notifications revoke private layout.
/// Absolute screen origins and scrolling are deliberately absent: neither changes source rows.
#[derive(PartialEq)]
struct Key {
    /// Source/row-plan incarnation, revoked on edits, IME, membership and alignment mode changes.
    epoch: u64,
    /// Native wrap width includes gutter/padding; viewport height does not change source rows.
    width: Pixels,
    /// Source glyph inset relative to its native input, cancelling independent horizontal scroll.
    inset: Pixels,
    /// Completed native physical line spacing, including font and display-scale rounding.
    line_height: Pixels,
    /// Inherited font features/fallbacks, rem size and DPI can reshape unchanged source bytes.
    style: TextStyle,
    /// Relative font lengths resolve against this window's current root size.
    rem_size: Pixels,
    /// A display change invalidates shaping even if logical allocation remains equal.
    scale: f32,
}

impl Key {
    /// Read actual wrap/style inputs, cancelling native horizontal scroll from the glyph inset.
    fn read(
        editor: &EditorState,
        epoch: u64,
        origin: Point<Pixels>,
        window: &Window,
    ) -> Option<Self> {
        let input = editor.input_bounds();
        Some(Self {
            epoch,
            width: input.size.width,
            inset: origin.x - input.left() - editor.scroll_offset().x,
            line_height: editor.line_height()?,
            style: window.text_style(),
            rem_size: window.rem_size(),
            scale: window.scale_factor(),
        })
    }
}

/// Only immutable row metadata is retained; the native entity still supplies current input state.
#[derive(Default)]
pub(super) struct GeometryCache {
    /// Only a successfully measured, still-admitted row map can be reused by later paints.
    measured: Option<(Key, Measurement)>,
    /// A failed bounded batch retries the original complete authority once, never guessed rows.
    force_full: bool,
}

impl GeometryCache {
    /// True only after native batches could not cover the complete source at their tentative bounds.
    pub fn full_required(&self) -> bool {
        self.force_full
    }

    /// Escalate a partial batch result to the established complete-layout admission path.
    pub fn require_full(&mut self) {
        self.measured = None;
        self.force_full = true;
    }
    /// Only an admitted current-epoch map permits a bounded native authority next frame.
    pub fn ready(&self, epoch: u64) -> bool {
        self.measured
            .as_ref()
            .is_some_and(|(key, _)| key.epoch == epoch)
    }

    /// Revoke on any native notification not already represented by the completed authority paint.
    /// In particular, a private native fold/highlight change must not be hidden by public equality.
    pub fn invalidate(&mut self) {
        self.measured = None;
    }

    /// Read current `editor` geometry for `source_rows` in this painted `viewport`/`native_scroll`.
    /// Reuse only source-relative rows at the same `epoch` and actual `window` layout signature;
    /// native bounds, caret, cursor and scroll translation are always refreshed for input.
    /// A `bounded` signature miss uses the completed native display map when supported, otherwise
    /// requests complete layout instead of querying unavailable offscreen glyphs.
    /// `acquired` is a complete native batch result; only its source-relative rows are retained.
    pub fn read(
        &mut self,
        editor: &EditorState,
        source_rows: &[Range<usize>],
        viewport: Bounds<Pixels>,
        native_scroll: Pixels,
        epoch: u64,
        bounded: bool,
        acquired: Option<Measurement>,
        minimum_x: Option<Pixels>,
        window: &Window,
    ) -> Option<Measurement> {
        let origin = self.origin(editor)?;
        let key = Key::read(editor, epoch, origin, window)?;
        let len = editor.text().len();
        let input = editor.input_bounds();
        if let Some(measured) = acquired {
            if measured.line_height != key.line_height
                || measured.native.size.width != input.size.width
            {
                return None;
            }
            self.measured = Some((key, measured));
            self.force_full = false;
            // Re-read below through the same native signature without retaining batch coordinates.
            return self.read(
                editor,
                source_rows,
                viewport,
                native_scroll,
                epoch,
                bounded,
                None,
                minimum_x,
                window,
            );
        }
        if let Some((old, measured)) = &self.measured
            && *old == key
        {
            let mut current = measured.clone();
            if let Some(minimum_x) = minimum_x {
                current.source_width = viewport.size.width - minimum_x;
                // Width growth discovered by native visible-line shaping also survives a
                // captured pointer sequence, during which boundary sampling is suspended.
                self.measured.as_mut()?.1.source_width = current.source_width;
            }
            current.native = input;
            current.viewport = viewport;
            current.source_origin = origin;
            current.native_inset_y = origin.y - (viewport.top() - native_scroll);
            current.cursor = editor.cursor();
            current.caret = geometry::painted_caret(editor);
            current.text_len = len;
            return Some(current);
        }
        // The original editor already prepared all wrap rows during its final native paint.
        // Reading those rows avoids hidden whole-source glyph work after cursor blink/private
        // notifications while preserving unconditional invalidation of unknown native changes.
        self.invalidate();
        if let Some(measured) = geometry_display_map::read(
            editor,
            source_rows,
            origin,
            viewport,
            native_scroll,
            minimum_x,
        ) {
            self.measured = Some((key, measured.clone()));
            self.force_full = false;
            return Some(measured);
        }
        // Unsupported width/font/fold changes still need a finite complete measurement.
        if bounded {
            return None;
        }
        let measured = Measurement::read(editor, source_rows, viewport, native_scroll)?;
        self.measured = Some((key, measured.clone()));
        self.force_full = false;
        Some(measured)
    }

    /// Share rows with full-width native backgrounds only for the same current authority layout.
    /// No measurement occurs on a miss; ordinary/native fallback resolves its own visible bands.
    pub fn bands(
        &self,
        editor: &EditorState,
        epoch: u64,
        window: &Window,
    ) -> Option<super::super::row_backgrounds::RowGeometry> {
        let origin = self.origin(editor)?;
        let key = Key::read(editor, epoch, origin, window)?;
        let (cached, measured) = self.measured.as_ref()?;
        (*cached == key).then(|| super::super::row_backgrounds::RowGeometry {
            origin_y: origin.y,
            rows: measured.logical_rows.clone(),
        })
    }

    /// Recover the source origin from a genuinely laid-out logical line, never an offscreen
    /// byte lookup (Base's saturating offsets can otherwise alias the first visible line).
    fn origin(&self, editor: &EditorState) -> Option<Point<Pixels>> {
        let row = editor.visible_row_range()?.start;
        let offset = editor.text().line_start_offset(row);
        let mut origin = editor.range_to_bounds(&(offset..offset))?.origin;
        // Native text_bounds is the scrolled source plane, independent of its first laid-out
        // logical line. It avoids cancellation of large cached row y values at deep positions.
        origin.y = editor.text_bounds()?.top();
        Some(origin)
    }
}
