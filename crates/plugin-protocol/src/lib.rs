//! Nanobug's public, platform-independent plugin contracts and optional WASM guest helpers.
//!
//! # Building an independent plugin
//!
//! Declare `plugin-protocol = { version = "=0.2.0", features = ["guest"] }`, then run
//! `editor-app --plugin-cargo <Cargo.toml> build --target wasm32-wasip2`. The host supplies
//! its embedded source cache through Cargo configuration; a plugin must not depend on a
//! repository checkout or carry a second SDK copy. `--export-plugin-sdk <directory>`
//! exports the same Rust sources, Cargo manifest, WIT and JSON resources. Interface usage
//! lives in these source comments; Markdown is not part of the independent SDK.
//!
//! # Compatibility and authority
//!
//! Package versions, component transport versions and capability versions are separate.
//! The current host release is 0.1.0. [`Manifest::minimum_host`] is an optional SemVer
//! floor, not a capability version or an upper-bound range. It is checked for both package
//! admission and restored records; a future floor diagnoses an update requirement without
//! deleting saved settings/data. Declarative `plugin.toml` host ranges are checked separately.
//! Current executable packages use protocol `7` and declare their base API and required
//! or optional capability ranges in [`api::Requirements`]. The host negotiates these
//! before activation: an unavailable required capability rejects the package, while an
//! unavailable optional capability must disable only the plugin's dependent behavior.
//! Old component protocols are rejected with an update requirement rather than silently
//! translated. This crate contains contracts, never editor, terminal or native UI code.
//!
//! Stable capabilities preserve their message shapes and meanings within a major version.
//! A plugin using an added minor feature declares that minimum capability version; a
//! breaking change requires a new capability major. Deprecation must identify the affected
//! version range, replacement, minimum replacement capability and migration, while keeping
//! the current stable major callable. Experimental contracts must be identified explicitly
//! and do not inherit that stable guarantee. Negotiation never grants permissions: callers
//! still need installation approval, a trusted scope and ownership of every resource.
//!
//! # Starting a call
//!
//! Guest helpers under `api::guest` (feature `guest`) return host-issued handles for asynchronous work.
//! Request acceptance is distinct from completion; consume the corresponding notification,
//! reject stale target revisions, and release or cancel retained handles when no longer used.
//! [`commands`] documents typed command composition and native menus; [`interaction`]
//! documents native dialogs and selected-file authority. Document identities, exact reads,
//! metadata subscriptions, virtual content and comparisons use [`api::DocumentInfo`],
//! [`api::DocumentSnapshot`] and [`api::EditorOperation`]. Instance retirement revokes
//! resources; opaque handles and snapshots cannot restore external side effects.
//!
//! # Feature lifetimes
//!
//! With `guest`, [`api::guest::ResourceGroup`] owns independent document subscriptions and timers;
//! [`api::guest::EditorIntent`] replaces asynchronous editor work without accepting late results;
//! [`http::HttpTask`] owns a stream and its polling timer. Feed each helper its notifications and
//! explicitly close it when the feature ends. Hiding a panel does not automatically end its work.
//! Cleanup reports distinguish release failure from the business result. HTTP terminal events still
//! require closing their stream, while editor terminal notifications and one-shot timer receipts
//! arrive after the host released those handles. Drop never calls the host; instance retirement
//! remains the final guard after a trap. Never serialize or duplicate live helper ownership.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Visible declarations, demand reasons and explicit continuous-work policy.
pub mod activation;
/// Capability-based lifecycle, notifications and correlated host operations.
pub mod api;
/// Typed command composition and stable native menu locations.
pub mod commands;
/// Public command-template, native-form and validation service contract.
pub mod configurations;
pub mod debug;
/// Native source-owned point and range markers with explicit edit and deletion semantics.
pub mod decorations;
pub mod dependencies;
/// Owned multi-source diagnostics and native, previewed business fixes.
pub mod diagnostics;
/// Experimental pure language features consumed by the native editor.
pub mod direct;
pub mod execution;
/// General HTTP streams with target grants, backpressure and cancellation.
pub mod http;
/// Native host dialogs and instance-bound user-selected file authority.
pub mod interaction;
pub mod language;
pub mod language_content;
/// Explicit native project membership and bounded metadata queries for shared source snapshots.
pub mod language_context;
/// Provider-owned native control over the existing language connection, with bounded inert values.
pub mod language_control;
pub mod presentation;
pub mod process;
/// System-protected credentials isolated by host profile and plugin identity.
pub mod secrets;
pub mod service;
pub mod settings;
/// Managed private data, cache and temporary directories with instance-owned cleanup.
pub mod storage;
/// Readonly document trees and fold ranges supplied by isolated, versioned plugin workers.
pub mod structure;
/// Dynamic target discovery and build preparation are provider policy, independent of execution.
pub mod targets;
/// Disposable timer and debounce resources revoked with their owning instance.
pub mod timers;
/// Generic native lazy trees and their bounded provider callbacks.
pub mod tree;
pub mod ui;
pub mod workspace;

