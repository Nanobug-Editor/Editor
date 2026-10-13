//! Loads installed declarative resources through the same registry as WASM plugins.

use plugin_runtime::{Installed, Manager};
use plugin_schema::{
    DiscoveredTarget, FileIconConfig, PluginManifest, ProviderFailure, RunTargetDiscovery,
    ThemeDefinition, ThemeFile, ThemeMode,
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, LazyLock, RwLock},
};

mod owner;
#[cfg(test)]
pub(crate) use owner::icon_rules;
pub(crate) use owner::{CatalogView, asset, project_icon_rules, theme};
use owner::{asset_prefix, project_catalog};

/// A snapshot lets render and document paths read plugin metadata without disk I/O.
#[derive(Default)]
struct Catalog {
    plugins: BTreeMap<String, Contribution>,
    icon_rules: Arc<Vec<(String, FileIconConfig, bool)>>,
}

#[derive(Clone)]
struct Contribution {
    root: PathBuf,
    digest: String,
    manifest: PluginManifest,
    icons: Option<FileIconConfig>,
    theme_icons: Option<FileIconConfig>,
    theme: Option<ThemeFile>,
    /// Discovery declarations this plugin contributes, already validated.
    run_targets: Vec<Arc<RunTargetDiscovery>>,
    assets: BTreeMap<String, Vec<u8>>,
}

#[cfg(test)]
static CATALOG: LazyLock<RwLock<Arc<Catalog>>> =
    LazyLock::new(|| RwLock::new(Arc::new(Catalog::default())));

/// Rebuild from enabled packages after a lifecycle change; failed assets stay unavailable.
#[cfg(test)]
pub fn refresh(root: &Path) -> anyhow::Result<()> {
    crate::language::providers::configure(root, Path::new(""));
    let installed = Manager::read_registry(root)?
        .into_values()
        .collect::<Vec<_>>();
    refresh_entries(root, &installed);
    Ok(())
}

/// Restore contributions enabled only for the active workspace before tabs load.
pub fn refresh_for_project(
    providers: &crate::language::providers::Providers,
    root: &Path,
    workspace: &Path,
) -> anyhow::Result<()> {
    let key = workspace.display().to_string();
    let installed = Manager::read_registry(root)?
        .into_values()
        .map(|mut entry| {
            entry.global_enabled = Some(entry.enabled);
            entry.enabled |= entry.project_enabled_in(&key);
            entry
        })
        .collect::<Vec<_>>();
    refresh_project_entries(providers, root, &installed, true);
    Ok(())
}

