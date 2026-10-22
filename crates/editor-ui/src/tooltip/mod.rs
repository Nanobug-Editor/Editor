//! Tooltip surface styled by the editor while gpui-base places it.

use crate::theme::ActiveTheme as _;
use gpui_base::Tooltip as BaseTooltip;
use gpui_kit::{
    AnyView, App, AppContext as _, Context, IntoElement, ParentElement, Render, SharedString,
    Styled, Window, div, px,
};

/// Local tooltip content; Base supplies placement and the owning control supplies hover lifetime.
pub struct Tooltip {
    text: SharedString,
}

impl Tooltip {
    /// Retain text for a tooltip without creating a native window.
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self { text: text.into() }
    }

    /// Create and return the tooltip view in cx; the caller mounts it through the control hover interface.
    pub fn build(self, _: &mut Window, cx: &mut App) -> AnyView {
        cx.new(|_| self).into()
    }
}

impl Render for Tooltip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = cx.theme();
        div().child(
            BaseTooltip::new("editor-tooltip")
                .m_2()
                .px_2()
                .py_1()
                .rounded(palette.radius)
                .border_1()
                .border_color(palette.border)
                .bg(palette.popover)
                .text_color(palette.popover_foreground)
                .text_size(px(12.))
                .shadow_md()
                .child(self.text.clone()),
        )
    }
}
