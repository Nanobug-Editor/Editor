//! Observe displayed native surfaces without converting a visibility change into plugin demand.
use super::*;
use gpui_base::dock::{DockArea, DockPlacement, PaneRef, PanelId};

/// Persisted show/hide intent, actual display and each live guest's last receipt are separate facts.
#[derive(Default)]
pub(super) struct PanelVisibility {
    pub(super) owner_window: Option<gpui_kit::AnyWindowHandle>,
    pub(super) displayed: bool,
    reported: Option<(u64, bool)>,
}

impl PanelVisibility {
    /// A withdrawn owner forgets its receipt; the next incarnation receives an initial state.
    fn change(&mut self, epoch: Option<u64>, visible: bool) -> Option<(u64, bool)> {
        let Some(epoch) = epoch else {
            self.reported = None;
            return None;
        };
        let next = (epoch, visible);
        if self.reported == Some(next) {
            return None;
        }
        self.reported = Some(next);
        Some(next)
    }
}

/// Read Base's current layout, including collapsed regions, zoom and the hidden-tab fallback.
fn dock_panel_displayed(area: &DockArea, panel: PanelId, cx: &App) -> bool {
    for placement in [
        DockPlacement::Center,
        DockPlacement::Left,
        DockPlacement::Right,
        DockPlacement::Bottom,
    ] {
        let Some(tree) = area.layout(placement) else {
            continue;
        };
        let Some(node) = tree.find_panel_node(panel) else {
            continue;
        };
        if (placement != DockPlacement::Center && !area.is_dock_open(placement))
            || area.zoomed_group().is_some_and(|zoomed| zoomed != node)
        {
            return false;
        }
        let Some(PaneRef::Tabs { panels, active_ix }) =
            tree.find_node(node).map(|node| node.kind())
        else {
            return false;
        };
        let visible = |id: &PanelId| area.panel(*id).is_some_and(|panel| panel.visible(cx));
        let active = panels
            .get(active_ix)
            .filter(|id| visible(id))
            .or_else(|| panels.iter().find(|id| visible(id)));
        return active == Some(&panel);
    }
    false
}

impl ExtensionPanel {
    /// Sampling sends only an opted-in live incarnation's state; it never uses view-demand admission.
    pub(super) fn publish_panel_visibility(&mut self) {
        let Some(plugin) = self.active.as_ref() else {
            return;
        };
        let Some(panel) = self.surface_id.as_ref() else {
            return;
        };
        let epoch = {
            let state = self.worker.state.lock().unwrap();
            let allowed = self
                .worker
                .trusted
                .load(std::sync::atomic::Ordering::Acquire)
                && state.entries.iter().any(|entry| {
                    entry.manifest.id == *plugin
                        && entry.enabled
                        && entry.error.is_none()
                        && entry.grants.contains("ui.panels")
                        && entry.supports_capability("ui.visibility")
                        && entry.manifest.panels.iter().any(|item| item.id == *panel)
                });
            allowed.then(|| state.live_instance_epoch(plugin)).flatten()
        };
        let visible = self.visible.get() && self.panel_visibility.displayed;
        let Some((epoch, visible)) = self.panel_visibility.change(epoch, visible) else {
            return;
        };
        let _ = self.worker.tx.send(Work::Event(
            plugin.clone(),
            epoch,
            Some(panel.clone()),
            PluginEvent::PanelVisibility {
                panel: panel.clone(),
                visible,
            },
        ));
    }
}

impl EditorApp {
    /// Read existing windows and native layout only. Hidden windows remain sampled by the manager's
    /// existing UI publication poll, without requesting a frame or waking a cold plugin.
    pub(super) fn sync_plugin_panel_visibility(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.extensions.update(cx, |owner, _| {
            owner.panel_visibility.owner_window = Some(window.window_handle());
        });
        let selected = self.active_editor_preview(cx);
        let snapshots = self
            .plugin_panels
            .iter()
            .map(|(key, panel)| {
                let (shown, work_window, editor_preview) = {
                    let state = panel.read(cx);
                    (state.visible.get(), state.work_window, state.editor_preview)
                };
                let displayed = shown
                    && if work_window {
                        self.plugin_windows.get(key).is_some_and(|handle| {
                            handle
                                .update(cx, |_, window, _| window.is_visible())
                                .unwrap_or(false)
                        })
                    } else if editor_preview {
                        window.is_visible()
                            && self.file_panel_is_active(panel, &selected, cx)
                            && dock_panel_displayed(
                                self.dock_area.read(cx),
                                PanelId::from(self.editor_panel.entity_id()),
                                cx,
                            )
                    } else {
                        window.is_visible()
                            && dock_panel_displayed(
                                self.dock_area.read(cx),
                                PanelId::from(panel.entity_id()),
                                cx,
                            )
                    };
                (panel.clone(), displayed)
            })
            .collect::<Vec<_>>();
        for (panel, displayed) in snapshots {
            panel.update(cx, |panel, _| {
                panel.panel_visibility.displayed = displayed;
                panel.publish_panel_visibility();
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Initial false, both edges and same-value replay after a restart are distinct receipts.
    #[test]
    fn panel_visibility_receipts_follow_live_epochs_and_do_not_wake_withdrawn_owners() {
        let mut visibility = PanelVisibility::default();
        assert_eq!(visibility.change(None, true), None);
        assert_eq!(visibility.change(Some(1), false), Some((1, false)));
        assert_eq!(visibility.change(Some(1), false), None);
        assert_eq!(visibility.change(Some(1), true), Some((1, true)));
        assert_eq!(visibility.change(Some(2), true), Some((2, true)));
        assert_eq!(visibility.change(None, false), None);
        assert_eq!(visibility.change(Some(3), false), Some((3, false)));
    }
}