/// Project errors mask executable/effective declarations; immutable user resources follow global installation authority.
pub fn refresh_project_entries(
    providers: &crate::language::providers::Providers,
    root: &Path,
    installed: &[Installed],
    publish_user_resources: bool,
) {
    let mut catalog = Catalog::default();
    let mut user_catalog = publish_user_resources.then(Catalog::default);
    for installed in installed {
        let id = &installed.manifest.id;
        let project_enabled = installed.enabled && installed.error.is_none();
        let user_enabled =
            publish_user_resources && installed.global_enabled.unwrap_or(installed.enabled);
        if (!project_enabled && !user_enabled)
            || installed.compatibility_error().is_some()
            || !id.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'.' || byte == b'-'
            })
            || installed.digest.len() != 64
            || !installed
                .digest
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            continue;
        }
        let Some(path) = &installed.manifest.contributions else {
            continue;
        };
        let version = root.join("packages").join(&id).join(&installed.digest);
        match Contribution::read(&version, path, id, &installed.digest) {
            Ok(contribution) => {
                if user_enabled {
                    // A failed project provider cannot remove a validated user's theme from healthy peers.
                    user_catalog
                        .as_mut()
                        .unwrap()
                        .plugins
                        .insert(id.clone(), contribution.clone());
                }
                if project_enabled {
                    catalog.plugins.insert(id.clone(), contribution);
                }
            }
            Err(error) => {
                tracing::warn!(plugin = %id, %error, "declarative plugin assets unavailable")
            }
        }
    }
    // Keep icon mappings immutable so drawing each explorer row only clones an Arc.
    let mut rules = Vec::new();
    for (id, entry) in &catalog.plugins {
        if let Some(config) = &entry.theme_icons {
            rules.push((asset_prefix(id, &entry.digest), config.clone(), true));
        }
    }
    for (id, entry) in &catalog.plugins {
        if let Some(config) = &entry.icons {
            rules.push((asset_prefix(id, &entry.digest), config.clone(), false));
        }
    }
    catalog.icon_rules = Arc::new(rules);
    providers.refresh_with_language_features(
        root,
        catalog
            .plugins
            .iter()
            .map(|(id, entry)| {
                (
                    id.clone(),
                    entry.root.clone(),
                    entry.manifest.language_definitions.clone(),
                    entry.manifest.highlighters.clone(),
                )
            })
            .collect(),
        installed
            .iter()
            .filter(|entry| {
                entry.enabled && entry.error.is_none() && entry.compatibility_error().is_none()
            })
            .map(|entry| {
                (
                    entry.manifest.id.clone(),
                    entry.manifest.language_servers.clone(),
                )
            })
            .collect(),
        installed
            .iter()
            .filter(|entry| {
                entry.enabled && entry.error.is_none() && entry.compatibility_error().is_none()
            })
            .map(|entry| {
                (
                    entry.manifest.id.clone(),
                    entry.manifest.structure_providers.clone(),
                )
            })
            .collect(),
        // An omitted optional contract contributes no direct role. Publishing its declaration
        // without an admitted executor would shadow a valid selected standard LSP provider.
        installed
            .iter()
            .filter(|entry| {
                entry.enabled
                    && entry.error.is_none()
                    && entry.compatibility_error().is_none()
                    && entry.supports_capability("language.direct")
            })
            .map(|entry| {
                (
                    entry.manifest.id.clone(),
                    entry.manifest.direct_language_providers.clone(),
                )
            })
            .collect(),
    );
    owner::publish(providers, catalog, user_catalog);
}

/// Existing single-project package fixtures use the same project projection as their native editor.
#[cfg(test)]
pub fn refresh_for_workspace(root: &Path, workspace: &Path) -> anyhow::Result<()> {
    crate::language::providers::configure(root, workspace);
    refresh_for_project(&crate::language::providers::legacy(), root, workspace)
}
#[cfg(test)]
pub fn refresh_entries(root: &Path, installed: &[Installed]) {
    refresh_project_entries(&crate::language::providers::legacy(), root, installed, true);
}

impl Contribution {
    /// Only canonical paths inside the immutable package version may be read.
    fn read(root: &Path, manifest_path: &str, id: &str, digest: &str) -> anyhow::Result<Self> {
        let root = root.canonicalize()?;
        let source = read_asset(&root, Path::new(manifest_path))?;
        let manifest = PluginManifest::parse(std::str::from_utf8(&source)?)?;
        // Historical identity import belongs to the registry migration; current packages keep exact author IDs.
        anyhow::ensure!(manifest.plugin.id == id, "Contribution identity mismatch");
        let icons = manifest
            .file_icons
            .as_ref()
            .map(|path| read_icons(&root, path))
            .transpose()?;
        let theme_icons = manifest
            .theme
            .as_ref()
            .and_then(|theme| theme.file_icons.as_ref())
            .map(|path| read_icons(&root, path))
            .transpose()?;
        let theme = manifest
            .theme
            .as_ref()
            .map(|theme| -> anyhow::Result<_> {
                Ok(ThemeFile::parse(std::str::from_utf8(&read_asset(
                    &root,
                    &theme.file,
                )?)?)?)
            })
            .transpose()?;
        let (run_targets, failures) = read_run_targets(&root, &manifest.run_targets);
        for failure in &failures {
            tracing::warn!(
                plugin = %id,
                provider = %failure.provider,
                error = %failure.error,
                "run target discovery unavailable"
            );
        }
        let mut assets = BTreeMap::new();
        for config in [&icons, &theme_icons].into_iter().flatten() {
            for icon in &config.icons {
                for path in [&icon.light, &icon.dark] {
                    let bytes = read_asset(&root, path)?;
                    let svg = std::str::from_utf8(&bytes)?;
                    let lower = svg.to_ascii_lowercase();
                    anyhow::ensure!(
                        bytes.len() <= 64 * 1024
                            && svg.trim_start().starts_with("<svg")
                            && !lower.contains("<!doctype")
                            && !lower.contains("<script")
                            && !lower.contains("href="),
                        "Invalid SVG asset"
                    );
                    assets.insert(path.to_string_lossy().replace('\\', "/"), bytes);
                }
            }
        }
        Ok(Self {
            root,
            digest: digest.to_owned(),
            manifest,
            icons,
            theme_icons,
            theme,
            run_targets,
            assets,
        })
    }
}

