//! Reviewed versions, metadata and consent controls embedded in the shared native manager.
use super::*;
use plugin_runtime::marketplace::Listing;

impl ExtensionPanel {
    /// All native update indicators and installed actions choose the same reviewed compatible upgrade.
    /// Returns no candidate for an uninstalled ID or without a newer admissible release; no install is authorized here.
    pub(in crate::extensions) fn market_upgrade<'a>(
        &self,
        listing: &'a Listing,
    ) -> Option<&'a Release> {
        let entry = self
            .entries
            .iter()
            .find(|entry| entry.manifest.id == listing.id)?;
        let platform = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
        listing.upgrade(
            &entry.manifest.version,
            env!("CARGO_PKG_VERSION"),
            &platform,
        )
    }

    /// Choose a compatible upgrade/default without conflating it with the installed manifest.
    pub(in crate::extensions) fn market_release<'a>(
        &self,
        listing: &'a Listing,
        selected: bool,
    ) -> &'a Release {
        let platform = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
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
            .or_else(|| self.market_upgrade(listing))
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
    /// Returns no control when unavailable; already admitted operations retain their loading receipt.
    pub(in crate::extensions) fn market_install_action(
        &self,
        release: &Release,
        snapshot: &Snapshot,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let installed = self
            .entries
            .iter()
            .find(|entry| entry.manifest.id == release.manifest.id);
        let platform = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
        let issue = market_version_issue(release, installed, &platform);
        let loading = self.progress.as_ref().is_some_and(|progress| {
            progress.action == LifecycleAction::Install && progress.id == release.manifest.id
        });
        // Unavailable operations have no focus/hit target; an admitted in-flight operation keeps its receipt.
        if !loading
            && (!snapshot.ready
                || issue.is_some()
                || !snapshot.state.fresh()
                || self.progress.is_some()
                || !self
                    .worker
                    .trusted
                    .load(std::sync::atomic::Ordering::Acquire))
        {
            return None;
        }
        let candidate = release.clone();
        let generation = snapshot.state.generation();
        Some(
            Button::new("market-install")
                .debug_selector(|| "market-install".into())
                .label(if installed.is_some() {
                    t!("plugins.update").to_string()
                } else {
                    t!("plugins.install").to_string()
                })
                .primary()
                .outline()
                .loading(loading)
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
                    if installed.is_some() && new_permissions.is_empty() && replacements.is_empty()
                    {
                        let _ =
                            this.worker
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
                .into_any_element(),
        )
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
        // Installed history marks its actual version; market history marks only the browsed release.
        let chosen = if self.manager_market {
            Some(self.market_release(listing, true).version.as_str())
        } else {
            installed.map(|entry| entry.manifest.version.as_str())
        };
        let mut versions = listing.versions.iter().collect::<Vec<_>>();
        versions.sort_by_key(|version| semver::Version::parse(&version.version).ok());
        versions.reverse();
        let mut body = v_flex()
            .debug_selector(|| "market-versions-region".into())
            .p_5()
            .gap_1()
            .child(
                div()
                    .pb_2()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(t!("plugins.market_versions").to_string()),
            );
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
                Button::new(SharedString::from(selector.clone()))
                    .debug_selector(move || selector.clone())
                    .ghost()
                    .content_full_width()
                    .w_full()
                    .h_auto()
                    .min_h(px(64.))
                    .px_3()
                    .py_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .when(chosen == Some(version.version.as_str()), |button| {
                        button.custom(
                            ui::controls::ButtonCustomVariant::new(cx)
                                .color(cx.theme().list_active)
                                .hover(cx.theme().list_active)
                                .active(cx.theme().list_active)
                                .foreground(cx.theme().foreground),
                        )
                    })
                    .child(
                        v_flex()
                            .w_full()
                            .min_w(px(0.))
                            .gap_1()
                            .child(
                                h_flex()
                                    .gap_3()
                                    .items_center()
                                    .flex_wrap()
                                    .child(div().font_semibold().child(version.version.clone()))
                                    .when(
                                        installed.is_some_and(|entry| {
                                            entry.manifest.version == version.version
                                        }),
                                        |row| {
                                            row.child(
                                                div()
                                                    .debug_selector({
                                                        let selector = format!(
                                                            "market-version-{}-installed",
                                                            version.version
                                                        );
                                                        move || selector.clone()
                                                    })
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child(
                                                        t!("plugins.version_installed").to_string(),
                                                    ),
                                            )
                                        },
                                    )
                                    .when(
                                        self.manager_market
                                            && chosen == Some(version.version.as_str()),
                                        |row| {
                                            row.child(
                                                div()
                                                    .text_xs()
                                                    .text_color(cx.theme().primary)
                                                    .child(
                                                        t!("plugins.version_browsing").to_string(),
                                                    ),
                                            )
                                        },
                                    ),
                            )
                            .child(
                                h_flex()
                                    .flex_wrap()
                                    .gap_3()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(
                                        t!(
                                            "plugins.market_published_at",
                                            time = publication_time(&version.published_at)
                                        )
                                        .to_string(),
                                    )
                                    .child(count),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(
                                        market_version_issue(version, installed, &platform)
                                            .unwrap_or_else(|| {
                                                t!("plugins.market_available").to_string()
                                            }),
                                    ),
                            ),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if !this.manager_market {
                            // Market search uses the newest name; a local old-name query must not discard this identity.
                            // Base's programmatic set_value is silent, preserving the route while user edits still reset it.
                            if let Some(input) = &this.manager_search {
                                input.update(cx, |input, cx| input.set_value("", window, cx));
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
        let mut basic = vec![
            ("id", manifest.id.clone()),
            ("version", manifest.version.clone()),
        ];
        if let Some(listing) = listing {
            basic.extend([
                ("maintainers", listing.maintainers.join(", ")),
                ("category", listing.category.clone()),
                ("tags", listing.tags.join(" · ")),
                (
                    "repository",
                    format!("https://github.com/{}", listing.repository),
                ),
            ]);
        }
        if let Some(release) = release {
            basic.extend([
                ("license", release.license.clone()),
                ("published", publication_time(&release.published_at)),
            ]);
        }
        let basic = information_group("basic", basic, cx).when_some(listing, |group, listing| {
            let source = format!("https://github.com/{}", listing.repository);
            let feedback = format!("{source}/issues");
            group.child(
                h_flex()
                    .flex_wrap()
                    .gap_2()
                    .pt_2()
                    .child(
                        Button::new("market-source")
                            .debug_selector(|| "market-source".into())
                            .label(t!("plugins.market_source").to_string())
                            .small()
                            .outline()
                            .on_click(move |_, _, cx| cx.open_url(&source)),
                    )
                    .child(
                        Button::new("market-feedback")
                            .debug_selector(|| "market-feedback".into())
                            .label(t!("plugins.market_feedback").to_string())
                            .small()
                            .outline()
                            .on_click(move |_, _, cx| cx.open_url(&feedback)),
                    ),
            )
        });
        let scope = match manifest.scope {
            protocol::api::InstanceScope::Workspace => t!("plugins.scope_workspace").to_string(),
            protocol::api::InstanceScope::Application => {
                t!("plugins.scope_application").to_string()
            }
        };
        let mut compatibility = vec![
            ("scope", scope),
            (
                "permissions",
                manifest
                    .permissions
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
        ];
        if let Some(release) = release {
            compatibility.extend([
                ("host", release.host_version.clone()),
                (
                    "platforms",
                    if release.platforms.is_empty() {
                        t!("plugins.all_platforms").to_string()
                    } else {
                        release.platforms.join(", ")
                    },
                ),
            ]);
        }
        let mut body = v_flex()
            .debug_selector(|| "market-information-region".into())
            .w_full()
            .min_w(px(0.))
            .p_5()
            .gap_5()
            .child(basic)
            .child(information_group("compatibility", compatibility, cx));
        let mut statistics = Vec::new();
        if let Some(listing) = listing {
            statistics.push((
                "downloads",
                snapshot
                    .statistics
                    .as_deref()
                    .and_then(|stats| stats.total(&listing.id))
                    .map(|count| count.to_string())
                    .unwrap_or_else(|| t!("plugins.market_downloads_unavailable").to_string()),
            ));
        }
        if let Some(at) = snapshot
            .state
            .fetched_at
            .and_then(|seconds| chrono::DateTime::from_timestamp(i64::try_from(seconds).ok()?, 0))
        {
            statistics.push(("fetched", at.format("%Y-%m-%d %H:%M:%S UTC").to_string()));
        }
        if let Some(at) = snapshot
            .statistics
            .as_ref()
            .and_then(|stats| stats.measured_at.as_ref())
        {
            statistics.push((
                "measured",
                t!("plugins.market_statistics_at", time = at.clone()).to_string(),
            ));
        }
        if !statistics.is_empty() {
            body = body.child(information_group("statistics", statistics, cx));
        }
        if let Some(error) = &snapshot.statistics_error {
            body = body
                .child(t!("plugins.market_statistics_stale", error = error.clone()).to_string());
        }
        body.into_any_element()
    }
}

/// Readable dates keep the original catalog value when a historical timestamp has a different format.
fn publication_time(value: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|at| {
            at.with_timezone(&chrono::Utc)
                .format("%Y-%m-%d %H:%M UTC")
                .to_string()
        })
        .unwrap_or_else(|_| value.to_owned())
}

/// One aligned, wrapping field renderer supplies all information groups without making catalog text interactive.
fn information_group(
    section: &'static str,
    fields: Vec<(&'static str, String)>,
    cx: &Context<ExtensionPanel>,
) -> gpui_kit::Div {
    let mut group = v_flex()
        .w_full()
        .min_w(px(0.))
        .gap_1()
        .debug_selector(move || format!("market-info-{section}"))
        .child(
            div()
                .pb_2()
                .font_semibold()
                .child(t!(format!("plugins.info_{section}")).to_string()),
        );
    for (key, value) in fields {
        group = group.child(
            h_flex()
                .w_full()
                .min_w(px(0.))
                .items_start()
                .gap_4()
                .py_2()
                .border_b_1()
                .border_color(cx.theme().border.opacity(0.5))
                .child(
                    div()
                        .w(gpui_kit::relative(0.27))
                        .min_w(px(96.))
                        .max_w(px(160.))
                        .flex_shrink_0()
                        .debug_selector(move || format!("market-info-{key}-label"))
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(t!(format!("plugins.info_{key}")).to_string()),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .text_sm()
                        .debug_selector(move || format!("market-info-{key}-value"))
                        .child(if value.is_empty() {
                            t!("plugins.info_none").to_string()
                        } else {
                            value
                        }),
                ),
        );
    }
    group
}
