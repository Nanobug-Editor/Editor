//! Shares runtime-log reminders with a fixed summary snapshot beside the status bar.

use crate::extensions::{level_label, log_time, severity_icon};
use crate::*;
use editor_ui::{Spinner, StatusIcon};
use gpui_base::PopoverState;
use plugin_runtime::logs::{LogLevel, LogRecord};
use std::collections::BTreeMap;
mod loading;
mod status_popup;

/// Independent plugin and external-startup entries share one native status popover lifecycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PluginPopupKind {
    /// Plugin restoration and contributed syntax resources are still being loaded.
    Loading,
    /// External language or project engines have not completed preparation and initialization.
    Services,
    /// The plugin log contains an unconfirmed warning reminder.
    Warning,
    /// The plugin log or package manager contains an unconfirmed error reminder.
    Error,
}

/// Captured records never change while open; the base state owns focus and dismissal resources.
pub(crate) struct PluginPopupSnapshot {
    summaries: Vec<PluginSummary>,
    /// Store failures without a plugin owner alongside the captured log rows.
    manager_error: Option<String>,
    state: Entity<PopoverState>,
    scroll: ScrollHandle,
    /// A fresh identity rejects paint callbacks from a closed or replaced popup.
    token: Rc<()>,
    /// The original trigger can disappear and return with the same kind but a different Base focus identity.
    trigger: PluginPopupKind,
    trigger_removed: bool,
    /// Focus observers live only as long as their currently routable rows and this popup.
    row_focus: BTreeMap<String, PluginPopupRowFocus>,
}

/// The current layout index changes with loading tasks; the handle remains stable across repaints.
struct PluginPopupRowFocus {
    handle: FocusHandle,
    index: usize,
    _subscription: Subscription,
}

/// Store the installed display name with the event so uninstall cannot rewrite an open summary.
struct PluginSummary {
    name: String,
    record: LogRecord,
}

/// A live loading row retains its optional owner, visible name and current preparation details.
struct PluginLoadingSummary {
    plugin: Option<String>,
    name: String,
    details: Vec<String>,
}

impl EditorApp {
    /// Persist authority outside the repository and immediately withdraw all workspace presentation.
    pub(crate) fn set_workspace_trusted(&mut self, trusted: bool, cx: &mut Context<Self>) {
        self.session_state.workspace_trusted = trusted;
        if !trusted {
            self.plugin_command_intents.revoke();
        }
        self.persist_session();
        self.extensions
            .update(cx, |panel, cx| panel.set_workspace_trusted(trusted, cx));
        // Native processes have the same workspace authority and must be revoked immediately.
        self.terminal
            .update(cx, |panel, cx| panel.set_trusted(trusted, cx));
        // The editor window (not the settings dialog) owns dock and editor synchronization.
        self.pending_contribution_sync = true;
        self.refresh_dialog(cx);
        cx.notify();
    }

    /// Reconcile every installed declaration through the same generation-checked provider registry.
    pub(crate) fn sync_runtime_contributions(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        apply_theme(&theme::active_theme(self.dark_theme), cx);
        self.sync_dynamic_languages(cx);
        self.reset_syntax_diagnostics(cx);
        cx.notify();
    }

    /// Counts current reminders, rather than repeatedly advertising already-confirmed failures.
    pub(crate) fn plugin_count(&self, kind: PluginPopupKind, cx: &App) -> usize {
        match kind {
            PluginPopupKind::Loading => self.plugin_loading_groups(cx).len(),
            PluginPopupKind::Services => self.external_service_loading_groups(cx).len(),
            PluginPopupKind::Warning | PluginPopupKind::Error => {
                self.extensions
                    .read(cx)
                    .runtime_logs()
                    .pending_reminders()
                    .iter()
                    .filter(|(_, level)| {
                        *level
                            == if kind == PluginPopupKind::Error {
                                LogLevel::Error
                            } else {
                                LogLevel::Warning
                            }
                    })
                    .count()
                    + usize::from(
                        kind == PluginPopupKind::Error && self.unowned_manager_error(cx).is_some(),
                    )
            }
        }
    }

