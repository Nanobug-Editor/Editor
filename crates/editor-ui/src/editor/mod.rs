//! Native editor appearance is projected onto any existing entity without changing its text or behavior.
use super::{Button, Icon};
use crate::theme::ActiveTheme as _;
use crate::theme::component_styles;
use gpui_base::input::{DiagnosticColors, EditorState, InputEditorStyle};
use gpui_kit::assets::IconName;
use gpui_kit::{AnyElement, App, Entity, IntoElement, Styled, px};
use plugin_schema::ThemeComponent;
use std::rc::Rc;

/// Retain a transparent separator at the ordinary source gutter's complete trailing edge.
/// `editor` remains the sole input owner; this overlay adds no hitbox or text padding.
pub fn gutter_separator(editor: &Entity<EditorState>) -> AnyElement {
    let editor = editor.clone();
    gpui_kit::canvas(
        |_, _, _| (),
        move |clip, _, window, cx| {
            let state = editor.read(cx);
            let Some(text) = state.text_bounds() else {
                return;
            };
            let Some(start) = state.range_to_bounds(&(0..0)) else {
                return;
            };
            // Subtract the scrolling text origin to retain Base's exact number/fold width.
            let width = start.left() - text.left();
            if width <= px(1.) {
                return;
            }
            let input = state.input_bounds();
            let line = gpui_kit::Bounds::new(
                // Keep the complete folding control inside the gutter instead of reclaiming
                // its trailing space for a visible rule beside the text.
                gpui_kit::point(input.left() + width - px(1.), input.top()),
                gpui_kit::size(px(1.), input.size.height),
            )
            .intersect(&clip);
            if line.size.width > px(0.) && line.size.height > px(0.) {
                window.paint_quad(gpui_kit::fill(line, cx.theme().border.opacity(0.)));
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
    .into_any_element()
}

/// Refresh palette, highlight resolver and folding glyphs for active or comparison editors.
/// Base continues to own selection, IME, scroll, focus and folds; no document state is copied.
pub fn synchronize_editor_appearance(editor: &Entity<EditorState>, cx: &mut App) {
    synchronize_editor_style(editor, None, cx);
}

/// During a clipped full-row native paint, use the same opaque color for its gutter and body.
/// The caller restores ordinary appearance after its bands; native input state stays untouched.
pub(super) fn synchronize_editor_row_background(
    editor: &Entity<EditorState>,
    background: gpui_kit::Hsla,
    cx: &mut App,
) {
    synchronize_editor_style(editor, Some(background), cx);
}

/// Resolve the normal local theme, optionally replacing only a display band's background colors.
fn synchronize_editor_style(
    editor: &Entity<EditorState>,
    background: Option<gpui_kit::Hsla>,
    cx: &mut App,
) {
    let palette = cx.theme();
    let appearance = component_styles(cx, ThemeComponent::Editor).base;
    let style = InputEditorStyle {
        foreground: appearance.foreground.unwrap_or(palette.foreground),
        muted_foreground: palette.muted_foreground,
        background: background.unwrap_or(appearance.background.unwrap_or(palette.background)),
        border: palette.border,
        selection: palette.selection,
        caret: palette.caret,
        diagnostics: DiagnosticColors {
            error: palette.highlight_theme.style.status.error(cx),
            warning: palette.highlight_theme.style.status.warning(cx),
            info: palette.highlight_theme.style.status.info(cx),
            hint: palette.highlight_theme.style.status.hint(cx),
        },
        highlight_styles: palette.highlight_theme.clone(),
        editor_invisible: palette.highlight_theme.style.editor_invisible,
        editor_active_line: palette.highlight_theme.style.editor_active_line,
        editor_gutter_background: background
            .or(palette.highlight_theme.style.editor_gutter_background),
        fold_icon_renderer: Some(Rc::new(|index, folded| {
            Button::new(("document-fold", index))
                .ghost()
                .small()
                .compact()
                .icon(
                    Icon::new(if folded {
                        IconName::ChevronRight
                    } else {
                        IconName::ChevronDown
                    })
                    .xsmall(),
                )
                .w(px(14.))
                .h(px(14.))
                .into_any_element()
        })),
    };
    editor.update(cx, |state, _| state.set_editor_style(style));
}
