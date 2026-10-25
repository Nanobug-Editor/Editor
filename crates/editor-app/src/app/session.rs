//! Persists window layout and open document state for one workspace.

use serde::{Deserialize, Serialize};
use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};

#[cfg(test)]
mod tests;

/// Version 2 distinguishes editor-local columns from historical center-tree sidebar bands.
pub(crate) const DOCK_LAYOUT_VERSION: u32 = 2;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "provider", rename_all = "snake_case")]
pub(crate) enum FileProviderChoice {
    /// Explicit native recovery leaves plugin preferences intact.
    Native,
    /// Stable package/panel identity, preserved through temporary failures and new installations.
    Plugin(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionState {
    /// A valid saved empty document session must remain empty instead of using first-open fallback.
    #[serde(skip)]
    pub(crate) document_session_loaded: bool,
    /// Startup failures must not replace damaged data or targets of an unfinished migration.
    #[serde(skip)]
    persistence_blocked: bool,
    /// A window retains its selected profile; serialized data cannot redirect future writes.
    #[serde(skip)]
    profile_directory: Option<PathBuf>,
    /// This private workspace record namespaces choices by lowercase file type.
    #[serde(default)]
    pub file_view_providers: std::collections::BTreeMap<String, FileProviderChoice>,
    pub workspace: String,
    /// Host-local authority; repository files and project plugin overrides cannot change it.
    #[serde(default = "default_true")]
    pub workspace_trusted: bool,
    pub window_width: f32,
    pub window_height: f32,
    pub explorer_width: f32,
    #[serde(default = "default_true")]
    pub explorer_visible: bool,
    /// New and older workspaces without this field hide Outline; an explicit saved choice still restores.
    #[serde(default)]
    pub outline_visible: bool,
    /// Tool-window navigation starts visible; inline tab placements retain this local choice unused.
    #[serde(default = "default_true")]
    pub open_files_visible: bool,
    /// Cursor tracking may be disabled without disabling tree updates or explicit navigation.
    #[serde(default = "default_true")]
    pub outline_follow_cursor: bool,
    /// Fresh and legacy sessions hide messages; explicit visibility restores without message history.
    #[serde(default)]
    pub messages_visible: bool,
    /// Problems lives in the native dock; older sessions keep it hidden until explicitly opened.
    #[serde(default)]
    pub problems_visible: bool,
    /// Newly introduced project roots start expanded for previously saved workspaces too.
    #[serde(default = "default_true")]
    pub explorer_root_expanded: bool,
    /// Tab switches leave the explorer untouched unless this workspace preference is enabled.
    #[serde(default)]
    pub explorer_reveal_on_tab_switch: bool,
    /// Generic plugin-manager dock dimensions persist independently of plugin-owned state.
    #[serde(default = "default_extension_height", alias = "terminal_height")]
    pub extension_height: f32,
    #[serde(default = "default_true", alias = "terminal_visible")]
    pub extensions_visible: bool,
    /// Native terminal visibility is independent from the historical plugin-manager flag.
    #[serde(default)]
    pub native_terminal_visible: bool,
    /// Native dock dimensions and panel visibility are generic host presentation state.
    #[serde(default)]
    pub plugin_dock_sizes: std::collections::BTreeMap<String, f32>,
    #[serde(default)]
    pub plugin_panel_visibility: std::collections::BTreeMap<String, bool>,
    /// Stable window-button identities in user order; absent entries append in default/install order.
    /// Keep temporarily disabled plugin slots so re-enabling does not discard the user's arrangement.
    #[serde(default)]
    pub window_button_order: Vec<String>,
    /// Legacy records default to zero; successful sidebar restoration acknowledges this migration once.
    /// Version 2 also preserves editor-local columns above the bottom dock instead of promoting them.
    #[serde(default)]
    pub(crate) plugin_sidebar_layout_version: u32,
    /// Import-only records preserve historical data until the authorized private write is acknowledged.
    /// They never select a layout or control current guest behavior.
    #[serde(
        default,
        rename = "editor_preview_modes",
        skip_serializing_if = "std::collections::BTreeMap::is_empty"
    )]
    pub legacy_preview_modes: std::collections::BTreeMap<String, serde_json::Value>,
    #[serde(
        default,
        rename = "editor_preview_sync",
        skip_serializing_if = "std::collections::BTreeMap::is_empty"
    )]
    pub legacy_preview_sync: std::collections::BTreeMap<String, bool>,
    /// Accept optional historical visibility records without assuming they existed in every version.
    #[serde(
        default,
        rename = "editor_preview_toolbar",
        alias = "editor_toolbar_visibility",
        skip_serializing_if = "std::collections::BTreeMap::is_empty"
    )]
    pub legacy_preview_toolbar: std::collections::BTreeMap<String, bool>,
    /// Base serializes the complete split tree, dock extents and open state.
    #[serde(default)]
    pub dock_layout: Option<gpui_base::dock::DockAreaState>,
    /// Stable selected panel names restore exclusive/shared regional presentation without a second layout.
    #[serde(default)]
    pub dock_active_panels: Vec<String>,
    /// Stable whole-region drop selections preserve ordinary replacements across fresh entity binding.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dock_replacement_panels: Vec<String>,
    pub open_tabs: Vec<String>,
    pub active_file: Option<String>,
    pub expanded_directories: Vec<String>,
    /// Local workspace plugin exclusions are applied before startup loading begins.
    #[serde(default)]
    pub disabled_plugins: Vec<String>,
}

