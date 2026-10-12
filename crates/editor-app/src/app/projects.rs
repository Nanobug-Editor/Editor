//! Native window ownership before and after a single project session is selected.

mod close;
mod recent;
mod view;

#[cfg(test)]
// The cross-feature scenarios stay in tests/ while observing this production owner's lifecycle.
#[path = "../tests/project_home.rs"]
mod tests;

use crate::*;
use gpui_kit::PathPromptOptions;
use recent::RecentProjects;

/// Leaving a project and closing a window share consent and resource cleanup.
#[derive(Clone, Copy)]
pub(crate) enum CloseIntent {
    Home,
    Window,
}

/// A failed cleanup retains the project and admits retry, while an in-flight cleanup blocks input.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CloseState {
    Idle,
    Cleaning,
    Failed,
}

/// Immutable close-consent identity: document path, session revision and native text fingerprint.
type DocumentVersions = Vec<(PathBuf, u64, [u8; 32])>;

/// A pending decision retains immutable identity metadata, not another mutable document.
enum Decision {
    Trust {
        workspace: Workspace,
        initial_file: Option<PathBuf>,
    },
    Leave {
        intent: CloseIntent,
        documents: DocumentVersions,
        dirty: bool,
        running: bool,
    },
}

/// One native window owns a home or one editor; an unopened home owns no project resources.
pub(crate) struct ProjectWindow {
    editor: Option<Entity<EditorApp>>,
    profile: PathBuf,
    recent: RecentProjects,
    decision: Option<Decision>,
    error: Option<String>,
    choosing: bool,
    close_state: CloseState,
    focus: FocusHandle,
    modal_focus: FocusHandle,
    scroll: ScrollHandle,
}

impl ProjectWindow {
    /// Create a home in the selected user profile without initializing workspace tools.
    pub(crate) fn new(profile: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let recent = RecentProjects::load(&profile);
        let error = recent.error.clone();
        let this = Self {
            editor: None,
            profile,
            recent,
            decision: None,
            error,
            choosing: false,
            close_state: CloseState::Idle,
            focus: cx.focus_handle(),
            modal_focus: cx.focus_handle(),
            scroll: ScrollHandle::new(),
        };
        this.install_close_gate(window, cx);
        this.focus.focus(window, cx);
        this
    }

    /// GUI startup and folder selection share validation and explicit local authority.
    /// Invalid or cancelled requests preserve the current view without a fallback directory.
    pub(crate) fn open_path(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if self.close_state == CloseState::Cleaning
            || self.editor.is_some()
            || self.decision.is_some()
        {
            return;
        }
        self.error = None;
        let (root, file) = if path.is_file() {
            (
                path.parent().unwrap_or(Path::new(".")).to_owned(),
                Some(path),
            )
        } else {
            (path, None)
        };
        let workspace = match Workspace::open(root) {
            Ok(workspace) => workspace,
            Err(error) => {
                self.error = Some(error.to_string());
                cx.notify();
                return;
            }
        };
        if let Some(trusted) = SessionState::recorded_trust(workspace.root(), &self.profile) {
            self.activate_project(workspace, file, trusted, window, cx);
        } else {
            self.decision = Some(Decision::Trust {
                workspace,
                initial_file: file,
            });
            self.modal_focus.focus(window, cx);
            cx.notify();
        }
    }

    /// Persist authority before tools start; failed or damaged session storage cannot be overwritten.
    fn activate_project(
        &mut self,
        workspace: Workspace,
        file: Option<PathBuf>,
        trusted: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // The folder can disappear while consent is open; do not persist authority or start tools
        // against a stale Workspace that was validated before the user made a decision.
        let workspace = match Workspace::open(workspace.root()) {
            Ok(current) if current.root() == workspace.root() => current,
            Ok(_) => {
                self.error = Some(t!("projects.changed_before_open").to_string());
                cx.notify();
                return;
            }
            Err(error) => {
                self.error = Some(error.to_string());
                cx.notify();
                return;
            }
        };
        // Finish the established finite data import before writing any new trust/session decision.
        if let Err(error) = terminal::upgrade::prepare(workspace.root(), &self.profile) {
            self.error = Some(error.to_string());
            cx.notify();
            return;
        }
        let mut session = SessionState::load_in_profile(workspace.root(), &self.profile);
        session.workspace_trusted = trusted;
        if let Err(error) = session.try_save() {
            self.error = Some(error.to_string());
            cx.notify();
            return;
        }
        let root = workspace.root().to_owned();
        window.resize(size(px(session.window_width), px(session.window_height)));
        let editor =
            cx.new(|cx| EditorApp::new_with_session(workspace, file, Some(session), window, cx));
        let owner = cx.entity().downgrade();
        editor.update(cx, |app, _| app.project_window = Some(owner));
        self.editor = Some(editor);
        self.decision = None;
        // Construction installs the legacy standalone gate; the project window supersedes it.
        self.install_close_gate(window, cx);
        window.set_window_title(&format!("{} — {}", root.display(), app::APP_NAME));
        if let Err(error) = self.recent.opened(&root) {
            self.error = Some(error.to_string());
        }
        cx.notify();
    }

    /// A delayed platform selection can affect only this same unopened window.
    fn choose_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.choosing || self.close_state == CloseState::Cleaning || self.editor.is_some() {
            return;
        }
        self.choosing = true;
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(t!("projects.open_folder").into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = receiver.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.choosing = false;
                match result {
                    Ok(Ok(Some(paths))) => {
                        if let Some(path) = paths.into_iter().next() {
                            this.open_path(path, window, cx);
                        }
                    }
                    Ok(Err(error)) => this.error = Some(error.to_string()),
                    _ => {}
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// Native chrome keeps the window alive through decisions and asynchronous resource cleanup.
    fn install_close_gate(&self, window: &mut Window, cx: &mut Context<Self>) {
        let owner = cx.entity().downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            owner
                .update(cx, |this, cx| {
                    this.request_close(CloseIntent::Window, window, cx)
                })
                .unwrap_or(true)
        });
    }

    /// Cancelling restores editing focus and leaves unsaved data and running programs untouched.
    fn cancel_decision(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.decision = None;
        self.error = None;
        if let Some(editor) = &self.editor {
            editor.read(cx).editor.focus_handle(cx).focus(window, cx);
        } else {
            self.focus.focus(window, cx);
        }
        cx.notify();
    }
}
