//! Reviewed metadata, version history and explicit install/update controls for one listing.
use super::*;
use plugin_runtime::marketplace::{CatalogState, Listing, Statistics};

impl ExtensionPanel {
    /// Version selection stays local; the worker rechecks review generation before download and commit.
    pub(super) fn market_listing_details(
        &mut self,
        listing: &Listing,
        state: &CatalogState,
        statistics: Option<&Statistics>,
        ready: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut body = v_flex().gap_3();
        let platform = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
        let installed = self.entries.iter().find(|e| e.manifest.id == listing.id);
        let preferred = installed
            .and_then(|entry| {
                listing.upgrade(
                    &entry.manifest.version,
                    env!("CARGO_PKG_VERSION"),
                    &platform,
                )
            })
            .or_else(|| listing.compatible(env!("CARGO_PKG_VERSION"), &platform));
        let release = self
            .market_view
            .selected_version
            .as_ref()
            .and_then(|selected| listing.versions.iter().find(|v| &v.version == selected))
            .or(preferred)
            .unwrap_or_else(|| {
                listing
                    .versions
                    .iter()
                    .max_by_key(|v| semver::Version::parse(&v.version).ok())
                    .unwrap()
            });
        let incompatibility = market_version_issue(release, installed, &platform);
        let candidate = release.clone();
        let generation = state.generation();
        let source = format!("https://github.com/{}", listing.repository);
        let feedback = format!("{source}/issues");
        body = body
            .child(
                div()
                    .text_xl()
                    .font_semibold()
                    .child(release.manifest.name.clone()),
            )
            .child(format!(
                "{} · {} · {}",
                release.version,
                listing.maintainers.join(", "),
                release.license
            ))
            .child(listing.summary.clone())
            .child(
                t!(
                    "plugins.market_published_at",
                    time = release.published_at.clone()
                )
                .to_string(),
            )
            .child(
                h_flex()
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
                t!(
                    "plugins.market_identity",
                    id = listing.id.clone(),
                    repository = listing.repository.clone()
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
            )
            .child(
                t!(
                    "plugins.market_permissions",
                    permissions = release
                        .manifest
                        .permissions
                        .iter()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                )
                .to_string(),
            )
            .child(listing.tags.join(" · "))
            .child(
                statistics
                    .as_ref()
                    .and_then(|s| s.total(&listing.id))
                    .map(|count| t!("plugins.market_downloads", count = count).to_string())
                    .unwrap_or_else(|| t!("plugins.market_downloads_unavailable").to_string()),
            );
        let mut versions = listing.versions.iter().collect::<Vec<_>>();
        versions.sort_by_key(|version| semver::Version::parse(&version.version).ok());
        versions.reverse();
        let mut history = v_flex().gap_1().child(
            div()
                .font_semibold()
                .child(t!("plugins.market_versions").to_string()),
        );
        for version in versions {
            let selected_version = version.version.clone();
            let selector = format!("market-version-{}", version.version);
            let version_count = statistics
                .as_ref()
                .and_then(|s| s.version(&listing.id, &version.version))
                .map(|count| t!("plugins.market_downloads", count = count).to_string())
                .unwrap_or_else(|| t!("plugins.market_downloads_unavailable").to_string());
            let status = market_version_issue(version, installed, &platform)
                .unwrap_or_else(|| t!("plugins.market_available").to_string());
            history = history.child(
                v_flex()
                    .gap_1()
                    .child(
                        Button::new(SharedString::from(selector.clone()))
                            .debug_selector(move || selector.clone())
                            .label(format!("{} · {}", version.version, version_count))
                            .w_full()
                            .when(version.version == release.version, |button| {
                                button.primary()
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.market_view.selected_version = Some(selected_version.clone());
                                cx.notify();
                            })),
                    )
                    .child(div().text_sm().child(status)),
            );
        }
        body = body.child(history);
        if let Some(reason) = &incompatibility {
            body = body.child(
                div()
                    .text_color(cx.theme().muted_foreground)
                    .child(reason.clone()),
            );
        }
        body = body.child(
            Button::new("market-install")
                .debug_selector(|| "market-install".into())
                .label(if installed.is_some() {
                    t!("plugins.update").to_string()
                } else {
                    t!("plugins.install").to_string()
                })
                .disabled(
                    !ready
                        || incompatibility.is_some()
                        || !state.fresh()
                        || self.progress.is_some()
                        || !self
                            .worker
                            .trusted
                            .load(std::sync::atomic::Ordering::Acquire),
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    let installed = this
                        .entries
                        .iter()
                        .find(|e| e.manifest.id == candidate.manifest.id);
                    let new_permissions = candidate
                        .manifest
                        .permissions
                        .iter()
                        .filter(|permission| {
                            installed.is_none_or(|entry| !entry.grants.contains(*permission))
                        })
                        .cloned()
                        .collect::<Vec<_>>();
                    if installed.is_some() && new_permissions.is_empty() {
                        let _ = this.worker.install_market(candidate.clone(), generation);
                        cx.notify();
                    } else {
                        this.confirm_market_install(
                            candidate.clone(),
                            generation,
                            installed.is_some(),
                            new_permissions,
                            window,
                            cx,
                        );
                    }
                })),
        );
        if let Some(bitmap) = self
            .worker
            .state
            .lock()
            .unwrap()
            .market_icons
            .get(&release.sha256)
        {
            body = body.child(gpui_kit::img(bitmap.image.clone()).size(px(24.)));
        }
        body = body
            .child(ui::controls::untrusted_markdown_view(
                "market-readme",
                release.readme.clone(),
                typography::font_size(cx),
                cx,
            ))
            .child(
                div()
                    .font_semibold()
                    .child(t!("plugins.changes").to_string()),
            )
            .child(ui::controls::untrusted_markdown_view(
                "market-changelog",
                release.changelog.clone(),
                typography::font_size(cx),
                cx,
            ));
        body.into_any_element()
    }
}
