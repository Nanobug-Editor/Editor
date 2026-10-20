//! Network preparation feeds immutable packages to the existing serialized installation actor.
use super::*;
use plugin_runtime::marketplace::{Catalog, CatalogCache, Release, Statistics};
use rust_i18n::t;
#[cfg(test)]
mod tests;

impl Worker {
    /// Restore bounded inert cache, then fetch without occupying the actor or UI thread.
    pub fn refresh_market(&self, cache: PathBuf, url: String) {
        // Recorded UI tests must not spend bounded download slots on an ambient public endpoint.
        // Explicit fixture URLs and real background actors still use the ordinary HTTP adapter.
        #[cfg(test)]
        if self.recording && url == plugin_runtime::marketplace::CATALOG_URL {
            return;
        }
        let generation = {
            let mut state = self.state.lock().unwrap();
            if state.market.refreshing {
                return;
            }
            state.market_revision += 1;
            state.market.begin_refresh()
        };
        let output = self.state.clone();
        std::thread::spawn(move || {
            // Counts are a separate advisory snapshot; a failed statistics endpoint cannot
            // downgrade a successfully refreshed review catalog to browse-only state.
            let statistics_cache = cache.with_file_name("stats.json");
            let cached_statistics = std::fs::File::open(&statistics_cache)
                .ok()
                .and_then(|file| {
                    use std::io::Read;
                    let mut bytes = Vec::new();
                    file.take(4 * 1024 * 1024 + 1)
                        .read_to_end(&mut bytes)
                        .ok()?;
                    Some(bytes)
                });
            if let Some(bytes) = cached_statistics
                && let Ok(statistics) = Statistics::parse(&bytes)
            {
                let mut state = output.lock().unwrap();
                if state.market.generation() == generation && state.market_statistics.is_none() {
                    state.market_statistics = Some(Arc::new(statistics));
                    state.market_revision += 1;
                }
            }
            let cached = std::fs::File::open(&cache).ok().and_then(|file| {
                use std::io::Read;
                let mut bytes = Vec::new();
                file.take(16 * 1024 * 1024 + 129)
                    .read_to_end(&mut bytes)
                    .ok()?;
                CatalogCache::parse(&bytes).ok()
            });
            let cached_icons = cached
                .as_ref()
                .map(|c| icons(&c.catalog))
                .unwrap_or_default();
            {
                let mut state = output.lock().unwrap();
                if state.market.generation() == generation && state.market.catalog.is_none() {
                    if let Some(cache) = cached {
                        state.market.fetched_at = Some(cache.fetched_at);
                        state.market.catalog = Some(Arc::new(cache.catalog));
                        state.market_icons = cached_icons;
                    }
                    state.market_revision += 1;
                }
            }
            let result = Catalog::fetch(&url, &plugin_runtime::InstallControl::default())
                .map_err(|e| format!("{e:#}"));
            let decoded = result.as_ref().ok().map(icons).unwrap_or_default();
            let mut state = output.lock().unwrap();
            if state.market.finish_refresh(generation, result) {
                if state.market.fresh() {
                    state.market_icons = decoded;
                    // Cache persistence never grants authority; failure leaves the live result usable.
                    let _ = (|| -> anyhow::Result<()> {
                        let parent = cache
                            .parent()
                            .ok_or_else(|| anyhow::anyhow!("Missing cache parent"))?;
                        std::fs::create_dir_all(parent)?;
                        let mut file = tempfile::NamedTempFile::new_in(parent)?;
                        serde_json::to_writer(
                            file.as_file_mut(),
                            &CatalogCache {
                                fetched_at: state.market.fetched_at.unwrap_or(0),
                                catalog: state.market.catalog.as_deref().unwrap().clone(),
                            },
                        )?;
                        file.persist(&cache)?;
                        Ok(())
                    })();
                }
                state.market_revision += 1;
            }
            drop(state);
            if let Some(prefix) = url.strip_suffix("catalog.json") {
                let result = Statistics::fetch(&format!("{prefix}stats.json"))
                    .map_err(|error| format!("{error:#}"));
                let mut state = output.lock().unwrap();
                if state.market.generation() == generation {
                    match result {
                        Ok(statistics) => {
                            // Persistence is best-effort; the current measurement stays visible.
                            let _ = (|| -> anyhow::Result<()> {
                                let parent = statistics_cache.parent().ok_or_else(|| {
                                    anyhow::anyhow!("Missing statistics cache parent")
                                })?;
                                std::fs::create_dir_all(parent)?;
                                let mut file = tempfile::NamedTempFile::new_in(parent)?;
                                serde_json::to_writer(file.as_file_mut(), &statistics)?;
                                file.persist(&statistics_cache)?;
                                Ok(())
                            })();
                            state.market_statistics = Some(Arc::new(statistics));
                            state.market_statistics_error = None;
                        }
                        Err(error) => state.market_statistics_error = Some(error),
                    }
                    state.market_revision += 1;
                }
            }
        });
    }

    /// Capture installed replacement targets for the native consent dialog, without granting approval.
    pub fn market_replacements(&self, release: &Release) -> BTreeMap<String, Installed> {
        replacement_records(&release.manifest, &self.state.lock().unwrap().entries)
    }

