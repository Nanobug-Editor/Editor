//! Rich text colors and spacing are resolved from the active editor theme.

pub mod canvas;

use crate::theme::ActiveTheme as _;
use gpui_base::{TextView, TextViewStyle};
use gpui_kit::{
    App, HighlightStyle, Hsla, ImageSource, Pixels, SharedString, StyleRefinement, Styled, rems,
};
use std::sync::Arc;

/// Render selectable Markdown with a highlight that keeps its glyphs readable.
pub fn markdown_view(id: &'static str, markdown: String, font_size: Pixels, cx: &App) -> TextView {
    TextView::markdown(id, markdown)
        .style(rich_text_style(font_size, cx))
        .selectable(true)
}

/// Display untrusted marketplace snapshots without fetching images or activating embedded links.
pub fn untrusted_markdown_view(
    id: &'static str,
    markdown: String,
    font_size: Pixels,
    cx: &App,
) -> TextView {
    markdown_view(id, markdown, font_size, cx)
        .on_link_click(|_, _, _, _| {})
        .image_source(|_| ImageSource::Custom(Arc::new(|_, _| None)))
}

/// Render guest-produced rich markup natively with project colors and no implicit external access.
/// Navigation and images must use separately negotiated host capabilities instead of Base defaults.
pub fn rich_text_view(
    id: SharedString,
    html: String,
    font_size: Pixels,
    colors: RichTextColors,
    presentation: Option<&plugin_runtime::plugin_protocol::ui::ContentStyle>,
    rem_size: Pixels,
    cx: &App,
) -> TextView {
    TextView::html(id, html)
        .style(content_document_style(
            font_size,
            colors,
            presentation,
            rem_size,
            cx,
        ))
        .selectable(true)
        .on_link_click(|_, _, _, _| {})
        .image_source(|_| ImageSource::Custom(Arc::new(|_, _| None)))
}

/// Project bounded guest typography onto Base's public style seam, preserving selection and links.
fn content_document_style(
    font_size: Pixels,
    colors: RichTextColors,
    preset: Option<&plugin_runtime::plugin_protocol::ui::ContentStyle>,
    rem_size: Pixels,
    cx: &App,
) -> TextViewStyle {
    let border = colors.border;
    let muted = colors.muted_foreground;
    let foreground = colors.foreground;
    let code_background = colors.code_background;
    let mut style = document_style(font_size, colors, cx);
    let Some(preset) = preset else {
        return style;
    };
    if let Some(gap) = preset.paragraph_gap {
        style = style.with_paragraph_gap(rems(gap * f32::from(font_size) / f32::from(rem_size)));
    }
    if !preset.headings.is_empty() {
        let headings = preset.headings.clone();
        style = style.with_heading(move |level| {
            let heading = &headings[usize::from(level.clamp(1, 6) - 1)];
            let size = font_size * heading.scale;
            StyleRefinement::default()
                .text_size(size)
                .font_weight(gpui_kit::FontWeight(f32::from(heading.weight)))
                .line_height(gpui_kit::relative(heading.line_height))
                .pb(size * heading.padding_bottom)
                .border_b(gpui_kit::px(heading.border_bottom))
                .border_color(border)
                .text_color(if heading.muted { muted } else { foreground })
        });
    }
    if let Some(code) = &preset.code {
        style = style.with_code_block(
            StyleRefinement::default()
                .bg(code_background)
                .p(gpui_kit::px(code.padding))
                .rounded(gpui_kit::px(code.radius))
                .border(gpui_kit::px(code.border))
                .border_color(border)
                .text_size(font_size * code.scale)
                .line_height(gpui_kit::relative(code.line_height)),
        );
    }
    if let Some(edges) = preset.table_cell_padding {
        let mut table = StyleRefinement::default().rounded(gpui_kit::px(0.));
        table.overflow.x = Some(gpui_kit::Overflow::Scroll);
        style = style
            .with_table_cell(
                StyleRefinement::default()
                    .pt(gpui_kit::px(edges.top))
                    .pr(gpui_kit::px(edges.right))
                    .pb(gpui_kit::px(edges.bottom))
                    .pl(gpui_kit::px(edges.left)),
            )
            .with_table(table)
            .with_table_head(
                StyleRefinement::default()
                    .bg(gpui_kit::transparent_black())
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD),
            );
    }
    style
}

