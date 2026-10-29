//! Measure the original native source in small windows instead of quadratic whole-layout lookups.
use super::*;
use gpui_kit::{
    ContentMask, Element, ElementId, GlobalElementId, InspectorElementId, InteractiveElement as _,
    LayoutId, ParentElement as _, Styled as _, Window, div, point, size,
};
use std::cell::RefCell;

/// Batches amortize native control setup while bounding Base's linear per-row range lookup.
/// The 128-row limit balances measured native layout and query cost on long wrapped sources.
const BATCH: usize = 128;
#[cfg(test)]
thread_local! {
    /// Observe real batch planning work without asserting elapsed time on the test platform.
    pub(super) static PLANNED_ROWS: std::cell::Cell<[usize; 3]> = const { std::cell::Cell::new([0; 3]) };
    /// Count actual native windows; logical row counts alone hide byte-budget over-allocation.
    pub(super) static PLANNED_BATCHES: Cell<[usize; 3]> = const { Cell::new([0; 3]) };
    /// Full admission must reject stale hints with missing rows; glyph prefetch is not admission.
    pub(super) static INCOMPLETE_BATCHES: Cell<[usize; 3]> = const { Cell::new([0; 3]) };
}
/// One-frame output contains only measured rows, never another text buffer or editing state.
pub(super) type Output = Rc<RefCell<Option<Measurement>>>;

/// Transparent admission surface. Final input authority is painted afterwards by the pane.
pub(super) struct Batches {
    /// Exact original control factory and entity; every window shares that same editing state.
    source: AlignmentPane,
    /// Admitted immutable byte ranges for the current source revision.
    rows: Rc<AlignedRows>,
    /// Membership selects this source's logical ranges.
    index: usize,
    /// Last valid rows are a planning hint only; incomplete coverage never becomes geometry.
    hint: Option<Rc<[Range<f32>]>>,
    /// Conservative original layout budget for first admission and unhinted changes.
    limit: Pixels,
    /// Native line spacing used only to allocate provisional measurement windows.
    line: Pixels,
    /// Already measured header/padding allowance, independent of source bytes.
    inset: Pixels,
    /// Preserve an explicit local horizontal request across all native mounts.
    horizontal: Option<Pixels>,
    /// Published only after every source row and EOF were measured by native controls.
    output: Output,
    /// Empty absolute layout surface keeps these temporary windows outside parent sizing.
    surface: AnyElement,
    /// Warm-only snapshot caches need nearby native glyphs, not all offscreen source rows.
    prefetch: Option<Range<usize>>,
}

impl Batches {
    /// Plan native windows for `source`/`index` in `rows`. `hint` may be stale: all rows must still
    /// be observed. `limit`, `line`, `inset` and `horizontal` retain the pane's native allocation.
    pub fn new(
        source: AlignmentPane,
        rows: Rc<AlignedRows>,
        index: usize,
        hint: Option<Rc<[Range<f32>]>>,
        limit: Pixels,
        line: Pixels,
        inset: Pixels,
        horizontal: Option<Pixels>,
        output: Output,
    ) -> Self {
        Self {
            source,
            rows,
            index,
            hint,
            limit,
            line,
            inset,
            horizontal,
            output,
            surface: div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .into_any_element(),
            prefetch: None,
        }
    }

    /// Restrict glyph prewarming to original logical `rows`. Validation never uses this option:
    /// partial coverage cannot produce a Measurement, so it cannot admit guessed input geometry.
    pub fn prefetch(mut self, rows: Range<usize>) -> Self {
        self.prefetch = Some(rows);
        self
    }
}

