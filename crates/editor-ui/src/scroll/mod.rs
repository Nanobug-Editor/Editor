//! Native scroll surfaces share bounded wheel motion; Base retains layout and scrollbar behavior.
mod inertia;
mod router;
mod smooth;
mod targets;
#[cfg(test)]
mod tests;

pub(crate) use router::init;
pub use smooth::{ScrollAxis, SmoothScroll};
pub(crate) use targets::{editor as smooth_editor, textarea as smooth_textarea};

/// Projected editors retain their original capture fallback in windows without the shared router.
pub(crate) fn routes_wheel(window: &Window, cx: &App) -> bool {
    router::for_window(window, cx).is_some()
}

/// Cancel before projected pointer capture can redraw native geometry and advance the viewport.
/// The ordinary router also cancels on native capture; this covers events deferred by projection.
pub(crate) fn cancel_motion(window: &Window, cx: &mut App) {
    if let Some(router) = router::for_window(window, cx) {
        router.cancel(None, cx);
    }
}

/// Attach common wheel motion after composing any native tree, list or scroll surface.
pub trait SmoothScrollable: IntoElement + Sized {
    /// Reuse `handle` and `axis`; the call-site identity remains stable across ordinary rerenders.
    /// Loops sharing one call site must use `SmoothScroll::new` with distinct viewport IDs.
    #[track_caller]
    fn smooth_scroll<H: gpui_base::ScrollbarHandle + Clone>(
        self,
        handle: &H,
        axis: ScrollAxis,
    ) -> SmoothScroll {
        let caller = std::panic::Location::caller();
        SmoothScroll::new(
            SharedString::from(format!(
                "wheel:{}:{}:{}",
                caller.file(),
                caller.line(),
                caller.column()
            )),
            handle,
            axis,
            self,
        )
    }
}
impl<E: IntoElement> SmoothScrollable for E {}
use gpui_kit::{
    App, Element, ElementId, InteractiveElement, Interactivity, IntoElement, ParentElement,
    RenderOnce, ScrollHandle, SharedString, StatefulInteractiveElement, StyleRefinement, Styled,
    Window,
};

/// Attach the product's vertical scrollbar without introducing an extra scroll container.
pub trait ScrollableElement:
    Element + StatefulInteractiveElement + ParentElement + Styled + Sized + 'static
{
    /// Use a persistent Base scroll handle keyed by this caller's source location.
    #[track_caller]
    fn overflow_y_scrollbar(self) -> Scrollable<Self> {
        Scrollable::new(self, false, None)
    }

    /// Reuse `handle` so keyboard reveal and caller-restored offsets keep their original owner.
    #[track_caller]
    fn overflow_y_scrollbar_with(self, handle: &ScrollHandle) -> Scrollable<Self> {
        Scrollable::new(self, false, Some(handle.clone()))
    }

    /// Horizontal viewports use the same activity hold and thickness contraction.
    #[track_caller]
    fn overflow_x_scrollbar(self) -> Scrollable<Self> {
        Scrollable::new(self, true, None)
    }
}
impl<E: Element + StatefulInteractiveElement + ParentElement + Styled + 'static> ScrollableElement
    for E
{
}
/// A viewport plus its local scrollbar; the handle owns offsets independently of rerenders.
#[derive(IntoElement)]
pub struct Scrollable<E: Element + StatefulInteractiveElement + ParentElement + Styled + 'static> {
    /// Original caller-owned scroll viewport.
    element: E,
    /// Stable per-call-site scroll identity.
    id: ElementId,
    /// Horizontal surfaces contract vertically; the content viewport never changes size.
    horizontal: bool,
    /// Explicit caller-owned handles preserve keyboard and programmatic reveal behavior.
    handle: Option<ScrollHandle>,
}

impl<E: Element + StatefulInteractiveElement + ParentElement + Styled + 'static> Scrollable<E> {
    /// Capture source-location identity for `element`, its axis and an optional existing handle.
    #[track_caller]
    fn new(element: E, horizontal: bool, handle: Option<ScrollHandle>) -> Self {
        let caller = std::panic::Location::caller();
        let viewport_id = Element::id(&element);
        Self {
            element,
            id: SharedString::from(format!(
                "local-scroll:{}:{}:{}:{viewport_id:?}",
                caller.file(),
                caller.line(),
                caller.column()
            ))
            .into(),
            horizontal,
            handle,
        }
    }
}
impl<E: Element + StatefulInteractiveElement + ParentElement + Styled + 'static> RenderOnce
    for Scrollable<E>
{
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let motion_id = self.id.clone();
        let handle = self.handle.unwrap_or_else(|| {
            window
                .use_keyed_state(self.id, cx, |_, _| ScrollHandle::new())
                .read(cx)
                .clone()
        });
        let element = if self.horizontal {
            self.element.overflow_x_scroll()
        } else {
            self.element.overflow_y_scroll()
        };
        let bar = super::vertical_scrollbar(&handle, cx).axis(if self.horizontal {
            gpui_base::ScrollbarAxis::Horizontal
        } else {
            gpui_base::ScrollbarAxis::Vertical
        });
        SmoothScroll::new(
            motion_id,
            &handle,
            if self.horizontal {
                ScrollAxis::Horizontal
            } else {
                ScrollAxis::Vertical
            },
            element.track_scroll(&handle).child(bar),
        )
    }
}

impl<E: Element + StatefulInteractiveElement + ParentElement + Styled + 'static> Styled
    for Scrollable<E>
{
    fn style(&mut self) -> &mut StyleRefinement {
        self.element.style()
    }
}

impl<E: Element + StatefulInteractiveElement + ParentElement + Styled + 'static> InteractiveElement
    for Scrollable<E>
{
    fn interactivity(&mut self) -> &mut Interactivity {
        self.element.interactivity()
    }
}

impl<E: Element + StatefulInteractiveElement + ParentElement + Styled + 'static>
    StatefulInteractiveElement for Scrollable<E>
{
}
impl<E: Element + StatefulInteractiveElement + ParentElement + Styled + 'static> ParentElement
    for Scrollable<E>
{
    fn extend(&mut self, elements: impl IntoIterator<Item = gpui_kit::AnyElement>) {
        self.element.extend(elements);
    }
}
