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
    /// Coalesced cards navigate to the latest represented receipt without comparing translated text on click.
    message_id: Rc<std::cell::Cell<u64>>,
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
    fn push(
        &mut self,
        level: MessageLevel,
        title: String,
        message: String,
        id: u64,
        cx: &mut Context<EditorApp>,
    ) {
        self.alerts
            .retain(|alert| !alert.view.read(cx).is_dismissed());
        if let Some(alert) = self
            .alerts
            .iter()
            .find(|alert| alert.level == level && alert.title == title && alert.message == message)
        {
            alert.message_id.set(id);
            return;
        }
        let message_id = Rc::new(std::cell::Cell::new(id));
        let target = message_id.clone();
        let on_click = cx.listener(move |this, _, window, cx| {
            this.reveal_host_message(target.get(), window, cx);
        });
        let view = cx.new(|_| {
            editor_ui::Notification::alert(level, title.clone(), message.clone()).on_click(on_click)
        });
        self.alerts.push_back(Alert {
            level,
            title,
            message,
            view,
            message_id,
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

    /// Publish and return one panel receipt's identity; explicit notices already have a request-owned card.
    fn record_message(
        &mut self,
        level: MessageLevel,
        message: String,
        source: Option<String>,
        automatic_alert: bool,
        cx: &mut Context<Self>,
    ) -> u64 {
        let id = self.messages.update(cx, |panel, cx| {
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
            self.message_delivery.push(level, title, message, id, cx);
        }
        cx.notify();
        id
    }

    /// Consume runtime receipts only on extension publication, independently of rendering and log reads.
    pub(crate) fn sync_plugin_messages(&mut self, cx: &mut Context<Self>) {
        self.import_plugin_messages(None, cx);
    }

    /// Import each runtime receipt once; return the panel identity corresponding to optional runtime `target`.
    fn import_plugin_messages(
        &mut self,
        target: Option<u64>,
        cx: &mut Context<Self>,
    ) -> Option<u64> {
        let logs = self.extensions.read(cx).runtime_logs();
        let (through, records) = logs.records_since(
            self.message_delivery.through,
            &self.workspace.root().to_string_lossy(),
        );
        self.message_delivery.through = through;
        let mut target_id = None;
        for record in records {
            let matched = Some(record.id) == target;
            let id = self.record_plugin_message(record, cx);
            if matched {
                target_id = Some(id);
            }
        }
        target_id
    }

    /// Capture plugin identity and transport origin before removal can change installed metadata.
    fn record_plugin_message(&mut self, record: LogRecord, cx: &mut Context<Self>) -> u64 {
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
        )
    }

    /// Retain an admitted public message and return its panel identity, or None for non-message operations.
    /// Notify has a request-owned card; PublishMessage uses host alerts. Both retain their runtime origin.
    pub(crate) fn record_plugin_notice(
        &mut self,
        plugin: &str,
        operation: &plugin_runtime::plugin_protocol::interaction::Operation,
        cx: &mut Context<Self>,
    ) -> Option<u64> {
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
            _ => return None,
        };
        let level = match severity {
            Severity::Information => LogLevel::Info,
            Severity::Warning => LogLevel::Warning,
            Severity::Error => LogLevel::Error,
        };
        let receipt = self
            .extensions
            .read(cx)
            .runtime_logs()
            .for_workspace(&self.workspace.root().to_string_lossy())
            .append(plugin, level, source, format!("{title}\n{message}"));
        // Import through the normal cursor now so the request-owned card can retain an exact target.
        // The extension observer still publishes the change and cannot replay this receipt later.
        let id = self.import_plugin_messages(Some(receipt), cx);
        self.extensions.update(cx, |_, cx| cx.notify());
        id
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

    /// Explicit card activation opens this window's panel and reveals retained `id`, including undisclosed history.
    /// Missing identities open the current history without selecting a different receipt or recreating old content.
    pub(crate) fn reveal_host_message(
        &mut self,
        id: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let panel_id = gpui_base::dock::PanelId::from(self.messages.entity_id());
        self.dock_presentation.activate(panel_id);
        self.messages.update(cx, |panel, cx| {
            panel.acknowledge(cx);
            panel.set_visible(true, cx);
            panel.reveal_message(id, cx);
            panel.focus_handle(cx).focus(window, cx);
        });
        self.dock_area.update(cx, |area, cx| {
            local_dock::reveal_panel(area, panel_id, window, cx);
            cx.notify();
        });
        cx.notify();
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