impl IntoElement for Batches {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for Batches {
    type RequestLayoutState = ();
    type PrepaintState = Vec<AnyElement>;
    fn id(&self) -> Option<ElementId> {
        Some("aligned-native-row-batches".into())
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.surface.request_layout(window, cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Vec<AnyElement> {
        self.surface.prepaint(window, cx);
        let count = self.rows.source_rows[self.index].len();
        #[cfg(test)]
        PLANNED_ROWS.with(|rows| {
            let mut counts = rows.get();
            counts[self.index] += self.prefetch.as_ref().map_or(count, |scope| scope.len());
            rows.set(counts);
        });
        let mut spans = Vec::new();
        // A remounted source can retain its native map while its new comparison has no measured
        // hint. That map only plans windows: paint below still requires every real row and EOF.
        let hint = self
            .hint
            .clone()
            .filter(|hint| hint.len() == count)
            .or_else(|| {
                native_planning_hint(
                    self.source.editor.read(cx),
                    &self.rows.source_rows[self.index],
                )
            });
        if let Some(hint) = hint {
            let scope = self.prefetch.clone().unwrap_or(0..count);
            for first in (scope.start.min(count)..scope.end.min(count)).step_by(BATCH) {
                let end = (first + BATCH + 1).min(scope.end).min(count);
                spans.push((
                    px(hint[first].start),
                    px(hint[end - 1].end - hint[first].start),
                ));
            }
        } else {
            let step = (self.line * BATCH).max(px(1.));
            let mut top = px(0.);
            while top < self.limit {
                // Overlap covers native leading/trailing insets without predicting wraps.
                spans.push((top, step * 2.));
                top += step;
            }
        }
        #[cfg(test)]
        PLANNED_BATCHES.with(|batches| {
            let mut counts = batches.get();
            counts[self.index] += spans.len();
            batches.set(counts);
        });
        let mut elements = Vec::with_capacity(spans.len());
        for (piece, (top, height)) in spans.into_iter().enumerate() {
            let element = crate::row_backgrounds::with_geometry_only(window, cx, |window, cx| {
                let native = if let Some(factory) = &self.source.native_authority_factory {
                    factory(&self.source.editor, window, cx)
                } else {
                    crate::row_backgrounds::native_editor(&self.source.editor, cx)
                };
                let height = height.max(self.line) + self.inset.max(px(0.)) + self.line * 2.;
                let mut element = div()
                    .id(("native-row-batch", piece))
                    .w(bounds.size.width)
                    .h(height)
                    .opacity(0.)
                    .child(native_viewport::NativeViewport::new(
                        &self.source.editor,
                        self.horizontal,
                        -top,
                        native,
                    ))
                    .into_any_element();
                element.prepaint_as_root(
                    bounds.origin,
                    size(bounds.size.width.into(), height.into()),
                    window,
                    cx,
                );
                element
            });
            elements.push(element);
        }
        elements
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        elements: &mut Vec<AnyElement>,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.surface.paint(window, cx);
        #[cfg(test)]
        let started = std::time::Instant::now();
        let rows = &self.rows.source_rows[self.index];
        let mut starts = vec![None; rows.len()];
        let mut right = bounds.size.width;
        let mut eof_y = None;
        let mut signature = None;
        let mut consistent = true;
        for element in elements {
            // Base publishes its exact last_layout only during paint. An empty content mask
            // keeps that state transition while excluding transient glyphs from raster/scene
            // submission; the real source authority paints afterwards with the actual mask.
            window.with_content_mask(Some(ContentMask::default()), |window| {
                element.paint(window, cx);
            });
            let editor = self.source.editor.read(cx);
            let Some(visible) = editor.visible_row_range() else {
                continue;
            };
            #[cfg(test)]
            geometry::MAX_LOOKUP_ROWS.with(|max| max.set(max.get().max(visible.len())));
            let Some(plane) = editor.text_bounds() else {
                continue;
            };
            let current = (editor.input_bounds().size.width, editor.line_height());
            if signature.is_some_and(|old| old != current) {
                consistent = false;
            }
            signature = Some(current);
            for row in visible {
                let Some(range) = rows.get(row) else { continue };
                if starts[row].is_some() {
                    continue;
                }
                let mut end = range.end.min(editor.text().len());
                if range.start > end
                    || !editor.text().is_char_boundary(range.start)
                    || !editor.text().is_char_boundary(end)
                {
                    continue;
                }
                while end > range.start && matches!(editor.text().byte(end - 1), b'\r' | b'\n') {
                    end -= 1;
                }
                let Some(rect) = editor.range_to_bounds(&(range.start..end)) else {
                    continue;
                };
                starts[row] = Some(f32::from(rect.top() - plane.top()));
                right = right.max(rect.right() - plane.left());
                #[cfg(test)]
                geometry::MEASURED_ROWS.with(|count| count.set(count.get() + 1));
            }
            if visible_contains_eof(editor) {
                let eof = editor.text().len();
                eof_y = editor
                    .range_to_bounds(&(eof..eof))
                    .map(|rect| rect.top() - plane.top());
            }
        }
        let editor = self.source.editor.read(cx);
        let measured = (|| {
            if !consistent {
                return None;
            }
            let line = editor.line_height()?;
            let mut starts = starts.into_iter().collect::<Option<Vec<_>>>()?;
            starts.push(f32::from(eof_y? + line));
            if starts.first().is_none_or(|y| y.abs() > 0.5) {
                return None;
            }
            let logical_rows = starts.windows(2).map(|ys| ys[0]..ys[1]).collect::<Vec<_>>();
            if logical_rows.iter().any(|row| {
                !row.start.is_finite()
                    || !row.end.is_finite()
                    || row.end < row.start
                    || row.end > MAX_LAYOUT_HEIGHT
            }) {
                return None;
            }
            let height = *starts.last()?;
            let visual_rows = (0..(height / f32::from(line)).ceil() as usize)
                .map(|row| row as f32 * f32::from(line))
                .collect::<Vec<_>>();
            Some(Measurement {
                native: editor.input_bounds(),
                viewport: bounds,
                source_origin: point(px(0.), px(0.)),
                native_inset_y: px(0.),
                logical_rows: logical_rows.into(),
                visual_rows: visual_rows.into(),
                line_height: line,
                source_width: right,
                cursor: editor.cursor(),
                caret: None,
                text_len: editor.text().len(),
            })
        })();
        #[cfg(test)]
        geometry::MEASURE_NANOS.with(|time| time.set(time.get() + started.elapsed().as_nanos()));
        #[cfg(test)]
        if measured.is_none() && self.prefetch.is_none() {
            INCOMPLETE_BATCHES.with(|batches| {
                let mut counts = batches.get();
                counts[self.index] += 1;
                batches.set(counts);
            });
        }
        *self.output.borrow_mut() = measured;
    }
}

/// Reuse native row metadata only to allocate measurement windows, never to publish coordinates.
/// Width/font changes can make this map stale; incomplete paint returns None and the pane retries
/// full native layout. Unsupported or partial maps retain the conservative unhinted planner.
fn native_planning_hint(
    editor: &EditorState,
    source_rows: &[Range<usize>],
) -> Option<Rc<[Range<f32>]>> {
    if !geometry_display_map::supported_rows(editor, source_rows) {
        return None;
    }
    let line = f32::from(editor.line_height()?);
    let map = editor.display_map();
    let count = map.display_row_count();
    let height = count as f32 * line;
    if !line.is_finite() || line <= 0. || !height.is_finite() || height > MAX_LAYOUT_HEIGHT {
        return None;
    }
    let mut rows = Vec::with_capacity(source_rows.len());
    let mut end = 0;
    for row in 0..source_rows.len() {
        let range = map.buffer_line_to_display_row_range(row)?;
        if range.start != end || range.end <= range.start || range.end > count {
            return None;
        }
        rows.push(range.start as f32 * line..range.end as f32 * line);
        end = range.end;
    }
    (end == count).then(|| rows.into())
}

/// Query EOF only when the native window actually laid out its logical row.
fn visible_contains_eof(editor: &EditorState) -> bool {
    use gpui_base::input::RopeExt as _;
    editor.visible_row_range().is_some_and(|range| {
        range.contains(&editor.text().offset_to_point(editor.text().len()).row)
    })
}
