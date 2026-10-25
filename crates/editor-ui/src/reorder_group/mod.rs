//! Long-press reordering over ordinary native controls; callers retain the canonical item order.

mod gesture;
mod state;
#[cfg(test)]
mod tests;

pub use state::{ReorderEvent, ReorderGroupState};

use crate::{Icon, theme::ActiveTheme as _};
use gpui_base::{ElementExt as _, StyledExt as _};
use gpui_kit::{
    Anchor, AnyElement, App, AppContext as _, Context, ElementId, InteractiveElement, IntoElement,
    ParentElement, Render, RenderOnce, SharedString, StatefulInteractiveElement, StyleRefinement,
    Styled, Window, anchored, canvas, div, point, prelude::FluentBuilder as _, px,
};
use std::rc::Rc;

/// One visible item and its native control; IDs must be unique within the group.
struct Item {
    /// Caller-owned identity, stable across native frames and reorders.
    id: SharedString,
    /// Already localized text for the cursor-following preview.
    label: SharedString,
    /// Existing control whose short click and keyboard behavior remain intact.
    child: AnyElement,
}

/// A horizontal (or vertical) group that emits validated moves after a 350 ms hold and drag.
/// Keep `state` across renders, and apply emitted moves to the caller's canonical order.
#[derive(IntoElement)]
pub struct ReorderGroup {
    /// Retained GPUI element identity for the group.
    id: ElementId,
    /// Gesture state shared with delayed arming and the native drag preview.
    state: ReorderGroupState,
    /// Current visible items in the caller's order.
    items: Vec<Item>,
    /// Select Y-axis insertion geometry for overflow lists.
    vertical: bool,
    /// Caller-supplied layout refinements, applied to the group only.
    style: StyleRefinement,
    /// The only mutation seam; the caller applies and persists a validated move.
    sink: Rc<dyn Fn(&ReorderEvent, &mut Window, &mut App)>,
}

impl ReorderGroup {
    /// Create a horizontal group with stable element `id` and retained gesture `state`.
    pub fn new(id: impl Into<ElementId>, state: ReorderGroupState) -> Self {
        Self {
            id: id.into(),
            state,
            items: Vec::new(),
            vertical: false,
            style: StyleRefinement::default(),
            sink: Rc::new(|_, _, _| {}),
        }
    }

    /// Stack items vertically and use their vertical midpoints as insertion targets.
    pub fn vertical(mut self) -> Self {
        self.vertical = true;
        self
    }

    /// Append visible `child` with stable `id`; `label` names its native drag preview.
    /// Hidden items are omitted; a move to the end refers to this visible list only.
    pub fn item(
        mut self,
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        child: impl IntoElement,
    ) -> Self {
        self.items.push(Item {
            id: id.into(),
            label: label.into(),
            child: child.into_any_element(),
        });
        self
    }

