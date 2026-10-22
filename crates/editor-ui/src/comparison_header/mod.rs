//! Local, theme-aware chrome separates comparison navigation/source titles from native text.
use crate::theme::ActiveTheme as _;
use gpui_kit::{AnyElement, App, Div, InteractiveElement, IntoElement, ParentElement, Styled, div};

/// Shared semantic surface for comparison headers; no input, document or scroll state lives here.
pub fn comparison_header_surface(cx: &App) -> Div {
    div()
        .w_full()
        .flex_shrink_0()
        .bg(cx.theme().muted)
        .text_color(cx.theme().foreground)
}

/// Keep each source title outside its scrolling editor, on the same surface as navigation.
/// `index` names the existing comparison source; truncation keeps long revisions on one row.
pub fn comparison_source_header(index: usize, title: String, cx: &App) -> AnyElement {
    comparison_header_surface(cx)
        .id(("comparison-source-header", index))
        .debug_selector(move || format!("comparison-source-header-{index}"))
        .px_2()
        .truncate()
        .border_b_1()
        .border_color(cx.theme().border)
        .child(title)
        .into_any_element()
}
