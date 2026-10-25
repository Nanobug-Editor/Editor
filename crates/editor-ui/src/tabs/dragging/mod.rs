//! Floating document-tab reordering retains Base activation and commits one move on release.

mod gesture;
mod preview;
mod state;
#[cfg(test)]
mod tests;

pub use state::{FileTabDragState, FileTabMove};

use crate::{
    scroll::{ScrollAxis, SmoothScroll},
    theme::ActiveTheme as _,
};
use gpui_base::{ElementExt as _, Tab};
use gpui_kit::{
    Along as _, AnimationExt as _, AnyElement, App, Axis, ElementId, ImageSource,
    InteractiveElement, IntoElement, ParentElement, RenderOnce, ScrollHandle, SharedString,
    SpringAnimation, SpringConfig, SpringPlayback, StatefulInteractiveElement, Styled, Window,
    canvas, div, prelude::FluentBuilder as _, px, relative,
};
use std::rc::Rc;

/// Immutable visual information displayed in the native cursor-following tab card.
#[derive(Clone)]
pub struct FileTabPreview {
    /// Already localized title, including any caller-owned dirty/conflict marks.
    pub label: SharedString,
    /// The same immutable icon used by the source tab.
    pub icon: ImageSource,
    /// Show the local badge for a replaceable preview document.
    pub preview: bool,
    /// Preserve the italic title of a document outside the workspace.
    pub external: bool,
}

/// Caller-supplied Base tab and stable identity; the strip never owns a document model.
struct Item {
    /// Identity independent of visible index and display text.
    id: u64,
    /// Read-only data for the native floating card.
    preview: FileTabPreview,
    /// Original pointer/accessibility behavior and caller-owned tab content.
    tab: Tab,
}

/// A tab viewport with floating reorder previews and caller-owned canonical order.
/// Keep `state` and `scroll` across frames; IDs must be unique within the visible strip.
#[derive(IntoElement)]
pub struct FileTabStrip {
    /// Stable identity for the native scroll viewport and wheel routing.
    id: ElementId,
    /// Derived geometry and the current gesture, never the canonical order.
    state: FileTabDragState,
    /// Original handle keeps native reveal-by-index and scrollbar behavior intact.
    scroll: ScrollHandle,
    /// Horizontal content-sized tabs or full-width vertical rows share one drag algorithm.
    axis: Axis,
    /// Reorderable visible tabs in their canonical order.
    items: Vec<Item>,
    /// Nonreorderable content, such as comparison tabs, after the document tabs.
    trailing: Vec<AnyElement>,
    /// Called only for a changed, valid release.
    sink: Rc<dyn Fn(&FileTabMove, &mut Window, &mut App)>,
}

impl FileTabStrip {
    /// Create a strip identified by `id`, retaining gesture `state` and the existing `scroll`.
    pub fn new(id: impl Into<ElementId>, state: FileTabDragState, scroll: ScrollHandle) -> Self {
        Self {
            id: id.into(),
            state,
            scroll,
            axis: Axis::Horizontal,
            items: Vec::new(),
            trailing: Vec::new(),
            sink: Rc::new(|_, _, _| {}),
        }
    }

    /// Arrange tabs along `axis`, defaulting to horizontal; changing it cancels a live drag.
    /// Vertical rows fill the viewport width at `PANEL_HEADER_HEIGHT`, with horizontal text.
    /// After changing layout, queue `scroll_to_item` with `Window::on_next_frame`: GPUI's
    /// first prepaint consumes reveal requests before updating the handle's axis and bounds.
    pub fn axis(mut self, axis: Axis) -> Self {
        self.axis = axis;
        self
    }

    /// Append `tab` under stable `id`; `preview` is its read-only floating representation.
    pub fn tab(mut self, id: u64, preview: FileTabPreview, tab: Tab) -> Self {
        self.items.push(Item { id, preview, tab });
        self
    }

    /// Append nonreorderable `child` after all visible document tabs, preserving scroll indices.
    pub fn trailing(mut self, child: impl IntoElement) -> Self {
        self.trailing.push(child.into_any_element());
        self
    }

