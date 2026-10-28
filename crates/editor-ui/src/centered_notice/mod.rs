//! Nonmodal viewport notices; the caller owns their lifetime and localized message.
use crate::theme::ActiveTheme as _;
use gpui_kit::{App, InteractiveElement, IntoElement, ParentElement, Styled, div, px};

/// Draw a centered, nonfocusable notice over a relative content viewport.
pub fn centered_notice(message: String, cx: &App) -> impl IntoElement {
    div()
        .debug_selector(|| "centered-notice".into())
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .debug_selector(|| "centered-notice-card".into())
                .px_3()
                .py_2()
                .rounded(px(5.))
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().popover)
                .text_color(cx.theme().foreground)
                .text_sm()
                .child(message),
        )
}
