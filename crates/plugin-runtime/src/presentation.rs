//! Immutable native workspace projections retain their exact publisher and resource lifetime.
use crate::EditorAuthority;
use plugin_protocol::{
    api::{ErrorCode, Failure, ResourceHandle},
    presentation, tree, ui,
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

/// One cheap native snapshot; filename updates never invalidate an unrelated open action menu.
#[derive(Clone)]
pub struct WorkspacePresentationSnapshot {
    /// Advances only when entry metadata or action meaning changes.
    pub revision: u64,
    /// Advances independently for filename colors, including clears.
    pub file_revision: u64,
    /// Only this provider's relative path metadata; hosts select one provider before display.
    pub files: Arc<BTreeMap<String, presentation::FileDecoration>>,
    /// Ordered package-local entries; final user ordering belongs to the host.
    pub entries: Arc<Vec<presentation::Entry>>,
    /// Validated geometry-only package artwork, never guest-provided native paths.
    pub icons: Arc<BTreeMap<String, Arc<[u8]>>>,
    /// Queued commands retain authority only while their exact enabled action meaning survives.
    callbacks: BTreeMap<(String, Option<String>), (tree::Command, Arc<AtomicBool>)>,
}

impl Default for WorkspacePresentationSnapshot {
    /// Empty publication starts with a live callback generation, independently of the instance.
    fn default() -> Self {
        Self {
            revision: 0,
            file_revision: 0,
            files: Default::default(),
            entries: Default::default(),
            icons: Default::default(),
            callbacks: Default::default(),
        }
    }
}

/// A workspace collection grants presentation only; retained clones cannot revive retired authority.
pub struct WorkspacePresentationResource {
    handle: ResourceHandle,
    plugin: String,
    workspace: PathBuf,
    authority: EditorAuthority,
    alive: Arc<AtomicBool>,
    state: Mutex<WorkspacePresentationSnapshot>,
}

impl WorkspacePresentationResource {
    /// Issue one checked collection for `handle`'s owner and canonical `workspace`.
    pub(crate) fn new(
        handle: ResourceHandle,
        plugin: String,
        workspace: PathBuf,
        authority: EditorAuthority,
    ) -> Self {
        // Environments may spell Windows roots without the extended prefix. Resolve once on the
        // worker so native readers compare one identity without performing filesystem I/O.
        let workspace = workspace.canonicalize().unwrap_or(workspace);
        Self {
            handle,
            plugin,
            workspace,
            authority,
            alive: Arc::new(AtomicBool::new(true)),
            state: Mutex::new(Default::default()),
        }
    }
    /// Exact runtime handle; its instance and scope are never selected by native display metadata.
    pub fn handle(&self) -> &ResourceHandle {
        &self.handle
    }
    /// Stable publisher ID used only to namespace settings and ordinary command routing.
    pub fn plugin(&self) -> &str {
        &self.plugin
    }
    /// Canonical root captured at allocation; the resource cannot move to another project window.
    pub fn workspace(&self) -> &Path {
        &self.workspace
    }
    /// Revocation includes the instance and every delegated caller lifetime.
    pub fn is_live(&self) -> bool {
        self.alive.load(Ordering::Acquire) && self.authority.is_live()
    }
    /// Seal retained native snapshots before removing the runtime slot.
    pub(crate) fn revoke(&self) {
        self.alive.store(false, Ordering::Release);
    }
    /// Read immutable metadata without disk I/O or calling the guest from the UI thread.
    pub fn snapshot(&self) -> WorkspacePresentationSnapshot {
        self.state.lock().unwrap().clone()
    }
    /// Replace a complete filename collection after every item has passed admission.
    pub(crate) fn publish_files(
        &self,
        files: Vec<presentation::FileDecoration>,
    ) -> Result<(), Failure> {
        self.live()?;
        if files.len() > 8192
            || serde_json::to_vec(&files).map_or(true, |bytes| bytes.len() > 2 * 1024 * 1024 - 4096)
        {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Filename decoration quota exceeded",
            ));
        }
        let mut candidate = BTreeMap::new();
        for file in files {
            file.validate()?;
            if candidate.insert(file.path.clone(), file).is_some() {
                return Err(Failure::new(
                    ErrorCode::Conflict,
                    "Duplicate filename decoration",
                ));
            }
        }
        let mut state = self.state.lock().unwrap();
        if *state.files != candidate {
            state.files = Arc::new(candidate);
            state.file_revision = state.file_revision.saturating_add(1);
        }
        Ok(())
    }
    /// Publish a previously validated entry list and its package-owned artwork atomically.
    pub(crate) fn publish_entries(
        &self,
        entries: Vec<presentation::Entry>,
        icons: BTreeMap<String, Arc<[u8]>>,
    ) -> Result<(), Failure> {
        self.live()?;
        let mut state = self.state.lock().unwrap();
        if *state.entries != entries || *state.icons != icons {
            // Rapidly opening two unchanged lazy submenu targets is valid. Only changed, removed
            // or disabled command meanings revoke an already queued action; stale painted rows
            // are separately rejected by the native menu generation and exact command() revision.
            let previous = std::mem::take(&mut state.callbacks);
            for ((entry, action), (before, seal)) in previous {
                let current = resolve_command(&entries, &entry, action.as_deref());
                if current.as_ref().is_some_and(|after| {
                    before.command == after.command && before.arguments == after.arguments
                }) {
                    state.callbacks.insert((entry, action), (before, seal));
                } else {
                    seal.store(false, Ordering::Release);
                }
            }
            state.entries = Arc::new(entries);
            state.icons = Arc::new(icons);
            state.revision = state.revision.saturating_add(1);
        }
        Ok(())
    }
    /// Resolve an enabled captured menu item, or the entry's explicit open-refresh command.
    /// Any changed entry publication, absent item or retirement returns StaleRevision.
    pub fn command(
        &self,
        revision: u64,
        entry: &str,
        action: Option<&str>,
    ) -> Result<tree::Command, Failure> {
        self.live()?;
        let state = self.state.lock().unwrap();
        if revision != state.revision {
            return Err(stale());
        }
        resolve_command(&state.entries, entry, action).ok_or_else(stale)
    }
    /// Narrow queued commands to the captured enabled meaning and original resource lifetime.
    /// Filename or label updates retain it; changed/disabled actions, close and retirement revoke it.
    pub fn callback_authority(
        &self,
        revision: u64,
        entry: &str,
        action: Option<&str>,
    ) -> Result<EditorAuthority, Failure> {
        self.live()?;
        let mut state = self.state.lock().unwrap();
        if revision != state.revision {
            return Err(stale());
        }
        let command = resolve_command(&state.entries, entry, action).ok_or_else(stale)?;
        // Old callers may retain revoked seals. A later enabled incarnation needs a fresh seal;
        // deleting its cache key never revives the independently retained predecessor.
        state
            .callbacks
            .retain(|_, (_, seal)| seal.load(Ordering::Acquire) && Arc::strong_count(seal) > 1);
        if state.callbacks.len() >= 128 {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Too many queued workspace actions",
            ));
        }
        let (_, seal) = state
            .callbacks
            .entry((entry.into(), action.map(str::to_owned)))
            .or_insert_with(|| (command, Arc::new(AtomicBool::new(true))));
        Ok(self
            .authority
            .with_guard(seal.clone())
            .with_guard(self.alive.clone()))
    }
    /// Recheck sealed authority on every mutation and callback, including retained Arc clones.
    fn live(&self) -> Result<(), Failure> {
        if self.is_live() { Ok(()) } else { Err(stale()) }
    }
}

/// Presentation callbacks are ordinary stale input, never guest crashes.
fn stale() -> Failure {
    Failure::new(
        ErrorCode::StaleRevision,
        "Workspace presentation was replaced or retired",
    )
}

/// Native callbacks and queued-meaning seals share disabled-ancestor and entry admission.
fn resolve_command(
    entries: &[presentation::Entry],
    entry_id: &str,
    action: Option<&str>,
) -> Option<tree::Command> {
    let entry = entries
        .iter()
        .find(|entry| entry.id == entry_id && !entry.disabled);
    let entry = entry?;
    match action {
        Some(id) => ui::action_menu_command(&entry.menu, id),
        None => entry.on_open.clone().filter(|command| !command.disabled),
    }
}
