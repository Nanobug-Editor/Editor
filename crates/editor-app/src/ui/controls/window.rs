//! Local native caption controls; each window supplies its own title-bar composition.

use super::Icon;
use crate::ui::theme::ActiveTheme as _;
use gpui_base::h_flex;
use gpui_kit::assets::IconName;
use gpui_kit::{
    App, Div, InteractiveElement, ParentElement, Stateful, StatefulInteractiveElement, Styled,
    Window, WindowControlArea, div, prelude::FluentBuilder as _, px,
};

/// Native window creation policy shared by editor windows and local dialogs.
pub(crate) struct TitleBar;

impl TitleBar {
    /// Reserve the native caption while the local title bar owns dragging and controls.
    pub(crate) fn window_options() -> gpui_kit::WindowOptions {
        gpui_kit::WindowOptions {
            titlebar: Some(gpui_kit::TitlebarOptions {
                appears_transparent: true,
                traffic_light_position: Some(gpui_kit::point(px(9.), px(9.))),
                ..Default::default()
            }),
            app_owns_titlebar_drag: true,
            ..Default::default()
        }
    }
}

/// Restore the selected project's client size and center a normal Windows editor on its monitor.
/// `size` is in logical pixels; native placement includes the frame and respects display scaling.
/// `cx` synchronizes GPUI's viewport after the synchronous native placement has finished.
pub(crate) fn restore_editor_geometry(
    window: &mut Window,
    size: gpui_kit::Size<gpui_kit::Pixels>,
    cx: &mut App,
) {
    #[cfg(windows)]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        if let Ok(handle) = HasWindowHandle::window_handle(window)
            && let RawWindowHandle::Win32(native) = handle.as_raw()
        {
            let scale = window.scale_factor();
            match platform_windows::window::restore_centered_window(
                native.hwnd.get(),
                (size.width / px(1.) * scale).round() as i32,
                (size.height / px(1.) * scale).round() as i32,
            ) {
                Ok(()) => {
                    // SetWindowPos emits WM_SIZE while this GPUI window is already borrowed;
                    // its normal callback cannot re-enter the same window. Read the updated
                    // platform bounds here so layout, clipping and input share the new viewport.
                    window.bounds_changed(cx);
                    return;
                }
                Err(error) => tracing::warn!(%error, "could not restore centered editor window"),
            }
        }
    }
    // Headless tests and other platforms retain GPUI's size restoration path.
    window.resize(size);
}

/// The same editor caption appearance is used before and after a workspace is selected.
pub(crate) fn editor_title_bar(cx: &App) -> Stateful<Div> {
    let style =
        crate::ui::theme::component_styles(cx, plugin_schema::ThemeComponent::WindowTitleBar).base;
    h_flex()
        .id("custom-title-bar")
        .debug_selector(|| "custom-title-bar".into())
        .w_full()
        .h(px(34.))
        .flex_shrink_0()
        .items_center()
        .bg(style.background.unwrap_or(cx.theme().tab_bar))
        .text_color(style.foreground.unwrap_or(cx.theme().foreground))
        .text_size(px(style.font_size_px.unwrap_or(14.)))
        .border_b_1()
        .border_color(style.border.unwrap_or(cx.theme().border))
}

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
