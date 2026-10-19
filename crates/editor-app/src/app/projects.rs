//! Native project ownership and shared application routing before and after selection.

mod clone_repository;
mod close;
mod controller;
mod empty;
mod recent;
pub(crate) mod switcher;
mod view;

#[cfg(test)]
// The cross-feature scenarios stay in tests/ while observing this production owner's lifecycle.
#[path = "../tests/project_home.rs"]
mod tests;

use crate::*;
use gpui_base::input::InputState;
use gpui_kit::PathPromptOptions;
use recent::RecentProjects;

/// Empty startup uses the editor's default geometry without loading any project's session.
pub(crate) const DEFAULT_WINDOW_WIDTH: f32 = 1280.;
pub(crate) const DEFAULT_WINDOW_HEIGHT: f32 = 820.;

/// Leaving a project and closing a window share consent and resource cleanup.
#[derive(Clone, Copy)]
pub(crate) enum CloseIntent {
    Window,
    /// All native roots gather consent before any project begins resource retirement.
    Application,
}
actions!(projects, [OpenFolder, ShowHome, CloseWindow]);
/// Empty and active editor windows use the same application-wide consent coordinator.
pub(crate) fn quit_application(cx: &mut App) {
    controller::quit(cx);
}

/// External GUI requests enter the existing native project lifecycle in this profile.
/// The caller reports dispatch errors instead of silently falling back to the process directory.
pub(crate) fn open_request(
    profile: PathBuf,
    path: Option<PathBuf>,
    cx: &mut App,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        !controller::is_quitting(cx),
        "{}",
        t!("projects.external_quitting")
    );
    let Some(path) = path else {
        return controller::activate_or_open(profile, cx);
    };
    let file = path.is_file().then(|| path.clone());
    let root = if file.is_some() {
        path.parent()
            .ok_or_else(|| anyhow::anyhow!("Missing file parent"))?
    } else {
        path.as_path()
    };
    let workspace = Workspace::open(root)?;
    let identity = platform_windows::directory_identity::DirectoryIdentity::open(workspace.root())?;
    if let Some(existing) = controller::owner(&identity, cx) {
        let host = existing
            .host
            .upgrade()
            .ok_or_else(|| anyhow::anyhow!("Project window closed"))?;
        anyhow::ensure!(
            host.read(cx).close_state != CloseState::Cleaning
                && !matches!(host.read(cx).decision, Some(Decision::Leave { .. })),
            "{}",
            t!("projects.external_busy")
        );
        controller::activate(existing, file, cx);
        Ok(())
    } else {
        controller::open(
            profile,
            Some(file.unwrap_or_else(|| workspace.root().to_owned())),
            cx,
        )
    }
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

/// One native editor window may be empty; workspace resources exist only after explicit selection.
pub(crate) struct ProjectWindow {
    editor: Option<Entity<EditorApp>>,
    profile: PathBuf,
    recent: Rc<std::cell::RefCell<RecentProjects>>,
    decision: Option<Decision>,
    error: Option<String>,
    choosing: bool,
    close_state: CloseState,
    focus: FocusHandle,
    modal_focus: FocusHandle,
    /// A user-requested clone owns its inputs and native process until completion or cancellation.
    clone_project: Option<clone_repository::CloneProject>,
    project_menu: switcher::ProjectMenuState,
    titlebar_should_move: bool,
    /// Global dialogs can be opened before selecting a workspace.
    pub(in crate::app) dialog: Option<Entity<app_dialog::AppDialog>>,
    pub(in crate::app) dialog_window: Option<WindowHandle<Root>>,
    pub(in crate::app) _dialog_closed_subscription: Option<Subscription>,
    pub(in crate::app) settings_section: app::settings::SettingsSection,
    extensions: Option<Entity<extensions::ExtensionPanel>>,
    extensions_window: Option<WindowHandle<Root>>,
    _extensions_closed_subscription: Option<Subscription>,
    pub(in crate::app) plugin_settings: Option<Entity<extensions::SettingsView>>,
    application_popup: Option<Entity<crate::ui::controls::menu::PopupMenu>>,
    application_anchor: Rc<Cell<Point<Pixels>>>,
    _activation_subscription: Subscription,
}

