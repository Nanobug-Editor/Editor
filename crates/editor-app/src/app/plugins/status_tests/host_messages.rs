//! Native host acknowledgement and clearing preserve publicly installed plugins' logs and reminders.

use super::*;
use crate::app::messages::MessageLevel;

/// Service logs stay out of host history; viewing and clearing host messages preserve plugin reminders.
#[gpui::test]
fn host_messages_confirmation_and_clear_preserve_plugin_logs(cx: &mut TestAppContext) {
    with_status(cx, 2, "en", false, |form, app, manager| {
        let logs = manager.runtime_logs();
        logs.append(
            "status-00",
            LogLevel::Warning,
            "plugin.stderr",
            "Plugin warning remains unread",
        );
        logs.append(
            "status-01",
            LogLevel::Error,
            "lsp/analysis",
            "Service failure remains unread",
        );
        let records = [logs.records("status-00"), logs.records("status-01")];
        let generation = logs.generation();
        let reminders = logs.pending_reminders();
        assert_eq!(
            reminders,
            vec![
                ("status-00".into(), LogLevel::Warning),
                ("status-01".into(), LogLevel::Error),
            ]
        );
        // RuntimeLogs exposes no individual read flag. Its change counter includes reads and
        // reminder acknowledgements; equality plus one anomaly per owner proves neither is read.
        let assert_plugins_unchanged = |form: &mut VisualTestContext| {
            assert_eq!(logs.records("status-00"), records[0]);
            assert_eq!(logs.records("status-01"), records[1]);
            assert_eq!(logs.generation(), generation);
            assert_eq!(logs.pending_reminders(), reminders);
            assert_eq!(logs.unread_severity("status-00"), Some(LogLevel::Warning));
            assert_eq!(logs.unread_severity("status-01"), Some(LogLevel::Error));
            assert!(form.debug_bounds("plugin-error-indicator").is_some());
            assert!(form.debug_bounds("plugin-status-popup").is_none());
        };
        form.update(|_, cx| {
            app.read(cx)
                .extensions
                .clone()
                .update(cx, |_, cx| cx.notify());
        });
        draw(form);
        // Fresh workspaces keep messages closed. Opening cannot import even a service error.
        assert!(form.debug_bounds("host-messages-panel").is_none());
        click(form, "host-messages-toggle");
        assert!(form.debug_bounds("host-messages-empty").is_some());
        assert!(form.debug_bounds("host-message-1").is_none());
        assert!(form.debug_bounds("host-message-toast").is_none());
        assert!(form.debug_bounds("host-messages-dot").is_none());
        assert_plugins_unchanged(form);

        form.update(|_, cx| {
            app.update(cx, |app, cx| {
                app.record_host_message(MessageLevel::Error, "Host operation failed", cx);
            });
        });
        draw(form);
        assert!(form.debug_bounds("host-message-1").is_some());
        assert!(form.debug_bounds("host-message-error").is_some());
        assert!(form.debug_bounds("host-messages-dot").is_some());
        assert_plugins_unchanged(form);

        // Both transitions use the actual bottom button: hiding preserves the host alert and
        // opening acknowledges only that history, never the neighboring plugin reminder.
        click(form, "host-messages-toggle");
        assert!(form.debug_bounds("host-messages-panel").is_none());
        assert!(form.debug_bounds("host-messages-dot").is_some());
        assert_plugins_unchanged(form);
        click(form, "host-messages-toggle");
        assert!(form.debug_bounds("host-message-1").is_some());
        assert!(form.debug_bounds("host-messages-dot").is_none());
        assert_plugins_unchanged(form);

        click(form, "host-messages-clear");
        assert!(form.debug_bounds("host-messages-empty").is_some());
        assert!(form.debug_bounds("host-message-1").is_none());
        assert!(form.debug_bounds("host-messages-dot").is_none());
        assert_plugins_unchanged(form);

        // A receipt arriving after clear owns a fresh host reminder while plugin checkpoints stay put.
        form.update(|_, cx| {
            app.update(cx, |app, cx| {
                app.record_host_message(MessageLevel::Warning, "New host warning", cx);
            });
        });
        draw(form);
        assert!(form.debug_bounds("host-message-2").is_some());
        assert!(form.debug_bounds("host-message-warning").is_some());
        assert!(form.debug_bounds("host-messages-dot").is_some());
        assert_plugins_unchanged(form);
    });
}

/// Excluded service logs still drive the real footer summary and its link to complete plugin history.
#[gpui::test]
fn host_messages_service_logs_remain_in_footer_and_plugin_management(cx: &mut TestAppContext) {
    with_status(cx, 3, "zh-CN", false, |form, app, manager| {
        let logs = manager.runtime_logs();
        let mut warnings = Vec::new();
        for (plugin, source) in [
            ("status-00", "dependencies/prepare"),
            ("status-01", "language.prepare:status-01/analysis"),
            ("status-02", "lsp/analysis/stderr"),
        ] {
            logs.append(plugin, LogLevel::Info, source, "服务状态记录");
            warnings.push(logs.append(plugin, LogLevel::Warning, source, "服务异常摘要"));
        }
        form.update(|_, cx| {
            app.read(cx)
                .extensions
                .clone()
                .update(cx, |_, cx| cx.notify());
        });
        draw(form);
        click(form, "host-messages-toggle");
        assert!(form.debug_bounds("host-messages-empty").is_some());
        assert!(form.debug_bounds("host-message-toast").is_none());
        assert!(form.debug_bounds("host-messages-dot").is_none());
        click(form, "plugin-warning-indicator");
        for id in warnings {
            assert!(form.debug_bounds(record_selector(id)).is_some());
        }
        let original = logs.records("status-01");
        let editor_window = form.update(|window, _| window.window_handle());
        click(form, "plugin-summary-open-status-01");
        let manager_window = form
            .update(|_, cx| cx.windows())
            .into_iter()
            .find(|window| *window != editor_window)
            .unwrap();
        let dialog = VisualTestContext::from_window(manager_window, form).into_mut();
        draw(dialog);
        assert!(dialog.debug_bounds("plugin-runtime-log").is_some());
        for record in &original {
            let selector = Box::leak(format!("plugin-log-record-{}", record.id).into_boxed_str());
            assert!(dialog.debug_bounds(selector).is_some());
        }
        assert_eq!(logs.records("status-01"), original);
        assert_eq!(logs.unread_severity("status-01"), None);
    });
}
