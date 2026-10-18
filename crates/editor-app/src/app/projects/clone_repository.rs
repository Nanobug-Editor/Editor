//! User-requested Git cloning has its own bounded native lifetime and normal project-open route.

use super::*;
use crate::ui::controls::{Input, project_dialog};
use gpui_kit::AnyElement;
use plugin_runtime::native_processes::{
    NativeLaunch, NativeProcessEvent, NativeProcessGroup, resolve_program,
};
use plugin_runtime::plugin_protocol::process::{Transport, Update};
use std::time::{Duration, Instant};

/// Dialog input is unrelated to document state; its supervisor belongs to the initiating editor window.
pub(super) struct CloneProject {
    remote: Entity<InputState>,
    destination: Entity<InputState>,
    browsing: bool,
    error: Option<String>,
    process: Option<NativeProcessGroup>,
    target: Option<PathBuf>,
}

impl ProjectWindow {
    /// A close decision includes the explicit Git process, retaining it if the user cancels closing.
    pub(super) fn has_running_clone(&self) -> bool {
        self.clone_project
            .as_ref()
            .is_some_and(|draft| draft.process.is_some())
    }
    /// Stage inputs without launching Git or accessing project tool settings.
    pub(super) fn show_clone(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.clone_project.is_some()
            || self.decision.is_some()
            || self.choosing
            || controller::is_quitting(cx)
        {
            return;
        }
        let remote = cx.new(|cx| {
            InputState::new(window, cx).placeholder("https://github.com/owner/repository.git")
        });
        let destination = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(t!("projects.clone_destination_hint").to_string())
        });
        self.clone_project = Some(CloneProject {
            remote: remote.clone(),
            destination,
            browsing: false,
            error: None,
            process: None,
            target: None,
        });
        self.modal_focus.focus(window, cx);
        // Start inside the modal boundary; Tab enters the URL field, and Enter never launches Git implicitly.
        cx.notify();
    }

    /// Choose a parent directory; a delayed picker cannot change a replacement clone dialog.
    fn browse_clone(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(draft) = &mut self.clone_project else {
            return;
        };
        if draft.browsing || draft.process.is_some() {
            return;
        }
        draft.browsing = true;
        let destination = draft.destination.clone();
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(t!("projects.clone_destination").into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = receiver.await;
            let _ = this.update_in(cx, |this, window, cx| {
                let Some(draft) = &mut this.clone_project else {
                    return;
                };
                if draft.destination.entity_id() != destination.entity_id() {
                    return;
                }
                draft.browsing = false;
                if let Ok(Ok(Some(paths))) = result
                    && let Some(parent) = paths.into_iter().next()
                {
                    let remote = draft.remote.read(cx).text().to_string();
                    let name = repository_name(&remote).unwrap_or_else(|| "repository".into());
                    let target = editor_core::display_path(&parent.join(name)).to_string();
                    destination.update(cx, |input, cx| input.set_value(target, window, cx));
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// Explicit activation grants one host Git operation; argv stays literal and no project path is consulted.
    fn start_clone(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if controller::is_quitting(cx) {
            return;
        }
        let Some(draft) = &mut self.clone_project else {
            return;
        };
        if draft.process.is_some() || draft.browsing {
            return;
        }
        let remote = draft.remote.read(cx).text().to_string().trim().to_owned();
        if remote_credentials(&remote) {
            draft.error = Some(t!("projects.clone_credentials").to_string());
            cx.notify();
            return;
        }
        let path = PathBuf::from(draft.destination.read(cx).text().to_string().trim());
        let Some((target, parent)) = clone_target(&remote, &path) else {
            draft.error = Some(t!("projects.clone_invalid").to_string());
            cx.notify();
            return;
        };
        let Ok(program) = resolve_program("git") else {
            draft.error = Some(t!("projects.clone_missing_git").to_string());
            cx.notify();
            return;
        };
        // This authority comes from the Clone button, independently of any workspace trust.
        // The destination remains untrusted until the ordinary open route asks for consent.
        let process = NativeProcessGroup::new(true);
        let launch = NativeLaunch {
            program: program.to_string_lossy().into_owned(),
            args: vec![
                "clone".into(),
                "--".into(),
                remote,
                // Git for Windows rejects verbatim drive prefixes in destination arguments.
                // Retain the canonical target for opening/identity; adapt only this literal argv value.
                editor_core::display_path(&target).to_string(),
            ],
            cwd: parent.to_string_lossy().into_owned(),
            env: [
                ("GIT_TERMINAL_PROMPT".into(), "0".into()),
                ("GCM_INTERACTIVE".into(), "never".into()),
            ]
            .into(),
            transport: Transport::Stdio,
        };
        if process.launch(1, launch).is_err() {
            draft.error = Some(t!("projects.clone_failure").to_string());
            cx.notify();
            return;
        }
        draft.error = None;
        draft.process = Some(process);
        draft.target = Some(target);
        let identity = draft.remote.entity_id();
        cx.spawn_in(window, async move |this, cx| {
            let started = Instant::now();
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(50))
                    .await;
                let running = this
                    .update_in(cx, |this, window, cx| {
                        // Reopening the dialog invalidates every result from its old process/picker.
                        if this
                            .clone_project
                            .as_ref()
                            .is_none_or(|draft| draft.remote.entity_id() != identity)
                        {
                            return false;
                        }
                        this.poll_clone(started, window, cx)
                    })
                    .unwrap_or(false);
                if !running {
                    break;
                }
            }
        })
        .detach();
        cx.notify();
    }

    /// Ignore raw Git output: it can echo credential-bearing URLs, and is never stored in host messages.
    fn poll_clone(
        &mut self,
        started: Instant,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if matches!(self.decision, Some(Decision::Leave { .. })) {
            return true;
        }
        let Some(draft) = &mut self.clone_project else {
            return false;
        };
        let Some(process) = &draft.process else {
            return false;
        };
        let mut outcome = None;
        for event in process.poll() {
            match event {
                NativeProcessEvent::Update {
                    update: Update::Exited { code: 0 },
                    ..
                } => outcome = Some(Ok(())),
                NativeProcessEvent::Update {
                    update: Update::Exited { .. } | Update::Terminated,
                    ..
                }
                | NativeProcessEvent::Failed { .. } => {
                    outcome = Some(Err("projects.clone_failure"))
                }
                _ => {}
            }
        }
        if outcome.is_none() && started.elapsed() >= Duration::from_secs(600) {
            let _ = process.close(1);
            outcome = Some(Err("projects.clone_timeout"));
        }
        match outcome {
            Some(Ok(())) => {
                let target = draft
                    .target
                    .take()
                    .expect("a launched clone owns its target");
                self.cancel_clone(window, cx);
                self.open_path(target, window, cx);
                false
            }
            Some(Err(key)) => {
                draft.process = None;
                draft.error = Some(t!(key).to_string());
                cx.notify();
                false
            }
            None => true,
        }
    }

    /// Retire this clone's process tree without deleting destination files, then restore the source editor's focus.
    fn cancel_clone(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(draft) = self.clone_project.take()
            && let Some(process) = draft.process
        {
            let _ = process.close(1);
        }
        if let Some(editor) = &self.editor {
            editor.read(cx).editor.focus_handle(cx).focus(window, cx);
        } else {
            self.focus.focus(window, cx);
        }
        cx.notify();
    }

    /// Compose local controls while Base keeps modal keyboard input separate from project commands.
    pub(super) fn render_clone(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.decision.is_some() {
            return None;
        }
        let draft = self.clone_project.as_ref()?;
        let running = draft.process.is_some();
        let mut body = v_flex()
            .debug_selector(|| "project-clone-dialog".into())
            .gap_4()
            .child(
                div()
                    .text_lg()
                    .font_semibold()
                    .child(t!("projects.clone").to_string()),
            );
        if running {
            body = body.child(
                div()
                    .debug_selector(|| "project-clone-progress".into())
                    .py_4()
                    .child(t!("projects.clone_progress").to_string()),
            );
        } else {
            body = body
                .child(
                    v_flex()
                        .gap_2()
                        .child(t!("projects.clone_url").to_string())
                        .child(
                            div().debug_selector(|| "project-clone-url".into()).child(
                                Input::new(&draft.remote)
                                    .h(px(32.))
                                    .accessibility_label(t!("projects.clone_url").to_string()),
                            ),
                        ),
                )
                .child(
                    v_flex()
                        .gap_2()
                        .child(t!("projects.clone_destination").to_string())
                        .child(
                            h_flex()
                                .gap_2()
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .debug_selector(|| "project-clone-path".into())
                                        .child(
                                            Input::new(&draft.destination)
                                                .h(px(32.))
                                                .accessibility_label(
                                                    t!("projects.clone_destination").to_string(),
                                                ),
                                        ),
                                )
                                .child(
                                    Button::new("project-clone-browse")
                                        .debug_selector(|| "project-clone-browse".into())
                                        .icon(IconName::Folder)
                                        .accessibility_label(
                                            t!("projects.clone_destination").to_string(),
                                        )
                                        .disabled(draft.browsing)
                                        .h(px(32.))
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.browse_clone(window, cx)
                                        })),
                                ),
                        ),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(t!("projects.clone_auth").to_string()),
                );
        }
        if let Some(error) = &draft.error {
            body = body.child(
                div()
                    .debug_selector(|| "project-clone-error".into())
                    .text_sm()
                    .text_color(cx.theme().danger)
                    .child(error.clone()),
            );
        }
        body = body.child(
            h_flex()
                .justify_end()
                .gap_2()
                .child(
                    Button::new("project-clone-cancel")
                        .debug_selector(|| "project-clone-cancel".into())
                        .label(t!("interaction.cancel").to_string())
                        .on_click(cx.listener(|this, _, window, cx| this.cancel_clone(window, cx))),
                )
                .when(!running, |row| {
                    row.child(
                        Button::new("project-clone-start")
                            .debug_selector(|| "project-clone-start".into())
                            .primary()
                            .label(t!("projects.clone_start").to_string())
                            .disabled(draft.browsing)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.start_clone(window, cx)),
                            ),
                    )
                }),
        );
        let owner = cx.entity().downgrade();
        Some(project_dialog::clone_modal(
            self.modal_focus.clone(),
            body.into_any_element(),
            move |window, cx| {
                let _ = owner.update(cx, |this, cx| this.cancel_clone(window, cx));
            },
            cx,
        ))
    }
}

