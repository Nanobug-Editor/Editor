//! Home layout and consent composition use project-owned controls over native Base behavior.

use super::*;
use crate::ui::controls::file_operation::{FileChoice, conflict_prompt};
use gpui_kit::AnyElement;

impl ProjectWindow {
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
                        editor_core::display_path(&workspace.root()).to_string(),
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
                    if self.has_running_clone() {
                        details.push(t!("projects.clone_close_details").to_string());
                    }
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
        if self.editor.is_none() {
            self.sync_project_menu(window, cx);
        }
        let content = if let Some(editor) = &self.editor {
            editor.clone().into_any_element()
        } else {
            self.empty_shell(window, cx).into_any_element()
        };
        let consent = self.consent(cx);
        let clone = self.render_clone(cx);
        v_flex()
            .id("project-window")
            .key_context("EditorShell")
            .size_full()
            .relative()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .track_focus(&self.focus)
            .on_action(cx.listener(|_, _: &extensions::QuitEditor, _, cx| {
                controller::quit(cx);
            }))
            .on_action(cx.listener(|this, _: &CloseWindow, window, cx| {
                this.request_close(CloseIntent::Window, window, cx);
            }))
            .on_action(
                cx.listener(|this, _: &OpenFolder, window, cx| this.choose_folder(window, cx)),
            )
            .on_action(cx.listener(|this, _: &ShowHome, window, cx| this.show_home(window, cx)))
            .on_action(cx.listener(|this, _: &ToggleTheme, window, cx| {
                if this.editor.is_none() {
                    apply_theme(&theme::active_theme(!cx.theme().is_dark()), cx);
                    cx.refresh_windows();
                    window.refresh();
                }
            }))
            .on_action(
                cx.listener(|this, _: &extensions::ToggleExtensions, _, cx| {
                    if this.editor.is_none() {
                        this.open_empty_extensions(cx);
                    }
                }),
            )
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
            .when(self.close_state == CloseState::Cleaning, |root| {
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
            .children(clone)
    }
}
