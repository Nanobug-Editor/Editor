//! Native editor appearance is projected onto any existing entity without changing its text or behavior.
use super::{Button, Icon};
use crate::theme::component_styles;
use gpui_base::InputBase;
use gpui_base::input::{DiagnosticColors, Editor as BaseEditor, EditorState, InputEditorStyle};
use gpui_kit::component::{ActiveTheme as _, IconName};
use gpui_kit::{
    AccessibleAction, AnyElement, App, Entity, Focusable as _, InteractiveElement, IntoElement,
    ParentElement, Role, SharedString, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder as _, px,
};
use plugin_schema::ThemeComponent;
use std::rc::Rc;

/// Expose a readonly native document's source name, current value and actual editing focus.
/// The Base editor still owns input and layout. Document is a readonly accessibility role;
/// GPUI's input frame has no public readonly flag, so TextInput would falsely report writable.
/// No SetValue action is registered: assistive clients cannot obtain another editing path.
pub(crate) fn readonly_editor(
    id: &'static str,
    label: String,
    editor: &Entity<EditorState>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    readonly_editor_named(id.into(), label, editor, window, cx)
}

/// Keyed readonly sources use the same local frame, retaining native focus across plugin revisions.
pub(crate) fn readonly_editor_named(
    id: gpui_kit::SharedString,
    label: String,
    editor: &Entity<EditorState>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    readonly_editor_body(id, label, editor, None, window, cx)
}

/// Keep the same readonly accessibility/focus frame around a local source presentation.
/// The supplied body changes pixels only; the original native entity still provides its value.
pub(crate) fn readonly_editor_body(
    id: gpui_kit::SharedString,
    label: String,
    editor: &Entity<EditorState>,
    body: Option<AnyElement>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    if let Some(body) = body {
        // The aligned pane provides the only physical accessibility node for this editor.
        return aligned_source_frame(id, body, cx);
    }
    // Keep the new readonly frame's palette and typography in the local control boundary.
    // Layout callers provide only placement; EditorState still owns all text/input behavior.
    synchronize_editor_appearance(editor, cx);
    let appearance = component_styles(cx, ThemeComponent::Editor).base;
    let focus = editor.read(cx).focus_handle(cx);
    // Mirror the component's lazy accessibility value; ordinary painting need not clone text.
    let value =
        (window.is_a11y_active() || cfg!(test)).then(|| editor.read(cx).value().to_string());
    let target = editor.clone();
    let selector = id.clone();
    InputBase::new(id.clone())
        .debug_selector(move || selector.to_string())
        .role(Role::Document)
        .accessibility_id(id)
        .accessibility_label(label)
        .track_focus(&focus)
        .focused(focus.is_focused(window))
        .when_some(value, |frame, value| frame.aria_value(value))
        .on_a11y_action(AccessibleAction::Focus, move |_, window, cx| {
            target.update(cx, |state, cx| state.focus(window, cx));
        })
        .flex()
        .size_full()
        .min_h_0()
        .bg(appearance.background.unwrap_or(cx.theme().background))
        .text_color(appearance.foreground.unwrap_or(cx.theme().foreground))
        .font_family(cx.theme().mono_font_family.clone())
        .text_size(
            appearance
                .font_size_px
                .map(px)
                .unwrap_or(cx.theme().mono_font_size),
        )
        .child(BaseEditor::new(editor))
        .into_any_element()
}

/// Mount an existing document in a work window. Base retains IME, selection and Undo; the
/// project owner suppresses its other mount until the window is hidden or selects another file.
pub(crate) fn window_editor(editor: &Entity<EditorState>, cx: &mut App) -> AnyElement {
    window_editor_body(editor, None, cx)
}

/// Retain the original native source frame while a local component projects aligned source rows.
pub(crate) fn window_editor_body(
    editor: &Entity<EditorState>,
    body: Option<AnyElement>,
    cx: &mut App,
) -> AnyElement {
    if let Some(body) = body {
        return aligned_source_frame("work-window-document".into(), body, cx);
    }
    synchronize_editor_appearance(editor, cx);
    InputBase::new("work-window-document")
        .role(if editor.read(cx).presentation().is_readonly() {
            Role::Document
        } else {
            Role::TextInput
        })
        .track_focus(&editor.read(cx).focus_handle(cx))
        .key_context("NativeEditorSource")
        .flex()
        .size_full()
        .min_h_0()
        .font_family(cx.theme().mono_font_family.clone())
        .text_size(cx.theme().mono_font_size)
        .bg(cx.theme().background)
        .text_color(cx.theme().foreground)
        .child(BaseEditor::new(editor))
        .into_any_element()
}

/// Display-only wrappers keep source shortcuts and local appearance without registering a second
/// input/document node. The aligned native pane owns physical accessibility bounds and focus.
fn aligned_source_frame(id: SharedString, body: AnyElement, cx: &App) -> AnyElement {
    let selector = id.clone();
    let appearance = component_styles(cx, ThemeComponent::Editor).base;
    div()
        .id(id)
        .debug_selector(move || selector.to_string())
        .key_context("NativeEditorSource")
        .flex()
        .size_full()
        .min_h_0()
        .min_w_0()
        .bg(appearance.background.unwrap_or(cx.theme().background))
        .text_color(appearance.foreground.unwrap_or(cx.theme().foreground))
        .font_family(cx.theme().mono_font_family.clone())
        .text_size(
            appearance
                .font_size_px
                .map(px)
                .unwrap_or(cx.theme().mono_font_size),
        )
        .child(body)
        .into_any_element()
}

/// Refresh palette, highlight resolver and folding glyphs for active or comparison editors.
/// Base continues to own selection, IME, scroll, focus and folds; no document state is copied.
pub(crate) fn synchronize_editor_appearance(editor: &Entity<EditorState>, cx: &mut App) {
    let palette = cx.theme();
    let appearance = component_styles(cx, ThemeComponent::Editor).base;
    let style = InputEditorStyle {
        foreground: appearance.foreground.unwrap_or(palette.foreground),
        muted_foreground: palette.muted_foreground,
        background: appearance.background.unwrap_or(palette.background),
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
        editor_gutter_background: palette.highlight_theme.style.editor_gutter_background,
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
