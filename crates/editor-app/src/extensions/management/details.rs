//! Fixed manager headers and data-aware detail pages for both local and reviewed market versions.
use super::*;

impl ExtensionPanel {
    /// Keep actions and tab navigation outside the single scrolling content viewport.
    pub(super) fn manager_details(
        &mut self,
        selection: &Selection,
        snapshot: &marketplace::Snapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut detail = v_flex()
            .id("plugin-manager-detail")
            .debug_selector(|| "plugin-manager-detail".into())
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .min_h(px(0.))
            .overflow_hidden()
            .bg(cx.theme().background);
        // Failed first installs have no installed log owner; market action failures stay visible here.
        if let Some(status) = self
            .status
            .as_ref()
            .filter(|status| status.plugin.is_none() || self.manager_market)
        {
            detail = detail.child(
                div()
                    .debug_selector(|| "plugin-manager-operation-error".into())
                    .p_3()
                    .flex_shrink_0()
                    .text_color(cx.theme().danger)
                    .child(t!("plugins.manager_error", error = status.message.clone()).to_string()),
            );
        }
        if self.manager_market && !snapshot.state.fresh() {
            detail = detail.child(
                div()
                    .debug_selector(|| "market-catalog-status".into())
                    .px_5()
                    .py_2()
                    .flex_shrink_0()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(if snapshot.state.refreshing {
                        t!("plugins.market_refreshing").to_string()
                    } else {
                        t!(
                            "plugins.market_offline",
                            error = snapshot.state.error.as_deref().unwrap_or("")
                        )
                        .to_string()
                    }),
            );
        }
        let Some(manifest) = selection.manifest(self.manager_market) else {
            self.manager_log_view = None;
            return detail
                .child(div().p_5().text_color(cx.theme().muted_foreground).child(
                    if self.manager_market {
                        t!("plugins.empty_market").to_string()
                    } else {
                        t!("plugins.empty_installed").to_string()
                    },
                ))
                .into_any_element();
        };
        let id = manifest.id.clone();
        let tabs = [
            false,
            selection
                .release
                .as_ref()
                .is_none_or(|release| release.changelog.is_empty()),
            true,
            selection.listing.is_none(),
            false,
            selection.installed.is_none(),
        ];
        // Uninstall and catalog refresh can remove the data required by a previously selected page.
        if tabs[self.manager_detail_tab.index()] {
            self.manager_detail_tab = DetailTab::Overview;
            self.manager_detail_scroll.set_offset(Default::default());
        }
        let mut title = h_flex()
            .gap_3()
            .items_center()
            .flex_wrap()
            .child(div().text_xl().font_semibold().child(manifest.name.clone()))
            .child(
                div()
                    .debug_selector({
                        let version = manifest.version.clone();
                        move || format!("plugin-manager-browse-version-{version}").into()
                    })
                    .text_color(cx.theme().muted_foreground)
                    .child(if self.manager_market {
                        t!(
                            "plugins.market_browsing_version",
                            version = manifest.version.clone()
                        )
                        .to_string()
                    } else {
                        format!("v{}", manifest.version)
                    }),
            );
        if self.manager_market {
            if let Some(entry) = &selection.installed {
                title = title.child(
                    div()
                        .debug_selector({
                            let version = entry.manifest.version.clone();
                            move || format!("plugin-manager-installed-version-{version}").into()
                        })
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(
                            t!(
                                "plugins.market_installed_version",
                                version = entry.manifest.version.clone()
                            )
                            .to_string(),
                        ),
                );
            }
            if let Some(bitmap) = selection.release.as_ref().and_then(|release| {
                self.worker
                    .state
                    .lock()
                    .unwrap()
                    .market_icons
                    .get(&release.sha256)
                    .cloned()
            }) {
                title = title.child(gpui_kit::img(bitmap.image).size(px(24.)));
            }
        }
        detail = detail.child(
            v_flex()
                .id("plugin-manager-header")
                .debug_selector(|| "plugin-manager-header".into())
                .p_5()
                .gap_4()
                .flex_shrink_0()
                .child(title)
                .child(self.manager_actions(selection, snapshot, cx)),
        );
        if let Some(reason) = selection
            .installed
            .as_ref()
            .and_then(Installed::compatibility_error)
        {
            detail = detail.child(
                div()
                    .debug_selector(|| "plugin-incompatible-status".into())
                    .px_5()
                    .py_3()
                    .text_color(cx.theme().danger)
                    .child(t!("plugins.installed_incompatible", reason = reason).to_string()),
            );
        }
        if let Some(release) = &selection.release {
            if release.withdrawn {
                detail = detail.child(
                    div()
                        .debug_selector(|| "plugin-withdrawn-status".into())
                        .px_5()
                        .py_2()
                        .child(
                            t!(
                                "plugins.market_withdrawn",
                                reason = release.withdrawal_reason.as_deref().unwrap_or("")
                            )
                            .to_string(),
                        ),
                );
            }
        }
        let owner = cx.entity().downgrade();
        detail = detail.child(tab_strip(
            "plugin-detail-tabs",
            self.manager_detail_tab.index(),
            [
                (t!("plugins.overview").into(), tabs[0]),
                (t!("plugins.changes").into(), tabs[1]),
                (t!("plugins.reviews").into(), tabs[2]),
                (t!("plugins.versions").into(), tabs[3]),
                (t!("plugins.information").into(), tabs[4]),
                (t!("plugins.runtime_log").into(), tabs[5]),
            ],
            [
                None,
                None,
                None,
                None,
                None,
                selection
                    .installed
                    .as_ref()
                    .and_then(|_| self.log_badge(&id)),
            ],
            &self.manager_detail_focus,
            move |index, _, cx| {
                if let Some(tab) = DetailTab::from_index(index) {
                    let _ = owner.update(cx, |this, cx| {
                        this.manager_detail_tab = tab;
                        this.manager_detail_scroll.set_offset(Default::default());
                        cx.notify();
                    });
                }
            },
            cx,
        ));
        if self.manager_detail_tab != DetailTab::RuntimeLog {
            self.manager_log_view = None;
        }
        let body = match self.manager_detail_tab {
            DetailTab::Overview => self.manager_overview(selection, cx),
            DetailTab::Changes => div()
                .debug_selector(|| "market-changelog-region".into())
                .p_5()
                .child(ui::controls::untrusted_markdown_view(
                    "market-changelog",
                    selection.release.as_ref().unwrap().changelog.clone(),
                    typography::font_size(cx),
                    cx,
                ))
                .into_any_element(),
            DetailTab::Versions => {
                self.market_versions(selection.listing.as_ref().unwrap(), snapshot, cx)
            }
            DetailTab::Information => self.market_information(
                manifest,
                selection.listing.as_ref(),
                selection.release.as_ref(),
                snapshot,
                cx,
            ),
            DetailTab::RuntimeLog => self.runtime_status(&id, cx),
        };
        detail
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h(px(0.))
                    .w_full()
                    .child(
                        div()
                            .id("plugin-detail-scroll")
                            .debug_selector(|| "plugin-manager-detail-content".into())
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

    /// Local installed documentation and inert reviewed Markdown retain their distinct trust boundaries.
    fn manager_overview(&self, selection: &Selection, cx: &Context<Self>) -> AnyElement {
        let mut body = v_flex()
            .id("plugin-readme-region")
            .debug_selector(|| "plugin-readme-region".into())
            .p_5()
            .gap_3();
        if self.manager_market {
            let release = selection.release.as_ref().unwrap();
            if let Some(listing) = &selection.listing {
                body = body.child(listing.summary.clone());
            }
            body.child(ui::controls::untrusted_markdown_view(
                "market-readme",
                release.readme.clone(),
                typography::font_size(cx),
                cx,
            ))
            .into_any_element()
        } else {
            let readme = selection
                .installed
                .as_ref()
                .and_then(|entry| {
                    std::fs::read_to_string(
                        self.root
                            .join("packages")
                            .join(&entry.manifest.id)
                            .join(&entry.digest)
                            .join("README.md"),
                    )
                    .ok()
                })
                .unwrap_or_else(|| t!("plugins.no_readme").to_string());
            body.child(ui::controls::markdown_view(
                "plugin-readme",
                readme,
                typography::font_size(cx),
                cx,
            ))
            .into_any_element()
        }
    }
}