/// Keep all native rich-text color roles, including inline code, on the same resolved palette.
fn document_style(font_size: Pixels, colors: RichTextColors, cx: &App) -> TextViewStyle {
    rich_text_style(font_size, cx)
        .with_foreground(colors.foreground)
        .with_muted_foreground(colors.muted_foreground)
        .with_code_background(colors.code_background)
        .with_inline_code(HighlightStyle {
            // Base merges omitted highlight fields with its accent theme; pin glyphs to the document.
            color: Some(colors.foreground),
            background_color: Some(colors.inline_code_background),
            ..Default::default()
        })
        .with_border(colors.border)
        .with_link(colors.link)
        .with_heading(move |level| {
            // Full-size document previews need a readable hierarchy independent of compact hover cards.
            StyleRefinement::default().text_size(
                font_size
                    * match level {
                        1 => 1.8,
                        2 => 1.5,
                        3 => 1.25,
                        4 => 1.1,
                        _ => 1.,
                    },
            )
        })
}

/// The caller resolves plugin role overrides; the local renderer preserves them on native glyphs.
pub struct RichTextColors {
    pub foreground: Hsla,
    /// Quote/muted text remains on the same resolved document palette.
    pub muted_foreground: Hsla,
    /// Block/table surfaces are distinct from translucent inline spans.
    pub code_background: Hsla,
    pub inline_code_background: Hsla,
    pub border: Hsla,
    pub link: Hsla,
}

/// Existing hover Markdown and plugin rich markup share typography without sharing their parsers.
fn rich_text_style(font_size: Pixels, cx: &App) -> TextViewStyle {
    let palette = cx.theme();
    TextViewStyle::default()
        .with_foreground(palette.foreground)
        .with_muted_foreground(palette.muted_foreground)
        .with_link(palette.primary)
        // Base draws selected ranges over the glyphs. Cap the overlay opacity
        // so an opaque theme selection color cannot hide the selected text.
        .with_selection(palette.selection.alpha(palette.selection.a.min(0.25)))
        .with_code_background(palette.background)
        // Inline spans have a separate Base accent fallback from fenced blocks;
        // resolve both glyphs and fill so hover paths remain readable in dark themes.
        .with_inline_code(HighlightStyle {
            color: Some(palette.foreground),
            background_color: Some(palette.background),
            ..Default::default()
        })
        .with_border(palette.border)
        .with_paragraph_gap(rems(0.5))
        // Base 0.7 accepts a heading style; preserve the existing size hierarchy.
        .with_heading(move |level| {
            StyleRefinement::default().text_size(match level {
                1..=3 => font_size,
                4 => font_size * 0.9,
                _ => font_size * 0.8,
            })
        })
        .with_code_block(
            StyleRefinement::default()
                .bg(palette.transparent)
                .p_0()
                .text_size(font_size),
        )
}

/// Verify the style actually passed to Base inherits live and custom colors instead of Base defaults.
#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::{TestAppContext, gpui, px, rgb};

    #[gpui::test]
    fn document_rich_text_preserves_role_colors_in_both_themes(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::typography::init(cx);
            for light in [false, true] {
                crate::theme::apply_theme(crate::theme::builtin_theme(light), cx);
                let palette = cx.theme();
                let style = document_style(
                    px(14.),
                    RichTextColors {
                        foreground: palette.foreground,
                        muted_foreground: palette.muted_foreground,
                        code_background: palette.background,
                        inline_code_background: palette.background,
                        border: palette.border,
                        link: palette.primary,
                    },
                    cx,
                );
                assert_eq!(style.foreground(), palette.foreground);
                assert_eq!(
                    style.inline_code().color,
                    Some(palette.foreground),
                    "inline code glyphs must not inherit Base's accent foreground"
                );
                assert_eq!(
                    style.inline_code().background_color,
                    Some(palette.background)
                );
                let color = rgb(0x123456).into();
                let custom = document_style(
                    px(14.),
                    RichTextColors {
                        foreground: color,
                        muted_foreground: color,
                        code_background: color,
                        inline_code_background: color,
                        border: color,
                        link: color,
                    },
                    cx,
                );
                assert_eq!(custom.foreground(), color);
                assert_eq!(custom.link(), color);
                assert_eq!(custom.border(), color);
                assert_eq!(custom.inline_code().background_color, Some(color));
            }
        });
    }

    /// Hover inline glyphs and fill must avoid Base's default light-accent fallback.
    /// Inspect the span style itself; the painted-card test only observes its surrounding surface.
    #[gpui::test]
    fn hover_rich_text_preserves_inline_code_colors_in_both_themes(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::typography::init(cx);
            for dark in [true, false] {
                crate::theme::apply_theme(crate::theme::builtin_theme(dark), cx);
                let palette = cx.theme();
                let style = rich_text_style(px(14.), cx);
                assert_eq!(
                    style.inline_code().color,
                    Some(palette.foreground),
                    "hover inline code must use the live foreground in dark={dark}"
                );
                assert_eq!(
                    style.inline_code().background_color,
                    Some(palette.background),
                    "hover inline code must use the live background in dark={dark}"
                );
            }
        });
    }
}
