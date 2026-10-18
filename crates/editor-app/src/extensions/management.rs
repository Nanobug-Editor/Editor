//! Native plugin management is separate from plugin-owned document surfaces and consent overlays.
use super::surface::{package_action, uninstall_button_style, update_button_style};
use plugin_runtime::marketplace::{Listing, Release};
mod actions;
mod details;
use super::*;
use crate::ui::controls::{Checkbox, SegmentedTabs, tab_strip, vertical_viewport_scrollbar};
use gpui_kit::component::scroll::ScrollableElement;

/// Shared detail pages; unavailable data never becomes a stored selection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum DetailTab {
    #[default]
    Overview,
    Changes,
    Versions,
    Information,
    RuntimeLog,
}
impl DetailTab {
    /// Map only supported tab positions; placeholders remain unavailable by keyboard and mouse.
    pub(super) fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::Overview),
            1 => Some(Self::Changes),
            3 => Some(Self::Versions),
            4 => Some(Self::Information),
            5 => Some(Self::RuntimeLog),
            _ => None,
        }
    }
    fn index(self) -> usize {
        match self {
            Self::Overview => 0,
            Self::Changes => 1,
            Self::Versions => 3,
            Self::Information => 4,
            Self::RuntimeLog => 5,
        }
    }
}

/// A render-local selection keeps market versions separate from the installed lifecycle target.
pub(super) struct Selection {
    installed: Option<Installed>,
    package: Option<Package>,
    listing: Option<Listing>,
    release: Option<Release>,
}

impl Selection {
    /// Installed overview uses its own manifest even when a different market release is available.
    fn manifest(&self, market: bool) -> Option<&protocol::Manifest> {
        if market {
            self.release.as_ref().map(|release| &release.manifest)
        } else {
            self.installed.as_ref().map(|entry| &entry.manifest)
        }
    }
}

