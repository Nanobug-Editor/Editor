//! Generic workspace presentation consumes checked plugin resources without knowing their domain.
use super::ExtensionPanel;
use crate::*;
use gpui_base::input::InputState;
use gpui_kit::SharedString;
use plugin_runtime::{
    WorkspacePresentationResource,
    plugin_protocol::{api, presentation, tree, ui},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
mod preferences;
pub(crate) mod settings;
mod view;
use editor_ui::menu::ActionMenu;
use preferences::Preferences;
/// Native project windows observe successful user-setting writes without polling the filesystem.
static PREFERENCES_EPOCH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// One project window retains presentation snapshots and its own popup/focus lifetime.
#[derive(Default)]
pub(crate) struct State {
    resources: Vec<Arc<WorkspacePresentationResource>>,
    stamps: Vec<(api::ResourceHandle, u64, u64)>,
    candidates: BTreeMap<String, String>,
    preferences: Preferences,
    profile: Option<PathBuf>,
    error: Option<String>,
    loaded: bool,
    preferences_epoch: u64,
    popup: Option<Entity<ActionMenu>>,
    popup_entries: Vec<Entry>,
    popup_overflow: bool,
    /// Native row identities map to an exact resource snapshot and original publisher action.
    popup_actions: BTreeMap<String, (Entry, String)>,
    serial: u64,
    color_inputs: BTreeMap<String, Entity<InputState>>,
    subscriptions: Vec<Subscription>,
}

/// A stable entry projection keeps its real resource rather than resolving an ID to a newer owner.
#[derive(Clone)]
struct Entry {
    key: String,
    resource: Arc<WorkspacePresentationResource>,
    revision: u64,
    data: presentation::Entry,
}

impl ExtensionPanel {
    /// Read native presentation under one publication lock; metadata never activates a guest.
    fn workspace_presentation_snapshot(
        &self,
    ) -> (
        Vec<Arc<WorkspacePresentationResource>>,
        BTreeMap<String, String>,
    ) {
        let state = self.worker.state.lock().unwrap();
        if !self
            .worker
            .trusted
            .load(std::sync::atomic::Ordering::Acquire)
        {
            return Default::default();
        }
        let candidates = state
            .entries
            .iter()
            .filter(|entry| {
                entry.enabled
                    && entry.error.is_none()
                    && entry.manifest.scope == api::InstanceScope::Workspace
            })
            .filter(|entry| entry.supports_capability("explorer.decorations"))
            .map(|entry| (entry.manifest.id.clone(), entry.manifest.name.clone()))
            .collect();
        (
            state
                .presentations
                .iter()
                .filter(|resource| resource.is_live())
                .cloned()
                .collect(),
            candidates,
        )
    }
}

impl EditorApp {
    /// Observe resource generations without rebuilding Explorer membership or losing its scroll state.
    pub(crate) fn sync_workspace_presentation(&mut self, cx: &mut Context<Self>) {
        let epoch = PREFERENCES_EPOCH.load(std::sync::atomic::Ordering::Acquire);
        if !self.workspace_presentation.loaded {
            self.workspace_presentation.loaded = true;
            self.workspace_presentation.profile = self
                .session_state
                .file_path()
                .and_then(|path| path.parent().map(Path::to_owned));
        }
        let preferences_changed = self.workspace_presentation.preferences_epoch != epoch;
        if preferences_changed {
            self.workspace_presentation.preferences_epoch = epoch;
            if let Some(profile) = &self.workspace_presentation.profile {
                match Preferences::load(profile) {
                    Ok(preferences) => self.workspace_presentation.preferences = preferences,
                    Err(error) => self.workspace_presentation.error = Some(error.to_string()),
                }
            }
        }
        let (mut resources, candidates) =
            self.extensions.read(cx).workspace_presentation_snapshot();
        resources.retain(|resource| resource.workspace() == self.workspace.root());
        resources.sort_by(|a, b| a.plugin().cmp(b.plugin()));
        let stamps = resources
            .iter()
            .map(|resource| {
                let s = resource.snapshot();
                (resource.handle().clone(), s.revision, s.file_revision)
            })
            .collect::<Vec<_>>();
        let changed = preferences_changed
            || stamps != self.workspace_presentation.stamps
            || candidates != self.workspace_presentation.candidates;
        self.workspace_presentation.resources = resources;
        self.workspace_presentation.candidates = candidates;
        self.workspace_presentation.stamps = stamps;
        let workspace = self.workspace.root().to_string_lossy().to_string();
        if self.workspace_presentation.error.is_none()
            && !self
                .workspace_presentation
                .preferences
                .providers
                .contains_key(&workspace)
            && self.workspace_presentation.candidates.len() == 1
        {
            let provider = self
                .workspace_presentation
                .candidates
                .keys()
                .next()
                .unwrap()
                .clone();
            self.update_presentation_preferences(
                move |preferences| {
                    preferences.providers.insert(workspace, provider);
                },
                cx,
            );
        }
        if changed {
            self.explorer_panel.update(cx, |_, cx| cx.notify());
            cx.notify();
        }
    }
    /// Decorate existing files only. A selected provider's missing path never borrows another source.
    pub(crate) fn explorer_file_decoration(
        &self,
        path: &Path,
        directory: bool,
        cx: &App,
    ) -> Option<(gpui_kit::Hsla, String)> {
        let state = &self.workspace_presentation;
        if directory || !state.preferences.enabled || !self.session_state.workspace_trusted {
            return None;
        }
        let provider = state
            .preferences
            .providers
            .get(self.workspace.root().to_string_lossy().as_ref())?;
        let resource = state
            .resources
            .iter()
            .find(|resource| resource.plugin() == provider && resource.is_live())?;
        let relative = path
            .strip_prefix(resource.workspace())
            .ok()?
            .to_str()?
            .replace('\\', "/");
        let snapshot = resource.snapshot();
        let decoration = snapshot.files.get(&relative)?;
        Some((
            state.preferences.color(decoration.tone, cx),
            decoration
                .tooltip
                .for_locale(rust_i18n::locale().as_ref())
                .to_owned(),
        ))
    }
    /// Merge current settings from disk before an atomic write so peer project windows keep changes.
    fn update_presentation_preferences(
        &mut self,
        update: impl FnOnce(&mut Preferences),
        cx: &mut Context<Self>,
    ) {
        let result = (|| {
            let profile = self
                .workspace_presentation
                .profile
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Missing presentation profile"))?;
            let mut preferences = Preferences::load(profile)?;
            update(&mut preferences);
            preferences.save(profile)?;
            Ok::<_, anyhow::Error>(preferences)
        })();
        match result {
            Ok(preferences) => {
                self.workspace_presentation.preferences = preferences;
                self.workspace_presentation.error = None;
                self.workspace_presentation.preferences_epoch =
                    PREFERENCES_EPOCH.fetch_add(1, std::sync::atomic::Ordering::AcqRel) + 1;
            }
            Err(error) => self.workspace_presentation.error = Some(error.to_string()),
        }
        self.explorer_panel.update(cx, |_, cx| cx.notify());
        self.refresh_dialog(cx);
        cx.notify();
    }
    /// Sort by user-entry order first, then stable package defaults; hidden entries keep preferences.
    fn presentation_entries(&self, include_hidden: bool) -> Vec<Entry> {
        if !self.session_state.workspace_trusted {
            return Vec::new();
        }
        let preferences = &self.workspace_presentation.preferences;
        let mut entries = self
            .workspace_presentation
            .resources
            .iter()
            .filter(|resource| resource.is_live())
            .flat_map(|resource| {
                let snapshot = resource.snapshot();
                snapshot
                    .entries
                    .iter()
                    .cloned()
                    .map(move |data| Entry {
                        key: format!("{}/{}", resource.plugin(), data.id),
                        resource: resource.clone(),
                        revision: snapshot.revision,
                        data,
                    })
                    .collect::<Vec<_>>()
            })
            .filter(|entry| include_hidden || !preferences.hidden.contains(&entry.key))
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| {
            (
                preferences
                    .order
                    .iter()
                    .position(|id| id == &entry.key)
                    .unwrap_or(usize::MAX),
                entry.data.order,
                entry.key.clone(),
            )
        });
        entries
    }
}