/// Guest imports and exports use the WIT contract supplied by the building editor.
#[cfg(feature = "guest")]
pub mod bindings {
    wit_bindgen::generate!({
        path: "wit",
        world: "plugin",
        pub_export_macro: true,
        default_bindings_module: "::plugin_protocol::bindings",
    });
}

/// A package's identity, compatibility range and explicitly requested capabilities.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Explicitly approved, finite package consolidation; it grants no cross-plugin file access.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub replaces: Vec<Replacement>,
    /// Optional minimum host release as a full SemVer of at most 128 bytes (for example `0.1.0`).
    /// This is a floor, not `^0.1`; comparison ignores build metadata and retains prerelease precedence.
    /// Omission retains capability-based compatibility; unsupported floors reject before activation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum_host: Option<String>,
    /// Business instances default to demand activation; static contributions remain discoverable.
    #[serde(default, skip_serializing_if = "activation::Policy::is_default")]
    pub activation: activation::Policy,
    /// Opt-in private-data format; the host never interprets the plugin's business fields.
    #[serde(default)]
    pub data_format: Option<DataFormat>,
    /// Versioned plugin-to-plugin contracts, separate from native executable declarations.
    #[serde(default)]
    pub plugin_services: service::Declarations,
    /// Standard LSP bindings consume native service declarations without requiring a lifecycle component.
    #[serde(default)]
    pub language_servers: Vec<language::Provider>,
    /// Pure structure contributors are independent of the selected native language service.
    #[serde(default)]
    pub structure_providers: Vec<structure::Provider>,
    /// Experimental readonly language providers; independent of the selected native LSP.
    #[serde(default)]
    pub direct_language_providers: Vec<direct::Provider>,
    /// Fixed native service definitions; each key requires its own installation grant.
    #[serde(default)]
    pub services: std::collections::BTreeMap<String, process::Service>,
    /// Sealed package resources for separately managed native analysis workers; never host modules.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub module_exports: std::collections::BTreeMap<String, language_control::ModuleExport>,
    /// Plugin namespace only; these declarations cannot alter host trust, grants or editor preferences.
    #[serde(default)]
    pub settings: std::collections::BTreeMap<String, settings::Definition>,
    /// Opt in to a bounded, preparation-only validation/discovery callback.
    #[serde(default)]
    pub settings_hook: bool,
    /// dependencies 1.4: readonly selection from each method's immutable native-plan allowlist.
    /// Omission prepares all declared plans; the hook cannot add plans or start native work.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub service_preparation_hook: bool,
    pub id: String,
    pub name: String,
    pub version: String,
    pub protocol: u32,
    /// Protocol 7 selects the capability transport; interface versions negotiate independently.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api: Option<api::Requirements>,
    /// Execution ownership is independent of package-wide installation and enablement.
    #[serde(default)]
    pub scope: api::InstanceScope,
    /// Executable plugins provide a WASM component; declarative packages use host lifecycle only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component: Option<String>,
    /// Optional package-relative declarative language, theme, and file-icon manifest.
    #[serde(default)]
    pub contributions: Option<String>,
    #[serde(default)]
    pub permissions: BTreeSet<String>,
    #[serde(default)]
    pub panels: Vec<Panel>,
    #[serde(default)]
    pub commands: Vec<Command>,
    /// Maximum opaque persisted snapshot size, also bounded by the host quota.
    pub storage_limit: usize,
}

