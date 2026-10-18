//! Project-owned dialogs use Base modal behavior and the editor's local surface tokens.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{AnyElement, App, FocusHandle, IntoElement, ParentElement, Styled, Window, px};

/// Clone inputs use a Base focus trap, occluding backdrop and explicit button activation.
/// Escape delegates cancellation to the owner; Enter never launches Git from an input field.
pub(crate) fn clone_modal(
    focus: FocusHandle,
    content: AnyElement,
    cancel: impl Fn(&mut Window, &mut App) + 'static,
    cx: &mut App,
) -> AnyElement {
    gpui_base::Dialog::new(cx)
        .focus_handle(focus)
        .close_on_backdrop_press(false)
        .on_cancel(move |_, window, cx| {
            cancel(window, cx);
            false
        })
        .on_ok(|_, _, _| false)
        .backdrop(
            gpui_base::DialogBackdrop::new()
                .size_full()
                .bg(cx.theme().overlay.alpha(0.55)),
        )
        .popup(
            gpui_base::DialogPopup::new()
                .w(px(520.))
                .max_w_full()
                .p_5()
                .rounded(px(8.))
                .border_1()
                .border_color(cx.theme().border)
                .shadow_lg()
                .bg(cx.theme().popover)
                .text_color(cx.theme().popover_foreground)
                .child(content),
        )
        .into_any_element()
}
