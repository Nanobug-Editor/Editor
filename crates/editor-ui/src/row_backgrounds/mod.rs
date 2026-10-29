//! Owned full-row backgrounds reuse native text/gutter paints without copying layout or input logic.
use super::editor::{synchronize_editor_appearance, synchronize_editor_row_background};
use crate::theme::ActiveTheme as _;
use gpui_base::input::{EditorState, RangeDecoration, RopeExt as _};
use gpui_kit::{
    AnyElement, App, Bounds, Entity, EntityId, Global, IntoElement, ParentElement as _, Pixels,
    Styled as _, WeakEntity, Window, canvas, div, fill, point, px, size,
};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    rc::{Rc, Weak},
};

#[cfg(test)]
thread_local! {
    /// Count actual native row-coordinate lookups; warm diff bands should not scan distant rows.
    pub(super) static NATIVE_ROW_READS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Weak presentation owners never keep a document, source snapshot or Undo stack alive.
#[derive(Default)]
struct Registry(
    /// Independent owners of colored UTF-8 ranges for each original native entity.
    BTreeMap<EntityId, Vec<Weak<RefCell<Vec<RangeDecoration>>>>>,
    /// Optional geometry leases for admitted layouts; dead readers are pruned on registration.
    BTreeMap<EntityId, Vec<Weak<GeometryReader>>>,
);
impl Global for Registry {}

/// Temporary layout-only mounts must not create colored copies or schedule missing-band paints.
#[derive(Default)]
struct GeometryOnly(usize);
impl Global for GeometryOnly {}

/// Run synchronous native `content` in `window` without row-paint layers, restoring nested scopes.
/// Syntax, text layout and the original input remain native; only these local visual bands yield.
pub(super) fn with_geometry_only<R>(
    window: &mut Window,
    cx: &mut App,
    content: impl FnOnce(&mut Window, &mut App) -> R,
) -> R {
    let old = cx.default_global::<GeometryOnly>().0;
    cx.default_global::<GeometryOnly>().0 = old + 1;
    let result = content(window, cx);
    cx.default_global::<GeometryOnly>().0 = old;
    result
}

/// A completed source-relative native row map, without text, selection or mutable editing state.
pub(super) struct RowGeometry {
    /// Current first-glyph screen y of this exact native paint, including its scroll translation.
    pub origin_y: Pixels,
    /// Logical-row native wrap ranges shared with an admitted geometry owner.
    pub rows: Rc<[std::ops::Range<f32>]>,
}

/// Read native rows without extending the source lifetime or confusing a band repaint with editing.
struct GeometryReader {
    /// The owning presentation validates the original layout in this window/application context.
    read: Box<dyn Fn(&EditorState, &Window, &App) -> Option<RowGeometry>>,
    /// Missing bands invalidate this presentation, preserving its already validated source rows.
    owner: EntityId,
    /// Old painted elements cannot read geometry or repaint an owner after its lease is dropped.
    active: Rc<std::cell::Cell<bool>>,
}

impl GeometryReader {
    /// Return the owner's current validated rows, or None after retirement or a native change.
    fn read(&self, editor: &EditorState, window: &Window, cx: &App) -> Option<RowGeometry> {
        self.active
            .get()
            .then(|| (self.read)(editor, window, cx))
            .flatten()
    }
}

/// The presentation owner holds this lease; registry entries are weak and cannot retain a source.
pub(super) struct RowGeometryProvider {
    /// Keep the reader available to native controls/factories only for this owner's lifetime.
    _reader: Rc<GeometryReader>,
    /// Revoke even a reader retained by an old painted element when the owner drops its lease.
    active: Rc<std::cell::Cell<bool>>,
}

impl RowGeometryProvider {
    /// Attach validated `read` geometry and its repaint `owner` to native factories of `editor`.
    /// Band follow-ups notify the presentation, not the editor's private-state observers.
    /// Dropping the lease withdraws both capabilities; stale readers use native lookup.
    pub fn new(
        editor: &Entity<EditorState>,
        owner: EntityId,
        read: impl Fn(&EditorState, &Window, &App) -> Option<RowGeometry> + 'static,
        cx: &mut App,
    ) -> Self {
        let active = Rc::new(std::cell::Cell::new(true));
        let reader = Rc::new(GeometryReader {
            read: Box::new(read),
            owner,
            active: active.clone(),
        });
        let registry = cx.default_global::<Registry>();
        registry.1.retain(|_, readers| {
            readers.retain(|reader| reader.strong_count() > 0);
            !readers.is_empty()
        });
        registry
            .1
            .entry(editor.entity_id())
            .or_default()
            .push(Rc::downgrade(&reader));
        Self {
            _reader: reader,
            active,
        }
    }
}

impl Drop for RowGeometryProvider {
    /// Revocation is immediate; old elements may retain the callback but cannot use its rows.
    fn drop(&mut self) {
        self.active.set(false);
    }
}

/// A comparison owns row ranges separately from native character/diagnostic decorations.
pub struct RowBackgroundCollection {
    /// Weak native identity used only to request repaint when this owner's metadata changes.
    editor: WeakEntity<EditorState>,
    /// Current half-open UTF-8 ranges/colors; the comparison owner refreshes them with its revision.
    ranges: Rc<RefCell<Vec<RangeDecoration>>>,
}

impl RowBackgroundCollection {
    /// Attach `ranges` to an existing `editor`; dropping or disposing the owner withdraws them.
    pub fn new(editor: &Entity<EditorState>, ranges: Vec<RangeDecoration>, cx: &mut App) -> Self {
        let ranges = Rc::new(RefCell::new(ranges));
        let registry = cx.default_global::<Registry>();
        registry.0.retain(|_, owners| {
            owners.retain(|owner| owner.strong_count() > 0);
            !owners.is_empty()
        });
        registry
            .0
            .entry(editor.entity_id())
            .or_default()
            .push(Rc::downgrade(&ranges));
        cx.notify(editor.entity_id());
        Self {
            editor: editor.downgrade(),
            ranges,
        }
    }

    /// Replace current revision/color metadata only; identical presentation causes no idle repaints.
    pub fn set(&self, ranges: Vec<RangeDecoration>, cx: &mut App) {
        if *self.ranges.borrow() != ranges {
            *self.ranges.borrow_mut() = ranges;
            if let Some(editor) = self.editor.upgrade() {
                cx.notify(editor.entity_id());
            }
        }
    }

    /// Withdraw this owner's backgrounds, retaining unrelated layers and native source state.
    pub fn dispose(&self, cx: &mut App) {
        self.set(Vec::new(), cx);
    }
}

/// Resolve current owners without retaining the source or any previous document revision.
fn ranges(editor: &Entity<EditorState>, cx: &App) -> Vec<RangeDecoration> {
    cx.try_global::<Registry>()
        .and_then(|registry| registry.0.get(&editor.entity_id()))
        .into_iter()
        .flatten()
        .filter_map(Weak::upgrade)
        .flat_map(|owner| owner.borrow().clone())
        .collect()
}

/// Native caret geometry supplies wrap/fold/EOF height; half-open LF ranges exclude the next row.
fn row_bounds(
    editor: &EditorState,
    range: &std::ops::Range<usize>,
    geometry: Option<&RowGeometry>,
) -> Option<Bounds<Pixels>> {
    let text = editor.text();
    if range.is_empty()
        || range.end > text.len()
        || !text.is_char_boundary(range.start)
        || !text.is_char_boundary(range.end)
    {
        return None;
    }
    let first = text.offset_to_point(range.start).row;
    let end_row = text.offset_to_point(range.end).row;
    let last = if range.end == text.line_start_offset(end_row) {
        end_row.saturating_sub(1)
    } else {
        end_row
    };
    if let Some(geometry) = geometry {
        // The provider verified the current native width/font/epoch. Resolve any changed band
        // from shared rows, including offscreen bands, without another linear native lookup.
        let top = geometry.origin_y + px(geometry.rows.get(first)?.start);
        let bottom = geometry.origin_y + px(geometry.rows.get(last)?.end);
        let input = editor.input_bounds();
        return (bottom > top).then(|| {
            Bounds::new(
                point(input.left(), top),
                size(input.size.width, bottom - top),
            )
        });
    }
    // Ordinary/unified editors lay out only their visible rows. An offscreen end must not
    // suppress a visible part of the change; folded interior rows have no caret geometry.
    let visible = editor.visible_row_range()?;
    let first = first.max(visible.start);
    let last = last.min(visible.end.checked_sub(1)?);
    if first > last {
        return None;
    }
    let input = editor.input_bounds();
    let row_top = |row| {
        let offset = text.line_start_offset(row);
        #[cfg(test)]
        NATIVE_ROW_READS.with(|count| count.set(count.get() + 1));
        editor
            .range_to_bounds(&(offset..offset))
            .map(|bounds| bounds.top())
    };
    let top = row_top(first)
        .or_else(|| (first == visible.start).then_some(input.top()))
        .or_else(|| (first + 1..=last).find_map(row_top))?;
    let bottom = (last + 1..visible.end)
        .find_map(row_top)
        .or_else(|| {
            (visible.end >= text.lines_len())
                .then(|| {
                    #[cfg(test)]
                    NATIVE_ROW_READS.with(|count| count.set(count.get() + 1));
                    editor
                        .range_to_bounds(&(text.len()..text.len()))
                        .map(|bounds| bounds.bottom())
                })
                .flatten()
        })
        .unwrap_or(input.bottom());
    (bottom > top).then(|| {
        Bounds::new(
            point(input.left(), top),
            size(input.size.width, bottom - top),
        )
    })
}

/// Resolve the logical byte envelope of `clip` from admitted native rows translated by `delta`.
/// Only two Rope lookups remain; offscreen changed bands need no per-band text/position queries.
fn visible_bytes(
    editor: &EditorState,
    geometry: Option<&RowGeometry>,
    clip: Bounds<Pixels>,
    delta: Pixels,
) -> Option<std::ops::Range<usize>> {
    let (first, end) = if let Some(geometry) = geometry {
        let top = f32::from(clip.top() - geometry.origin_y - delta);
        let bottom = f32::from(clip.bottom() - geometry.origin_y - delta);
        (
            geometry.rows.partition_point(|row| row.end <= top),
            geometry.rows.partition_point(|row| row.start < bottom),
        )
    } else {
        let visible = editor.visible_row_range()?;
        (visible.start, visible.end)
    };
    let text = editor.text();
    let start = text.line_start_offset(first).min(text.len());
    let end = text.line_start_offset(end).min(text.len());
    (start < end).then_some(start..end)
}

/// Half-open byte ranges preserve overlapping owners and exclude the next line after a final LF.
fn intersects_bytes(mark: &RangeDecoration, visible: &Option<std::ops::Range<usize>>) -> bool {
    visible
        .as_ref()
        .is_none_or(|visible| mark.range().start < visible.end && mark.range().end > visible.start)
}

/// Paint admitted native `pieces` in `bounds`, using current `marks` only for the same editor.
/// A newly visible band schedules one finite follow-up; ordinary input/scroll still belongs to Base.
fn paint_bands(
    editor: &Entity<EditorState>,
    bounds: Bounds<Pixels>,
    marks: &[RangeDecoration],
    mut pieces: Vec<(RangeDecoration, AnyElement)>,
    readers: &[Rc<GeometryReader>],
    window: &mut Window,
    cx: &mut App,
) {
    let clip = bounds.intersect(&window.content_mask().bounds);
    let geometry = readers
        .iter()
        .find_map(|reader| reader.read(editor.read(cx), window, cx));
    let visible = visible_bytes(editor.read(cx), geometry.as_ref(), clip, px(0.));
    let missing = marks.iter().any(|mark| {
        intersects_bytes(mark, &visible)
            && row_bounds(editor.read(cx), mark.range(), geometry.as_ref())
                .is_some_and(|row| row.intersect(&clip).size.height > px(0.))
            && !pieces.iter().any(|(admitted, _)| admitted == mark)
    });
    if missing {
        // Native paint can expose a band absent during prepaint. Repaint its presentation:
        // notifying the source here would revoke that owner's geometry again, creating an
        // endless complete-validation loop after a blink or another private native change.
        let id = editor.entity_id();
        let owners = readers
            .iter()
            .filter(|reader| reader.active.get())
            .cloned()
            .collect::<Vec<_>>();
        window.on_next_frame(move |_, cx| {
            if owners.is_empty() {
                // Ordinary native editors have no independent geometry/presentation owner.
                cx.notify(id);
            } else {
                for reader in owners {
                    if reader.active.get() {
                        cx.notify(reader.owner);
                    }
                }
            }
        });
    }
    for (mark, native) in &mut pieces {
        let Some(row) = row_bounds(editor.read(cx), mark.range(), geometry.as_ref()) else {
            continue;
        };
        let row = row.intersect(&clip);
        if row.size.height <= px(0.) {
            continue;
        }
        let appearance =
            crate::theme::component_styles(cx, plugin_schema::ThemeComponent::Editor).base;
        let background = appearance
            .background
            .unwrap_or(cx.theme().background)
            .blend(mark.color().unwrap_or(cx.theme().warning));
        window.with_content_mask(Some(gpui_kit::ContentMask { bounds: row }), |window| {
            window.paint_quad(fill(row, background));
            synchronize_editor_row_background(editor, background, cx);
            native.paint(window, cx);
        });
    }
    synchronize_editor_appearance(editor, cx);
}

/// Repaint visible changed bands using the original native glyphs, selection and an opaque colored
/// gutter. This keeps foregrounds unchanged and prevents horizontally scrolled text bleeding into
/// line numbers. The original native entity remains the only input and accessibility authority.
pub fn native_editor(editor: &Entity<EditorState>, cx: &App) -> AnyElement {
    if cx
        .try_global::<GeometryOnly>()
        .is_some_and(|scope| scope.0 > 0)
    {
        return super::scrollbar::native_editor(editor);
    }
    let marks = ranges(editor, cx);
    let native = super::scrollbar::native_editor(editor);
    if marks.is_empty() {
        return super::scroll::smooth_editor(editor, native);
    }
    let source = editor.clone();
    let paint_source = source.clone();
    let paint_marks = marks.clone();
    let readers = cx
        .try_global::<Registry>()
        .and_then(|registry| registry.1.get(&editor.entity_id()))
        .into_iter()
        .flatten()
        .filter_map(Weak::upgrade)
        .collect::<Vec<_>>();
    let paint_readers = readers.clone();
    let content = div()
        .relative()
        .size_full()
        .child(native)
        .child(
            canvas(
                move |bounds, window, cx| {
                    // Admission uses the preceding native frame; paint rechecks current geometry
                    // and schedules a finite follow-up if the new native viewport exposes a band.
                    let previous = source.read(cx);
                    let geometry = readers
                        .iter()
                        .find_map(|reader| reader.read(previous, window, cx));
                    let clip = bounds.intersect(&window.content_mask().bounds);
                    // Native scrollbar prepaint has applied this copy's requested offset, while
                    // text_bounds still describes the preceding paint. Include that difference
                    // so newly visible bands are admitted immediately rather than notifying an
                    // unchanged editor (which would revoke its private-layout geometry cache).
                    let old_scroll_y = previous
                        .text_bounds()
                        .map_or(previous.scroll_offset().y, |text| {
                            text.top() - previous.input_bounds().top()
                        });
                    let delta = bounds.top() - previous.input_bounds().top()
                        + previous.scroll_offset().y
                        - old_scroll_y;
                    let visible = visible_bytes(previous, geometry.as_ref(), clip, delta);
                    let admitted = marks
                        .iter()
                        .filter(|mark| intersects_bytes(mark, &visible))
                        .filter(|mark| {
                            row_bounds(previous, mark.range(), geometry.as_ref()).is_some_and(
                                |mut row| {
                                    row.origin.y += delta;
                                    row.intersect(&clip).size.height > px(0.)
                                },
                            )
                        })
                        .cloned()
                        .collect::<Vec<_>>();
                    let mut pieces = Vec::new();
                    for mark in admitted {
                        let mut native = super::scrollbar::native_editor(&source);
                        native.prepaint_as_root(bounds.origin, bounds.size.into(), window, cx);
                        pieces.push((mark, native));
                    }
                    pieces
                },
                move |bounds, pieces, window, cx| {
                    paint_bands(
                        &paint_source,
                        bounds,
                        &paint_marks,
                        pieces,
                        &paint_readers,
                        window,
                        cx,
                    )
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
        .into_any_element();
    super::scroll::smooth_editor(editor, content)
}
