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

/// Origin, level and diagnostic identity identify a still-visible alert; displayed text stays intact.
struct Alert {
    /// Receipt severity remains part of identity, so an escalation cannot be hidden by a warning.
    level: MessageLevel,
    /// The captured plugin/transport origin, or the translated host alert title.
    title: String,
    /// Comparison-only text; stderr timestamps do not distinguish repeated diagnostics.
    key: String,
    /// The native card owns the original message and its independent dismissal state.
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

    /// Bound alerts by `level`, `title` and `key`, while `message` remains the original display text.
    /// `id` follows the latest represented receipt; `cx` creates the card without moving focus.
    fn push(
        &mut self,
        level: MessageLevel,
        title: String,
        message: String,
        key: String,
        id: u64,
        cx: &mut Context<EditorApp>,
    ) {
        self.alerts
            .retain(|alert| !alert.view.read(cx).is_dismissed());
        if let Some(alert) = self
            .alerts
            .iter()
            .find(|alert| alert.level == level && alert.title == title && alert.key == key)
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
            editor_ui::Notification::alert(level, title.clone(), message).on_click(on_click)
        });
        self.alerts.push_back(Alert {
            level,
            title,
            key,
            view,
            message_id,
        });
        while self.alerts.len() > ALERT_LIMIT {
            self.alerts.pop_front();
        }
    }
}

/// Return a comparison key for `message` from the host-owned transport `source`.
/// Only stderr's valid leading RFC 3339 timestamp is metadata; all remaining text and severity stay exact.
fn plugin_alert_key<'a>(source: &str, message: &'a str) -> &'a str {
    let stderr =
        source == "wasi/stderr" || (source.starts_with("lsp/") && source.ends_with("/stderr"));
    if stderr
        && let Some((timestamp, body)) = message.split_once([' ', '\t'])
        && chrono::DateTime::parse_from_rfc3339(timestamp).is_ok()
    {
        let body = body.trim_start_matches([' ', '\t']);
        if !body.is_empty() {
            return body;
        }
    }
    message
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
        let message = message.into();
        let alert_key = Some(message.clone());
        self.record_message(level, message, None, alert_key, cx);
    }

    /// Publish `message` with `level` and optional `source`, returning its retained panel identity.
    /// `alert_key` opts into a deduplicated card that navigates to this latest identity.
    /// Explicit plugin notices pass no key because they already have a request-owned floating card.
    fn record_message(
        &mut self,
        level: MessageLevel,
        message: String,
        source: Option<String>,
        alert_key: Option<String>,
        cx: &mut Context<Self>,
    ) -> u64 {
        let id = self.messages.update(cx, |panel, cx| {
            panel.push_attributed(level, message.clone(), source.clone(), cx)
        });
        if let Some(key) = alert_key
            && level > MessageLevel::Info
        {
            let title = source.unwrap_or_else(|| {
                t!(if level == MessageLevel::Error {
                    "messages.error"
                } else {
                    "messages.warning"
                })
                .to_string()
            });
            self.message_delivery
                .push(level, title, message, key, id, cx);
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
        // Normalize only the comparison key: both histories and the first visible card keep the raw log.
        let alert_key = (record.source != NOTIFICATION_SOURCE)
            .then(|| plugin_alert_key(&record.source, &record.message).to_owned());
        self.record_message(
            level,
            record.message,
            Some(format!("{} · {}", record.plugin, record.source)),
            alert_key,
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

#[cfg(test)]
mod tests {
    use super::plugin_alert_key;

    /// Both host-owned stderr transports ignore timestamp metadata without parsing diagnostic wording.
    #[test]
    fn unified_messages_stderr_keys_keep_severity_and_diagnostic_text() {
        for source in [
            "lsp/analysis/stderr",
            "lsp/another-provider/stderr",
            "wasi/stderr",
        ] {
            for timestamp in ["2026-10-30T21:43:07.4372163+08:00", "2026-10-30T13:43:08Z"] {
                for body in [
                    "WARN notify error: 中文",
                    "ERROR failure at 2026-10-30T13:00:00Z",
                ] {
                    let message = format!("{timestamp}  {body}");
                    assert_eq!(plugin_alert_key(source, &message), body);
                }
            }
        }
    }

    /// Unrecognized prefixes and timestamps that belong to a message retain their exact identity.
    #[test]
    fn unified_messages_stderr_keys_preserve_unstructured_or_non_stderr_messages() {
        let timestamped = "2026-10-30T21:43:07+08:00 WARN notify error";
        for source in [
            "lsp/analysis",
            "host.message",
            "wasi/stdout",
            "other/stderr",
        ] {
            assert_eq!(plugin_alert_key(source, timestamped), timestamped);
        }
        for message in [
            "2026-02-30T21:43:07+08:00 WARN invalid date",
            "2026-10-30T21:43:07+08:00 ",
            "2026-10-30T21:43:07+08:00\nWARN separate line",
            "WARN notify error at 2026-10-30T21:43:07+08:00",
            "[2026-10-30T21:43:07+08:00] WARN other format",
            "中文警告",
            "",
        ] {
            assert_eq!(plugin_alert_key("lsp/analysis/stderr", message), message);
        }
    }
}
