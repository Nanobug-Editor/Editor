//! Window shell, layout, settings, dialogs, and persisted app state.

/// Product name shared by native window titles, the application shell, and LSP identification.
pub(crate) const APP_NAME: &str = "Nanobug";

pub(crate) mod dialog;
pub(crate) mod distribution;
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

pub(crate) use settings::SettingsSection;

#[cfg(target_os = "windows")]
pub(crate) use shell::WindowsTimerResolution;
pub(crate) use shell::{EditorDockPanel, EditorDockPanelKind, bind_editor_shell_keys};