    /// Receive a validated `source`/`before` move in `window`/`cx`; cancellation emits nothing.
    pub fn on_reorder(
        mut self,
        sink: impl Fn(&FileTabMove, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.sink = Rc::new(sink);
        self
    }
}

impl RenderOnce for FileTabStrip {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let axis = self.axis;
        if self.state.synchronize(
            self.items.iter().map(|item| item.id).collect(),
            axis,
            window,
            cx,
        ) {
            // One retained handle can move between a horizontal bar and a vertical list.
            // Discard the previous axis's offset without losing a restored current-axis offset.
            self.scroll
                .set_offset(self.scroll.offset().apply_along(axis.invert(), |_| px(0.)));
        }
        let mut viewport = div()
            .id(self.id.clone())
            .relative()
            .w_full()
            .h_full()
            .flex()
            .track_scroll(&self.scroll)
            .map(|viewport| match axis {
                Axis::Horizontal => viewport.overflow_x_scroll(),
                Axis::Vertical => viewport.flex_col().overflow_y_scroll(),
            });
        for item in self.items {
            let measured = self.state.clone();
            let dragging = self.state.clone();
            let scroll = self.scroll.clone();
            let id = item.id;
            let (source, shift) = self.state.0.borrow().presentation(id);
            let source_size = self
                .state
                .0
                .borrow()
                .session
                .as_ref()
                .filter(|session| session.source == id)
                .map(|session| session.source_bounds.size);
            let active = self.state.active();
            let tab = item
                .tab
                .relative()
                .when(axis == Axis::Vertical, |tab| {
                    tab.w_full()
                        .min_w(px(0.))
                        .max_w(relative(1.))
                        .h(px(crate::PANEL_HEADER_HEIGHT))
                        .min_h(px(crate::PANEL_HEADER_HEIGHT))
                        .max_h(px(crate::PANEL_HEADER_HEIGHT))
                })
                .when(source, |tab| tab.opacity(0.))
                // Dirty markers and renamed titles may change during a drag. Freeze both
                // allocation and child dimensions so the gap and neighbor shifts stay identical.
                .when_some(source_size, |tab, size| {
                    tab.w(size.width)
                        .min_w(size.width)
                        .max_w(size.width)
                        .h(size.height)
                        .min_h(size.height)
                        .max_h(size.height)
                })
                .on_drag(gesture::DraggedTab(id), move |drag, _, window, cx| {
                    dragging.start(drag.0, scroll.clone(), window, cx);
                    preview::floating(&dragging, item.preview.clone(), window, cx)
                })
                .with_spring(
                    ("file-tab-shift", id),
                    SpringAnimation::new(SpringConfig::new(900., 60., 1.))
                        .to(shift)
                        .from(px(0.))
                        .with_epsilon(0.1)
                        // The canonical layout changes on commit; old relative offsets must
                        // not be added to the new item positions on the release frame.
                        .playback(if active {
                            SpringPlayback::Running
                        } else {
                            SpringPlayback::Completed
                        }),
                    move |tab, shift| match axis {
                        Axis::Horizontal => tab.left(shift),
                        Axis::Vertical => tab.top(shift),
                    },
                );
            viewport = viewport.child(
                div()
                    .id(("file-tab-slot", id))
                    .debug_selector(move || format!("file-tab-slot-{id}"))
                    .relative()
                    .map(|slot| match axis {
                        Axis::Horizontal => slot.h_full(),
                        Axis::Vertical => slot.w_full().h(px(crate::PANEL_HEADER_HEIGHT)),
                    })
                    .flex_shrink_0()
                    .when_some(source_size, |slot, size| slot.w(size.width).h(size.height))
                    // Observe the static wrapper, not the animated child's translated bounds.
                    .on_prepaint(move |bounds, _, _| {
                        measured.0.borrow_mut().items.insert(id, bounds);
                    })
                    .child(tab),
            );
        }
        viewport = viewport.children(self.trailing);
        if let Some(bounds) = self.state.0.borrow().placeholder() {
            let origin = self.state.0.borrow().viewport.origin;
            // Inset only across the strip so the gap always consumes the source's full extent.
            let inset = axis.invert();
            let local = (bounds.origin - origin - self.scroll.offset())
                .apply_along(inset, |value| value + px(3.));
            let size = bounds
                .size
                .apply_along(inset, |value| (value - px(6.)).max(px(0.)));
            viewport = viewport.child(
                div()
                    .debug_selector(|| "file-tab-drag-slot".into())
                    .absolute()
                    // Absolute children also inherit the native scroll offset.
                    .left(local.x)
                    .top(local.y)
                    .w(size.width)
                    .h(size.height)
                    .rounded(px(3.))
                    .border_1()
                    .border_color(cx.theme().primary.opacity(0.55))
                    .bg(cx.theme().primary.opacity(0.10)),
            );
        }
        let measured = self.state.clone();
        let state = self.state;
        let sink = self.sink;
        div()
            .debug_selector(|| "file-tab-strip".into())
            .relative()
            .size_full()
            // A non-scrolling parent supplies a true viewport; a canvas inside the scroll
            // content would itself inherit the scroll offset.
            .on_prepaint(move |bounds, window, _| {
                measured.0.borrow_mut().viewport = bounds.intersect(&window.content_mask().bounds);
            })
            .child(SmoothScroll::new(
                self.id,
                &self.scroll,
                match axis {
                    Axis::Horizontal => ScrollAxis::Horizontal,
                    Axis::Vertical => ScrollAxis::Vertical,
                },
                viewport,
            ))
            // Last paint registration runs first in the native bubble phase, after every
            // child has cleared its pressed state during capture but before click callbacks.
            .child(
                canvas(
                    |_, _, _| (),
                    move |_, _, window, _| gesture::install(&state, sink, window),
                )
                .absolute()
                .size_full(),
            )
    }
}