    /// Recovery can fail before any registry entry exists, so no plugin log can own that failure.
    fn unowned_manager_error<'a>(&self, cx: &'a App) -> Option<&'a str> {
        self.extensions.read(cx).unowned_manager_error()
    }

    /// A single entry guarantees anomalies are never hidden by simultaneous normal loading.
    pub(crate) fn plugin_indicator(&self, cx: &App) -> Option<PluginPopupKind> {
        let severity = self
            .extensions
            .read(cx)
            .runtime_logs()
            .pending_reminders()
            .into_iter()
            .map(|(_, level)| level)
            .max();
        match severity {
            _ if self.unowned_manager_error(cx).is_some() => Some(PluginPopupKind::Error),
            Some(LogLevel::Error) => Some(PluginPopupKind::Error),
            Some(LogLevel::Warning) => Some(PluginPopupKind::Warning),
            _ if self.plugin_count(PluginPopupKind::Loading, cx) > 0 => {
                Some(PluginPopupKind::Loading)
            }
            _ => None,
        }
    }

    /// Remember actual rendered absence, even when a later arrival recreates an entry of the same severity.
    pub(crate) fn track_plugin_indicator_frame(
        &mut self,
        indicator: Option<PluginPopupKind>,
        services_loading: bool,
    ) {
        if let Some(snapshot) = &mut self.plugin_popup_snapshot {
            snapshot.trigger_removed |= if snapshot.trigger == PluginPopupKind::Services {
                !services_loading
            } else {
                indicator != Some(snapshot.trigger)
            };
        }
    }

    /// Capture records and their confirmation boundary atomically so concurrent arrivals stay outside this viewing round.
    fn toggle_plugin_popup(
        &mut self,
        kind: PluginPopupKind,
        event: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.stop_propagation();
        if self.plugin_popup.is_some() {
            self.close_plugin_popup(window, cx);
            return;
        }
        let panel = self.extensions.read(cx);
        let services = kind == PluginPopupKind::Services;
        let manager_error = if services {
            None
        } else {
            let error = self.unowned_manager_error(cx).map(str::to_owned);
            panel.confirm_unowned_manager_error();
            error
        };
        let logs = panel.runtime_logs();
        // Inspecting external startup must not confirm or read an unrelated plugin warning.
        let (checkpoint, records) = if services {
            (None, Vec::new())
        } else {
            let (checkpoint, records) = logs.reminder_snapshot();
            (Some(checkpoint), records)
        };
        let summaries: Vec<_> = records
            .into_iter()
            .map(|record| {
                let name = panel
                    .entries
                    .iter()
                    .find(|entry| entry.manifest.id == record.plugin)
                    .map_or_else(
                        || record.plugin.clone(),
                        |entry| entry.manifest.name.clone(),
                    );
                PluginSummary { name, record }
            })
            .collect();
        let token = Rc::new(());
        let state = cx.new(|cx| PopoverState::new(false, cx));
        let parent = cx.entity().downgrade();
        let captured = token.clone();
        state.update(cx, |state, _| {
            state.set_on_open_change(Some(Rc::new(move |open, _, cx| {
                if !*open {
                    let parent = parent.clone();
                    let captured = captured.clone();
                    cx.defer(move |cx| {
                        let _ = parent.update(cx, |app, cx| {
                            if app
                                .plugin_popup_snapshot
                                .as_ref()
                                .is_some_and(|snapshot| Rc::ptr_eq(&snapshot.token, &captured))
                            {
                                app.plugin_popup = None;
                                app.plugin_popup_snapshot = None;
                                cx.notify();
                            }
                        });
                    });
                }
            })));
        });
        // Existing completion callbacks may close a pure loading list. An anomaly snapshot remains reviewable.
        let trigger = kind;
        let kind = if manager_error.is_some() {
            // A stale loading trigger must not let normal completion close a captured recovery error.
            PluginPopupKind::Error
        } else if kind == PluginPopupKind::Loading && !summaries.is_empty() {
            if summaries
                .iter()
                .any(|summary| summary.record.level == LogLevel::Error)
            {
                PluginPopupKind::Error
            } else {
                PluginPopupKind::Warning
            }
        } else {
            kind
        };
        self.plugin_popup = Some((kind, event.position()));
        self.plugin_popup_snapshot = Some(PluginPopupSnapshot {
            summaries,
            manager_error,
            state: state.clone(),
            scroll: ScrollHandle::new(),
            token,
            trigger,
            trigger_removed: false,
            row_focus: BTreeMap::new(),
        });
        if let Some(checkpoint) = checkpoint {
            logs.confirm_reminders(&checkpoint);
        }
        state.update(cx, |state, cx| state.show(window, cx));
        cx.notify();
    }

    /// Base popover dismissal restores focus and releases its deferred-popup registration; logs stay intact.
    pub(crate) fn close_plugin_popup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.plugin_popup = None;
        if let Some(snapshot) = self.plugin_popup_snapshot.take() {
            let focused = snapshot
                .state
                .read(cx)
                .focus_handle(cx)
                .contains_focused(window, cx);
            snapshot
                .state
                .update(cx, |state, cx| state.dismiss(window, cx));
            // A matching new kind cannot revive a trigger removed in an earlier frame.
            let trigger_present = if snapshot.trigger == PluginPopupKind::Services {
                self.plugin_count(PluginPopupKind::Services, cx) > 0
            } else {
                self.plugin_indicator(cx) == Some(snapshot.trigger)
            };
            if focused && (snapshot.trigger_removed || !trigger_present) {
                self.editor
                    .update(cx, |editor, cx| editor.focus(window, cx));
            }
        }
        cx.notify();
    }
}

#[cfg(test)]
mod status_tests;
#[cfg(test)]
mod tests;
