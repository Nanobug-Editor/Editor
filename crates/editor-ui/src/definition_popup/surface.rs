//! Shared theme surface for type documentation and diagnostic details.

use crate::theme::ActiveTheme as _;
use gpui_kit::{App, BoxShadow, Div, Stateful, Styled, div, point, prelude::*, px};

/// Shared documentation/diagnostic surface; `id` retains the caller's focus and scroll identity.
/// Severity belongs to content accents, while the outline and elevation follow the live theme.
pub fn details_card(id: &'static str, cx: &App) -> Stateful<Div> {
    let theme = cx.theme();
    div()
        .id(id)
        .debug_selector(move || id.into())
        .occlude()
        .rounded(theme.radius_lg)
        .border_1()
        .border_color(theme.border.opacity(0.65))
        .bg(theme.popover)
        .text_color(theme.popover_foreground)
        .font_family(theme.font_family.clone())
        .text_size(crate::typography::font_size(cx))
        .line_height(gpui_kit::relative(1.5))
        .shadow(vec![
            BoxShadow {
                color: gpui_kit::black().opacity(if theme.is_dark() { 0.28 } else { 0.12 }),
                blur_radius: px(18.),
                spread_radius: px(-3.),
                offset: point(px(0.), px(4.)),
                inset: false,
            },
            BoxShadow {
                color: gpui_kit::black().opacity(0.06),
                blur_radius: px(4.),
                spread_radius: px(0.),
                offset: point(px(0.), px(1.)),
                inset: false,
            },
        ])
        .overflow_hidden()
}
