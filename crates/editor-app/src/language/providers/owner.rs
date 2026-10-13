//! Project projections share one preference writer but never share active provider selections.
use super::*;
use std::{
    ops::{Deref, DerefMut},
    sync::{Arc, Mutex, MutexGuard, RwLockReadGuard, RwLockWriteGuard, Weak, atomic::AtomicBool},
};

/// A store has one authoritative settings value; each project's registry is a derived projection.
struct Preferences {
    saved: Saved,
    error: Option<String>,
}
static STORES: LazyLock<Mutex<BTreeMap<PathBuf, Weak<Mutex<Preferences>>>>> =
    LazyLock::new(|| Mutex::new(BTreeMap::new()));
static NEXT_SCOPE: AtomicU64 = AtomicU64::new(1);
/// Normalize a future host store without creating it; canonical parent keys stay stable after its first real write.
fn store_key(root: &Path) -> PathBuf {
    for parent in root.ancestors() {
        if let Ok(existing) = parent.canonicalize() {
            return existing.join(root.strip_prefix(parent).unwrap());
        }
    }
    root.into()
}
/// Weak routing metadata lets immutable renderer jobs capture an explicit project owner at construction.
static PROJECTS: LazyLock<
    Mutex<
        BTreeMap<
            String,
            (
                Weak<RwLock<Registry>>,
                Weak<Mutex<Preferences>>,
                Weak<AtomicBool>,
                u64,
            ),
        >,
    >,
> = LazyLock::new(|| Mutex::new(BTreeMap::new()));

/// Explicit project ownership travels into document actions and background grammar jobs.
/// Clones refer to the same project incarnation, while reopening obtains a new identity.
#[derive(Clone)]
pub(crate) struct Providers {
    pub(super) state: Arc<RwLock<Registry>>,
    preferences: Arc<Mutex<Preferences>>,
    scope: u64,
    alive: Arc<AtomicBool>,
}
impl std::fmt::Debug for Providers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Providers").field(&self.scope).finish()
    }
}
impl PartialEq for Providers {
    fn eq(&self, other: &Self) -> bool {
        self.scope == other.scope
    }
}
impl Eq for Providers {}

impl Providers {
    /// Construct an independent project view; reopening reloads host preferences through the same sole writer.
    pub(crate) fn new(root: &Path, workspace: &Path) -> Self {
        let root = store_key(root);
        let mut stores = STORES.lock().unwrap();
        stores.retain(|_, owner| owner.strong_count() != 0);
        let preferences = stores
            .get(&root)
            .and_then(Weak::upgrade)
            .unwrap_or_else(|| {
                let (saved, error) = match Saved::read(&root) {
                    Ok(saved) => (saved, None),
                    Err(error) => (Saved::default(), Some(format!("{error:#}"))),
                };
                let owner = Arc::new(Mutex::new(Preferences { saved, error }));
                stores.insert(root.clone(), Arc::downgrade(&owner));
                owner
            });
        drop(stores);
        let mut snapshot = preferences.lock().unwrap();
        // Startup/reopen is an explicit file-read boundary. A hand-edited file cannot be hidden by an older live projection.
        match Saved::read(&root) {
            Ok(saved) => {
                snapshot.saved = saved;
                snapshot.error = None;
            }
            Err(error) => snapshot.error = Some(format!("{error:#}")),
        }
        let registry = Registry {
            root,
            workspace: workspace
                .canonicalize()
                .unwrap_or_else(|_| workspace.into())
                .display()
                .to_string(),
            saved: snapshot.saved.clone(),
            error: snapshot.error.clone(),
            highlight_epoch: next_highlight_epoch(),
            ..Default::default()
        };
        drop(snapshot);
        let owner = Self {
            state: Arc::new(RwLock::new(registry)),
            preferences,
            scope: NEXT_SCOPE.fetch_add(1, Ordering::AcqRel),
            alive: Arc::new(AtomicBool::new(true)),
        };
        let key = owner.state.read().unwrap().workspace.clone();
        PROJECTS.lock().unwrap().insert(
            key,
            (
                Arc::downgrade(&owner.state),
                Arc::downgrade(&owner.preferences),
                Arc::downgrade(&owner.alive),
                owner.scope,
            ),
        );
        #[cfg(test)]
        {
            *LEGACY.write().unwrap() = Some(owner.clone());
        }
        owner
    }

    /// Process-local incarnation keys keep independently reopened projects out of parser caches and scenes.
    pub(crate) fn scope(&self) -> u64 {
        self.scope
    }
    /// Shared resource catalogs use the canonical host store, never a project-supplied location.
    pub(crate) fn store_root(&self) -> PathBuf {
        self.read().root.clone()
    }

