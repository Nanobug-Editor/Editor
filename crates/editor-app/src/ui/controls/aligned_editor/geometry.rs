//! Native logical-row measurements and source/display conversion contain no editing behavior.
use super::super::aligned_rows::PaneLayout;
use super::*;
use gpui_kit::{Point, point};

/// A group shares a physical first-glyph origin, not merely the same source-relative row y.
/// Header heights and native editor padding can differ without becoming source text or rows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct DisplayViewport {
    pub origin_y: Pixels,
    pub height: Pixels,
}

impl DisplayViewport {
    pub fn from_measurements(measurements: &[Option<Measurement>]) -> Option<Self> {
        let all = measurements
            .iter()
            .map(Option::as_ref)
            .collect::<Option<Vec<_>>>()?;
        let origin_y = all
            .iter()
            .map(|m| m.viewport.top() + m.native_inset_y)
            .max()?;
        let bottom = all.iter().map(|m| m.viewport.bottom()).min()?;
        // A zero-height clip would hide every source while falsely reporting Ready. The caller
        // distinguishes incomplete measurements first, then exposes the native geometry fallback.
        if bottom <= origin_y {
            return None;
        }
        Some(Self {
            origin_y,
            height: bottom - origin_y,
        })
    }

    pub fn pane(self, measured: &Measurement) -> PaneViewport {
        PaneViewport {
            inset_y: self.origin_y - measured.viewport.top(),
            height: self.height,
            native_inset_y: measured.native_inset_y,
        }
    }
}

/// Render captures this value before any pane paints. Native geometry measured later in that
/// frame must not retroactively move slices or scrollbars that already used these coordinates.
#[derive(Clone, Copy, Debug)]
pub(super) struct PaneViewport {
    pub inset_y: Pixels,
    pub height: Pixels,
    pub native_inset_y: Pixels,
}

impl PaneViewport {
    pub fn bounds(self, viewport: Bounds<Pixels>) -> Bounds<Pixels> {
        Bounds::new(
            point(viewport.left(), viewport.top() + self.inset_y),
            gpui_kit::size(viewport.size.width, self.height),
        )
        .intersect(&viewport)
    }
}

/// This is the projection that actually painted, distinct from the next requested scroll/layout.
/// Input and rails must never consume a future group layout while the old native frame is visible.
#[derive(Clone)]
pub(super) struct Projection {
    pub measured: Measurement,
    pub pane: PaneLayout,
    pub scroll: Pixels,
    pub inset_y: Pixels,
    pub viewport: Bounds<Pixels>,
}

impl Projection {
    pub fn project(&self, mut native: Bounds<Pixels>) -> Option<Bounds<Pixels>> {
        let source = self.measured.source_y(native);
        let display = self.pane.source_to_display(source)?;
        native.origin.y = self.measured.viewport.top() + self.inset_y + px(display) - self.scroll;
        Some(native)
    }

    pub fn native_point(&self, physical: Point<Pixels>) -> Option<Point<Pixels>> {
        let display =
            f32::from(physical.y - self.measured.viewport.top() - self.inset_y + self.scroll);
        let source = self
            .pane
            .display_to_source(display.clamp(0., self.pane.display_height))?;
        Some(point(
            physical.x,
            self.measured.source_origin.y + px(source),
        ))
    }
}

/// All values describe the same completed final native paint, including its unprojected origin.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Measurement {
    pub native: Bounds<Pixels>,
    pub viewport: Bounds<Pixels>,
    pub source_origin: Point<Pixels>,
    pub native_inset_y: Pixels,
    pub logical_rows: Vec<Range<f32>>,
    pub visual_rows: Vec<f32>,
    pub line_height: Pixels,
    pub source_width: Pixels,
    pub cursor: usize,
    pub caret: Option<Bounds<Pixels>>,
    pub text_len: usize,
}

