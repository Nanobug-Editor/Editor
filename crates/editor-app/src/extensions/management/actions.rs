//! The same lifecycle controls manage installed instances in either discovery mode.
use super::*;

impl ExtensionPanel {
    /// Market release authority affects install/update only; every lifecycle action uses the local ID.
    pub(super) fn manager_actions(
        &self,
        selection: &Selection,
        snapshot: &marketplace::Snapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected_entry = selection.installed.as_ref();
        let selected_package = selection.package.as_ref();
        let manifest = selection.manifest(self.manager_market).unwrap();
        let id = manifest.id.clone();
        let selected_id = Some(id.clone());
        let selected_global =
            selected_entry.map(|entry| entry.global_enabled.unwrap_or(entry.enabled));
        let busy = self.progress.is_some();
        let market_release = self
            .manager_market
            .then_some(selection.release.as_ref())
            .flatten();
        // Installed preferences survive the protocol transition, but cannot authorize an old component.
        let incompatibility = selected_entry.and_then(Installed::compatibility_error);
        let incompatible = incompatibility.is_some();
        let installed_version = selected_entry.map(|entry| entry.manifest.version.as_str());
        let package_path = selected_package
            .and_then(|package| package.source.as_ref())
            .map(PathBuf::from);
        // Installation and updates have distinct controls; older packages expose no downgrade action.
        let can_install = market_release.is_none() && selected_entry.is_none();
        let can_update = market_release.is_none()
            && selected_package.is_some_and(|package| {
                package_action(&package.manifest.version, installed_version) == "更新"
                    || (incompatible
                        && selected_entry.is_some_and(|entry| entry.digest != package.digest))
            });
        let global_enabled =
            selected_entry.is_some_and(|entry| entry.global_enabled.unwrap_or(entry.enabled));
        let project_enabled = selected_entry
            .is_some_and(|entry| entry.project_enabled_in(&self.workspace.to_string_lossy()));
        let uninstall_id = id.clone();
        // Package inspection belongs to install/update; removal has its own loading state.
        let install_loading = self.progress.as_ref().is_some_and(|progress| {
            progress.action == LifecycleAction::Inspect
                || (progress.action == LifecycleAction::Install && progress.id == id)
        });
        let uninstall_loading = self.progress.as_ref().is_some_and(|progress| {
            progress.action == LifecycleAction::Uninstall && progress.id == id
        });
        let project_id = id.clone();
        let project_owner = cx.entity().downgrade();

        h_flex()
            .gap_2()
            // Preserve action order when the window narrows or interface text grows.
            .flex_wrap()
            .items_center()
            .when_some(market_release, |row, release| {
                row.child(self.market_install_action(release, snapshot, cx))
            })
            .when(can_install, |row| {
                let package_path = package_path.clone();
                row.child(
                    div()
                        .id("plugin-install-action-region")
                        .debug_selector(|| "plugin-install-action-region".into())
                        .child(
                            Button::new("plugin-install-action")
                                .label(t!("plugins.install").to_string())
                                .primary()
                                .outline()
                                .loading(install_loading)
                                .disabled(busy || package_path.is_none())
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if let Some(path) = package_path.clone() {
                                        this.queue_lifecycle(Work::Inspect(path));
                                    }
                                    cx.notify();
                                })),
                        ),
                )
            })
            .when(can_update, |row| {
                let package_path = package_path.clone();
                row.child(
                    div()
                        .id("plugin-update-action-region")
                        .debug_selector(|| "plugin-update-action-region".into())
                        .child(
                            Button::new("plugin-update-action")
                                .label(t!("plugins.update").to_string())
                                .custom(update_button_style(cx))
                                .outline()
                                .loading(install_loading)
                                .disabled(busy || package_path.is_none())
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if let Some(path) = package_path.clone() {
                                        this.queue_lifecycle(Work::Inspect(path));
                                    }
                                    cx.notify();
                                })),
                        ),
                )
            })
            .when(selected_entry.is_some(), |row| {
                row.child(
                    div()
                        .id("plugin-uninstall-action-region")
                        .debug_selector(|| "plugin-uninstall-action-region".into())
                        .child(
                            Button::new("plugin-uninstall-action")
                                .label(t!("plugins.uninstall").to_string())
                                .custom(uninstall_button_style(cx))
                                .outline()
                                .loading(uninstall_loading)
                                .disabled(busy)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.confirm = Some((uninstall_id.clone(), true));
                                    this.confirm_dialog_open = false;
                                    cx.notify();
                                })),
                        ),
                )
            })
            .when(selected_entry.is_some() && !incompatible, |row| {
                row.child(self.restart_action(&id, busy, cx))
            })
            .when_some(
                selected_id.clone().zip(selected_global),
                |row, (id, enabled)| {
                    let owner = cx.entity().downgrade();
                    row.child(
                        div()
                            .id("plugin-global-scope-region")
                            .debug_selector(|| "plugin-global-scope-region".into())
                            // Equal options grow with the same interface font used by Root.
                            .w(px(176.) * (cx.theme().font_size / px(14.)))
                            .flex_shrink_0()
                            .child(
                                SegmentedTabs::new("plugin-global-scope")
                                    .selected_index(usize::from(!enabled))
                                    .labels([
                                        t!("plugins.global_enabled").to_string(),
                                        t!("plugins.global_disabled").to_string(),
                                    ])
                                    .disabled(busy || incompatible)
                                    .on_change(move |index, _, cx| {
                                        let enable = index == 0;
                                        if busy || incompatible || enable == enabled {
                                            return;
                                        }
                                        let _ = owner.update(cx, |this, cx| {
                                            this.queue_lifecycle(if enable {
                                                Work::Enable(id.clone())
                                            } else {
                                                Work::Disable(id.clone())
                                            });
                                            cx.notify();
                                        });
                                    }),
                            ),
                    )
                },
            )
            // Project overrides follow the global choice in the same row.
            .when(selected_entry.is_some() && !global_enabled, |row| {
                row.child(
                    div()
                        .id("plugin-project-scope-region")
                        .debug_selector(|| "plugin-project-scope-region".into())
                        .child(
                            Checkbox::new("plugin-project-enabled")
                                .label(t!("plugins.project_enabled").to_string())
                                .checked(project_enabled)
                                .disabled(busy || incompatible)
                                .on_change(move |checked, _, cx| {
                                    if busy || incompatible {
                                        return;
                                    }
                                    let checked = *checked;
                                    let _ = project_owner.update(cx, |this, cx| {
                                        let _ = this.worker.tx.send(Work::SetProjectEnabled(
                                            project_id.clone(),
                                            checked,
                                        ));
                                        cx.notify();
                                    });
                                }),
                        ),
                )
            })
            .into_any_element()
    }
}
