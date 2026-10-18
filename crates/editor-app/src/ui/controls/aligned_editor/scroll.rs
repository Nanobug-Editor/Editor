//! Local scrollbar metrics reuse Base's scrollbar gestures and the group's shared pixel viewport.
use super::*;
use gpui_base::ScrollbarHandle;
use gpui_kit::{Point, Size, point, size};

pub(super) const RAIL: f32 = 12.;

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
