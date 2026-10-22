//! Editor-owned underline tabs; gpui-base supplies pointer and accessibility semantics.
//! Compound keyboard navigation stays here so callers cannot activate disabled placeholders.
use super::StatusIcon;
use std::rc::Rc;

use crate::Tooltip;
use crate::theme::ActiveTheme as _;
use gpui_base::{Tab, Tabs};
use gpui_kit::{
    App, ElementId, FocusHandle, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder as _, px,
};

/// Document tabs keep local appearance while Base owns pointer activation and tab accessibility.
/// Content determines width up to 400 logical pixels; the title may shrink while controls stay visible.
pub fn file_tab(id: String, title: String, selected: bool, index: usize, count: usize) -> Tab {
    Tab::new(ElementId::Name(id.into()))
        .selected(selected)
        .accessibility_label(title)
        .set_position(index + 1, count)
        .justify_start()
        .w_auto()
        .max_w(px(400.))
        .flex_shrink_0()
}

/// A derived document view shares local file-tab appearance and Base activation semantics.
/// Closing it affects only that presentation; callbacks retain the source session's identity.
pub fn comparison_tab(
    id: String,
    title: String,
    selected: bool,
    preview: bool,
    index: usize,
    count: usize,
    activate: impl Fn(bool, &mut Window, &mut App) + 'static,
    close: impl Fn(&mut Window, &mut App) + 'static,
    cx: &App,
) -> gpui_kit::AnyElement {
    let styles = crate::theme::component_styles(cx, plugin_schema::ThemeComponent::EditorTab);
    let accent = styles.selected.border.unwrap_or(cx.theme().primary);
    file_tab(id.clone(), title.clone(), selected, index, count)
        .debug_selector(move || format!("comparison-tab-{index}").into())
        .h_full()
        .px_2()
        .gap_2()
        .border_r_1()
        .border_color(cx.theme().border)
        .when(selected, |tab| tab.border_b_1().border_color(accent))
        .bg(if selected {
            styles.selected.background.unwrap_or(cx.theme().background)
        } else {
            styles.base.background.unwrap_or(cx.theme().tab_bar)
        })
        .text_color(if selected {
            accent
        } else {
            styles.base.foreground.unwrap_or(cx.theme().foreground)
        })
        .hover(|style| style.bg(styles.hover.background.unwrap_or(cx.theme().list_hover)))
        .child(div().min_w(px(0.)).truncate().child(title))
        .when(preview, |tab| tab.child(preview_tab_badge(cx)))
        .child(
            super::Button::new(format!("close-{id}"))
                .compact()
                .icon(super::Icon::new(gpui_kit::assets::IconName::Close))
                .accessibility_label(rust_i18n::t!("editor.close_diff").to_string())
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    close(window, cx);
                }),
        )
        .on_click(move |event, window, cx| activate(event.click_count() == 2, window, cx))
        .into_any_element()
}

/// A localized muted label distinguishes replaceable previews from external-file italic titles.
/// Both themes use the local palette; the surrounding Base tab remains the interaction target.
pub fn preview_tab_badge(cx: &App) -> impl IntoElement {
    div()
        .id("editor-preview-badge")
        .debug_selector(|| "editor-preview-badge".into())
        .flex_shrink_0()
        .text_size(px(10.))
        .text_color(cx.theme().muted_foreground)
        .child(rust_i18n::t!("editor.preview_label").to_string())
}

/// Render a controlled strip of localized labels and disabled flags.
/// The owner supplies selection and focus; changes report an enabled index only.
/// Returns the local underline appearance over Base's tab/list semantics.
pub fn tab_strip<const N: usize>(
    id: &'static str,
    selected: usize,
    labels: [(SharedString, bool); N],
    badges: [Option<StatusIcon>; N],
    focus: &FocusHandle,
    on_change: impl Fn(usize, &mut Window, &mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    let palette = cx.theme();
    let on_change = Rc::new(on_change);
    let enabled: Vec<_> = labels
        .iter()
        .enumerate()
        .filter_map(|(index, (_, disabled))| (!disabled).then_some(index))
        .collect();
    let keyboard_change = on_change.clone();
    let mut tabs = Tabs::new(id)
        .track_focus(focus)
        .tab_stop(true)
        .flex()
        .flex_shrink_0()
        .w_full()
        .border_b_1()
        .border_color(palette.border)
        // A focused strip keeps a full-width accent, identifying which group owns arrow keys.
        .in_focus(|style| style.border_color(palette.ring))
        .on_key_down(move |event, window, cx| {
            if event.keystroke.modifiers != Default::default() || enabled.is_empty() {
                return;
            }
            // Navigation wraps between available tabs, never through marketplace placeholders.
            let current = enabled
                .iter()
                .position(|index| *index == selected)
                .unwrap_or(0);
            let next = match event.keystroke.key.as_str() {
                "right" => enabled[(current + 1) % enabled.len()],
                "left" => enabled[(current + enabled.len() - 1) % enabled.len()],
                "home" => enabled[0],
                "end" => enabled[enabled.len() - 1],
                "enter" | "space" => enabled[current],
                _ => return,
            };
            cx.stop_propagation();
            keyboard_change(next, window, cx);
        });
    for (index, (label, disabled)) in labels.into_iter().enumerate() {
        let handler = on_change.clone();
        let focus = focus.clone();
        let tooltip_label = label.clone();
        tabs = tabs.child(
            Tab::new(ElementId::Name(format!("{id}-{index}").into()))
                .debug_selector(move || format!("{id}-{index}").into())
                .selected(index == selected)
                .disabled(disabled)
                .accessibility_label(label.clone())
                .tooltip(move |window, cx| Tooltip::new(tooltip_label.clone()).build(window, cx))
                .set_position(index + 1, N)
                .when(!disabled, |tab| {
                    tab.on_click(move |_, window, cx| {
                        focus.focus(window, cx);
                        handler(index, window, cx);
                    })
                })
                .h(px(32.))
                .flex_1()
                .min_w(px(0.))
                .px_2()
                .border_b_2()
                .border_color(if index == selected {
                    palette.primary
                } else {
                    palette.transparent
                })
                .bg(palette.sidebar)
                .text_color(if index == selected {
                    palette.foreground
                } else {
                    palette.muted_foreground
                })
                .when(disabled, |tab| tab.opacity(0.45))
                .when(!disabled, |tab| {
                    tab.hover(|style| style.bg(palette.list_hover))
                })
                .child(
                    // Severity and title share one line without allowing a long label to squash its icon.
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap_1()
                        .min_w(px(0.))
                        .when_some(badges[index], |row, badge| {
                            row.child(
                                div()
                                    .debug_selector(move || {
                                        format!("{id}-{index}-{:?}", badge).into()
                                    })
                                    .flex_shrink_0()
                                    .child(badge.icon(cx)),
                            )
                        })
                        .child(div().min_w(px(0.)).truncate().child(label)),
                ),
        );
    }
    tabs
}
