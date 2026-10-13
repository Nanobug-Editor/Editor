//! Application user resources and project contribution views own immutable caches independently.
use super::*;
use std::sync::{Mutex, Weak};
/// Native project views retain their effective declarations; the routing table holds no live owner.
pub(crate) struct CatalogView {
    _project: Arc<RwLock<Arc<Catalog>>>,
    _user: Arc<RwLock<Arc<Catalog>>>,
}
static PROJECT_CATALOGS: LazyLock<Mutex<BTreeMap<u64, Weak<RwLock<Arc<Catalog>>>>>> =
    LazyLock::new(Default::default);
/// Themes are user resources from globally enabled packages, independent of project trust and focus.
static USER_CATALOGS: LazyLock<Mutex<BTreeMap<PathBuf, Weak<RwLock<Arc<Catalog>>>>>> =
    LazyLock::new(Default::default);
/// Only the GPUI application owns user resource catalogs; test/application shutdown releases the cache.
#[derive(Default)]
struct UserCatalogs(BTreeMap<PathBuf, Arc<RwLock<Arc<Catalog>>>>);
impl gpui_kit::Global for UserCatalogs {}
impl CatalogView {
    /// Construct before publishing project entries so immutable icons/assets survive another window's refresh.
    pub(crate) fn new(
        providers: &crate::language::providers::Providers,
        cx: &mut gpui_kit::App,
    ) -> Self {
        if !cx.has_global::<UserCatalogs>() {
            cx.set_global(UserCatalogs::default());
        }
        let root = providers.store_root();
        let user = cx
            .global_mut::<UserCatalogs>()
            .0
            .entry(root.clone())
            .or_default()
            .clone();
        USER_CATALOGS
            .lock()
            .unwrap()
            .insert(root, Arc::downgrade(&user));
        let state = Arc::new(RwLock::new(Arc::new(Catalog::default())));
        let mut catalogs = PROJECT_CATALOGS.lock().unwrap();
        catalogs.retain(|_, state| state.strong_count() != 0);
        catalogs.insert(providers.scope(), Arc::downgrade(&state));
        Self {
            _project: state,
            _user: user,
        }
    }
    /// Additional panels of one project borrow the same declaration snapshot.
    pub(crate) fn for_project(providers: &crate::language::providers::Providers) -> Self {
        Self {
            _project: PROJECT_CATALOGS
                .lock()
                .unwrap()
                .get(&providers.scope())
                .and_then(Weak::upgrade)
                .expect("project contribution owner"),
            _user: USER_CATALOGS
                .lock()
                .unwrap()
                .get(&providers.store_root())
                .and_then(Weak::upgrade)
                .expect("user contribution owner"),
        }
    }
}
pub(super) fn project_catalog(providers: &crate::language::providers::Providers) -> Arc<Catalog> {
    PROJECT_CATALOGS
        .lock()
        .unwrap()
        .get(&providers.scope())
        .and_then(Weak::upgrade)
        .map(|state| state.read().unwrap().clone())
        .unwrap_or_default()
}

/// Publish one project snapshot and optional user resources without replacing another project.
pub(super) fn publish(
    providers: &crate::language::providers::Providers,
    catalog: Catalog,
    user_catalog: Option<Catalog>,
) {
    if let Some(catalog) = user_catalog {
        if let Some(state) = USER_CATALOGS
            .lock()
            .unwrap()
            .get(&providers.store_root())
            .and_then(Weak::upgrade)
        {
            *state.write().unwrap() = Arc::new(catalog);
        }
    }
    let catalog = Arc::new(catalog);
    if let Some(state) = PROJECT_CATALOGS
        .lock()
        .unwrap()
        .get(&providers.scope())
        .and_then(Weak::upgrade)
    {
        *state.write().unwrap() = catalog.clone();
    }
    #[cfg(test)]
    {
        *CATALOG.write().unwrap() = catalog;
    }
}
/// Installed theme packages override the built-in safety palette for their declared mode.
pub fn theme(dark: bool) -> Option<ThemeDefinition> {
    let mode = if dark {
        ThemeMode::Dark
    } else {
        ThemeMode::Light
    };
    let catalogs = USER_CATALOGS
        .lock()
        .unwrap()
        .values()
        .filter_map(Weak::upgrade)
        .map(|state| state.read().unwrap().clone())
        .collect::<Vec<_>>();
    #[cfg(test)]
    let fixture = CATALOG.read().unwrap().clone();
    catalogs
        .iter()
        .flat_map(|catalog| catalog.plugins.values())
        .filter_map(|entry| entry.theme.as_ref())
        .flat_map(|file| &file.themes)
        .find(|theme| theme.mode == mode)
        .cloned()
        .or_else(|| {
            #[cfg(test)]
            {
                return fixture
                    .plugins
                    .values()
                    .filter_map(|entry| entry.theme.as_ref())
                    .flat_map(|file| &file.themes)
                    .find(|theme| theme.mode == mode)
                    .cloned();
            }
            #[cfg(not(test))]
            {
                None
            }
        })
}

/// Theme icon rules precede language rules, preserving the existing file-tree priority.
pub fn project_icon_rules(
    providers: &crate::language::providers::Providers,
) -> Arc<Vec<(String, FileIconConfig, bool)>> {
    let catalog = project_catalog(providers);
    catalog.icon_rules.clone()
}

#[cfg(test)]
pub fn icon_rules() -> Arc<Vec<(String, FileIconConfig, bool)>> {
    CATALOG.read().unwrap().icon_rules.clone()
}

pub(super) fn asset_prefix(id: &str, digest: &str) -> String {
    format!("plugins/{id}/{digest}")
}

/// Resolve a renderer asset only from an enabled installed plugin version.
pub fn asset(path: &str) -> Option<Vec<u8>> {
    let mut catalogs = PROJECT_CATALOGS
        .lock()
        .unwrap()
        .values()
        .filter_map(Weak::upgrade)
        .map(|state| state.read().unwrap().clone())
        .collect::<Vec<_>>();
    catalogs.extend(
        USER_CATALOGS
            .lock()
            .unwrap()
            .values()
            .filter_map(Weak::upgrade)
            .map(|state| state.read().unwrap().clone()),
    );
    #[cfg(test)]
    catalogs.push(CATALOG.read().unwrap().clone());
    catalogs
        .iter()
        .flat_map(|catalog| &catalog.plugins)
        .find_map(|(id, entry)| {
            path.strip_prefix(&format!("{}/", asset_prefix(id, &entry.digest)))
                .and_then(|relative| entry.assets.get(relative).cloned())
        })
}