/// Validate before launch; an existing destination is never repurposed or cleared on failure.
fn clone_target(remote: &str, target: &Path) -> Option<(PathBuf, PathBuf)> {
    if remote.is_empty()
        || remote.starts_with('-')
        || remote.chars().any(char::is_control)
        || !target.is_absolute()
    {
        return None;
    }
    let name = target.file_name()?.to_str()?;
    if name.chars().any(char::is_control) || (cfg!(windows) && name.contains(':')) {
        return None;
    }
    let parent = std::fs::canonicalize(target.parent()?).ok()?;
    if !parent.is_dir() {
        return None;
    }
    let target = parent.join(name);
    match std::fs::symlink_metadata(&target) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Some((target, parent)),
        _ => None,
    }
}

/// Suggest one safe folder component; repository arguments remain otherwise unchanged.
fn repository_name(remote: &str) -> Option<String> {
    let name = remote
        .trim()
        .trim_end_matches('/')
        .rsplit(['/', ':', '\\'])
        .next()?
        .trim_end_matches(".git");
    (!name.is_empty()
        && name != "."
        && name != ".."
        && name
            .chars()
            .all(|character| character.is_alphanumeric() || matches!(character, '-' | '_' | '.')))
    .then(|| name.to_owned())
}

/// Credentials stay with Git's configured helpers instead of becoming argv or .git/config content.
fn remote_credentials(remote: &str) -> bool {
    let Some((scheme, rest)) = remote.split_once("://") else {
        return false;
    };
    let authority = rest.split('/').next().unwrap_or_default();
    authority.rsplit_once('@').is_some_and(|(user, _)| {
        matches!(scheme.to_ascii_lowercase().as_str(), "http" | "https") || user.contains(':')
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Git credential helpers and SSH user names are allowed; inline tokens/passwords never reach Git.
    #[test]
    fn project_home_clone_rejects_inline_credentials() {
        for remote in [
            "https://token@example.test/repo.git",
            "https://user:password@example.test/repo.git",
            "ssh://user:password@example.test/repo.git",
        ] {
            assert!(remote_credentials(remote));
        }
        for remote in [
            "https://example.test/repo.git",
            "ssh://git@example.test/repo.git",
            "git@example.test:repo.git",
            "C:/repos/local",
        ] {
            assert!(!remote_credentials(remote));
        }
    }
}