impl ExtensionPanel {
    /// A profile-only actor cancels installation and acknowledges retirement without closing any project.
    pub(crate) fn shutdown_management(
        &mut self,
        cx: &mut Context<Self>,
    ) -> gpui_kit::Task<Result<(), String>> {
        self.worker.cancel_installation();
        let worker = self.worker.clone();
        let receipt = worker.request_shutdown();
        cx.background_executor().spawn(async move {
            match receipt?.await {
                Ok(result) => result,
                Err(_)
                    if worker
                        .shutdown_complete
                        .load(std::sync::atomic::Ordering::Acquire) =>
                {
                    Ok(())
                }
                Err(error) => Err(error.to_string()),
            }
        })
    }
    /// Global and project roots use the same manager lifecycle and freshness policy.
    pub(crate) fn open_manager(&mut self, cx: &mut Context<Self>) {
        let needs_refresh = {
            let state = self.worker.state.lock().unwrap();
            !state.market.refreshing
                && (!state.market.fresh()
                    || self.market_view.needs_check(std::time::Instant::now()))
        };
        if needs_refresh {
            self.refresh_online_market();
        }
        if !self.manager_open {
            self.manager_detail_tab = DetailTab::Overview;
            self.manager_log_view = None;
            self.manager_detail_scroll.set_offset(Default::default());
        }
        self.manager_open = true;
        cx.notify();
    }
    /// Closing the native manager withdraws only its viewing state, not other project resources.
    pub(crate) fn close_manager(&mut self, cx: &mut Context<Self>) {
        self.manager_open = false;
        self.manager_log_view = None;
        cx.notify();
    }
    /// Install and destructive lifecycle actions have explicit, reviewable native controls.
    pub(super) fn manager(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.manager_search.is_none() {
            // Installed details need available versions before the user visits the market tab.
            if self.manager_packages.is_empty() {
                self.load_market_packages();
            }
            // Keep search input state alive across manager repaints and focus changes.
            let input = cx.new(|cx| {
                InputState::new(window, cx).placeholder(t!("plugins.search").to_string())
            });
            self.manager_search_subscription =
                Some(cx.subscribe(&input, |this, _, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.manager_selected = None;
                        this.market_view.leave_history();
                        this.manager_detail_tab = DetailTab::Overview;
                        this.manager_detail_scroll.set_offset(Default::default());
                        cx.notify();
                    }
                }));
            self.manager_search = Some(input);
        }
        let busy = self.progress.is_some();
        if self.installation.is_some() && !self.installation_dialog_open {
            self.installation_dialog_open = true;
            cx.defer_in(window, |this, window, cx| {
                this.open_installation_progress(window, cx)
            });
        }
        let snapshot = self.market_snapshot();
        if let Some(package) = self.pending.clone().filter(|_| !self.pending_dialog_open) {
            // Defer the overlay until the manager render finishes updating its Root.
            self.pending_dialog_open = true;
            cx.defer_in(window, move |this, window, cx| {
                if this
                    .pending
                    .as_ref()
                    .is_some_and(|pending| pending.digest == package.digest)
                {
                    this.open_install_dialog(package, window, cx);
                }
            });
        }
        if let Some((id, remove)) = self.confirm.clone().filter(|_| !self.confirm_dialog_open) {
            // The confirmation opens after the manager has finished this render pass.
            self.confirm_dialog_open = true;
            cx.defer_in(window, move |this, window, cx| {
                if this.confirm.as_ref() == Some(&(id.clone(), remove)) {
                    // The sessions this plugin is serving are read from the editor, which owns the run
                    // controls, so the user sees what the change would take away before deciding.
                    let extra = this.parent.upgrade().and_then(|parent| {
                        let debug_provider = parent
                            .read(cx)
                            .run_controls
                            .debug_availability()
                            .ok()
                            .map(str::to_owned);
                        let impact = parent
                            .read(cx)
                            .run_controls
                            .plugin_session_impact(&id, debug_provider.as_deref());
                        impact.summary()
                    });
                    this.open_remove_dialog(id, remove, extra, window, cx);
                }
            });
        }
        let query = self
            .manager_search
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_default();
        let choices: Vec<(String, String, String)> = if self.manager_market {
            snapshot
                .state
                .catalog
                .as_ref()
                .map(|catalog| {
                    let history = self.market_view.history_ids();
                    catalog
                        .search_with(
                            &query,
                            self.market_view.category(),
                            self.market_view.tag(),
                            self.market_view.sort(),
                            snapshot.statistics.as_deref(),
                            &history,
                        )
                        .into_iter()
                        .map(|listing| {
                            let release = self.market_release(listing, false);
                            (
                                listing.id.clone(),
                                release.manifest.name.clone(),
                                release.version.clone(),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default()
        } else {
            let query = query.to_lowercase();
            self.entries
                .iter()
                .filter(|entry| {
                    entry.manifest.name.to_lowercase().contains(&query)
                        || entry.manifest.id.to_lowercase().contains(&query)
                })
                .map(|entry| {
                    (
                        entry.manifest.id.clone(),
                        entry.manifest.name.clone(),
                        entry.manifest.version.clone(),
                    )
                })
                .collect()
        };
        let selected_id = self
            .manager_selected
            .as_ref()
            .filter(|id| choices.iter().any(|choice| &choice.0 == *id))
            .cloned()
            .or_else(|| choices.first().map(|choice| choice.0.clone()));
        // Selection identity also gates delayed log acknowledgements after filtering or uninstall.
        if self.manager_selected != selected_id {
            self.manager_detail_tab = DetailTab::Overview;
            self.manager_detail_scroll.set_offset(Default::default());
        }
        self.manager_selected = selected_id.clone();
        let installed = selected_id
            .as_ref()
            .and_then(|id| self.entries.iter().find(|entry| &entry.manifest.id == id))
            .cloned();
        let listing = selected_id
            .as_ref()
            .and_then(|id| {
                snapshot
                    .state
                    .catalog
                    .as_ref()?
                    .plugins
                    .iter()
                    .find(|listing| &listing.id == id)
            })
            .cloned();
        let release = listing.as_ref().and_then(|listing| {
            if self.manager_market {
                Some(self.market_release(listing, true).clone())
            } else {
                // Local details cannot present another version's notes or permission claims as installed data.
                installed
                    .as_ref()
                    .and_then(|entry| {
                        listing
                            .versions
                            .iter()
                            .find(|release| release.version == entry.manifest.version)
                    })
                    .cloned()
            }
        });
        let selection = Selection {
            installed,
            package: selected_id
                .as_ref()
                .and_then(|id| {
                    self.manager_packages
                        .iter()
                        .find(|package| &package.manifest.id == id)
                })
                .cloned(),
            listing,
            release,
        };
        let mut list = v_flex().gap_1();
        for (id, name, version) in &choices {
            let row_id = id.clone();
            let selected = selected_id.as_ref() == Some(id);
            let upgrade = snapshot.state.catalog.as_ref().and_then(|catalog| {
                catalog
                    .plugins
                    .iter()
                    .find(|listing| &listing.id == id)
                    .and_then(|listing| self.market_upgrade(listing))
            });
            list = list.child(
                // Local Base buttons retain pointer, keyboard and accessibility behavior in both lists.
                Button::new(SharedString::from(format!("plugin-row-{id}")))
                    .ghost()
                    .content_full_width()
                    .debug_selector({
                        let id = id.clone();
                        move || format!("plugin-row-{id}").into()
                    })
                    .w_full()
                    .h(px(36.))
                    .px_3()
                    .py_2()
                    .gap_2()
                    .when(selected, |row| {
                        row.custom(
                            crate::ui::controls::ButtonCustomVariant::new(cx)
                                .color(cx.theme().list_active)
                                .foreground(cx.theme().foreground)
                                .hover(cx.theme().list_active)
                                .active(cx.theme().list_active),
                        )
                    })
                    .child(
                        h_flex()
                            .w_full()
                            .gap_2()
                            .items_center()
                            .text_size(cx.theme().font_size)
                            .child(div().flex_1().min_w(px(0.)).truncate().child(name.clone()))
                            .when_some(upgrade, |row, upgrade| {
                                row.child(
                                    div()
                                        .debug_selector({
                                            let id = id.clone();
                                            move || format!("plugin-row-{id}-update")
                                        })
                                        .flex_shrink_0()
                                        .text_xs()
                                        .text_color(cx.theme().primary)
                                        .child(
                                            t!(
                                                "plugins.update_to_version",
                                                version = upgrade.version.clone()
                                            )
                                            .to_string(),
                                        ),
                                )
                            })
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(version.clone()),
                            ),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if this.manager_selected.as_ref() != Some(&row_id) {
                            this.market_view.leave_history();
                        }
                        this.manager_selected = Some(row_id.clone());
                        this.manager_detail_tab = DetailTab::Overview;
                        this.manager_detail_scroll.set_offset(Default::default());
                        this.pending = None;
                        this.confirm = None;
                        this.confirm_dialog_open = false;
                        cx.notify();
                    })),
            );
        }
        let search = if self.manager_market {
            Input::new(self.manager_search.as_ref().unwrap()).prefix(self.market_filters(cx))
        } else {
            Input::new(self.manager_search.as_ref().unwrap())
        };
        let sidebar = v_flex()
            .id("plugin-manager-sidebar")
            .debug_selector(|| "plugin-manager-sidebar".into())
            .w(px(258.))
            .h_full()
            .flex_shrink_0()
            .border_r_1()
            .border_color(cx.theme().sidebar_border)
            .bg(cx.theme().sidebar)
            .child(
                v_flex()
                    .p_3()
                    .gap_2()
                    // Keep a stable hit target for native search and summary-to-log routing tests.
                    .child(
                        div()
                            .debug_selector(|| "plugin-manager-search".into())
                            .w_full()
                            .child(search),
                    )
                    .child(
                        div()
                            .id("plugin-manager-tab-strip")
                            .debug_selector(|| "plugin-manager-tab-strip".into())
                            .child(tab_strip(
                                "plugin-manager-tabs",
                                usize::from(self.manager_market),
                                [
                                    (t!("plugins.installed").into(), false),
                                    (t!("plugins.market").into(), false),
                                ],
                                [None; 2],
                                &self.manager_tabs_focus,
                                {
                                    let owner = cx.entity().downgrade();
                                    move |index, _, cx| {
                                        let _ = owner.update(cx, |this, cx| {
                                            let market = index == 1;
                                            if market && !this.manager_market {
                                                this.load_market_packages();
                                            }
                                            this.manager_market = market;
                                            this.market_view.leave_history();
                                            this.manager_selected = None;
                                            this.manager_detail_tab = DetailTab::Overview;
                                            this.manager_detail_scroll
                                                .set_offset(Default::default());
                                            cx.notify();
                                        });
                                    }
                                },
                                cx,
                            )),
                    ),
            )
            .child(div().flex_1().overflow_y_scrollbar().child(list))
            .when(self.manager_market, |sidebar| {
                sidebar.child(
                    Button::new("market-refresh")
                        .debug_selector(|| "market-refresh".into())
                        .label(t!("plugins.market_refresh").to_string())
                        .loading(snapshot.state.refreshing)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.refresh_online_market();
                            cx.notify();
                        })),
                )
            })
            .when(!busy, |sidebar| {
                sidebar.child(
                    div()
                        .px_2()
                        .py_1()
                        .border_t_1()
                        .border_color(cx.theme().sidebar_border)
                        .child(
                            Button::new("install-local-plugin")
                                .debug_selector(|| "install-local-plugin".into())
                                .label(t!("plugins.install_local").to_string())
                                .disabled(busy)
                                .on_click(cx.listener(|this, _, _, cx| this.choose_package(cx))),
                        ),
                )
            });
        let detail = self.manager_details(&selection, &snapshot, cx);
        h_flex()
            .id("runtime-plugin-manager")
            .debug_selector(|| "runtime-plugin-manager".into())
            .size_full()
            .items_start()
            .child(sidebar)
            .child(detail)
            .into_any_element()
    }
}
