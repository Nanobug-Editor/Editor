//! Unified receipts retain source attribution while the original runtime log owns its diagnostic history.

use crate::*;

// The same severity drives host publication and shared native message presentation.
pub(crate) use editor_ui::message_panel::MessageLevel;
use plugin_runtime::logs::{LogLevel, LogRecord};

/// At most three automatic alerts remain visible; older content stays in the bounded message history.
const ALERT_LIMIT: usize = 3;
/// An accepted explicit notification already owns its native card and request completion.
const NOTIFICATION_SOURCE: &str = "host.notification";

/// Per-window delivery never shares cursors or native cards across workspace windows.
#[derive(Default)]
pub(crate) struct MessageDelivery {
    /// Global receipt boundary advances even when only another workspace produced messages.
    through: u64,
    /// Only visible automatic alerts are deduplicated; history retains each distinct receipt.
    alerts: std::collections::VecDeque<Alert>,
}

/// Exact origin, level and text identify a still-visible alert without parsing translated messages.
struct Alert {
    level: MessageLevel,
    title: String,
    message: String,
    view: Entity<editor_ui::Notification>,
}

impl MessageDelivery {
    /// Return the surviving native cards in arrival order; painting never imports or acknowledges logs.
    pub(crate) fn notifications<'a>(
        &'a self,
        cx: &'a App,
    ) -> impl Iterator<Item = Entity<editor_ui::Notification>> + 'a {
        self.alerts
            .iter()
            .filter(|alert| !alert.view.read(cx).is_dismissed())
            .map(|alert| alert.view.clone())
    }

    /// Bound and deduplicate automatic alerts from the supplied immutable receipt.
    fn push(&mut self, level: MessageLevel, title: String, message: String, cx: &mut App) {
        self.alerts
            .retain(|alert| !alert.view.read(cx).is_dismissed());
        if self
            .alerts
            .iter()
            .any(|alert| alert.level == level && alert.title == title && alert.message == message)
        {
            return;
        }
        let view =
            cx.new(|_| editor_ui::Notification::alert(level, title.clone(), message.clone()));
        self.alerts.push_back(Alert {
            level,
            title,
            message,
            view,
        });
        while self.alerts.len() > ALERT_LIMIT {
            self.alerts.pop_front();
        }
    }
}

impl EditorApp {
    /// Retain an important host result and surface warning/error receipts without taking focus.
    /// Routine file, tree, theme and configuration successes do not call this publication boundary.
    pub(crate) fn record_host_message(
        &mut self,
        level: MessageLevel,
        message: impl Into<String>,
        cx: &mut Context<Self>,
    ) {
        self.record_message(level, message.into(), None, true, cx);
    }

    /// Publish one receipt; explicit plugin notices already have a request-owned floating card.
    fn record_message(
        &mut self,
        level: MessageLevel,
        message: String,
        source: Option<String>,
        automatic_alert: bool,
        cx: &mut Context<Self>,
    ) {
        self.messages.update(cx, |panel, cx| {
            panel.push_attributed(level, message.clone(), source.clone(), cx)
        });
        if automatic_alert && level > MessageLevel::Info {
            let title = source.unwrap_or_else(|| {
                t!(if level == MessageLevel::Error {
                    "messages.error"
                } else {
                    "messages.warning"
                })
                .to_string()
            });
            self.message_delivery.push(level, title, message, cx);
        }
        cx.notify();
    }

    /// Consume runtime receipts only on extension publication, independently of rendering and log reads.
    pub(crate) fn sync_plugin_messages(&mut self, cx: &mut Context<Self>) {
        let logs = self.extensions.read(cx).runtime_logs();
        let (through, records) = logs.records_since(
            self.message_delivery.through,
            &self.workspace.root().to_string_lossy(),
        );
        self.message_delivery.through = through;
        for record in records {
            self.record_plugin_message(record, cx);
        }
    }

    /// Capture plugin identity and transport origin before removal can change installed metadata.
    fn record_plugin_message(&mut self, record: LogRecord, cx: &mut Context<Self>) {
        let level = match record.level {
            LogLevel::Info => MessageLevel::Info,
            LogLevel::Warning => MessageLevel::Warning,
            LogLevel::Error => MessageLevel::Error,
        };
        let automatic_alert = record.source != NOTIFICATION_SOURCE;
        self.record_message(
            level,
            record.message,
            Some(format!("{} · {}", record.plugin, record.source)),
            automatic_alert,
            cx,
        );
    }

    /// Retain an admitted public message. Notify has a request-owned card; PublishMessage uses host alerts.
    pub(crate) fn record_plugin_notice(
        &mut self,
        plugin: &str,
        operation: &plugin_runtime::plugin_protocol::interaction::Operation,
        cx: &mut Context<Self>,
    ) {
        use plugin_runtime::plugin_protocol::interaction::{Operation, Severity};
        let (title, message, severity, source) = match operation {
            Operation::Notify {
                title,
                message,
                severity,
            } => (title, message, severity, NOTIFICATION_SOURCE),
            Operation::PublishMessage {
                title,
                message,
                severity,
            } => (title, message, severity, "host.message"),
            _ => return,
        };
        let level = match severity {
            Severity::Information => LogLevel::Info,
            Severity::Warning => LogLevel::Warning,
            Severity::Error => LogLevel::Error,
        };
        self.extensions
            .read(cx)
            .runtime_logs()
            .for_workspace(&self.workspace.root().to_string_lossy())
            .append(plugin, level, source, format!("{title}\n{message}"));
        // The existing observer imports on the next effect, outside the request's render dispatch.
        self.extensions.update(cx, |_, cx| cx.notify());
    }

    /// Publish an important result to bounded history while keeping the existing internal operation status.
    pub(crate) fn report_host_message(
        &mut self,
        level: MessageLevel,
        message: impl Into<String>,
        cx: &mut Context<Self>,
    ) {
        self.status = message.into();
        self.record_host_message(level, self.status.clone(), cx);
    }

    /// A closed dock region hides its leaves even when their own visibility flag remains enabled.
    pub(crate) fn messages_visible(&self, cx: &App) -> bool {
        let id = gpui_base::dock::PanelId::from(self.messages.entity_id());
        local_dock::panel_visible(self.dock_area.read(cx), id, cx)
    }

    /// Toggle only this host panel; other panels sharing a dock retain their own visibility.
    pub(crate) fn toggle_messages(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let visible = !self.messages_visible(cx);
        if visible {
            self.dock_presentation
                .activate(gpui_base::dock::PanelId::from(self.messages.entity_id()));
        }
        self.messages.update(cx, |panel, cx| {
            // Acknowledge before reopening, so a later receipt survives every deferred layout notification.
            if visible {
                panel.acknowledge(cx);
            }
            panel.set_visible(visible, cx);
        });
        if self.messages.read(cx).is_visible() {
            let id = gpui_base::dock::PanelId::from(self.messages.entity_id());
            self.dock_area.update(cx, |area, cx| {
                local_dock::reveal_panel(area, id, window, cx);
                cx.notify();
            });
        }
    }
}