impl SessionState {
    /// Bundle historical owner records as opaque data; the plugin decides whether fields are applicable.
    pub(crate) fn legacy_display_payload(&self, owner: &str) -> Option<serde_json::Value> {
        let mut data = serde_json::Map::new();
        if let Some(mode) = self.legacy_preview_modes.get(owner) {
            data.insert("mode".into(), mode.clone());
        }
        if let Some(sync) = self.legacy_preview_sync.get(owner) {
            data.insert("sync".into(), (*sync).into());
        }
        if let Some(toolbar) = self.legacy_preview_toolbar.get(owner) {
            data.insert("toolbar".into(), (*toolbar).into());
        }
        (!data.is_empty()).then_some(data.into())
    }

    /// Delete only the acknowledged source bundle; a newer local edit or another owner's data survives.
    pub(crate) fn acknowledge_display_import(
        &mut self,
        owner: &str,
        payload: &serde_json::Value,
    ) -> bool {
        if self.legacy_display_payload(owner).as_ref() != Some(payload) {
            return false;
        }
        self.legacy_preview_modes.remove(owner);
        self.legacy_preview_sync.remove(owner);
        self.legacy_preview_toolbar.remove(owner);
        true
    }

    pub fn for_workspace(workspace: &Path) -> Self {
        Self {
            document_session_loaded: false,
            persistence_blocked: false,
            profile_directory: None,
            file_view_providers: Default::default(),
            workspace: workspace.to_string_lossy().into_owned(),
            // Preserve the editor's existing trust default; users can restrict a workspace locally.
            workspace_trusted: true,
            window_width: 1280.,
            window_height: 820.,
            explorer_width: 280.,
            explorer_visible: true,
            outline_visible: false,
            open_files_visible: true,
            outline_follow_cursor: true,
            messages_visible: false,
            problems_visible: false,
            explorer_root_expanded: true,
            explorer_reveal_on_tab_switch: false,
            extension_height: default_extension_height(),
            extensions_visible: true,
            native_terminal_visible: false,
            plugin_dock_sizes: Default::default(),
            plugin_panel_visibility: Default::default(),
            window_button_order: Vec::new(),
            plugin_sidebar_layout_version: DOCK_LAYOUT_VERSION,
            legacy_preview_modes: Default::default(),
            legacy_preview_sync: Default::default(),
            legacy_preview_toolbar: Default::default(),
            dock_layout: None,
            dock_active_panels: Vec::new(),
            dock_replacement_panels: Vec::new(),
            open_tabs: Vec::new(),
            active_file: None,
            expanded_directories: Vec::new(),
            disabled_plugins: Vec::new(),
        }
    }

    /// Host-only migrations resolve the exact profile file without repeating its workspace digest.
    pub(crate) fn file_path(&self) -> Option<PathBuf> {
        let profile = self
            .profile_directory
            .clone()
            .or_else(|| {
                std::env::var_os("ME_EDITOR_PROFILE_HOME")
                    .filter(|root| !root.is_empty())
                    .map(PathBuf::from)
            })
            .or_else(|| dirs::config_dir().map(|root| root.join("MeEditor")))?;
        Some(Self::file_in_profile(Path::new(&self.workspace), &profile))
    }

    /// Resolve a workspace's existing storage identity in a host-selected profile without loading data.
    pub(crate) fn file_in_profile(workspace: &Path, profile: &Path) -> PathBuf {
        let mut hasher = DefaultHasher::new();
        workspace.to_string_lossy().hash(&mut hasher);
        profile.join(format!("{:016x}.json", hasher.finish()))
    }

    pub fn load(workspace: &Path) -> Self {
        Self::load_selected_profile(workspace, None)
    }

    /// Load the existing workspace format in an explicit profile, preserving damaged data.
    pub(crate) fn load_in_profile(workspace: &Path, profile: &Path) -> Self {
        Self::load_selected_profile(workspace, Some(profile.to_owned()))
    }

