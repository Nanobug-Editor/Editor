//! Home layout and consent composition use project-owned controls over native Base behavior.

use super::*;
use crate::ui::controls::file_operation::{FileChoice, conflict_prompt};
use gpui_kit::{AnyElement, SharedString};

impl ProjectWindow {
    /// Compose realistic project records; every row uses the same normal open request.
    fn home(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut list = v_flex()
            .id("project-recent-list")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .gap_1();
        for (index, entry) in self.recent.entries.iter().enumerate() {
            let path = entry.path.clone();
            let name = path
                .file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
                .into_owned();
            let remove = path.clone();
            list = list.child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        Button::new(SharedString::from(format!("project-recent-{index}")))
                            .debug_selector(move || format!("project-recent-{index}"))
                            .content_full_width()
                            .ghost()
                            .flex_1()
                            .h(px(58.))
                            .accessibility_label(path.display().to_string())
                            .tooltip(path.display().to_string())
                            .child(
                                v_flex().items_start().w_full().gap_1().child(name).child(
                                    div()
                                        .text_sm()
                                        .truncate()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(path.display().to_string()),
                                ),
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.open_path(path.clone(), window, cx)
                            })),
                    )
                    .child(
                        Button::new(SharedString::from(format!("project-remove-{index}")))
                            .ghost()
                            .small()
                            .debug_selector(move || format!("project-remove-{index}"))
                            .icon(IconName::Close)
                            .accessibility_label(t!("projects.remove_recent").to_string())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.error = this
                                    .recent
                                    .remove(&remove)
                                    .err()
                                    .map(|error| error.to_string());
                                cx.notify();
                            })),
                    ),
            );
        }
        h_flex()
            .size_full()
            .child(
                v_flex()
                    .w(px(180.))
                    .h_full()
                    .p_4()
                    .gap_5()
                    .bg(cx.theme().sidebar)
                    .child(div().text_xl().font_semibold().child(app::APP_NAME))
                    .child(div().child(t!("projects.projects").to_string())),
            )
            .child(
                v_flex()
                    .flex_1()
                    .h_full()
                    .min_w_0()
                    .p(px(36.))
                    .gap_3()
                    .child(div().text_2xl().child(t!("projects.welcome").to_string()))
                    .child(
                        div()
                            .text_color(cx.theme().muted_foreground)
                            .child(t!("projects.subtitle").to_string()),
                    )
                    .child(
                        h_flex().child(
                            Button::new("project-open-folder")
                                .debug_selector(|| "project-open-folder".into())
                                .label(t!("projects.open_folder").to_string())
                                .accessibility_label(t!("projects.open_folder").to_string())
                                .primary()
                                .disabled(self.choosing)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.choose_folder(window, cx)
                                })),
                        ),
                    )
                    .child(
                        div()
                            .mt_4()
                            .text_sm()
                            .child(t!("projects.recent").to_string()),
                    )
                    .child(
                        div()
                            .relative()
                            .flex_1()
                            .min_h_0()
                            .child(list.size_full())
                            .child(crate::ui::controls::vertical_scrollbar(&self.scroll, cx)),
                    ),
            )
            .into_any_element()
    }

    /// Explicit buttons carry consent; Enter does not implicitly trust a project or discard documents.
    fn consent(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let decision = self.decision.as_ref()?;
        let mut choices = Vec::new();
        let owner = cx.entity().downgrade();
        let cancel = owner.clone();
        choices.push(FileChoice {
            id: "project-decision-cancel",
            label: t!("interaction.cancel").into(),
            enabled: true,
            activate: Box::new(move |window, cx| {
                let _ = cancel.update(cx, |this, cx| this.cancel_decision(window, cx));
            }),
        });
        let (title, mut details) = match decision {
            Decision::Trust { workspace, .. } => {
                for (id, label, trusted) in [
                    ("project-trust-restricted", "projects.restricted", false),
                    ("project-trust-accept", "projects.trust", true),
                ] {
                    let owner = owner.clone();
                    choices.push(FileChoice {
                        id,
                        label: t!(label).into(),
                        enabled: true,
                        activate: Box::new(move |window, cx| {
                            let _ = owner.update(cx, |this, cx| {
                                if let Some(Decision::Trust {
                                    workspace,
                                    initial_file,
                                }) = this.decision.take()
                                {
                                    this.activate_project(
                                        workspace,
                                        initial_file,
                                        trusted,
                                        window,
                                        cx,
                                    );
                                }
                            });
                        }),
                    });
                }
                (
                    t!("projects.trust_question").to_string(),
                    vec![
                        workspace.root().display().to_string(),
                        t!("projects.trust_details").to_string(),
                    ],
                )
            }
            Decision::Leave { dirty, running, .. } => {
                let mut details = Vec::new();
                if *dirty {
                    details.push(t!("projects.unsaved_details").to_string());
                }
                if *running {
                    details.push(t!("projects.running_details").to_string());
                }
                for (id, label, save) in [
                    (
                        "project-close-discard",
                        if *dirty {
                            "projects.discard_close"
                        } else {
                            "projects.stop_close"
                        },
                        false,
                    ),
                    ("project-close-save", "projects.save_close", true),
                ] {
                    if save && !*dirty {
                        continue;
                    }
                    let owner = owner.clone();
                    choices.push(FileChoice {
                        id,
                        label: t!(label).into(),
                        enabled: true,
                        activate: Box::new(move |window, cx| {
                            let _ =
                                owner.update(cx, |this, cx| this.accept_leave(save, window, cx));
                        }),
                    });
                }
                (t!("projects.close_question").to_string(), details)
            }
        };
        if let Some(error) = &self.error {
            details.push(t!("projects.failure", details = error).to_string());
        }
        Some(conflict_prompt(
            self.modal_focus.clone(),
            title,
            details,
            choices,
            None,
            move |window, cx| {
                let _ = owner.update(cx, |this, cx| this.cancel_decision(window, cx));
            },
            cx,
        ))
    }
}

