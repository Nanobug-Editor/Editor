//! Settings operate on application controls; optional workspace controls retain their real editor owner.

use super::*;

/// An empty root has no editor entity and never creates one merely to show global settings.
#[derive(Clone)]
pub(super) enum SettingsOwner {
    Project(Entity<EditorApp>),
    Empty(Entity<app::projects::ProjectWindow>),
}

impl SettingsOwner {
    /// History and other user settings resolve the selected profile without reading unrelated preferences.
    pub(super) fn profile(&self, cx: &App) -> anyhow::Result<PathBuf> {
        match self {
            Self::Project(view) => view
                .read(cx)
                .session_state
                .file_path()
                .and_then(|path| path.parent().map(Path::to_owned))
                .ok_or_else(|| anyhow::anyhow!(t!("projects.launch_missing_default_profile"))),
            Self::Empty(view) => Ok(view.read(cx).profile_directory().to_owned()),
        }
    }

    /// Read the user profile and startup preferences belonging to this actual dialog owner.
    pub(super) fn startup_preferences(
        &self,
        cx: &App,
    ) -> anyhow::Result<(PathBuf, app::startup::preferences::Preferences)> {
        let profile = match self {
            Self::Project(view) => view
                .read(cx)
                .session_state
                .file_path()
                .and_then(|path| path.parent().map(Path::to_owned))
                .ok_or_else(|| anyhow::anyhow!(t!("projects.launch_missing_default_profile")))?,
            Self::Empty(view) => view.read(cx).profile_directory().to_owned(),
        };
        let preferences = app::startup::preferences::Preferences::load(&profile)?;
        Ok((profile, preferences))
    }

    /// Return the most recent write error for this settings dialog, if any.
    pub(super) fn startup_error(&self, cx: &App) -> Option<String> {
        match self {
            Self::Project(view) => view.read(cx).settings_startup_error.clone(),
            Self::Empty(view) => view.read(cx).settings_startup_error.clone(),
        }
    }

    /// Save `enabled` in the selected user profile and redraw only after the atomic write settles.
    pub(super) fn set_restore_last_project(&self, enabled: bool, cx: &mut App) {
        let error = self
            .startup_preferences(cx)
            .and_then(|(profile, mut preferences)| {
                preferences.restore_last_project = enabled;
                preferences.save(&profile)
            })
            .err()
            .map(|error| error.to_string());
        match self {
            Self::Project(view) => view.update(cx, |app, _| app.settings_startup_error = error),
            Self::Empty(view) => view.update(cx, |app, _| app.settings_startup_error = error),
        }
        self.refresh(cx);
    }

    /// Workspace-only categories receive an actual editor or stay absent.
    pub(super) fn project(&self) -> Option<Entity<EditorApp>> {
        match self {
            Self::Project(view) => Some(view.clone()),
            Self::Empty(_) => None,
        }
    }
    pub(super) fn section(&self, cx: &App) -> SettingsSection {
        match self {
            Self::Project(view) => view.read(cx).settings_section,
            Self::Empty(view) => view.read(cx).settings_section,
        }
    }
    /// Navigation and a global preference change redraw the retained native dialog.
    pub(super) fn select_section(&self, section: SettingsSection, cx: &mut App) {
        match self {
            Self::Project(view) => view.update(cx, |app, _| app.settings_section = section),
            Self::Empty(view) => view.update(cx, |app, _| app.settings_section = section),
        }
        self.refresh(cx);
    }
    fn refresh(&self, cx: &mut App) {
        let dialog = match self {
            Self::Project(view) => view.read(cx).dialog.clone(),
            Self::Empty(view) => view.read(cx).dialog.clone(),
        };
        if let Some(dialog) = dialog {
            dialog.update(cx, |_, cx| cx.notify());
        }
        cx.refresh_windows();
    }
    /// Apply through the existing global theme contract; no workspace authority is involved.
    pub(super) fn set_theme(&self, dark: bool, window: &mut Window, cx: &mut App) {
        apply_theme(&theme::active_theme(dark), cx);
        if let Self::Project(view) = self {
            view.update(cx, |app, _| app.dark_theme = dark);
        }
        self.refresh(cx);
        window.refresh();
    }
    /// The global font-size writer supplies the same interface/editor preference in empty startup.
    pub(super) fn step_font(&self, direction: i32, window: &mut Window, cx: &mut App) {
        typography::step_by(cx, direction);
        theme::sync_font_sizes(cx);
        self.refresh(cx);
        window.refresh();
    }
    /// Save the selected tab `placement` for this owner's user profile; settings display any error.
    pub(super) fn set_tab_placement(
        &self,
        placement: app::editor_preferences::TabPlacement,
        cx: &mut App,
    ) {
        if let Ok(profile) = self.profile(cx) {
            let _ = app::editor_preferences::set_placement(&profile, placement, cx);
        }
        self.refresh(cx);
    }
    pub(super) fn dialog_window(&self, cx: &App) -> Option<WindowHandle<Root>> {
        match self {
            Self::Project(view) => view.read(cx).dialog_window,
            Self::Empty(view) => view.read(cx).dialog_window,
        }
    }
    /// A close notification breaks the dialog's retained owner cycle and permits reopening.
    pub(super) fn retain_dialog(
        &self,
        dialog: Entity<app_dialog::AppDialog>,
        handle: WindowHandle<Root>,
        cx: &mut App,
    ) {
        let id = handle.window_id();
        match self {
            Self::Project(view) => {
                let weak = view.downgrade();
                let subscription = cx.on_window_closed(move |cx, closed| {
                    if closed == id {
                        let _ = weak.update(cx, |app, cx| {
                            app.dialog = None;
                            app.dialog_window = None;
                            cx.notify();
                        });
                    }
                });
                view.update(cx, |app, cx| {
                    app.dialog = Some(dialog);
                    app.dialog_window = Some(handle);
                    app._dialog_closed_subscription = Some(subscription);
                    cx.notify();
                });
            }
            Self::Empty(view) => {
                let weak = view.downgrade();
                let subscription = cx.on_window_closed(move |cx, closed| {
                    if closed == id {
                        let _ = weak.update(cx, |app, cx| {
                            app.dialog = None;
                            app.dialog_window = None;
                            cx.notify();
                        });
                    }
                });
                view.update(cx, |app, cx| {
                    app.dialog = Some(dialog);
                    app.dialog_window = Some(handle);
                    app._dialog_closed_subscription = Some(subscription);
                    cx.notify();
                });
            }
        }
    }
    /// Plugin preferences use the same manager and user scope before selecting a project.
    pub(super) fn plugin_form(&self, cx: &mut App) -> Entity<extensions::SettingsView> {
        match self {
            Self::Project(view) => {
                if let Some(form) = &view.read(cx).plugin_settings {
                    return form.clone();
                }
                let panel = view.read(cx).extensions.clone();
                let form = cx.new(|cx| extensions::SettingsView::new(panel, cx));
                view.update(cx, |app, _| app.plugin_settings = Some(form.clone()));
                form
            }
            Self::Empty(view) => {
                if let Some(form) = &view.read(cx).plugin_settings {
                    return form.clone();
                }
                let panel = view.update(cx, |app, cx| app.empty_extensions(cx));
                let form = cx.new(|cx| extensions::SettingsView::new(panel, cx));
                view.update(cx, |app, _| app.plugin_settings = Some(form.clone()));
                form
            }
        }
    }
}
