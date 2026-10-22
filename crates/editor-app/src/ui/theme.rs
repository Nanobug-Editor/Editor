//! Host selection joins enabled theme contributions to the shared UI palette and style resolver.
pub use editor_ui::theme::*;

/// Select an enabled package for `dark`, falling back to the bundled product theme.
pub fn active_theme(dark: bool) -> plugin_schema::ThemeDefinition {
    crate::extensions::contributions::theme(dark).unwrap_or_else(|| builtin_theme(dark).clone())
}