impl ProjectWindow {
    /// The legacy project-management command now opens this window's project dropdown.
    pub(super) fn show_home(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if controller::is_quitting(cx) {
            return;
        }
        if let Some(editor) = self.editor.clone() {
            editor.update(cx, |app, cx| {
                let focus = window
                    .focused(cx)
                    .unwrap_or_else(|| app.editor.focus_handle(cx));
                app.open_project_menu(focus, window, cx);
            });
        } else {
            let focus = window.focused(cx).unwrap_or_else(|| self.focus.clone());
            self.open_project_menu(focus, window, cx);
        }
    }
    /// Create an empty editor shell in the selected profile without initializing workspace tools.
    pub(crate) fn new(profile: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        app::bind_editor_shell_keys(cx);
        let recent = controller::recent(&profile, cx);
        let error = recent.borrow().error.clone();
        let handle = window.window_handle();
        controller::register(cx.entity().downgrade(), handle, profile.clone(), cx);
        cx.on_release(move |_, cx| controller::unregister(handle, cx))
            .detach();
        let activation_subscription = cx.observe_window_activation(window, |_, window, cx| {
            if window.is_window_active() {
                controller::mark_active(window.window_handle(), cx);
            }
        });
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
            clone_project: None,
            project_menu: Default::default(),
            titlebar_should_move: false,
            dialog: None,
            dialog_window: None,
            _dialog_closed_subscription: None,
            settings_section: app::settings::SettingsSection::AppearanceAndBehavior,
            extensions: None,
            extensions_window: None,
            _extensions_closed_subscription: None,
            plugin_settings: None,
            application_popup: None,
            application_anchor: Default::default(),
            _activation_subscription: activation_subscription,
        };
        window.set_window_title(app::APP_NAME);
        this.install_close_gate(window, cx);
        this.focus.focus(window, cx);
        this
    }

    /// GUI startup and folder selection share validation and explicit local authority.
    /// Invalid or cancelled requests preserve the current view without a fallback directory.
    pub(crate) fn open_path(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if self.close_state == CloseState::Cleaning
            || controller::is_quitting(cx)
            || self.decision.is_some()
            || self.clone_project.is_some()
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
        let identity =
            match platform_windows::directory_identity::DirectoryIdentity::open(workspace.root()) {
                Ok(identity) => identity,
                Err(error) => {
                    self.error = Some(error.to_string());
                    cx.notify();
                    return;
                }
            };
        if let Some(existing) = controller::owner(&identity, cx) {
            if existing.handle.window_id() == window.window_handle().window_id() {
                window.activate_window();
                if let (Some(editor), Some(file)) = (&self.editor, file) {
                    editor.update(cx, |app, cx| app.open_file(file, window, cx));
                }
            } else {
                controller::activate(existing, file, cx);
            }
            return;
        }
        if self.editor.is_some() {
            let path = file.unwrap_or_else(|| workspace.root().to_owned());
            self.error = controller::open(self.profile.clone(), Some(path), cx)
                .err()
                .map(|error| error.to_string());
            cx.notify();
            return;
        }
        controller::reserve(window.window_handle(), Some(identity), cx);
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
        // Every failed activation releases its reservation so the same home can accept another selection.
        (|| {
            if controller::is_quitting(cx) {
                return;
            }
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
            let identity =
                platform_windows::directory_identity::DirectoryIdentity::open(workspace.root());
            if identity.as_ref().ok() != controller::reserved(window.window_handle(), cx).as_ref() {
                self.error = Some(t!("projects.changed_before_open").to_string());
                cx.notify();
                return;
            }
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
            crate::ui::controls::window::restore_editor_geometry(
                window,
                size(px(session.window_width), px(session.window_height)),
                cx,
            );
            let editor = cx
                .new(|cx| EditorApp::new_with_session(workspace, file, Some(session), window, cx));
            let owner = cx.entity().downgrade();
            editor.update(cx, |app, _| app.project_window = Some(owner));
            self.editor = Some(editor);
            self.decision = None;
            controller::mark_opened(window.window_handle(), root.clone(), cx);
            // Construction installs the legacy standalone gate; the project window supersedes it.
            self.install_close_gate(window, cx);
            window.set_window_title(&format!(
                "{} — {}",
                editor_core::display_path(&root),
                app::APP_NAME
            ));
            if let Err(error) = self.recent.borrow_mut().opened(&root) {
                self.error = Some(error.to_string());
            }
            cx.notify();
            cx.refresh_windows();
        })();
        if self.editor.is_none() {
            controller::reserve(window.window_handle(), None, cx);
        }
    }

    /// A delayed platform selection can affect only this same unopened window.
    pub(super) fn choose_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.choosing
            || self.close_state == CloseState::Cleaning
            || controller::is_quitting(cx)
            || self.decision.is_some()
            || self.clone_project.is_some()
        {
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
        if matches!(
            self.decision,
            Some(Decision::Leave {
                intent: CloseIntent::Application,
                ..
            })
        ) {
            controller::cancel_quit(cx);
        }
        if matches!(self.decision, Some(Decision::Trust { .. })) {
            controller::reserve(window.window_handle(), None, cx);
        }
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