    /// Closing a project revokes delayed grammar jobs and releases its qualified parser factories.
    pub(crate) fn retire(&self) {
        self.alive.store(false, Ordering::Release);
        let mut registry = self.write();
        let languages = registry.highlighters.keys().cloned().collect::<Vec<_>>();
        registry.highlight_epoch = next_highlight_epoch();
        registry.highlighters.clear();
        registry.selected.clear();
        let epoch = registry.highlight_epoch;
        drop(registry);
        for language in languages {
            crate::language::plugins::mask_language(&self.parser_key(&language));
        }
        crate::language::code_highlighting::invalidate_prepared(self.scope, epoch);
    }
    /// A retired project cannot regain grammar authority through a preference refresh.
    pub(super) fn is_live(&self) -> bool {
        self.alive.load(Ordering::Acquire)
    }

    /// Renderer construction resolves once, then retains this incarnation rather than consulting focus.
    pub(crate) fn for_workspace(workspace: &Path) -> Option<Self> {
        let key = workspace
            .canonicalize()
            .unwrap_or_else(|_| workspace.into())
            .display()
            .to_string();
        let mut projects = PROJECTS.lock().unwrap();
        projects.retain(|_, (state, _, alive, _)| {
            state.strong_count() != 0
                && alive
                    .upgrade()
                    .is_some_and(|alive| alive.load(Ordering::Acquire))
        });
        let (state, preferences, alive, scope) = projects.get(&key)?;
        Some(Self {
            state: state.upgrade()?,
            preferences: preferences.upgrade()?,
            alive: alive.upgrade()?,
            scope: *scope,
        })
    }

    /// Upstream's global parser cache uses project-qualified keys; logical document language stays unchanged.
    pub(crate) fn parser_key(&self, language: &str) -> String {
        format!("nanobug:{}:{language}", self.scope)
    }

    /// Synchronize derived settings on reads, so an inactive window observes user edits on its next action.
    pub(super) fn read(&self) -> RwLockReadGuard<'_, Registry> {
        drop(self.write());
        self.state.read().unwrap()
    }

    /// Keep the sole preference writer locked throughout validation and persistence of a UI mutation.
    pub(super) fn write(&self) -> Projection<'_> {
        let preferences = self.preferences.lock().unwrap();
        let mut registry = self.state.write().unwrap();
        registry.error = preferences.error.clone();
        if registry.saved != preferences.saved {
            let selected = registry.selected.clone();
            let associations = registry.saved.associations.clone();
            registry.saved = preferences.saved.clone();
            registry.resolve();
            if registry.selected != selected || registry.saved.associations != associations {
                registry.highlight_epoch = next_highlight_epoch();
            }
        }
        Projection {
            registry,
            preferences,
        }
    }
}

/// Holding both guards prevents a stale window snapshot from replacing another project's changes.
pub(super) struct Projection<'a> {
    registry: RwLockWriteGuard<'a, Registry>,
    preferences: MutexGuard<'a, Preferences>,
}
impl Projection<'_> {
    /// Explicit mutations persist their fully resolved snapshot once, then publish both authority and projection.
    pub(super) fn commit(&mut self, next: Registry) -> anyhow::Result<()> {
        if next.saved != self.preferences.saved {
            next.saved.write(&next.root)?;
        }
        self.preferences.saved = next.saved.clone();
        *self.registry = next;
        Ok(())
    }
}
impl Deref for Projection<'_> {
    type Target = Registry;
    fn deref(&self) -> &Registry {
        &self.registry
    }
}
impl DerefMut for Projection<'_> {
    fn deref_mut(&mut self) -> &mut Registry {
        &mut self.registry
    }
}
impl Drop for Projection<'_> {
    fn drop(&mut self) {
        if self.preferences.saved != self.registry.saved && self.registry.error.is_none() {
            // Automatic adoption and formatter proofs are also durable preferences, never copied from another scope.
            match self.registry.saved.write(&self.registry.root) {
                Ok(()) => self.preferences.saved = self.registry.saved.clone(),
                Err(error) => {
                    let error = Some(format!("{error:#}"));
                    self.preferences.error = error.clone();
                    self.registry.error = error;
                }
            }
        }
    }
}

// Preserve the existing single-project fixture interface; production call sites require an explicit owner.
#[cfg(test)]
static LEGACY: LazyLock<RwLock<Option<Providers>>> = LazyLock::new(|| RwLock::new(None));
#[cfg(test)]
pub(crate) fn legacy() -> Providers {
    LEGACY
        .read()
        .unwrap()
        .clone()
        .expect("provider fixture must configure a workspace")
}
#[cfg(test)]
pub(super) fn configure_fixture(root: &Path, workspace: &Path) {
    let current = LEGACY.read().unwrap().clone();
    let key = workspace
        .canonicalize()
        .unwrap_or_else(|_| workspace.into())
        .display()
        .to_string();
    if current.as_ref().is_none_or(|owner| {
        let state = owner.read();
        !same_root(&state.root, root) || state.workspace != key
    }) {
        Providers::new(root, workspace);
    }
}
