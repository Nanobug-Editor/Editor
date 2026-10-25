//! A direct native owner supplies the authoritative range; Base owns scrollbar capture and appearance.

use super::*;
use gpui_base::ScrollbarHandle;
use std::cell::RefCell;

/// Shared rail position and one pending absolute request, without a second content buffer.
#[derive(Clone, Default)]
pub(super) struct GridScrollbar(Rc<RefCell<State>>);

/// Native bounds and extent follow the owner's most recent drawing.
#[derive(Default)]
struct State {
    bounds: Bounds<Pixels>,
    range: Option<GridScroll>,
    pending: Option<f32>,
}

impl GridScrollbar {
    /// Refresh actual viewport geometry; content remains solely owned by the grid's caller.
    pub fn update(&self, bounds: Bounds<Pixels>, range: Option<GridScroll>) {
        let mut state = self.0.borrow_mut();
        state.bounds = bounds;
        state.range = range;
    }

    /// Deliver the newest captured thumb position once in the native layout pass.
    pub fn take_offset(&self) -> Option<f32> {
        self.0.borrow_mut().pending.take()
    }
}

impl ScrollbarHandle for GridScrollbar {
    fn viewport_bounds(&self) -> Bounds<Pixels> {
        self.0.borrow().bounds
    }
    fn offset(&self) -> Point<Pixels> {
        let state = self.0.borrow();
        point(
            px(0.),
            px(-state
                .pending
                .unwrap_or_else(|| state.range.map_or(0., |range| range.offset))),
        )
    }
    fn content_size(&self) -> gpui_kit::Size<Pixels> {
        let state = self.0.borrow();
        size(
            state.bounds.size.width,
            px(state.range.map_or(0., |range| range.content)),
        )
    }
    fn set_offset(&self, offset: Point<Pixels>) {
        let mut state = self.0.borrow_mut();
        if let Some(range) = state.range {
            state.pending = Some((-offset.y / px(1.)).clamp(
                0.,
                (range.content - state.bounds.size.height / px(1.)).max(0.),
            ));
        }
    }
}
