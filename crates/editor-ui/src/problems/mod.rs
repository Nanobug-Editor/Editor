//! Local Problems chrome composes Base activation, focus and scrolling with the editor's theme.
use super::scroll::ScrollableElement as _;
use super::{Button, ButtonCustomVariant, Icon, StatusIcon};
use crate::theme::ActiveTheme as _;
use gpui_base::input::{DiagnosticSeverity, Position};
use gpui_base::{StyledExt as _, TestSupportExt as _, h_flex};
use gpui_kit::{
    AnyElement, App, ClickEvent, Div, FocusHandle, InteractiveElement as _, IntoElement,
    ParentElement as _, ScrollHandle, Stateful, StatefulInteractiveElement as _, Styled as _,
    Window,
};
use gpui_kit::{assets::IconName, div, px};
use rust_i18n::t;

/// Publish a named native heading and a Base hide button inside the shared draggable dock title.
pub fn problem_heading(
    count: usize,
    close_focus: &FocusHandle,
    hide: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let title = t!("problems.title", count = count).to_string();
    h_flex()
        .w_full()
        .items_center()
        .justify_between()
        .child(
            div()
                .id("native-problems-heading")
                .role(gpui_kit::Role::Heading)
                .aria_label(title.clone())
                .debug_selector(|| "native-problems-heading".into())
                .child(title)
                .test_support(),
        )
        .child(
            Button::new("problem-close")
                .track_focus(close_focus)
                .debug_selector(|| "problem-close".into())
                .icon(Icon::new(IconName::WindowMinimize))
                .small()
                .compact()
                .ghost()
                .tooltip(t!("problems.close").to_string())
                .accessibility_label(t!("problems.close").to_string())
                .on_click(hide),
        )
}

/// Show only nonzero severity counts; the controller omits the whole entry when the scope is empty.
pub fn problem_indicator(
    errors: usize,
    warnings: usize,
    others: usize,
    capped: bool,
    cx: &App,
) -> Button {
    let total = errors + warnings + others;
    let summary = t!(
        "problems.summary",
        count = total,
        errors = errors,
        warnings = warnings
    )
    .to_string();
    let error_count = if capped {
        format!("{errors}+")
    } else {
        errors.to_string()
    };
    let mut content = h_flex().items_center().gap_2();
    if errors > 0 {
        content = content.child(
            div()
                .id("problem-count-errors")
                .debug_selector(|| "problem-count-errors".into())
                .flex()
                .gap_1()
                .items_center()
                .child(StatusIcon::Error.icon(cx))
                .child(error_count),
        );
    }
    if warnings > 0 {
        content = content.child(
            div()
                .id("problem-count-warnings")
                .debug_selector(|| "problem-count-warnings".into())
                .flex()
                .gap_1()
                .items_center()
                .child(StatusIcon::Warning.icon(cx))
                .child(warnings.to_string()),
        );
    }
    if others > 0 {
        content = content.child(
            div()
                .id("problem-count-information")
                .debug_selector(|| "problem-count-information".into())
                .flex()
                .gap_1()
                .items_center()
                .child(
                    Icon::new(IconName::Info)
                        .small()
                        .text_color(cx.theme().muted_foreground),
                )
                .child(others.to_string()),
        );
    }
    Button::new("syntax-error-indicator")
        .debug_selector(|| "syntax-error-indicator".into())
        .small()
        .compact()
        .ghost()
        .tooltip(summary.clone())
        .accessibility_label(summary)
        .child(content)
}

/// The scroll viewport consumes the dock's remaining height, rather than a fixed dialog height.
pub fn problem_list(scroll: &ScrollHandle, _: &App) -> Stateful<Div> {
    div()
        .id("native-problems-list")
        .debug_selector(|| "native-problems-list".into())
        .role(gpui_kit::Role::List)
        .size_full()
        .overflow_y_scroll()
        .track_scroll(scroll)
        .flex()
        .flex_col()
}

