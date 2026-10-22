//! A native scrollbar projects a guest-owned range without creating a second content model.
use gpui_base::ScrollbarHandle;
use gpui_kit::{Bounds, Pixels, Point, Size, point, px, size};
use plugin_runtime::plugin_protocol::ui::ScrollRange;
use std::{cell::RefCell, collections::VecDeque, rc::Rc};

/// The original guest range plus a bounded native scrollbar/motion preview.
#[derive(Clone, Default)]
pub(super) struct CanvasScroll(Rc<RefCell<State>>);
/// Guest acknowledgements can lag several vsyncs; they must not rewind newer local requests.
#[derive(Default)]
struct State {
    /// Actual native viewport, used by both the common rail and smooth-scroll component.
    bounds: Bounds<Pixels>,
    /// Last guest-owned range; no drawn content is duplicated in the host.
    range: Option<ScrollRange>,
    /// Most recently requested local position until the guest acknowledges it.
    offset: f32,
    /// One absolute request is emitted per native frame.
    pending: Option<f32>,
    /// A small ordered receipt queue distinguishes acknowledgements from external navigation.
    issued: VecDeque<f32>,
}
impl CanvasScroll {
    /// Project the latest local position only inside the guest's completely painted interval.
    /// A slow provider may delay new rows, but cannot expose unpainted or invented content.
    pub(super) fn translation(
        &self,
        buffer: &plugin_runtime::plugin_protocol::ui::ScrollBuffer,
    ) -> f32 {
        let state = self.0.borrow();
        let Some(range) = &state.range else {
            return 0.;
        };
        let maximum = (buffer.end - state.bounds.size.height / px(1.)).max(buffer.start);
        range.offset - state.offset.clamp(buffer.start, maximum)
    }
    /// Retiring a content identity cannot lend its pending drag offset to the next content.
    pub(super) fn reset(&self) {
        *self.0.borrow_mut() = State::default();
    }
    /// Drawing-only frames retain the local preview. Ordered acknowledgements cannot rewind
    /// newer native motion; unrelated guest navigation or changed content extents take priority.
    pub(super) fn update(&self, bounds: Bounds<Pixels>, range: Option<ScrollRange>) {
        let mut state = self.0.borrow_mut();
        state.bounds = bounds;
        if state.range != range {
            let acknowledgement = range.as_ref().and_then(|range| {
                (state
                    .range
                    .as_ref()
                    .is_some_and(|old| old.content == range.content))
                .then(|| {
                    state
                        .issued
                        .iter()
                        .position(|offset| (*offset - range.offset).abs() < 0.5)
                })
                .flatten()
            });
            if let Some(index) = acknowledgement {
                state.issued.drain(..=index);
            } else {
                state.offset = range.as_ref().map_or(0., |range| range.offset);
                state.pending = None;
                state.issued.clear();
            }
            state.range = range;
        }
        let maximum = state.range.as_ref().map_or(0., |range| {
            (range.content - bounds.size.height / px(1.)).max(0.)
        });
        state.offset = state.offset.clamp(0., maximum);
        if state.pending.is_some() {
            state.pending = Some(state.offset);
        }
    }
    /// Publish the latest absolute preview and retain a bounded receipt for asynchronous repaint.
    pub(super) fn take_offset(&self) -> Option<f32> {
        let mut state = self.0.borrow_mut();
        let offset = state.pending.take()?;
        state.issued.push_back(offset);
        // At 240 Hz this covers more than a second; stalled guests cannot grow host memory.
        if state.issued.len() > 256 {
            state.issued.pop_front();
        }
        Some(offset)
    }
}
impl ScrollbarHandle for CanvasScroll {
    fn viewport_bounds(&self) -> Bounds<Pixels> {
        self.0.borrow().bounds
    }
    fn offset(&self) -> Point<Pixels> {
        point(px(0.), px(-self.0.borrow().offset))
    }
    fn content_size(&self) -> Size<Pixels> {
        let state = self.0.borrow();
        size(
            state.bounds.size.width,
            px(state.range.as_ref().map_or(0., |range| range.content)),
        )
    }
    fn set_offset(&self, offset: Point<Pixels>) {
        let mut state = self.0.borrow_mut();
        if let Some(range) = &state.range {
            let maximum = (range.content - state.bounds.size.height / px(1.)).max(0.);
            state.offset = (-offset.y / px(1.)).clamp(0., maximum);
            state.pending = Some(state.offset);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Late guest acknowledgements retain newer motion; external navigation and replacement win.
    #[test]
    fn ordered_acknowledgements_preserve_newer_preview_and_navigation_retires_it() {
        let scroll = CanvasScroll::default();
        let bounds = Bounds::new(point(px(0.), px(0.)), size(px(300.), px(200.)));
        let range = |offset| {
            Some(ScrollRange {
                content: 2000.,
                offset,
            })
        };
        scroll.update(bounds, range(0.));
        for offset in [20., 40.] {
            scroll.set_offset(point(px(0.), px(-offset)));
            assert_eq!(scroll.take_offset(), Some(offset));
        }
        scroll.set_offset(point(px(0.), px(-60.)));
        scroll.update(bounds, range(20.));
        assert_eq!(scroll.offset().y, px(-60.));
        assert_eq!(scroll.take_offset(), Some(60.));
        scroll.update(bounds, range(40.));
        assert_eq!(scroll.offset().y, px(-60.));
        scroll.update(bounds, range(500.));
        assert_eq!(scroll.offset().y, px(-500.));
        assert_eq!(scroll.take_offset(), None);
        scroll.reset();
        scroll.update(bounds, range(0.));
        assert_eq!(scroll.offset().y, px(0.));
    }
}