impl Measurement {
    /// The final authority is deliberately full-height, so even offscreen logical-line starts
    /// are measured through Base's own wrapping, glyph and gutter layout. No wrap algorithm is copied.
    pub fn read(
        editor: &EditorState,
        source_rows: &[Range<usize>],
        viewport: Bounds<Pixels>,
        native_scroll: Pixels,
    ) -> Option<Self> {
        let line_height = editor.line_height()?;
        let source_origin = editor.range_to_bounds(&(0..0))?.origin;
        let eof = editor.text().len();
        let eof_bounds = editor.range_to_bounds(&(eof..eof))?;
        let mut starts = Vec::with_capacity(source_rows.len() + 1);
        let mut right = editor.input_bounds().right();
        for range in source_rows {
            if range.start > eof || !editor.text().is_char_boundary(range.start) {
                return None;
            }
            let bounds = editor.range_to_bounds(&(range.start..range.start))?;
            starts.push(f32::from(bounds.top() - source_origin.y));
            // A no-wrap logical line's final caret is its public glyph extent. Strip only its
            // actual terminator bytes; CRLF is never rewritten into the native document.
            let mut end = range.end.min(eof);
            while end > range.start && matches!(editor.text().byte(end - 1), b'\r' | b'\n') {
                end -= 1;
            }
            let end_bounds = editor.range_to_bounds(&(end..end))?;
            right = right.max(end_bounds.right() - editor.scroll_offset().x);
        }
        starts.push(f32::from(eof_bounds.top() - source_origin.y + line_height));
        if starts.first().is_none_or(|first| first.abs() > 0.5) {
            return None;
        }
        let logical_rows = starts.windows(2).map(|ys| ys[0]..ys[1]).collect::<Vec<_>>();
        if logical_rows.iter().any(|range| {
            !range.start.is_finite()
                || !range.end.is_finite()
                || range.end < range.start
                || range.end > MAX_LAYOUT_HEIGHT
        }) {
            return None;
        }
        // Wrapping uses a single native line-height. A derived y grid is sufficient for page
        // distance; the native MoveUp/Down actions still own actual columns and wrap affinity.
        let height = *starts.last()?;
        let visual_rows = (0..(height / f32::from(line_height)).ceil() as usize)
            .map(|row| row as f32 * f32::from(line_height))
            .collect();
        Some(Self {
            native: editor.input_bounds(),
            viewport,
            source_origin,
            native_inset_y: source_origin.y - (viewport.top() - native_scroll),
            logical_rows,
            visual_rows,
            line_height,
            source_width: (right - editor.input_bounds().left()).max(viewport.size.width),
            cursor: editor.cursor(),
            caret: editor.cursor_layout().map(|(rect, _)| rect),
            text_len: eof,
        })
    }

    pub fn source_y(&self, bounds: Bounds<Pixels>) -> f32 {
        f32::from(bounds.top() - self.source_origin.y)
    }
}

impl GroupState {
    /// Mouse anchors may lie inside the original leading gutter rather than in the text body.
    pub(super) fn project_pointer(
        &self,
        index: usize,
        native: Bounds<Pixels>,
    ) -> Option<Bounds<Pixels>> {
        let mut projected = self.project(index, native)?;
        if let Some(gutter) = self.gutters.get(index).copied().flatten() {
            projected.origin.x = gutter.physical_point(native.origin).x;
        }
        Some(projected)
    }

    pub(super) fn project(&self, index: usize, native: Bounds<Pixels>) -> Option<Bounds<Pixels>> {
        if self.retired {
            return None;
        }
        let projected = if matches!(
            self.effective_status(),
            AlignmentStatus::Unavailable(_)
                | AlignmentStatus::Disabled
                | AlignmentStatus::NativeOverlays
        ) {
            self.sources.get(index).map(|_| native)
        } else {
            self.frames.get(index)?.as_ref()?.project(native)
        }?;
        Some(
            self.gutters
                .get(index)
                .copied()
                .flatten()
                .map_or(projected, |gutter| gutter.text_bounds(projected)),
        )
    }

    pub(super) fn native_point(
        &self,
        index: usize,
        physical: Point<Pixels>,
    ) -> Option<Point<Pixels>> {
        self.frames.get(index)?.as_ref()?.native_point(physical)
    }

    pub(super) fn native_scroll(&self, index: usize) -> Pixels {
        self.layout
            .as_ref()
            .and_then(|layout| layout.panes.get(index))
            .and_then(|pane| pane.display_to_source(f32::from(self.scroll.get())))
            .map(px)
            .unwrap_or(px(0.))
    }
}