/// Whether a declaration may stand for the contribution that announced it.
///
/// The declaration's own identity is what a saved configuration names, so it has to be the identity
/// the contribution file announced; otherwise the two could disagree silently and a configuration
/// would point at a provider nobody installed.
pub fn declaration_matches_contribution(
    contribution: &str,
    declaration: &str,
) -> Result<(), String> {
    if contribution == declaration {
        return Ok(());
    }
    Err(format!(
        "declaration names provider {declaration} but the contribution names {contribution}"
    ))
}

/// Read every discovery declaration a plugin contributes.
///
/// Each declaration is validated as it is read, so a plugin that ships an unusable one is reported
/// with the file that failed instead of taking the rest of its contributions down with it.
fn read_run_targets(
    root: &Path,
    contributions: &[plugin_schema::RunTargetContribution],
) -> (Vec<Arc<RunTargetDiscovery>>, Vec<ProviderFailure>) {
    let mut providers = Vec::new();
    let mut failures = Vec::new();
    // The package root is resolved once, so a declaration may only name files inside it.
    let resolved = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    for contribution in contributions {
        let provider = contribution.id.clone();
        let read = || -> anyhow::Result<RunTargetDiscovery> {
            let bytes = read_asset(&resolved, &contribution.file)?;
            Ok(RunTargetDiscovery::from_json(&bytes)?)
        };
        match read() {
            Ok(discovery) => match declaration_matches_contribution(&provider, &discovery.id) {
                Ok(()) => providers.push(Arc::new(discovery)),
                Err(error) => failures.push(ProviderFailure { provider, error }),
            },
            Err(error) => failures.push(ProviderFailure {
                provider,
                error: format!("{error:#}"),
            }),
        }
    }
    (providers, failures)
}

fn read_icons(root: &Path, path: &Path) -> anyhow::Result<FileIconConfig> {
    Ok(FileIconConfig::parse(std::str::from_utf8(&read_asset(
        root, path,
    )?)?)?)
}

fn read_asset(root: &Path, path: &Path) -> anyhow::Result<Vec<u8>> {
    anyhow::ensure!(
        path.components()
            .all(|part| matches!(part, std::path::Component::Normal(_))),
        "Unsafe contribution asset"
    );
    let resolved = root.join(path).canonicalize()?;
    anyhow::ensure!(
        resolved.starts_with(root) && resolved.is_file(),
        "Contribution asset escapes package"
    );
    Ok(std::fs::read(resolved)?)
}

/// Verify packaged selector resources independently from UI rendering.
#[cfg(test)]
fn language_for_path_in_catalog(
    catalog: &Catalog,
    path: &Path,
) -> Option<(String, plugin_schema::LanguageDefinition)> {
    let filename = path.file_name()?.to_str()?;
    for (id, contribution) in &catalog.plugins {
        if let Some(language) = contribution
            .manifest
            .language_definitions
            .iter()
            .find(|language| {
                language
                    .filenames
                    .iter()
                    .any(|candidate| candidate.eq_ignore_ascii_case(filename))
            })
        {
            return Some((id.clone(), language.clone()));
        }
    }

    let extension = path.extension()?.to_str()?;
    for (id, contribution) in &catalog.plugins {
        if let Some(language) = contribution
            .manifest
            .language_definitions
            .iter()
            .find(|language| {
                language
                    .extensions
                    .iter()
                    .any(|candidate| candidate.eq_ignore_ascii_case(extension))
            })
        {
            return Some((id.clone(), language.clone()));
        }
    }
    None
}