    /// Receive a changed, valid move in `window`/`cx`; no callback occurs on cancellation.
    pub fn on_reorder(
        mut self,
        sink: impl Fn(&ReorderEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.sink = Rc::new(sink);
        self
    }
}

impl Styled for ReorderGroup {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ReorderGroup {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ids = self.items.iter().map(|item| item.id.clone()).collect();
        self.state.synchronize(ids, self.vertical, window, cx);
        let measured = self.state.clone();
        let mut group = div()
            .id(self.id)
            .relative()
            .flex()
            .flex_shrink_0()
            .when(self.vertical, |group| group.flex_col())
            .refine_style(&self.style)
            .on_prepaint(move |bounds, window, _| {
                let mut state = measured.0.borrow_mut();
                state.origin = bounds.origin;
                state.bounds = bounds.intersect(&window.content_mask().bounds);
            });
        for item in self.items {
            let measured = self.state.clone();
            let measured_id = item.id.clone();
            let pressed = self.state.clone();
            let source = item.id.clone();
            let preview_label = item.label.clone();
            let dragged = self.state.clone();
            let can_drag = self
                .state
                .0
                .borrow()
                .gesture
                .as_ref()
                .is_some_and(|gesture| {
                    gesture.source == item.id && gesture.armed && !gesture.cancelled
                });
            let selector = format!("reorder-item-{}", item.id);
            group = group.child(
                div()
                    .id(item.id.clone())
                    .debug_selector(move || selector.clone())
                    .relative()
                    .flex_shrink_0()
                    .when(self.vertical, |item| item.w_full())
                    .on_prepaint(move |bounds, _, _| {
                        measured
                            .0
                            .borrow_mut()
                            .item_bounds
                            .insert(measured_id.clone(), bounds);
                    })
                    .capture_any_mouse_down(move |event, window, cx| {
                        if event.button == gpui_kit::MouseButton::Left {
                            pressed.begin(source.clone(), event.position, window, cx);
                        }
                    })
                    // A native click listener retains the initial down across frames, while
                    // the native drag listener is installed only after this source is armed.
                    .on_click(|_, _, _| {})
                    .when(can_drag, |item_element| {
                        item_element.on_drag(item.id, move |_, _, window, cx| {
                            dragged.start_drag(window);
                            cx.new(|_| DragPreview {
                                state: dragged.clone(),
                                label: preview_label.clone(),
                            })
                        })
                    })
                    .child(item.child),
            );
        }
        let indicator = self.state.0.borrow().indicator();
        if let Some(bounds) = indicator {
            let origin = self.state.0.borrow().origin;
            group = group.child(
                div()
                    .debug_selector(|| "reorder-insertion".into())
                    .absolute()
                    .left(bounds.left() - origin.x)
                    .top(bounds.top() - origin.y)
                    .w(bounds.size.width)
                    .h(bounds.size.height)
                    .bg(cx.theme().primary),
            );
        }
        // Register last: capture lets native buttons clear their presses, then bubble suppresses
        // only drag releases before any child click callback can execute.
        let state = self.state;
        let sink = self.sink;
        group.child(
            canvas(
                |_, _, _| (),
                move |_, _, window, _| gesture::install(&state, sink.clone(), window),
            )
            .absolute()
            .size_full(),
        )
    }
}

/// Native GPUI owns drag cleanup; its public anchored primitive keeps the label off the target.
struct DragPreview {
    /// Current drop validity, shared with the owning visible group.
    state: ReorderGroupState,
    /// Localized label captured from the dragged item.
    label: SharedString,
}

impl Render for DragPreview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let invalid = self.state.0.borrow().target().is_none();
        let pointer = window.mouse_position();
        // Preview above/right of the pointer leaves the insertion line exposed. Near the top
        // edge, place it below instead; window anchoring also avoids left/right label clipping.
        let (anchor, offset_y) = if pointer.y < px(52.) {
            (Anchor::TopLeft, px(20.))
        } else {
            (Anchor::BottomLeft, px(-20.))
        };
        let preview = div()
            .debug_selector(|| "reorder-drag-preview".into())
            .flex()
            .items_center()
            .gap_1()
            .px_2()
            .py_1()
            .rounded_md()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().popover)
            .text_color(cx.theme().foreground)
            .text_size(px(12.))
            .max_w((window.viewport_size().width - px(8.)).min(px(280.)))
            .when(invalid, |preview| {
                preview.child(
                    div().debug_selector(|| "reorder-forbidden".into()).child(
                        Icon::default()
                            .data(FORBIDDEN_ICON)
                            .small()
                            .text_color(cx.theme().danger),
                    ),
                )
            })
            .child(div().min_w_0().text_ellipsis().child(self.label.clone()));
        anchored()
            .anchor(anchor)
            .position(point(pointer.x + px(12.), pointer.y + offset_y))
            .snap_to_window_with_margin(px(4.))
            .child(preview)
    }
}

/// The local monochrome prohibition mark follows both light and dark theme colors.
const FORBIDDEN_ICON: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="9"/><path d="m5.6 5.6 12.8 12.8"/></svg>"#;
