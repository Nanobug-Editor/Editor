//! Local native caption controls; each window supplies its own title-bar composition.

use super::Icon;
use gpui_kit::component::{ActiveTheme as _, IconName, h_flex};
use gpui_kit::{
    App, Div, InteractiveElement, ParentElement, Stateful, StatefulInteractiveElement, Styled,
    Window, WindowControlArea, div, prelude::FluentBuilder as _, px,
};

/// Native caption regions retain the platform close gate in both shell states.
pub(crate) fn native_window_controls(window: &Window, cx: &App) -> Stateful<Div> {
    let supported = window.window_controls();
    h_flex()
        .id("window-controls")
        .h_full()
        .items_center()
        .flex_shrink_0()
        .when(!cfg!(target_os = "macos") && supported.minimize, |bar| {
            bar.child(title_bar_window_control(
                "minimize",
                IconName::WindowMinimize,
                WindowControlArea::Min,
                false,
                cx,
            ))
        })
        .when(!cfg!(target_os = "macos") && supported.maximize, |bar| {
            bar.child(title_bar_window_control(
                if window.is_maximized() {
                    "restore"
                } else {
                    "maximize"
                },
                if window.is_maximized() {
                    IconName::WindowRestore
                } else {
                    IconName::WindowMaximize
                },
                WindowControlArea::Max,
                false,
                cx,
            ))
        })
        .when(!cfg!(target_os = "macos"), |bar| {
            bar.child(title_bar_window_control(
                "close",
                IconName::WindowClose,
                WindowControlArea::Close,
                true,
                cx,
            ))
        })
}

/// Paint a native caption hit region using the current theme.
/// `area` supplies the OS action on Windows; other platforms dispatch the existing close gate.
/// Callers may resize the returned region to suit their window without changing close consent.
pub(crate) fn title_bar_window_control(
    id: &'static str,
    icon: IconName,
    area: WindowControlArea,
    close: bool,
    cx: &App,
) -> Stateful<Div> {
    let palette = cx.theme();
    let (foreground, hover, active) = if close {
        (
            palette.danger_foreground,
            palette.danger,
            palette.danger_active,
        )
    } else {
        (
            palette.secondary_foreground,
            palette.secondary_hover,
            palette.secondary_active,
        )
    };
    div()
        .id(id)
        .debug_selector(move || id.into())
        .flex()
        .w(px(34.))
        .h_full()
        .flex_shrink_0()
        .justify_center()
        .items_center()
        .text_color(palette.foreground)
        .hover(|style| style.bg(hover).text_color(foreground))
        .active(|style| style.bg(active).text_color(foreground))
        .when(cfg!(target_os = "windows"), |this| {
            this.window_control_area(area)
        })
        .when(!cfg!(target_os = "windows"), |this| {
            this.on_click(move |_, window, cx| {
                cx.stop_propagation();
                match area {
                    WindowControlArea::Min => window.minimize_window(),
                    WindowControlArea::Max => window.zoom_window(),
                    WindowControlArea::Close => {
                        window.dispatch_action(Box::new(crate::app::projects::CloseWindow), cx)
                    }
                    WindowControlArea::Drag => {}
                }
            })
        })
        .child(Icon::new(icon).small())
}