    /// Absence of explicit, matching local authority never implies consent for a new project.
    pub(crate) fn recorded_trust(workspace: &Path, profile: &Path) -> Option<bool> {
        let json: serde_json::Value =
            serde_json::from_slice(&fs::read(Self::file_in_profile(workspace, profile)).ok()?)
                .ok()?;
        (json.get("workspace")?.as_str()? == workspace.to_string_lossy())
            .then(|| {
                json.get("workspace_trusted")
                    .and_then(serde_json::Value::as_bool)
            })
            .flatten()
    }

    /// Restore storage ownership after deserialization rather than accepting it from persisted fields.
    fn load_selected_profile(workspace: &Path, profile: Option<PathBuf>) -> Self {
        let mut state = Self::for_workspace(workspace);
        state.profile_directory = profile.clone();
        if let Some(path) = state.file_path() {
            if let Ok(contents) = fs::read_to_string(&path) {
                if let Ok(saved) = serde_json::from_str::<Self>(&contents) {
                    if saved.workspace == state.workspace {
                        state = saved;
                        state.document_session_loaded = true;
                        state.profile_directory = profile.clone();
                        state.window_width = state.window_width.clamp(800., 7680.);
                        state.window_height = state.window_height.clamp(500., 4320.);
                        state.explorer_width = state.explorer_width.clamp(220., 520.);
                        state.extension_height = state.extension_height.clamp(140., 1200.);
                        // No legacy tree means the current default will be constructed. Mark it
                        // before a new user move is saved, without lowering future layout versions.
                        if state.dock_layout.is_none()
                            && state.plugin_sidebar_layout_version < DOCK_LAYOUT_VERSION
                        {
                            state.plugin_sidebar_layout_version = DOCK_LAYOUT_VERSION;
                        }
                    } else {
                        state.block_persistence();
                    }
                } else {
                    state.block_persistence();
                }
            } else if path.exists() {
                state.block_persistence();
            }
        }
        state.migrate_plugin_ids();
        state
    }

    /// Restore old exclusions, visibility choices and dock bindings using the current plugin IDs.
    fn migrate_plugin_ids(&mut self) {
        for id in &mut self.disabled_plugins {
            *id = plugin_schema::canonical_plugin_id(id).to_owned();
        }
        let visibility = std::mem::take(&mut self.plugin_panel_visibility);
        for (key, visible) in &visibility {
            if canonical_panel_key(key) == *key {
                self.plugin_panel_visibility.insert(key.clone(), *visible);
            }
        }
        for (key, visible) in visibility {
            self.plugin_panel_visibility
                .entry(canonical_panel_key(&key))
                .or_insert(visible);
        }
        if let Some(layout) = &mut self.dock_layout {
            migrate_panel_names(&mut layout.center);
            for slot in [
                &mut layout.left_dock,
                &mut layout.right_dock,
                &mut layout.bottom_dock,
            ] {
                if let Some(dock) = slot.take() {
                    let mut panel = dock.panel().clone();
                    migrate_panel_names(&mut panel);
                    *slot = Some(gpui_base::dock::DockState::new(
                        panel,
                        dock.placement(),
                        dock.size(),
                        dock.open(),
                    ));
                }
            }
        }
    }

    pub fn save(&self) {
        let _ = self.try_save();
    }

    /// Persist the logical session or return an error without replacing damaged/migrating data.
    pub(crate) fn try_save(&self) -> anyhow::Result<()> {
        if self.persistence_blocked {
            anyhow::bail!("Workspace session is protected after a failed load or migration");
        }
        use std::io::Write as _;
        let path = self
            .file_path()
            .ok_or_else(|| anyhow::anyhow!("User profile is unavailable"))?;
        let parent = path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Invalid session path"))?;
        fs::create_dir_all(parent)?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        temporary.write_all(&serde_json::to_vec_pretty(self)?)?;
        temporary.as_file().sync_all()?;
        temporary.persist(path).map_err(|error| error.error)?;
        Ok(())
    }

    /// Keep automatic layout saves away from a failed import until the next successful startup.
    pub(crate) fn block_persistence(&mut self) {
        self.persistence_blocked = true;
    }
}

/// Only the owner portion of a persisted panel key changes; the declared panel ID stays intact.
fn canonical_panel_key(key: &str) -> String {
    if let Some((plugin, panel)) = key.split_once('/') {
        format!("{}/{panel}", plugin_schema::canonical_plugin_id(plugin))
    } else {
        plugin_schema::canonical_plugin_id(key).to_owned()
    }
}

/// Rewrite stable dock names without altering split sizes, tab order or the open/closed state.
fn migrate_panel_names(panel: &mut gpui_base::dock::PanelState) {
    if let Some(key) = panel.panel_name.strip_prefix("plugin:") {
        panel.panel_name = format!("plugin:{}", canonical_panel_key(key));
    }
    for child in &mut panel.children {
        migrate_panel_names(child);
    }
}

fn default_true() -> bool {
    true
}

/// Initial plugin manager height leaves room for editing at the default window size.
fn default_extension_height() -> f32 {
    280.
}
