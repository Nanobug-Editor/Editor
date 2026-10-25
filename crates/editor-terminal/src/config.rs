//! Serializable user settings, validated before applying them to live views.
use rust_i18n::t;
use serde::{Deserialize, Serialize};

/// Native terminal sidebar placement. Its serialized values preserve existing settings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TabPosition {
    /// Render the retained session sidebar to the left of the text grid.
    Left,
    #[default]
    /// Preserve the existing right-side sidebar default.
    Right,
}

/// User-selected cursor default, superseded by an application's explicit DECSCUSR request.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cursor {
    #[default]
    /// Preserve the historical vertical caret.
    Beam,
    /// Invert the complete glyph cell while focused.
    Block,
    /// Draw a horizontal caret at the cell baseline.
    Underline,
}

impl Cursor {
    /// Map the user setting to the maintained terminal core's native cursor shape.
    pub fn shape(self) -> crate::engine::CursorShape {
        use crate::engine::CursorShape;
        match self {
            Self::Beam => CursorShape::Beam,
            Self::Block => CursorShape::Block,
            Self::Underline => CursorShape::Underline,
        }
    }
}

/// Named shell program and separately escaped arguments; never interpolated as a command.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    /// Human-readable profile label, independent from tab rename.
    pub name: String,
    /// Executable path/name resolved by the host's controlled process launcher.
    pub program: String,
    #[serde(default)]
    /// Separate literal arguments; never a host-constructed shell command.
    pub args: Vec<String>,
}

/// Optional palette entries inherit the editor theme. ANSI colors always contain 16 entries.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Palette {
    /// Optional default background RGB hex override.
    pub background: Option<String>,
    /// Optional default text RGB hex override.
    pub foreground: Option<String>,
    /// Optional cursor RGB hex override.
    pub cursor: Option<String>,
    /// Optional selection RGB hex override.
    pub selection: Option<String>,
    /// Optional complete normal/bright ANSI palette in core index order.
    pub ansi: Option<[String; 16]>,
}

/// User-level terminal settings; commands in projects are never executed on startup.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    /// Retained compatibility setting for terminal availability.
    pub enabled: bool,
    /// Sidebar placement also moves the native grid origin; older settings default to the right.
    pub tab_position: TabPosition,
    /// Native terminal font family; fallback remains owned by the platform text system.
    pub font_family: String,
    /// Native logical pixel size, validated to 8–32.
    pub font_size: f32,
    /// Cell line-height multiplier; 1.45 retains the existing viewport geometry.
    pub line_height: f32,
    /// Maximum retained rows per session, validated to 0–100000.
    pub history: usize,
    /// Wheel lines per measured input line; fractional pixel motion accumulates in the view.
    pub scroll_multiplier: f32,
    /// Optional copy on selection, disabled to preserve existing clipboard behavior.
    pub copy_on_select: bool,
    /// Keep the highlight after Copy by default, matching existing terminal behavior.
    pub keep_selection_on_copy: bool,
    /// Preserve the historical steady beam unless the user/application explicitly chooses otherwise.
    pub cursor: Cursor,
    /// Default blink preference; application DECSCUSR can override it.
    pub cursor_blink: bool,
    /// Show OSC titles as secondary status, never replacing an explicit tab name.
    pub show_titles: bool,
    /// Show pending BEL as a session status until focus/input acknowledges it.
    pub visual_bell: bool,
    /// Structured user-level environment passed only to newly authorized Shell processes.
    pub env: std::collections::BTreeMap<String, String>,
    /// New shells can explicitly inherit the selected session's current directory.
    pub inherit_working_directory: bool,
    /// Optional Zed-style Shift+Enter LF. False preserves Nanobug's original Enter byte.
    pub shift_enter_newline: bool,
    /// Optional image-clipboard Ctrl+V forwarding for applications that own image paste protocols.
    pub forward_image_paste: bool,
    /// Zero-based index of the profile used by the normal New Shell action.
    pub default_profile: usize,
    /// User-approved shell profiles; project files cannot supply process authority.
    pub profiles: Vec<Profile>,
    /// Optional legacy palette overrides, retained alongside the editor's theme tokens.
    pub theme: Palette,
    /// Optional command for the explicit Run Project button; never executed at startup.
    pub run_command: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        let profiles = default_profiles(cfg!(windows));
        Self {
            enabled: true,
            tab_position: TabPosition::Right,
            font_family: "Cascadia Mono".into(),
            font_size: 14.,
            line_height: 1.45,
            history: 10_000,
            scroll_multiplier: 3.,
            copy_on_select: false,
            keep_selection_on_copy: true,
            cursor: Cursor::Beam,
            cursor_blink: false,
            show_titles: false,
            visual_bell: true,
            env: Default::default(),
            inherit_working_directory: false,
            shift_enter_newline: false,
            forward_image_paste: false,
            default_profile: 0,
            profiles,
            theme: Palette::default(),
            run_command: None,
        }
    }
}

