//! Native marketplace uses inert catalog snapshots; consent never constructs a fake Package.
use super::*;
use gpui_base::DialogClose;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::dialog::{DialogAction, DialogFooter};
use plugin_runtime::marketplace::{CATALOG_URL, CompatibilityIssue, Release, Sort};
mod details;
mod filters;

/// Catalog freshness gates downloads; installed lifecycle authority remains independent.
pub(super) struct Snapshot {
    pub state: plugin_runtime::marketplace::CatalogState,
    pub statistics: Option<Arc<plugin_runtime::marketplace::Statistics>>,
    pub statistics_error: Option<String>,
    pub ready: bool,
}

const MARKET_CHECK_INTERVAL: std::time::Duration = std::time::Duration::from_secs(6 * 60 * 60);

/// Local filters survive repaints. The worker owns catalog generation and network results.
#[derive(Default)]
pub(super) struct ViewState {
    pub revision: u64,
    started: bool,
    checked_at: Option<std::time::Instant>,
    selected_version: Option<String>,
    history_id: Option<String>,
    category: String,
    tag: String,
    sort: Sort,
}

impl ViewState {
    /// Borrow active discovery filters without exposing mutation outside their UI owner.
    pub(super) fn category(&self) -> &str {
        &self.category
    }
    /// Return the retained tag constraint for the same bounded catalog search.
    pub(super) fn tag(&self) -> &str {
        &self.tag
    }
    /// Read the selected ordering without allowing another view to mutate discovery state.
    pub(super) fn sort(&self) -> Sort {
        self.sort
    }
    /// Explicit history navigation permits withdrawn entries for exactly the selected identity.
    pub(super) fn history_ids(&self) -> std::collections::BTreeSet<String> {
        self.history_id.iter().cloned().collect()
    }

    /// Installed-detail navigation opens all reviewed history without retaining earlier search filters.
    pub(super) fn show_history(&mut self, id: String) {
        self.category.clear();
        self.tag.clear();
        self.selected_version = None;
        self.history_id = Some(id);
    }

    /// Ordinary discovery hides fully withdrawn entries, even if a local copy is installed.
    pub(super) fn leave_history(&mut self) {
        self.history_id = None;
        // Version choices belong to the current listing; a new discovery must choose its own compatible default.
        self.selected_version = None;
    }

    /// A monotonic clock makes background check timing independent of system clock changes.
    pub(super) fn needs_check(&self, now: std::time::Instant) -> bool {
        self.checked_at
            .is_none_or(|checked| now.saturating_duration_since(checked) >= MARKET_CHECK_INTERVAL)
    }

    /// Record attempts, including failures, so an outage does not cause a tight retry loop.
    pub(super) fn mark_check(&mut self, now: std::time::Instant) {
        self.started = true;
        self.checked_at = Some(now);
    }
}

impl ExtensionPanel {
    /// Last successfully fetched history keeps an update indicator through a later outage.
    pub(crate) fn has_market_updates(&self) -> bool {
        let state = self.worker.state.lock().unwrap();
        let Some(catalog) = &state.market.catalog else {
            return false;
        };
        let platform = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
        self.entries.iter().any(|entry| {
            catalog
                .plugins
                .iter()
                .find(|listing| listing.id == entry.manifest.id)
                .is_some_and(|listing| {
                    listing
                        .upgrade(
                            &entry.manifest.version,
                            env!("CARGO_PKG_VERSION"),
                            &platform,
                        )
                        .is_some()
                })
        })
    }

    /// Startup, the six-hour scheduler, manager freshness and explicit refresh share this operation.
    pub(super) fn refresh_online_market(&mut self) {
        self.market_view.mark_check(std::time::Instant::now());
        self.worker.refresh_market(
            self.root.join("marketplace/catalog.json"),
            CATALOG_URL.into(),
        );
    }

    /// One immutable publication supplies both installed metadata and market discovery.
    pub(super) fn market_snapshot(&mut self) -> Snapshot {
        if self.manager_market && !self.market_view.started {
            self.market_view.started = true;
            if self.worker.state.lock().unwrap().market.generation() == 0 {
                self.refresh_online_market();
            }
        }
        let published = self.worker.state.lock().unwrap();
        Snapshot {
            state: published.market.clone(),
            statistics: published.market_statistics.clone(),
            statistics_error: published.market_statistics_error.clone(),
            ready: published.ready,
        }
    }