/// Version zero is reserved for a new scope; changed versions require an explicit WASM migration hook.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataFormat {
    pub version: u32,
    #[serde(default)]
    pub migration_hook: bool,
}

/// A bounded replacement of installed versions and selected public contracts.
/// Original private data and dependency receipts are retained for recovery; no guest reads them.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Replacement {
    pub id: String,
    pub versions: semver::VersionReq,
    pub contracts: Vec<String>,
    /// Boolean setting whose user/project values preserve the old package's enablement intent.
    pub enabled_setting: String,
}

/// Dock and file contributions declare placement; their domain controls arrive through `ui::Document`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Panel {
    /// Editor-local auxiliary contributions may publish tools but cannot become a whole-layout provider.
    #[serde(default)]
    pub auxiliary: bool,
    pub id: String,
    pub title: String,
    /// `window` (ui.windows 1.0) is a separate native work window, shown through
    /// SetPanelVisibility. Closing hides only this contribution and retains plugin/document state.
    pub position: String,
    /// Editor-local previews match these case-insensitive extensions without dots.
    /// Other panel positions leave this list empty and retain their independent dock behavior.
    #[serde(default)]
    pub file_extensions: Vec<String>,
    /// Non-text file types matched by this provider. They never create a text editing session.
    /// Requires `editor.files`; the declaration remains meaningful when the viewer is disabled.
    #[serde(default)]
    pub readonly_file_extensions: Vec<String>,
    /// Used only when this panel has no saved visibility preference yet.
    #[serde(default = "panel_visible_by_default")]
    pub default_visible: bool,
    /// Occupy the entire current side/bottom region when revealed, temporarily replacing peers.
    /// Closing restores their saved layout and visibility. Defaults to false; requires
    /// `ui.panels >=1.2, <2` and position `left`, `right` or `bottom`, never `editor`/`window`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub exclusive: bool,
    /// Whether a non-editor panel has an entry in the native window-control group (default true).
    /// False requires `ui.panels >=1.1` and only hides the entry; SetPanelVisibility and saved
    /// visibility remain independent, so a plugin can open contextual work windows itself.
    #[serde(
        default = "panel_visible_by_default",
        skip_serializing_if = "panel_control_is_default"
    )]
    pub show_in_window_controls: bool,
    /// Optional status-bar position among plugin panels; lower values appear first.
    #[serde(default)]
    pub status_order: Option<i32>,
    /// Optional package-relative SVG used when the editor has a light theme.
    #[serde(default)]
    pub icon_light: Option<String>,
    /// Optional package-relative SVG used when the editor has a dark theme.
    #[serde(default)]
    pub icon_dark: Option<String>,
}

/// Omitted visibility and window-control choices retain existing manifest behavior.
fn panel_visible_by_default() -> bool {
    true
}
/// Omit the unchanged default so rebuilding an existing package does not add a newer schema field.
fn panel_control_is_default(value: &bool) -> bool {
    *value
}
/// Menu and shortcut invocations remain commands; domain footer buttons use `ui::Document::tools`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub shortcut: Option<String>,
    #[serde(default)]
    pub menu: bool,
    /// Opt-in typed result contract; absent signatures preserve the existing one-way event behavior.
    #[serde(default)]
    pub signature: Option<service::Method>,
    /// Native contribution points are independent of plugin-owned footer tools.
    #[serde(default)]
    pub menus: Vec<commands::Menu>,
}

/// Opaque plugin-owned data; the host never interprets or migrates its contents.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Snapshot {
    pub schema: u32,
    pub data: String,
}

