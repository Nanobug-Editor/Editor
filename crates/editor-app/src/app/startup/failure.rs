//! Launch failures remain visible even when Explorer did not provide a console for the secondary process.
use crate::*;

/// Report failure in a native GPUI window with local Base controls, without loading a project or plugin store.
pub(crate) fn show(details: String) {
    gpui_kit::application()
        .with_assets(AppAssets)
        .run(move |cx| {
            gpui_kit::init(cx);
            typography::init(cx);
            ui::controls::Button::init_keys(cx);
            apply_theme(&theme::active_theme(false), cx);
            let mut options = TitleBar::window_options();
            options.window_bounds = Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(600.), px(280.)),
                cx,
            )));
            let _ = gpui_kit::open_window(options, cx, move |window, cx| {
                window.set_window_title(app::APP_NAME);
                window.on_window_should_close(cx, |_, cx| {
                    cx.quit();
                    true
                });
                cx.new(|cx| {
                    let focus = cx.focus_handle();
                    focus.focus(window, cx);
                    Failure { details, focus }
                })
            });
            cx.activate(true);
        });
}

/// This transient report owns no profile, workspace, document or mutable runtime state.
struct Failure {
    details: String,
    focus: FocusHandle,
}
impl Focusable for Failure {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl Render for Failure {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .track_focus(&self.focus)
            .on_key_down(|event, _, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "escape") {
                    cx.quit();
                }
            })
            // Reuse the project's native hit regions and local appearance for this transient report.
            .child(
                h_flex()
                    .h(px(crate::PANEL_HEADER_HEIGHT))
                    .px_2()
                    .items_center()
                    .bg(cx.theme().title_bar)
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .window_control_area(gpui_kit::WindowControlArea::Drag)
                            .child(app::APP_NAME),
                    )
                    .child(super::super::shell::title_bar_window_control(
                        "launch-error-close",
                        IconName::WindowClose,
                        gpui_kit::WindowControlArea::Close,
                        true,
                        cx,
                    )),
            )
            .child(
                v_flex()
                    .flex_1()
                    .p_5()
                    .gap_3()
                    .child(
                        div()
                            .text_lg()
                            .child(t!("projects.external_failure").to_string()),
                    )
                    .child(div().text_sm().child(self.details.clone()))
                    .child(
                        Button::new("dismiss-launch-error")
                            .label(t!("projects.external_dismiss").to_string())
                            .on_click(|_, _, cx| cx.quit()),
                    ),
            )
    }
}
