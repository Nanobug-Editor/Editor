//! Reorder native editor pixels at a diff seam without changing its text or line-number state.
use super::*;
use gpui_base::input::RopeExt as _;
use gpui_kit::{InteractiveElement as _, ParentElement as _, Point, Styled as _, div};

/// Exact public native geometry from the last authority paint; no digit/font estimates are used.
#[derive(Clone, Copy, Default, PartialEq)]
pub(super) struct Metrics {
    /// Native line numbers, folding reservation and outer left padding, in logical pixels.
    pub width: Pixels,
    /// Full-width native header above the code input, including search controls.
    pub header: Pixels,
    /// Last physical pane height for the independent native visual mount.
    pub height: Pixels,
    /// Full source width retained by the clipped gutter copy to preserve native wrapping.
    native_width: Pixels,
}

impl Metrics {
    /// Measure the original `editor` in `viewport`; `top` is its authority mount displacement.
    /// Keep `old` exact width when a wrapped line start is not currently laid out.
    pub fn read(editor: &EditorState, viewport: Bounds<Pixels>, top: Pixels, old: Self) -> Self {
        let input = editor.input_bounds();
        let row = editor.text().offset_to_point(editor.cursor()).row;
        let width = [0, editor.text().line_start_offset(row)]
            .into_iter()
            .find_map(|offset| {
                let caret = editor.range_to_bounds(&(offset..offset))?;
                // text_bounds contains horizontal scroll; subtracting it cancels that scroll.
                Some(caret.left() - editor.text_bounds()?.left() + input.left() - viewport.left())
            })
            .unwrap_or(old.width)
            // A narrow pane can devote more than half its width to native numbers/folding.
            // Clipping to half would leave part of that gutter stranded beside the text.
            .clamp(px(0.), viewport.size.width.max(px(0.)));
        Self {
            width,
            header: (input.top() - viewport.top() - top).max(px(0.)),
            height: viewport.size.height,
            native_width: viewport.size.width,
        }
    }
}

/// A painted horizontal permutation, independent of vertical alignment and native search mode.
#[derive(Clone, Copy, PartialEq)]
pub(super) struct Projection {
    /// Completed physical pane, excluding any external alignment scroll rails.
    pub viewport: Bounds<Pixels>,
    /// Width moved from the leading edge to the trailing edge.
    width: Pixels,
    /// Native full-width header omitted from pointer translation.
    header: Pixels,
}

impl Projection {
    /// Describe the pixels painted using `metrics`; zero-width gutters require no mapping.
    /// `header` preserves the native search/header surface during independent presentation.
    pub fn new(viewport: Bounds<Pixels>, metrics: Metrics, header: bool) -> Option<Self> {
        (metrics.width > px(0.)).then_some(Self {
            viewport,
            width: metrics.width,
            header: if header { metrics.header } else { px(0.) },
        })
    }

    /// The physical input capture surface, excluding the untouched native header.
    pub fn code_bounds(self) -> Bounds<Pixels> {
        let mut bounds = self.viewport;
        bounds.origin.y += self.header;
        bounds.size.height = (bounds.size.height - self.header).max(px(0.));
        bounds
    }

    /// Translate an original native text/IME rectangle into the visible code body.
    pub fn text_bounds(self, mut native: Bounds<Pixels>) -> Bounds<Pixels> {
        native.origin.x -= self.width;
        native
    }

    /// Translate a native mouse anchor, preserving which side of the gutter boundary owns it.
    pub fn physical_point(self, mut native: Point<Pixels>) -> Point<Pixels> {
        native.x += if native.x < self.viewport.left() + self.width {
            self.viewport.size.width - self.width
        } else {
            -self.width
        };
        native
    }

    /// Convert `physical` input into native coordinates; a captured `anchor` retains its initial
    /// region across the discontinuous body/gutter seam. Gutter drags preserve their native column.
    pub fn native_point(
        self,
        mut physical: Point<Pixels>,
        anchor: Option<Point<Pixels>>,
    ) -> Point<Pixels> {
        let edge = self.viewport.right() - self.width;
        if anchor.unwrap_or(physical).x >= edge {
            physical.x = anchor.unwrap_or(physical).x - self.viewport.size.width + self.width;
        } else {
            physical.x = physical.x.min(edge - px(0.5)) + self.width;
        }
        physical
    }
}

/// Split only painted pixels. Each copy has the original full width, so wrapping is unchanged;
/// the caller mounts the original native authority last for layout, input and accessibility.
/// `top`/`height` locate the original source mount inside its clip; `header` keeps search full width.
/// `native` repeats the owner's atomic control, including its existing native header and overlays.
pub(super) fn pixels(
    editor: &Entity<EditorState>,
    metrics: Metrics,
    top: Pixels,
    height: Pixels,
    header: bool,
    mut native: impl FnMut() -> AnyElement,
) -> AnyElement {
    if metrics.width <= px(0.) {
        return div()
            .absolute()
            .top(top)
            .left_0()
            .w_full()
            .h(height)
            .child(native())
            .into_any_element();
    }
    let header_height = if header { metrics.header } else { px(0.) };
    let mut surface = div().absolute().top_0().left_0().size_full();
    if header_height > px(0.) {
        surface = surface.child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .w_full()
                .h(header_height)
                .overflow_hidden()
                .child(
                    div()
                        .absolute()
                        .top(top)
                        .left_0()
                        .w_full()
                        .h(height)
                        .child(native()),
                ),
        );
    }
    let selector = format!("aligned-inner-gutter-{:?}", editor.entity_id());
    surface
        .child(
            div()
                .absolute()
                .top(header_height)
                .bottom_0()
                .left_0()
                .right(metrics.width)
                .overflow_hidden()
                .child(
                    div()
                        .absolute()
                        .top(top - header_height)
                        .left(-metrics.width)
                        .right_0()
                        .h(height)
                        .child(native()),
                ),
        )
        .child(
            div()
                .id("aligned-inner-gutter")
                .debug_selector(move || selector.clone())
                .absolute()
                .top(header_height)
                .bottom_0()
                .right_0()
                .w(metrics.width)
                .overflow_hidden()
                .child(
                    div()
                        .absolute()
                        .top(top - header_height)
                        .left_0()
                        .w(metrics.native_width)
                        .h(height)
                        .child(native()),
                ),
        )
        .into_any_element()
}