impl Render for ProjectWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = if let Some(editor) = &self.editor {
            editor.clone().into_any_element()
        } else {
            self.home(cx)
        };
        let consent = self.consent(cx);
        v_flex()
            .id("project-window")
            .size_full()
            .relative()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &extensions::QuitEditor, window, cx| {
                this.request_close(CloseIntent::Window, window, cx);
            }))
            .when(self.editor.is_none(), |root| {
                root.child(
                    h_flex()
                        .h(px(crate::PANEL_HEADER_HEIGHT))
                        .items_center()
                        .px_2()
                        .gap_2()
                        .bg(cx.theme().title_bar)
                        .child(
                            div()
                                .flex_1()
                                .h_full()
                                .window_control_area(gpui_kit::WindowControlArea::Drag)
                                .child(app::APP_NAME),
                        )
                        .child(super::super::shell::title_bar_window_control(
                            "project-minimize",
                            IconName::WindowMinimize,
                            gpui_kit::WindowControlArea::Min,
                            false,
                            cx,
                        ))
                        .child(super::super::shell::title_bar_window_control(
                            "project-maximize",
                            if window.is_maximized() {
                                IconName::WindowRestore
                            } else {
                                IconName::WindowMaximize
                            },
                            gpui_kit::WindowControlArea::Max,
                            false,
                            cx,
                        ))
                        .child(super::super::shell::title_bar_window_control(
                            "project-close-window",
                            IconName::WindowClose,
                            gpui_kit::WindowControlArea::Close,
                            true,
                            cx,
                        )),
                )
            })
            .when(self.error.is_some() && self.decision.is_none(), |root| {
                root.child(
                    div()
                        .debug_selector(|| "project-error".into())
                        .p_2()
                        .text_color(cx.theme().danger)
                        .child(
                            t!(
                                "projects.failure",
                                details = self.error.as_deref().unwrap_or_default()
                            )
                            .to_string(),
                        ),
                )
            })
            .child(div().flex_1().min_h_0().child(content))
            .when(self.closing, |root| {
                root.child(
                    div()
                        .absolute()
                        .inset_0()
                        .occlude()
                        .bg(cx.theme().background)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(t!("projects.closing").to_string()),
                )
            })
            .children(consent)
    }
}