/// Base owns mouse/keyboard activation and expansion semantics; the supplied artwork uses the shared file resolver.
pub fn problem_file(
    index: usize,
    path: &str,
    count: usize,
    expanded: bool,
    focus: &FocusHandle,
    file_icon: AnyElement,
    cx: &App,
) -> Button {
    let label = t!("problems.file", path = path, count = count).to_string();
    let selector = format!("native-problems-file-{index}");
    let icon_selector = format!("native-problems-file-icon-{index}");
    let appearance = ButtonCustomVariant::new(cx)
        .color(cx.theme().secondary)
        .foreground(cx.theme().foreground)
        .hover(cx.theme().list_hover)
        .active(cx.theme().list_active);
    Button::new(selector.clone())
        .debug_selector(move || selector.clone())
        .accessibility_label(label.clone())
        .tooltip(label.clone())
        .track_focus(focus)
        .expanded(expanded)
        .content_full_width()
        .custom(appearance)
        .w_full()
        .h_auto()
        .min_h(px(30.))
        .rounded_none()
        .px_3()
        .py_1()
        .flex_shrink_0()
        .text_sm()
        .font_semibold()
        .child(
            h_flex()
                .w_full()
                .items_center()
                .gap_2()
                .child(
                    Icon::new(if expanded {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    })
                    .xsmall()
                    .flex_shrink_0(),
                )
                .child(
                    div()
                        .id(icon_selector.clone())
                        .debug_selector(move || icon_selector.clone().into())
                        .flex_shrink_0()
                        .child(file_icon),
                )
                .child(div().min_w(px(0.)).truncate().child(label)),
        )
}

/// Base owns activation and focus; severity artwork and full-width list selection are local.
pub fn problem_row(
    index: usize,
    focus: &FocusHandle,
    severity: DiagnosticSeverity,
    message: &str,
    source: &str,
    position: Position,
    selected: bool,
    cx: &App,
) -> Button {
    let icon = match severity {
        DiagnosticSeverity::Error => StatusIcon::Error.icon(cx),
        DiagnosticSeverity::Warning => StatusIcon::Warning.icon(cx),
        DiagnosticSeverity::Info => Icon::new(IconName::Info)
            .small()
            .text_color(cx.theme().muted_foreground),
        DiagnosticSeverity::Hint => Icon::new(IconName::Info)
            .small()
            .text_color(cx.theme().muted_foreground),
    };
    let severity_label = t!(match severity {
        DiagnosticSeverity::Error => "problems.error",
        DiagnosticSeverity::Warning => "problems.warning",
        DiagnosticSeverity::Info => "problems.information",
        DiagnosticSeverity::Hint => "problems.hint",
    });
    let location = format!("{}:{}", position.line + 1, position.character + 1);
    let label = format!("{severity_label} · {source} · {location} · {message}");
    let appearance = ButtonCustomVariant::new(cx)
        .color(if selected {
            cx.theme().list_active
        } else {
            cx.theme().transparent
        })
        .foreground(cx.theme().foreground)
        .hover(cx.theme().list_hover)
        .active(cx.theme().list_active);
    Button::new(format!("native-problem-{index}"))
        .track_focus(focus)
        .content_full_width()
        .debug_selector(move || format!("native-problem-{index}"))
        .accessibility_label(label.clone())
        .tooltip(label)
        .custom(appearance)
        .w_full()
        .h_auto()
        .min_h(px(42.))
        .flex_shrink_0()
        .rounded_none()
        .px_3()
        .py_1()
        .child(
            h_flex()
                .w_full()
                .items_start()
                .gap_2()
                .child(icon.mt(px(2.)).flex_shrink_0())
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(div().text_sm().child(message.to_owned()))
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(format!("{source} · {location}")),
                        ),
                ),
        )
}

/// The empty state shares the list viewport and remains readable in either palette.
pub fn problem_empty(cx: &App) -> impl IntoElement {
    let label = t!("problems.empty").to_string();
    div()
        .id("native-problems-empty")
        .debug_selector(|| "native-problems-empty".into())
        .role(gpui_kit::Role::Label)
        .aria_label(label.clone())
        .p_3()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(label)
        .test_support()
}

/// Actions remain reachable while only the diagnostic list scrolls vertically.
pub fn problem_actions(cx: &App) -> impl IntoElement + gpui_kit::ParentElement {
    div()
        .id("native-problems-actions")
        .debug_selector(|| "native-problems-actions".into())
        .flex()
        .items_center()
        .gap_2()
        .px_2()
        .py_1()
        .flex_shrink_0()
        .overflow_x_scrollbar()
        .border_t_1()
        .border_color(cx.theme().border)
}

/// Compose a focusable panel without an overlay or a modal focus trap.
pub fn problem_body(
    focus: &FocusHandle,
    list: AnyElement,
    scrollbar: AnyElement,
    actions: Option<AnyElement>,
    cx: &App,
) -> Stateful<Div> {
    div()
        .id("native-problems-panel")
        .debug_selector(|| "native-problems-panel".into())
        .track_focus(focus)
        .key_context("Problems")
        .size_full()
        .flex()
        .flex_col()
        .bg(cx.theme().background)
        .text_color(cx.theme().foreground)
        .child(
            div()
                .relative()
                .flex_1()
                .min_h(px(0.))
                .child(list)
                .child(scrollbar),
        )
        .children(actions)
}