/// Run every enabled plugin's discovery declarations against one workspace.
///
/// Each declaration is applied independently, so a provider that offers nothing — or offers
/// something unusable — cannot stop another provider from offering its own targets. The candidates
/// are gathered here and handed on; nothing in this module decides what a candidate means.
pub fn discover_run_targets(workspace: &Path) -> Result<Vec<DiscoveredTarget>, String> {
    let catalog = crate::language::providers::Providers::for_workspace(workspace)
        .map(|providers| project_catalog(&providers))
        .unwrap_or_default();
    #[cfg(test)]
    let catalog = if catalog.plugins.is_empty() {
        CATALOG.read().unwrap().clone()
    } else {
        catalog
    };
    let providers = catalog
        .plugins
        .iter()
        .flat_map(|(owner, contribution)| {
            contribution
                .run_targets
                .iter()
                .map(move |provider| (owner.clone(), provider.clone()))
        })
        .collect::<Vec<_>>();
    if providers.is_empty() {
        return Ok(Vec::new());
    }
    let root = workspace
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let files = workspace_files(&root)?;
    let mut targets = Vec::new();
    for (owner, provider) in providers {
        let candidates = provider.discover(&files, |path| {
            let resolved = root.join(path).canonicalize().ok()?;
            // Recheck each selected input against the canonical root, including changed symlinks.
            if !resolved.starts_with(&root)
                || !resolved.is_file()
                || std::fs::metadata(&resolved).ok()?.len() > 256 * 1024
            {
                return None;
            }
            std::fs::read_to_string(resolved).ok()
        });
        for mut target in candidates {
            target.id = format!(
                "{}:{}:{}:{}:{}",
                owner.len(),
                owner,
                target.found_in.len(),
                target.found_in,
                target.id
            );
            target.provider = owner.clone();
            targets.push(target);
            if targets.len() > 128 {
                return Err("Discovery exceeds 128 candidates".into());
            }
        }
    }
    Ok(targets)
}

/// Canonical paths and a visited-directory set bound links, cycles and empty-directory floods.
fn workspace_files(workspace: &Path) -> Result<Vec<String>, String> {
    let root = workspace
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let mut files = std::collections::BTreeSet::new();
    let mut visited = std::collections::BTreeSet::new();
    let mut pending = vec![root.clone()];
    let mut scanned = 0usize;
    while let Some(directory) = pending.pop() {
        if !directory.starts_with(&root) || !visited.insert(directory.clone()) {
            continue;
        }
        if visited.len() > 20000 {
            return Err("Discovery directory limit exceeded".into());
        }
        let entries = std::fs::read_dir(directory).map_err(|error| error.to_string())?;
        for entry in entries {
            let entry = entry.map_err(|error| error.to_string())?;
            scanned += 1;
            if scanned > 60000 {
                return Err("Discovery entry limit exceeded".into());
            }
            if matches!(
                entry.file_name().to_str(),
                Some(".git" | "target" | "node_modules" | "vendor")
            ) {
                continue;
            }
            let Ok(path) = entry.path().canonicalize() else {
                continue;
            };
            if !path.starts_with(&root) {
                continue;
            }
            if path.is_dir() {
                pending.push(path);
            } else if path.is_file() {
                let relative = path
                    .strip_prefix(&root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                files.insert(relative);
                if files.len() > 20000 {
                    return Err("Discovery file limit exceeded".into());
                }
            }
        }
    }
    Ok(files.into_iter().collect())
}

/// Publish one plugin's declarative contributions the way the registry does.
///
/// Exists for the native acceptance, which must exercise the shipped package's own declaration
/// without installing a whole registry into a temporary runtime.
#[cfg(test)]
pub fn publish_declarative_plugin_for_test(root: &Path, id: &str) {
    let contribution = Contribution::read(root, "plugin.toml", id, &"c".repeat(64))
        .unwrap_or_else(|error| panic!("the plugin's contributions read: {error:#}"));
    let mut catalog = Catalog::default();
    catalog.plugins.insert(id.to_owned(), contribution);
    *CATALOG.write().unwrap() = Arc::new(catalog);
}

/// Locate an enabled package so its grammar can be validated off the UI thread.
#[cfg(test)]
pub fn plugin_root(id: &str) -> Option<PathBuf> {
    CATALOG
        .read()
        .unwrap()
        .plugins
        .get(id)
        .map(|entry| entry.root.clone())
}

#[cfg(test)]
mod tests;
