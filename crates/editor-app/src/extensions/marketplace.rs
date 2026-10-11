//! Native marketplace uses inert catalog snapshots; consent never constructs a fake Package.
use super::*;
use crate::ui::controls::vertical_viewport_scrollbar;
use gpui_base::DialogClose;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::dialog::{DialogAction, DialogFooter};
use gpui_kit::component::scroll::ScrollableElement;
use plugin_runtime::marketplace::{CATALOG_URL, CompatibilityIssue, Release, Sort};
mod details;

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

    /// Search, filters and details use local controls and the existing native scroll viewport.
    pub(super) fn online_market(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if !self.market_view.started {
            self.market_view.started = true;
            if self.worker.state.lock().unwrap().market.generation() == 0 {
                self.refresh_online_market();
            }
        }
        let (state, statistics, statistics_error, ready) = {
            let published = self.worker.state.lock().unwrap();
            (
                published.market.clone(),
                published.market_statistics.clone(),
                published.market_statistics_error.clone(),
                published.ready,
            )
        };
        let history_ids = self.market_view.history_id.iter().cloned().collect();
        let query = self
            .manager_search
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_default();
        let rows = state
            .catalog
            .as_ref()
            .map(|catalog| {
                catalog.search_with(
                    &query,
                    &self.market_view.category,
                    &self.market_view.tag,
                    self.market_view.sort,
                    statistics.as_deref(),
                    &history_ids,
                )
            })
            .unwrap_or_default();
        let selection = self
            .manager_selected
            .as_ref()
            .filter(|id| rows.iter().any(|r| &r.id == *id))
            .cloned()
            .or_else(|| rows.first().map(|r| r.id.clone()));
        self.manager_selected = selection.clone();
        let selected = rows
            .iter()
            .find(|r| Some(&r.id) == selection.as_ref())
            .copied();
        let mut list = v_flex().gap_1();
        for row in rows {
            let id = row.id.clone();
            list = list.child(
                Button::new(SharedString::from(format!("market-row-{id}")))
                    .label(row.versions[0].manifest.name.clone())
                    .w_full()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        // Selecting another row returns to ordinary discovery rather than retaining a withdrawal exception.
                        if this.manager_selected.as_ref() != Some(&id) {
                            this.market_view.leave_history();
                        }
                        this.manager_selected = Some(id.clone());
                        this.market_view.selected_version = None;
                        this.manager_detail_scroll.set_offset(Default::default());
                        cx.notify();
                    })),
            );
        }
        let categories = state
            .catalog
            .as_ref()
            .map(|c| {
                c.plugins
                    .iter()
                    .map(|p| p.category.clone())
                    .collect::<std::collections::BTreeSet<_>>()
            })
            .unwrap_or_default();
        let tags = state
            .catalog
            .as_ref()
            .map(|c| {
                c.plugins
                    .iter()
                    .flat_map(|p| p.tags.iter().cloned())
                    .collect::<std::collections::BTreeSet<_>>()
            })
            .unwrap_or_default();
        let category_label = if self.market_view.category.is_empty() {
            t!("plugins.market_all_categories").to_string()
        } else {
            self.market_view.category.clone()
        };
        let tag_label = if self.market_view.tag.is_empty() {
            t!("plugins.market_all_tags").to_string()
        } else {
            self.market_view.tag.clone()
        };
        let mut sidebar = v_flex()
            .w(px(258.))
            .h_full()
            .gap_2()
            .p_3()
            .bg(cx.theme().sidebar)
            .child(
                Button::new("market-installed")
                    .label(t!("plugins.installed").to_string())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.manager_market = false;
                        this.market_view.leave_history();
                        this.manager_selected = None;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .debug_selector(|| "market-search".into())
                    .child(Input::new(self.manager_search.as_ref().unwrap())),
            )
            .child(
                Button::new("market-category")
                    .label(category_label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.market_view.leave_history();
                        this.market_view.category =
                            next_filter(&this.market_view.category, &categories);
                        this.manager_selected = None;
                        cx.notify();
                    })),
            )
            .child(
                Button::new("market-tag")
                    .label(tag_label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.market_view.leave_history();
                        this.market_view.tag = next_filter(&this.market_view.tag, &tags);
                        this.manager_selected = None;
                        cx.notify();
                    })),
            );
        let sorts = [Sort::Relevance, Sort::Downloads, Sort::Updated, Sort::Name];
        sidebar = sidebar
            .child(crate::ui::controls::tab_strip(
                "market-sort",
                sorts
                    .iter()
                    .position(|s| *s == self.market_view.sort)
                    .unwrap_or_default(),
                [
                    (t!("plugins.market_sort_relevance").into(), false),
                    (t!("plugins.market_sort_downloads").into(), false),
                    (t!("plugins.market_sort_updated").into(), false),
                    (t!("plugins.market_sort_name").into(), false),
                ],
                [None; 4],
                &self.manager_tabs_focus,
                {
                    let owner = cx.entity().downgrade();
                    move |index, _, cx| {
                        let _ = owner.update(cx, |this, cx| {
                            this.market_view.leave_history();
                            this.market_view.sort = sorts[index];
                            cx.notify();
                        });
                    }
                },
                cx,
            ))
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scrollbar()
                    .child(list),
            )
            .child(
                Button::new("market-refresh")
                    .label(t!("plugins.market_refresh").to_string())
                    .disabled(state.refreshing)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.refresh_online_market();
                        cx.notify();
                    })),
            )
            .child(
                Button::new("market-local")
                    .label(t!("plugins.install_local").to_string())
                    .disabled(self.progress.is_some())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.manager_market = false;
                        this.choose_package(cx);
                    })),
            );
        let mut body = v_flex().gap_3().p_5();
        if let Some(at) = state
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
        if !state.fresh() {
            body = body.child(div().text_color(cx.theme().muted_foreground).child(
                if state.refreshing {
                    t!("plugins.market_refreshing").to_string()
                } else {
                    t!(
                        "plugins.market_offline",
                        error = state.error.as_deref().unwrap_or("")
                    )
                    .to_string()
                },
            ));
        }
        if let Some(statistics) = &statistics {
            if let Some(at) = &statistics.measured_at {
                body =
                    body.child(t!("plugins.market_statistics_at", time = at.clone()).to_string());
            }
        }
        if let Some(error) = &statistics_error {
            body = body
                .child(t!("plugins.market_statistics_stale", error = error.clone()).to_string());
        }
        if let Some(status) = &self.status {
            body = body.child(
                div()
                    .text_color(cx.theme().danger)
                    .child(status.message.clone()),
            );
        }
        if let Some(listing) = selected {
            body = body.child(self.market_listing_details(
                listing,
                &state,
                statistics.as_deref(),
                ready,
                cx,
            ));
        } else {
            body = body.child(t!("plugins.empty_market").to_string());
        }
        h_flex()
            .size_full()
            .child(sidebar)
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_w(px(0.))
                    .h_full()
                    .child(
                        div()
                            .id("online-market-details")
                            .debug_selector(|| "online-market-details".into())
                            .size_full()
                            .overflow_y_scroll()
                            .track_scroll(&self.manager_detail_scroll)
                            .child(body),
                    )
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .child(vertical_viewport_scrollbar(&self.manager_detail_scroll, cx)),
                    ),
            )
            .into_any_element()
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

/// Cycling includes the unfiltered choice and remains operable with the local button keyboard behavior.
fn next_filter(current: &str, values: &std::collections::BTreeSet<String>) -> String {
    values
        .iter()
        .find(|value| value.as_str() > current)
        .cloned()
        .unwrap_or_default()
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