    /// First installs confirm all permissions; upgrades confirm new grants and other owners to replace.
    fn confirm_market_install(
        &mut self,
        release: Release,
        generation: u64,
        upgrading: bool,
        permissions: Vec<String>,
        replacements: std::collections::BTreeMap<String, plugin_runtime::Installed>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let owner = cx.entity().downgrade();
        // Updating this owner or replacing another one retires its active targets only after commit.
        // Name the same real sessions used by disable/uninstall before the user authorizes that change.
        let impact = self
            .parent
            .upgrade()
            .map(|parent| {
                let ids = replacements
                    .keys()
                    .chain(std::iter::once(&release.manifest.id));
                ids.filter_map(|id| {
                    parent
                        .read(cx)
                        .run_controls
                        .plugin_session_impact(id, None)
                        .summary()
                })
                .collect::<Vec<_>>()
                .join("\n")
            })
            .filter(|text| !text.is_empty());
        window.open_dialog(cx, move |dialog, _, _| {
            let release = release.clone();
            let candidate = release.clone();
            let owner = owner.clone();
            let permissions = permissions.clone();
            let impact = impact.clone();
            let approved_replacements = replacements.clone();
            let replaced_versions = replacements
                .values()
                .map(|entry| format!("{} ({})", entry.manifest.id, entry.manifest.version))
                .collect::<Vec<_>>()
                .join(", ");
            let action = if upgrading {
                t!("plugins.update").to_string()
            } else {
                t!("plugins.install").to_string()
            };
            dialog
                .title(action.clone())
                .width(px(540.))
                .overlay_closable(false)
                .content(move |content, _, _| {
                    content.child(
                        v_flex()
                            .gap_2()
                            .child(release.manifest.name.clone())
                            .child(t!("plugins.consent_unsigned").to_string())
                            .child(release.url.clone())
                            .when(!replaced_versions.is_empty(), |details| {
                                details.child(
                                    div()
                                        .debug_selector(|| "market-consent-replacements".into())
                                        .child(
                                            t!(
                                                "plugins.consent_replacements",
                                                ids = replaced_versions
                                            )
                                            .to_string(),
                                        ),
                                )
                            })
                            .when_some(impact.clone(), |details, impact| {
                                details.child(
                                    div()
                                        .debug_selector(|| "market-consent-session-impact".into())
                                        .child(impact),
                                )
                            })
                            .child(
                                t!(
                                    "plugins.market_permissions",
                                    permissions = permissions.join(", ")
                                )
                                .to_string(),
                            )
                            .child(
                                t!(
                                    "plugins.market_native_services",
                                    services = release
                                        .manifest
                                        .services
                                        .keys()
                                        .cloned()
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                )
                                .to_string(),
                            ),
                    )
                })
                .footer(
                    DialogFooter::new()
                        .child(DialogClose::new().trigger(|base| {
                            Button::new("market-consent-cancel")
                                .with_base(base)
                                .debug_selector(|| "market-consent-cancel".into())
                                .label(t!("plugins.cancel").to_string())
                        }))
                        .child(
                            DialogAction::new().child(
                                Button::new("confirm-market-install")
                                    .debug_selector(|| "confirm-market-install".into())
                                    .label(action)
                                    .primary(),
                            ),
                        ),
                )
                .on_ok(move |_, _, cx| {
                    owner
                        .update(cx, |this, cx| {
                            let accepted = this.worker.install_market(
                                candidate.clone(),
                                generation,
                                approved_replacements.clone(),
                            );
                            cx.notify();
                            accepted
                        })
                        .unwrap_or(false)
                })
        });
    }
}

/// Explain why a historical release cannot be installed without hiding its review history.
fn market_version_issue(
    release: &Release,
    installed: Option<&plugin_runtime::Installed>,
    platform: &str,
) -> Option<String> {
    if release.withdrawn {
        return Some(
            t!(
                "plugins.market_withdrawn",
                reason = release.withdrawal_reason.as_deref().unwrap_or("")
            )
            .to_string(),
        );
    }
    if let Err(error) = release.compatible(env!("CARGO_PKG_VERSION"), platform) {
        return Some(match error {
            CompatibilityIssue::Host => t!("plugins.market_host_incompatible").to_string(),
            CompatibilityIssue::Platform => t!("plugins.market_platform_incompatible").to_string(),
            CompatibilityIssue::Withdrawn => t!(
                "plugins.market_withdrawn",
                reason = release.withdrawal_reason.as_deref().unwrap_or("")
            )
            .to_string(),
            CompatibilityIssue::Capabilities(reason) => {
                t!("plugins.market_capability_incompatible", reason = reason).to_string()
            }
        });
    }
    if installed.is_some_and(|entry| {
        semver::Version::parse(&release.version)
            .ok()
            .zip(semver::Version::parse(&entry.manifest.version).ok())
            .is_none_or(|(candidate, current)| candidate <= current)
    }) {
        return Some(t!("plugins.market_not_newer").to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Startup checks and periodic checks use one monotonic deadline; opening a page does not reset it.
    #[test]
    fn update_check_is_due_at_six_hours() {
        let now = std::time::Instant::now();
        let mut view = ViewState::default();
        assert!(view.needs_check(now));
        view.mark_check(now);
        assert!(!view.needs_check(now + MARKET_CHECK_INTERVAL - std::time::Duration::from_secs(1)));
        assert!(view.needs_check(now + MARKET_CHECK_INTERVAL));
    }
}