    /// Explicit native consent precedes this call. Check review and approved target records before I/O.
    /// A replacement approval belongs to these exact installed records, not just their plugin IDs.
    pub fn install_market(
        &self,
        release: Release,
        generation: u64,
        approved_replacements: BTreeMap<String, Installed>,
    ) -> bool {
        let mut state = self.state.lock().unwrap();
        let id = release.manifest.id.clone();
        let existing = state
            .entries
            .iter()
            .find(|entry| entry.manifest.id == id)
            .map(|entry| (entry.manifest.version.clone(), entry.digest.clone()));
        // A market action may only advance the installed version; the captured digest prevents
        // another local or market operation from silently changing the target during download.
        let advances = existing.as_ref().is_none_or(|(version, _)| {
            semver::Version::parse(&release.version)
                .ok()
                .zip(semver::Version::parse(version).ok())
                .is_some_and(|(candidate, current)| candidate > current)
        });
        if !state.ready
            || state.progress.is_some()
            || !state.market.fresh()
            || state.market.generation() != generation
            || !self.trusted.load(std::sync::atomic::Ordering::Acquire)
            || !advances
            || serde_json::to_value(replacement_records(&release.manifest, &state.entries)).ok()
                != serde_json::to_value(&approved_replacements).ok()
            || release
                .compatible(
                    env!("CARGO_PKG_VERSION"),
                    &format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
                )
                .is_err()
        {
            return false;
        }
        let reviewed = state
            .market
            .catalog
            .as_ref()
            .and_then(|c| c.plugins.iter().find(|p| p.id == id))
            .is_some_and(|p| {
                p.versions
                    .iter()
                    .any(|v| serde_json::to_value(v).ok() == serde_json::to_value(&release).ok())
            });
        if !reviewed {
            return false;
        }
        let output = self.state.clone();
        let guard_output = Arc::downgrade(&output);
        let trusted = self.trusted.clone();
        let report_output = Arc::downgrade(&output);
        let guarded_id = id.clone();
        let guarded_existing = existing.clone();
        let guarded_manifest = release.manifest.clone();
        let replacement_ids = approved_replacements.keys().cloned().collect::<Vec<_>>();
        let control = plugin_runtime::InstallControl::new(move |stage| {
            if let Some(output) = report_output.upgrade() {
                if let Some(report) = &mut output.lock().unwrap().installation {
                    report.advance(stage);
                }
            }
        })
        .with_installer_prompts()
        .with_replacements(replacement_ids)
        .with_guard(move || {
            trusted.load(std::sync::atomic::Ordering::Acquire)
                && guard_output.upgrade().is_some_and(|output| {
                    let state = output.lock().unwrap();
                    state.ready
                        && state.market.fresh()
                        && state.market.generation() == generation
                        && state
                            .entries
                            .iter()
                            .find(|e| e.manifest.id == guarded_id)
                            .map(|entry| (entry.manifest.version.clone(), entry.digest.clone()))
                            == guarded_existing
                        && serde_json::to_value(replacement_records(
                            &guarded_manifest,
                            &state.entries,
                        ))
                        .ok()
                            == serde_json::to_value(&approved_replacements).ok()
                })
        });
        state.install_control = Some(control.clone());
        state.installation = Some(InstallationProgress {
            id: id.clone(),
            message: t!("plugins.market_downloading", bytes = 0).to_string(),
            cancellable: true,
            installed: false,
        });
        state.progress = Some(OperationProgress {
            id: id.clone(),
            action: LifecycleAction::Install,
            delete_data: None,
        });
        state.status = None;
        let tx = self.tx.clone();
        drop(state);
        std::thread::spawn(move || {
            let result = release
                .download(&control, |bytes| {
                    if let Some(report) = &mut output.lock().unwrap().installation {
                        report.message =
                            t!("plugins.market_downloading", bytes = bytes).to_string();
                    }
                })
                .and_then(|package| {
                    tx.send(Work::Install(package))
                        .map_err(|_| anyhow::anyhow!("Plugin worker stopped"))
                });
            if let Err(error) = result {
                let mut state = output.lock().unwrap();
                state.progress = None;
                state.installation = None;
                state.install_control = None;
                state.status = Some(OperationStatus {
                    plugin: Some(id),
                    message: format!("{error:#}"),
                });
            }
        });
        true
    }
}

/// Ignore already retired targets; a new or changed target must receive its own visible approval.
fn replacement_records(manifest: &Manifest, entries: &[Installed]) -> BTreeMap<String, Installed> {
    entries
        .iter()
        .filter(|entry| {
            entry.replaced_by.is_none()
                && manifest
                    .replaces
                    .iter()
                    .any(|rule| rule.id == entry.manifest.id)
        })
        .map(|entry| (entry.manifest.id.clone(), entry.clone()))
        .collect()
}

/// Decode inert SVG snapshots off the UI thread, with external references disabled and a total quota.
fn icons(catalog: &Catalog) -> BTreeMap<String, crate::ui::plugin::bitmap::Bitmap> {
    let mut output = BTreeMap::new();
    let mut remaining = 8 * 1024 * 1024;
    for release in catalog.plugins.iter().flat_map(|p| &p.versions) {
        if release.icon.is_empty()
            || release.icon.len() > 256 * 1024
            || output.contains_key(&release.sha256)
        {
            continue;
        }
        if let Ok(bitmap) = crate::ui::plugin::bitmap::decode_with_budget(
            release.icon.as_bytes(),
            remaining.min(1024 * 1024),
        ) {
            remaining -= u64::from(bitmap.width) * u64::from(bitmap.height) * 4;
            output.insert(release.sha256.clone(), bitmap);
        }
    }
    output
}