/// Host theme and workspace context are supplied explicitly, without ambient access.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Environment {
    pub workspace: String,
    pub os: String,
    /// Current user interface locale, supplied at preparation and in Theme notifications.
    /// Missing or empty values preserve the existing Simplified Chinese default.
    #[serde(default)]
    pub locale: String,
    pub background: u32,
    pub foreground: u32,
    pub muted: u32,
    /// Editor text color used for subdued terminal text and metadata.
    #[serde(default)]
    pub muted_foreground: u32,
    pub border: u32,
    pub accent: u32,
    pub selection: u32,
    /// Whether the active editor theme requests a dark terminal palette.
    #[serde(default)]
    pub dark: bool,
    /// Generic theme color tokens; plugins interpret only keys they own.
    #[serde(default)]
    pub theme_colors: std::collections::BTreeMap<String, u32>,
    /// Resolved editor fonts are the fallback for every plugin's own text roles.
    #[serde(default)]
    pub ui_font: FontStyle,
    #[serde(default)]
    pub mono_font: FontStyle,
    /// Plugin-owned text roles, updated together with colors by api::Notification::Theme.
    #[serde(default)]
    pub theme_text_styles: std::collections::BTreeMap<String, FontStyle>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FontStyle {
    pub family: Option<String>,
    pub size_px: Option<f32>,
    pub bold: Option<bool>,
}

impl FontStyle {
    pub fn over(self, base: Self) -> Self {
        Self {
            family: self.family.or(base.family),
            size_px: self.size_px.or(base.size_px),
            bold: self.bold.or(base.bold),
        }
    }
}

impl Environment {
    /// A plugin consumes its own role without knowing how themes are installed or switched.
    pub fn font_style(&self, plugin: &str, role: &str, monospace: bool) -> FontStyle {
        let base = if monospace {
            &self.mono_font
        } else {
            &self.ui_font
        };
        self.theme_text_styles
            .get(&format!("{plugin}.{role}"))
            .cloned()
            .unwrap_or_default()
            .over(base.clone())
    }

    pub fn color(&self, plugin: &str, role: &str) -> Option<u32> {
        self.theme_colors.get(&format!("{plugin}.{role}")).copied()
    }
}

/// Physical surface coordinates let a guest lay out its own interface.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}
impl Rect {
    /// Hit testing is shared by arbitrary plugin widgets.
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }
}

/// A declarative drawing list; text is shaped by the native host text system.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Paint {
    /// Draw a full-color vector above preceding operations, preserving transparency.
    /// The explicit clip bounds raster allocation when the image is zoomed beyond its viewport.
    Svg {
        rect: Rect,
        clip: Rect,
        source: String,
    },
    Fill {
        rect: Rect,
        color: u32,
        /// Fill through the current native canvas bottom while resize events catch up.
        #[serde(default)]
        extend_to_bottom: bool,
    },
    Text {
        x: f32,
        y: f32,
        text: String,
        color: u32,
        size: f32,
        bold: bool,
        #[serde(default)]
        font: Option<String>,
    },
    /// `ui.canvas` 1.4: shape a single line with native font metrics inside a clipped rectangle.
    /// Text that exceeds the width is ellipsized by the host before horizontal alignment.
    TextBox {
        /// Canvas-local logical pixels; clipping and alignment use the same rectangle.
        rect: Rect,
        /// UTF-8 content; the original value remains in the scene while the host shortens its paint.
        text: String,
        /// Foreground RGB, independent of the current row's selected background.
        color: u32,
        /// Logical font size, subject to the ordinary canvas font-size quota.
        size: f32,
        /// Request the bold face before measuring and positioning its actual glyphs.
        bold: bool,
        /// Override the canvas family; absent inherits its resolved UI font.
        #[serde(default)]
        font: Option<String>,
        /// Physical left/center/right alignment through Start/Center/End, respectively.
        align: ui::Alignment,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_roles_override_only_declared_font_properties() {
        let mut environment = Environment {
            ui_font: FontStyle {
                family: Some("Editor UI".into()),
                size_px: Some(14.),
                bold: Some(false),
            },
            ..Default::default()
        };
        environment.theme_text_styles.insert(
            "example.body".into(),
            FontStyle {
                size_px: Some(18.),
                ..Default::default()
            },
        );
        environment
            .theme_colors
            .insert("example.body.foreground".into(), 0x123456);
        assert_eq!(
            environment.font_style("example", "body", false),
            FontStyle {
                family: Some("Editor UI".into()),
                size_px: Some(18.),
                bold: Some(false),
            }
        );
        assert_eq!(
            environment.color("example", "body.foreground"),
            Some(0x123456)
        );
        assert_eq!(environment.color("another.plugin", "body.foreground"), None);
    }
}
