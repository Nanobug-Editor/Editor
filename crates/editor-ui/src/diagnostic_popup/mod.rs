//! Theme-aware diagnostic cards over Base's selectable text and the measured popup frame.

use crate::scroll::ScrollableElement as _;
use crate::theme::ActiveTheme as _;
use gpui_base::input::{DiagnosticEntry, DiagnosticSeverity};
use gpui_kit::{
    App, Bounds, FocusHandle, FontWeight, InteractiveElement, IntoElement, ParentElement, Pixels,
    Styled, div, prelude::FluentBuilder as _, px,
};
use rust_i18n::t;

/// Present the complete error at `anchor`; report painted bounds so the owner can retain card gestures.
/// `focus` and `dismiss` share the source's lifecycle; `on_bounds` receives window-local geometry.
pub fn diagnostic_popup(
    anchor: Bounds<Pixels>,
    diagnostic: &DiagnosticEntry,
    focus: &FocusHandle,
    dismiss: impl Fn(&mut App) + 'static,
    on_bounds: impl Fn(Bounds<Pixels>) + 'static,
    cx: &App,
) -> impl IntoElement {
    // Semantic diagnostics keep their server severity, source, and error code.
    let (title, color) = match diagnostic.severity {
        DiagnosticSeverity::Error => (t!("diagnostics.error"), cx.theme().danger),
        DiagnosticSeverity::Warning => (t!("diagnostics.warning"), cx.theme().warning),
        DiagnosticSeverity::Info => (t!("diagnostics.info"), cx.theme().primary),
        DiagnosticSeverity::Hint => (t!("diagnostics.hint"), cx.theme().muted_foreground),
    };
    let location = t!(
        "diagnostics.location",
        line = diagnostic.diagnostic.range.start.line + 1,
        column = diagnostic.diagnostic.range.start.character + 1
    )
    .to_string();
    let source = diagnostic
        .source
        .as_ref()
        .map(|source| format!("{source} · {location}"))
        .unwrap_or(location);
    let mut message = crate::text::plain_markdown(&diagnostic.message);
    if let Some(related) = &diagnostic.related_information {
        for information in related {
            if !information.message.trim().is_empty() {
                // Related explanations remain selectable and literal; no URI is activated implicitly.
                message.push_str("\n\n> ");
                message.push_str(
                    &crate::text::plain_markdown(&information.message).replace('\n', "\n> "),
                );
            }
        }
    }
    let content = super::definition_popup::details_card("editor-diagnostic-card", cx)
        .track_focus(focus)
        .flex()
        .flex_col()
        .min_w(px(260.))
        .flex_1()
        .min_h_0()
        .on_mouse_move(|event, _, cx| {
            // Keep text selection usable without forwarding hover to the editor.
            if event.pressed_button.is_none() {
                cx.stop_propagation();
            }
        })
        // Selectable text can own keyboard focus; Escape must also work from that path.
        .on_key_down(move |event, _, cx| {
            if event.keystroke.key == "escape" {
                dismiss(cx);
                cx.stop_propagation();
            }
        })
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .px_3()
                .pt_3()
                .pb_2()
                .text_size(crate::typography::font_size(cx) * 0.9)
                .child(div().size(px(6.)).rounded_full().bg(color).flex_shrink_0())
                .child(
                    div()
                        .text_color(color)
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title.to_string()),
                )
                .when_some(diagnostic.code.clone(), |header, code| {
                    header.child(
                        div()
                            .px_1()
                            .rounded(px(3.))
                            .bg(color.opacity(0.08))
                            .font_family(cx.theme().mono_font_family.clone())
                            .text_color(cx.theme().muted_foreground)
                            .child(code),
                    )
                }),
        )
        .child(
            div()
                .id("editor-diagnostic-message")
                .debug_selector(|| "editor-diagnostic-message".into())
                .flex_1()
                .min_h_0()
                .px_3()
                .pb_3()
                .overflow_y_scrollbar()
                .child(crate::text::hover_markdown_view(
                    "editor-diagnostic-details",
                    message,
                    cx,
                )),
        )
        .child(
            div()
                .debug_selector(|| "editor-diagnostic-metadata".into())
                .flex_shrink_0()
                .px_3()
                .py_2()
                .border_t_1()
                .border_color(cx.theme().border.opacity(0.45))
                .text_size(crate::typography::font_size(cx) * 0.85)
                .text_color(cx.theme().muted_foreground)
                .child(source),
        );
    super::definition_popup(anchor, content.into_any_element(), on_bounds)
}