/// Choose platform defaults from the host environment, never the build target.
fn default_profiles(windows: bool) -> Vec<Profile> {
    if windows {
        vec![
            Profile {
                name: "PowerShell".into(),
                program: "powershell.exe".into(),
                args: vec!["-NoLogo".into()],
            },
            Profile {
                name: "Command Prompt".into(),
                program: "cmd.exe".into(),
                args: vec![],
            },
            Profile {
                name: "PowerShell 7".into(),
                program: "pwsh.exe".into(),
                args: vec!["-NoLogo".into()],
            },
            Profile {
                name: "WSL".into(),
                program: "wsl.exe".into(),
                args: vec![],
            },
        ]
    } else {
        vec![Profile {
            name: "Shell".into(),
            program: "/bin/sh".into(),
            args: vec![],
        }]
    }
}

impl Settings {
    /// Build startup settings for the OS on which the native process service will execute.
    pub fn for_os(os: &str) -> Self {
        Self {
            profiles: default_profiles(os == "windows"),
            ..Self::default()
        }
    }
    /// Fill omitted profiles before validation; explicit user programs are never replaced.
    pub fn parse_for_os(source: &str, os: &str) -> anyhow::Result<Self> {
        let mut value: serde_json::Value = serde_json::from_str(source)?;
        if let Some(object) = value.as_object_mut() {
            if !object.contains_key("profiles") {
                object.insert(
                    "profiles".into(),
                    serde_json::to_value(default_profiles(os == "windows"))?,
                );
            }
        }
        Self::parse(&serde_json::to_string(&value)?)
    }
    /// Reject malformed values instead of crashing or silently selecting another shell.
    pub fn parse(source: &str) -> anyhow::Result<Self> {
        let settings: Self = serde_json::from_str(source)?;
        anyhow::ensure!(
            !settings.profiles.is_empty()
                && settings.profiles.len() <= 32
                && settings.font_family.len() <= 256,
            t!("terminal.invalid_profiles")
        );
        anyhow::ensure!(
            (8. ..=32.).contains(&settings.font_size),
            t!("terminal.invalid_font_size")
        );
        anyhow::ensure!(
            settings.line_height.is_finite() && (1.0..=3.0).contains(&settings.line_height),
            t!("terminal.invalid_line_height")
        );
        anyhow::ensure!(
            settings.default_profile < settings.profiles.len(),
            t!("terminal.invalid_default_profile")
        );
        anyhow::ensure!(settings.history <= 100_000, t!("terminal.invalid_history"));
        anyhow::ensure!(
            settings.scroll_multiplier.is_finite()
                && (0.1..=20.).contains(&settings.scroll_multiplier),
            t!("terminal.invalid_scroll_multiplier")
        );
        anyhow::ensure!(
            settings.env.len() <= 128
                && settings.env.iter().all(|(key, value)| !key.is_empty()
                    && key.len() <= 256
                    && !key.contains(['=', '\0'])
                    && value.len() <= 32768
                    && !value.contains('\0')),
            t!("terminal.invalid_environment")
        );
        anyhow::ensure!(
            !settings.font_family.trim().is_empty(),
            t!("terminal.invalid_font_family")
        );
        for profile in &settings.profiles {
            anyhow::ensure!(
                profile.name.len() <= 128
                    && profile.program.len() <= 4096
                    && !profile.program.contains('\0')
                    && profile.args.len() <= 128
                    && profile
                        .args
                        .iter()
                        .all(|arg| arg.len() <= 32768 && !arg.contains('\0')),
                t!("terminal.invalid_profile")
            );
            anyhow::ensure!(
                !profile.name.trim().is_empty() && !profile.program.trim().is_empty(),
                t!("terminal.empty_profile")
            );
        }
        let p = &settings.theme;
        for color in [&p.background, &p.foreground, &p.cursor, &p.selection]
            .into_iter()
            .flatten()
            .chain(p.ansi.iter().flatten())
        {
            anyhow::ensure!(
                color.len() == 7
                    && color.starts_with('#')
                    && color[1..].bytes().all(|b| b.is_ascii_hexdigit()),
                t!("terminal.invalid_color", color = color)
            );
        }
        Ok(settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Older profiles omit the new options and retain their original input/clipboard/cursor defaults.
    #[test]
    fn old_settings_keep_compatible_defaults() {
        let settings =
            Settings::parse(r#"{"tab_position":"left","history":500,"font_size":16}"#).unwrap();
        assert_eq!(settings.tab_position, TabPosition::Left);
        assert_eq!(settings.cursor, Cursor::Beam);
        assert!(!settings.copy_on_select && settings.keep_selection_on_copy);
        assert!(!settings.shift_enter_newline && !settings.forward_image_paste);
        assert!(settings.env.is_empty());
        assert_eq!(settings.history, 500);
    }

    /// Invalid geometry/environment is rejected before it can affect a live viewport or native launch.
    #[test]
    fn invalid_native_settings_are_rejected() {
        for source in [
            r#"{"line_height":0.5}"#,
            r#"{"scroll_multiplier":0}"#,
            r#"{"env":{"BAD=NAME":"value"}}"#,
            r#"{"env":{"X":"\u0000"}}"#,
        ] {
            assert!(Settings::parse(source).is_err(), "{source}");
        }
    }
}
