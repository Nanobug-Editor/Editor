//! Reviewed versions, metadata and consent controls embedded in the shared native manager.
use super::*;
use plugin_runtime::marketplace::Listing;

impl ExtensionPanel {
    /// Choose a compatible upgrade/default without conflating it with the installed manifest.
    pub(in crate::extensions) fn market_release<'a>(
        &self,
        listing: &'a Listing,
        selected: bool,
    ) -> &'a Release {
        let platform = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
        let installed = self
            .entries
            .iter()
            .find(|entry| entry.manifest.id == listing.id);
        let chosen = selected
            .then_some(self.market_view.selected_version.as_ref())
            .flatten()
            .and_then(|version| {
                listing
                    .versions
                    .iter()
                    .find(|release| &release.version == version)
            });
        chosen
            .or_else(|| {
                installed.and_then(|entry| {
                    listing.upgrade(
                        &entry.manifest.version,
                        env!("CARGO_PKG_VERSION"),
                        &platform,
                    )
                })
            })
            .or_else(|| listing.compatible(env!("CARGO_PKG_VERSION"), &platform))
            .unwrap_or_else(|| {
                listing
                    .versions
                    .iter()
                    .max_by_key(|release| semver::Version::parse(&release.version).ok())
                    .unwrap()
            })
    }

    /// Install authority belongs to the reviewed candidate and catalog generation; local actions ignore it.
    pub(in crate::extensions) fn market_install_action(
        &self,
        release: &Release,
        snapshot: &Snapshot,
        cx: &Context<Self>,
    ) -> AnyElement {
        let installed = self
            .entries
            .iter()
            .find(|entry| entry.manifest.id == release.manifest.id);
        let platform = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
        let issue = market_version_issue(release, installed, &platform);
        let candidate = release.clone();
        let generation = snapshot.state.generation();
        Button::new("market-install")
            .debug_selector(|| "market-install".into())
            .label(if installed.is_some() {
                t!("plugins.update").to_string()
            } else {
                t!("plugins.install").to_string()
            })
            .primary()
            .outline()
            .loading(self.progress.as_ref().is_some_and(|progress| {
                progress.action == LifecycleAction::Install && progress.id == release.manifest.id
            }))
            .disabled(
                !snapshot.ready
                    || issue.is_some()
                    || !snapshot.state.fresh()
                    || self.progress.is_some()
                    || !self
                        .worker
                        .trusted
                        .load(std::sync::atomic::Ordering::Acquire),
            )
            .when_some(issue, |button, reason| button.tooltip(reason))
            .on_click(cx.listener(move |this, _, window, cx| {
                let installed = this
                    .entries
                    .iter()
                    .find(|entry| entry.manifest.id == candidate.manifest.id);
                let new_permissions = candidate
                    .manifest
                    .permissions
                    .iter()
                    .filter(|permission| {
                        installed.is_none_or(|entry| !entry.grants.contains(*permission))
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                let replacements = this.worker.market_replacements(&candidate);
                // Existing grants never authorize replacing another installed owner without visible consent.
                if installed.is_some() && new_permissions.is_empty() && replacements.is_empty() {
                    let _ = this
                        .worker
                        .install_market(candidate.clone(), generation, replacements);
                    cx.notify();
                } else {
                    this.confirm_market_install(
                        candidate.clone(),
                        generation,
                        installed.is_some(),
                        new_permissions,
                        replacements,
                        window,
                        cx,
                    );
                }
            }))
            .into_any_element()
    }

    /// Version selection changes reviewed browsing only; it cannot switch a running local instance.
    pub(in crate::extensions) fn market_versions(
        &self,
        listing: &Listing,
        snapshot: &Snapshot,
        cx: &Context<Self>,
    ) -> AnyElement {
        let platform = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
        let installed = self
            .entries
            .iter()
            .find(|entry| entry.manifest.id == listing.id);
        let chosen = self.market_release(listing, self.manager_market);
        let mut versions = listing.versions.iter().collect::<Vec<_>>();
        versions.sort_by_key(|version| semver::Version::parse(&version.version).ok());
        versions.reverse();
        let mut body = v_flex()
            .debug_selector(|| "market-versions-region".into())
            .p_5()
            .gap_2();
        for version in versions {
            let selector = format!("market-version-{}", version.version);
            let id = listing.id.clone();
            let selected_version = version.version.clone();
            let count = snapshot
                .statistics
                .as_deref()
                .and_then(|stats| stats.version(&listing.id, &version.version))
                .map(|count| t!("plugins.market_downloads", count = count).to_string())
                .unwrap_or_else(|| t!("plugins.market_downloads_unavailable").to_string());
            body = body.child(
                v_flex()
                    .gap_1()
                    .child(
                        Button::new(SharedString::from(selector.clone()))
                            .debug_selector(move || selector.clone())
                            .label(format!("{} · {}", version.version, count))
                            .w_full()
                            .when(version.version == chosen.version, |button| button.primary())
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if !this.manager_market {
                                    // Market search uses the newest name; a local old-name query must not discard this identity.
                                    // Base's programmatic set_value is silent, preserving the route while user edits still reset it.
                                    if let Some(input) = &this.manager_search {
                                        input.update(cx, |input, cx| {
                                            input.set_value("", window, cx)
                                        });
                                    }
                                    this.market_view.show_history(id.clone());
                                }
                                this.manager_market = true;
                                this.manager_selected = Some(id.clone());
                                this.market_view.selected_version = Some(selected_version.clone());
                                this.manager_detail_tab = management::DetailTab::Versions;
                                this.manager_detail_scroll.set_offset(Default::default());
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(
                                market_version_issue(version, installed, &platform)
                                    .unwrap_or_else(|| t!("plugins.market_available").to_string()),
                            ),
                    ),
            );
        }
        body.into_any_element()
    }

    /// Information describes exactly the displayed manifest, with optional reviewed provenance and statistics.
    pub(in crate::extensions) fn market_information(
        &self,
        manifest: &protocol::Manifest,
        listing: Option<&Listing>,
        release: Option<&Release>,
        snapshot: &Snapshot,
        cx: &Context<Self>,
    ) -> AnyElement {
        let mut body = v_flex()
            .debug_selector(|| "market-information-region".into())
            .p_5()
            .gap_3()
            .child(manifest.id.clone())
            .child(
                t!(
                    "plugins.market_permissions",
                    permissions = manifest
                        .permissions
                        .iter()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                )
                .to_string(),
            );
        if let Some(listing) = listing {
            let source = format!("https://github.com/{}", listing.repository);
            let feedback = format!("{source}/issues");
            body = body
                .child(listing.maintainers.join(", "))
                .child(listing.tags.join(" · "))
                .child(
                    t!(
                        "plugins.market_identity",
                        id = listing.id.clone(),
                        repository = listing.repository.clone()
                    )
                    .to_string(),
                )
                .child(
                    h_flex()
                        .flex_wrap()
                        .gap_2()
                        .child(
                            Button::new("market-source")
                                .debug_selector(|| "market-source".into())
                                .label(t!("plugins.market_source").to_string())
                                .on_click(move |_, _, cx| cx.open_url(&source)),
                        )
                        .child(
                            Button::new("market-feedback")
                                .debug_selector(|| "market-feedback".into())
                                .label(t!("plugins.market_feedback").to_string())
                                .on_click(move |_, _, cx| cx.open_url(&feedback)),
                        ),
                )
                .child(
                    snapshot
                        .statistics
                        .as_deref()
                        .and_then(|stats| stats.total(&listing.id))
                        .map(|count| t!("plugins.market_downloads", count = count).to_string())
                        .unwrap_or_else(|| t!("plugins.market_downloads_unavailable").to_string()),
                );
        }
        if let Some(release) = release {
            body = body
                .child(release.license.clone())
                .child(
                    t!(
                        "plugins.market_published_at",
                        time = release.published_at.clone()
                    )
                    .to_string(),
                )
                .child(
                    t!(
                        "plugins.market_compatibility",
                        host = release.host_version.clone(),
                        platforms = if release.platforms.is_empty() {
                            "*".into()
                        } else {
                            release.platforms.join(", ")
                        }
                    )
                    .to_string(),
                );
        }
        if let Some(at) = snapshot
            .state
            .fetched_at
            .and_then(|seconds| chrono::DateTime::from_timestamp(i64::try_from(seconds).ok()?, 0))
        {
            body = body.child(
                t!(
                    "plugins.market_fetched_at",
                    time = at.format("%Y-%m-%d %H:%M:%S UTC").to_string()
                )
                .to_string(),
            );
        }
        if let Some(at) = snapshot
            .statistics
            .as_ref()
            .and_then(|stats| stats.measured_at.as_ref())
        {
            body = body.child(t!("plugins.market_statistics_at", time = at.clone()).to_string());
        }
        if let Some(error) = &snapshot.statistics_error {
            body = body
                .child(t!("plugins.market_statistics_stale", error = error.clone()).to_string());
        }
        let _ = cx;
        body.into_any_element()
    }
}
