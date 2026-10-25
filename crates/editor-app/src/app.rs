//! Window shell, layout, settings, dialogs, and persisted app state.

/// Product name shared by native window titles, the application shell, and LSP identification.
pub(crate) const APP_NAME: &str = "Nanobug";

pub(crate) mod dialog;
pub(crate) mod distribution;
/// Profile-wide editor presentation preferences shared by all project windows.
pub(crate) mod editor_preferences;
pub(crate) mod language_servers;
pub(crate) mod languages;
mod layout;
pub(crate) mod messages;
pub(crate) mod plugins;
/// Project selection owns the native window independently of an opened editor session.
pub(crate) mod projects;
pub(crate) mod session;
mod settings;
mod shell;
pub(crate) mod shortcuts;
/// Ordinary GUI admission and external requests share one profile-scoped application owner.
pub(crate) mod startup;

pub(crate) use settings::SettingsSection;

#[cfg(target_os = "windows")]
pub(crate) use shell::WindowsTimerResolution;
pub(crate) use shell::{EditorDockPanel, EditorDockPanelKind, bind_editor_shell_keys};
