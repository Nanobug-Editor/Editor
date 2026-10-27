//! Local scrollbar metrics reuse Base's scrollbar gestures and the group's shared pixel viewport.
use super::*;
use gpui_base::ScrollbarHandle;
use gpui_kit::{Point, Size, point, size};

pub(super) const RAIL: f32 = 12.;

/// Completed outer-column and native chrome measurements; neither value depends on our inset.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct ColumnMetrics {
    /// Actual column allocation, including pixel rounding and its reading/rail reservations.
    pub width: Pixels,
    /// Native line numbers, folding controls and text padding that consume wrap width.
    pub native_inset: Pixels,
}

impl ColumnMetrics {
    /// Read the original native glyph origin after authority paint, cancelling horizontal scroll.
    pub fn read(width: Pixels, viewport: Bounds<Pixels>, editor: &EditorState) -> Option<Self> {
        use gpui_base::input::RopeExt as _;
        let row = editor.visible_row_range()?.start;
        let offset = editor.text().line_start_offset(row);
        let origin = editor.range_to_bounds(&(offset..offset))?.left();
        let text_width = editor.input_bounds().right() - origin + editor.scroll_offset().x;
        let native_inset = (viewport.size.width - text_width).max(px(0.));
        Some(Self {
            width,
            native_inset,
        })
    }
}

impl GroupState {
    /// Reserve the shared bottom rail only while at least one native source actually overflows.
    /// All panes keep one height, including a short peer beside an unwrapped long source.
    pub(super) fn horizontal_rail_height(&self) -> Pixels {
        if self
            .measurements
            .iter()
            .flatten()
            .any(|measured| measured.source_width > measured.viewport.size.width + px(0.5))
        {
            px(RAIL)
        } else {
            px(0.)
        }
    }

    /// Reserve equal total width in comparison columns: the first source already has reading
    /// clearance, while peers reserve the same amount beside their native input. Generic code
    /// groups keep their independent rails. `index` identifies the existing source membership.
    pub(super) fn source_right_inset(&self, index: usize) -> Pixels {
        if self.inner_gutters {
            let leading = if index == 0 {
                px(gutter::READING_INSET)
            } else {
                px(0.)
            };
            let reserved = px(RAIL.max(gutter::READING_INSET));
            if self.columns.iter().all(Option::is_some)
                && let Some(own) = self.columns.get(index).and_then(Option::as_ref)
            {
                // Outer widths are measured independently of this inset, preventing a feedback
                // loop when a digit boundary, resize or DPI change alters native gutter width.
                let common = self
                    .columns
                    .iter()
                    .flatten()
                    .map(|column| (column.width - reserved - column.native_inset).max(px(0.)))
                    .reduce(Pixels::min)
                    .unwrap_or(px(0.));
                return (own.width - leading - own.native_inset - common).max(px(0.));
            }
            reserved - leading
        } else {
            px(RAIL)
        }
    }

    /// Restore the same independent native envelope after a geometry refusal. Actual outer
    /// allocation avoids mistaking fractional inset rounding for another window resize.
    pub(super) fn independent_width(&self, index: usize, measured_width: Pixels) -> Pixels {
        if self.inner_gutters
            && let Some(column) = self.columns.get(index).and_then(Option::as_ref)
        {
            column.width
                - if index == 0 {
                    px(gutter::READING_INSET)
                } else {
                    px(0.)
                }
        } else {
            measured_width + self.source_right_inset(index)
        }
    }

    /// Comparison panes share one y viewport, so only the last source exposes its vertical rail.
    /// Native code groups without comparison gutters retain their existing per-pane controls.
    pub(super) fn shows_vertical_scrollbar(&self, index: usize) -> bool {
        !self.inner_gutters || index + 1 == self.sources.len()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Metrics {
    viewport: Bounds<Pixels>,
    content: Size<Pixels>,
    native_x: Pixels,
}

/// Only presentation coordinates live here. Horizontal requests are applied through the original
/// EditorState scroll API; vertical requests update the one shared projected scroll cell.
#[derive(Clone)]
pub(super) struct Handle {
    shared_y: Rc<Cell<Pixels>>,
    metrics: Rc<Cell<Metrics>>,
    requested_x: Rc<Cell<Option<Pixels>>>,
}

impl Handle {
    pub fn new(shared_y: Rc<Cell<Pixels>>) -> Self {
        Self {
            shared_y,
            metrics: Rc::new(Cell::new(Metrics {
                viewport: Bounds::default(),
                content: Size::default(),
                native_x: px(0.),
            })),
            requested_x: Rc::new(Cell::new(None)),
        }
    }

    /// Return a horizontal drag request once; the native source still clamps its actual offset.
    pub fn take_horizontal(&self) -> Option<Pixels> {
        self.requested_x.take()
    }

    pub fn update(
        &self,
        measurement: &Measurement,
        display_height: f32,
        viewport: Bounds<Pixels>,
        native_x: Pixels,
    ) -> bool {
        let metrics = Metrics {
            viewport,
            content: size(
                measurement
                    .source_width
                    .max(measurement.viewport.size.width),
                px(display_height),
            ),
            native_x,
        };
        let changed = self.metrics.get() != metrics;
        self.metrics.set(metrics);
        changed
    }
}

impl ScrollbarHandle for Handle {
    fn viewport_bounds(&self) -> Bounds<Pixels> {
        self.metrics.get().viewport
    }
    fn offset(&self) -> Point<Pixels> {
        point(self.metrics.get().native_x, -self.shared_y.get())
    }
    fn content_size(&self) -> Size<Pixels> {
        self.metrics.get().content
    }

    fn set_offset(&self, offset: Point<Pixels>) {
        let metrics = self.metrics.get();
        let max = (metrics.content.height - metrics.viewport.size.height).max(px(0.));
        self.shared_y.set((-offset.y).clamp(px(0.), max));
        if offset.x != metrics.native_x {
            self.requested_x.set(Some(offset.x));
        }
    }
}
