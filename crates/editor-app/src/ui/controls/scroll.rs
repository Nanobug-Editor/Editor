//! Local scroll surfaces keep the original element as viewport and use Base scrollbar behavior.
use gpui_kit::{
    App, Element, ElementId, IntoElement, ParentElement, RenderOnce, ScrollHandle, SharedString,
    StatefulInteractiveElement, Styled, Window,
};

/// Attach the product's vertical scrollbar without introducing an extra scroll container.
pub(crate) trait ScrollableElement:
    Element + StatefulInteractiveElement + ParentElement + Styled + Sized + 'static
{
    /// Use a persistent Base scroll handle keyed by this caller's source location.
    #[track_caller]
    fn overflow_y_scrollbar(self) -> Scrollable<Self> {
        let caller = std::panic::Location::caller();
        Scrollable {
            element: self,
            id: SharedString::from(format!(
                "local-scroll:{}:{}:{}",
                caller.file(),
                caller.line(),
                caller.column()
            ))
            .into(),
        }
    }
}
impl<E: Element + StatefulInteractiveElement + ParentElement + Styled + 'static> ScrollableElement
    for E
{
}
/// A viewport plus its local scrollbar; the handle owns offsets independently of rerenders.
#[derive(IntoElement)]
pub(crate) struct Scrollable<
    E: Element + StatefulInteractiveElement + ParentElement + Styled + 'static,
> {
    /// Original caller-owned scroll viewport.
    element: E,
    /// Stable per-call-site scroll identity.
    id: ElementId,
}
impl<E: Element + StatefulInteractiveElement + ParentElement + Styled + 'static> RenderOnce
    for Scrollable<E>
{
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let handle = window
            .use_keyed_state(self.id, cx, |_, _| ScrollHandle::new())
            .read(cx)
            .clone();
        self.element
            .overflow_y_scroll()
            .track_scroll(&handle)
            .child(super::vertical_scrollbar(&handle, cx))
    }
}
impl<E: Element + StatefulInteractiveElement + ParentElement + Styled + 'static> ParentElement
    for Scrollable<E>
{
    fn extend(&mut self, elements: impl IntoIterator<Item = gpui_kit::AnyElement>) {
        self.element.extend(elements);
    }
}
