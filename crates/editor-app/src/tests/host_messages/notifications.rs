//! Unified receipts use the real extension observer, message controls and nonmodal native layout.

use super::*;
use crate::app::messages::MessageLevel;
use plugin_runtime::LogLevel;

/// Publish a worker change through the same entity notification observed by the main window.
fn notify_extensions(form: &mut VisualTestContext, app: &Entity<EditorApp>) {
    form.update(|_, cx| {
        app.read(cx)
            .extensions
            .clone()
            .update(cx, |_, cx| cx.notify());
    });
    draw(form);
}

/// Errors and warnings retain focus, use the lower-right corner and dismiss independently of history.
#[gpui::test]
fn unified_messages_host_alerts_are_nonmodal_and_bottom_right(cx: &mut TestAppContext) {
    with_messages(cx, |form, app, root| {
        let path = root.join("editing.txt");
        std::fs::write(&path, "original").unwrap();
        form.update(|window, cx| app.update(cx, |app, cx| app.open_file(path, window, cx)));
        draw(form);
        for (dark, scale, level) in [
            (false, 1., MessageLevel::Warning),
            (true, 1.5, MessageLevel::Error),
        ] {
            form.update(|window, cx| {
                apply_theme(builtin_theme(dark), cx);
                app.read(cx).editor.focus_handle(cx).focus(window, cx);
                app.update(cx, |app, cx| {
                    app.record_host_message(level, "中文 warning/error remains reviewable", cx);
                });
                window.refresh();
            });
            form.simulate_scale_factor_change(scale);
            draw(form);
            let card = form.debug_bounds("host-message-toast").expect("alert card");
            assert!(card.right() <= px(1100.) && card.left() >= px(600.));
            assert!(card.bottom() <= px(800.) && card.top() > px(400.));
            assert!(
                form.update(|window, cx| {
                    app.read(cx).editor.focus_handle(cx).is_focused(window)
                })
            );
            form.simulate_input("中文");
            draw(form);
            assert!(form.update(|_, cx| app.read(cx).editor.read(cx).value().contains("中文")));
            click(form, "host-message-toast-close");
            assert!(form.debug_bounds("host-message-toast").is_none());
            assert!(form.debug_bounds("host-messages-empty").is_none());
        }
        form.update(|_, cx| {
            app.update(cx, |app, cx| {
                app.record_host_message(MessageLevel::Info, "ordinary receipt", cx)
            });
        });
        draw(form);
        assert!(form.debug_bounds("host-message-info").is_some());
        assert!(form.debug_bounds("host-message-toast").is_none());
    });
}

/// The extension observer imports each owned receipt once; clearing history never rewinds its cursor.
#[gpui::test]
fn unified_messages_import_plugin_logs_once_and_do_not_replay_after_clear(cx: &mut TestAppContext) {
    with_messages(cx, |form, app, _| {
        let logs = form.update(|_, cx| app.read(cx).extensions.read(cx).runtime_logs());
        logs.append("git", LogLevel::Info, "wasi/stdout", "Git ready");
        logs.append(
            "other",
            LogLevel::Warning,
            "language/status",
            "Service warning",
        );
        let error = logs.append("git", LogLevel::Error, "host.operation", "Git error");
        let generation = logs.generation();
        notify_extensions(form, &app);
        assert!(form.debug_bounds("host-message-1").is_some());
        assert!(form.debug_bounds("host-message-3").is_some());
        assert!(form.debug_bounds("host-message-source-1").is_some());
        assert!(form.debug_bounds("host-message-toast").is_some());
        assert_eq!(
            logs.generation(),
            generation,
            "import must not acknowledge plugin logs"
        );
        notify_extensions(form, &app);
        assert!(form.debug_bounds("host-message-4").is_none());
        click(form, "host-messages-clear");
        assert!(form.debug_bounds("host-messages-empty").is_some());
        logs.mark_read("git", &[error]);
        notify_extensions(form, &app);
        assert!(form.debug_bounds("host-messages-empty").is_some());
        assert!(form.debug_bounds("host-message-4").is_none());
        logs.append("git", LogLevel::Error, "host.operation", "Later Git error");
        notify_extensions(form, &app);
        assert!(form.debug_bounds("host-message-4").is_some());
        assert!(form.debug_bounds("host-message-source-4").is_some());
        assert!(form.debug_bounds("host-messages-dot").is_some());
    });
}

/// A shared runtime sink can serve multiple project actors without showing a peer's workspace logs.
#[gpui::test]
fn unified_messages_filter_shared_runtime_receipts_by_workspace(cx: &mut TestAppContext) {
    with_messages(cx, |form, app, root| {
        let logs = form.update(|_, cx| app.read(cx).extensions.read(cx).runtime_logs());
        let peer = tempfile::tempdir().unwrap();
        logs.for_workspace(&peer.path().to_string_lossy()).append(
            "same-plugin",
            LogLevel::Error,
            "language",
            "Other workspace error",
        );
        notify_extensions(form, &app);
        assert!(form.debug_bounds("host-messages-empty").is_some());
        assert!(form.debug_bounds("host-message-toast").is_none());
        logs.for_workspace(&root.to_string_lossy()).append(
            "same-plugin",
            LogLevel::Warning,
            "language",
            "Current workspace warning",
        );
        logs.for_workspace("").append(
            "installer",
            LogLevel::Info,
            "package",
            "Application receipt",
        );
        notify_extensions(form, &app);
        assert!(form.debug_bounds("host-message-1").is_some());
        assert!(form.debug_bounds("host-message-2").is_some());
        assert!(form.debug_bounds("host-message-3").is_none());
        assert!(form.debug_bounds("host-message-error").is_none());
        assert!(form.debug_bounds("host-message-warning").is_some());
    });
}

/// Flooded receipts retain history while visible duplicate alerts coalesce and the native queue stays bounded.
#[gpui::test]
fn unified_messages_alerts_are_bounded_and_repeat_after_dismissal(cx: &mut TestAppContext) {
    with_messages(cx, |form, app, _| {
        form.update(|_, cx| {
            app.update(cx, |app, cx| {
                for _ in 0..8 {
                    app.record_host_message(MessageLevel::Error, "Same error", cx);
                }
            })
        });
        draw(form);
        assert_eq!(
            form.update(|_, cx| app.read(cx).message_delivery.notifications(cx).count()),
            1
        );
        assert!(form.debug_bounds("host-message-8").is_some());
        click(form, "host-message-toast-close");
        assert!(form.debug_bounds("host-message-toast").is_none());
        form.update(|_, cx| {
            app.update(cx, |app, cx| {
                app.record_host_message(MessageLevel::Error, "Same error", cx);
                for index in 0..8 {
                    app.record_host_message(MessageLevel::Warning, format!("Warning {index}"), cx);
                }
            })
        });
        draw(form);
        assert_eq!(
            form.update(|_, cx| app.read(cx).message_delivery.notifications(cx).count()),
            3
        );
        assert!(form.debug_bounds("host-message-toast").is_some());
        assert!(form.debug_bounds("host-message-17").is_some());
    });
}

/// Repeated stderr diagnostics differ only in their transport timestamp, not in the warning to act on.
#[gpui::test]
fn unified_messages_timestamped_stderr_coalesces_without_losing_history(cx: &mut TestAppContext) {
    with_messages(cx, |form, app, _| {
        let logs = form.update(|_, cx| app.read(cx).extensions.read(cx).runtime_logs());
        let messages = [
            "2026-10-30T21:43:07.4372163+08:00  WARN notify error: Input watch path is neither a file nor a directory.",
            "2026-10-30T21:43:08.4750824+08:00  WARN notify error: Input watch path is neither a file nor a directory.",
            "2026-10-30T21:43:08.4751162+08:00  WARN notify error: Input watch path is neither a file nor a directory.",
        ];
        for message in messages {
            logs.append(
                "language-plugin",
                LogLevel::Warning,
                "lsp/analysis/stderr",
                message,
            );
            notify_extensions(form, &app);
        }
        assert_eq!(
            form.update(|_, cx| app.read(cx).message_delivery.notifications(cx).count()),
            1,
            "timestamps must not multiply the same visible stderr warning"
        );
        assert_eq!(
            logs.records("language-plugin")
                .iter()
                .map(|record| record.message.as_str())
                .collect::<Vec<_>>(),
            messages,
            "diagnostic history must keep each original timestamp and message"
        );
        assert!(form.debug_bounds("host-message-3").is_some());
        click(form, "host-message-toast-close");
        assert!(form.debug_bounds("host-message-toast").is_none());
        logs.append(
            "language-plugin",
            LogLevel::Warning,
            "lsp/analysis/stderr",
            messages[2],
        );
        notify_extensions(form, &app);
        assert!(form.debug_bounds("host-message-toast").is_some());
        logs.append(
            "language-plugin",
            LogLevel::Warning,
            "lsp/analysis/stderr",
            "2026-10-30T21:43:09.4751162+08:00  WARN permission denied while watching source",
        );
        notify_extensions(form, &app);
        assert_eq!(
            form.update(|_, cx| app.read(cx).message_delivery.notifications(cx).count()),
            2,
            "different diagnostics still need independent alerts"
        );
    });
}

/// Timestamp coalescing retains the newer receipt as the native card's history-navigation target.
#[gpui::test]
fn unified_messages_timestamped_stderr_click_reveals_latest(cx: &mut TestAppContext) {
    with_messages(cx, |form, app, _| {
        click(form, "host-messages-hide");
        let logs = form.update(|_, cx| app.read(cx).extensions.read(cx).runtime_logs());
        for (timestamp, following) in [("13:43:01", 30), ("13:43:02", 25)] {
            logs.append(
                "language-plugin",
                LogLevel::Warning,
                "lsp/analysis/stderr",
                format!("2026-10-30T{timestamp}Z WARN Watch failed"),
            );
            notify_extensions(form, &app);
            form.update(|_, cx| {
                app.update(cx, |app, cx| {
                    for _ in 0..following {
                        app.record_host_message(MessageLevel::Info, "Intermediate receipt", cx);
                    }
                })
            });
        }
        draw(form);
        assert_eq!(
            form.update(|_, cx| app.read(cx).message_delivery.notifications(cx).count()),
            1
        );
        click(form, "host-message-toast");
        form.update(|window, cx| window.simulate_next_frame(cx));
        draw(form);
        let viewport = form.debug_bounds("host-message-list").unwrap();
        let latest = form
            .debug_bounds("host-message-32")
            .expect("latest timestamped receipt is disclosed");
        assert!(latest.top() >= viewport.top() - px(1.));
        assert!(latest.bottom() <= viewport.bottom() + px(1.));
        assert!(
            form.debug_bounds("host-message-1").is_none(),
            "card must follow the newer timestamped duplicate"
        );
    });
}

/// Matching diagnostic bodies must not merge independent producers or severity escalations.
#[gpui::test]
fn unified_messages_timestamped_stderr_keeps_origin_and_severity(cx: &mut TestAppContext) {
    with_messages(cx, |form, app, _| {
        let logs = form.update(|_, cx| app.read(cx).extensions.read(cx).runtime_logs());
        for (index, (plugin, level, source)) in [
            ("one", LogLevel::Warning, "lsp/analysis/stderr"),
            ("two", LogLevel::Warning, "lsp/analysis/stderr"),
            ("one", LogLevel::Error, "lsp/analysis/stderr"),
        ]
        .into_iter()
        .enumerate()
        {
            logs.append(
                plugin,
                level,
                source,
                format!("2026-10-30T13:43:0{index}Z Watch failed"),
            );
            notify_extensions(form, &app);
            assert_eq!(
                form.update(|_, cx| app.read(cx).message_delivery.notifications(cx).count()),
                index + 1
            );
        }
    });
}

/// Full diagnostic text is retained, while long automatic cards keep every dismissal within the viewport.
#[gpui::test]
fn unified_messages_publication_preserves_source_and_bounded_long_cards(cx: &mut TestAppContext) {
    use plugin_runtime::plugin_protocol::interaction::{Operation, Severity};
    with_messages(cx, |form, app, _| {
        form.update(|_, cx| {
            app.update(cx, |app, cx| {
                for index in 0..3 {
                    app.record_plugin_notice(
                        "publisher",
                        &Operation::PublishMessage {
                            title: format!("Message {index}"),
                            message: "长消息\n".repeat(300),
                            severity: Severity::Error,
                        },
                        cx,
                    );
                }
            })
        });
        draw(form);
        let stack = form.debug_bounds("host-message-notice-stack").unwrap();
        assert!(stack.top() >= px(0.) && stack.bottom() <= px(800.));
        let card = form.debug_bounds("host-message-toast").unwrap();
        assert!(
            card.size.height <= px(150.),
            "long content must not bury newer cards"
        );
        let close = form.debug_bounds("host-message-toast-close").unwrap();
        assert!(close.top() >= stack.top() && close.bottom() <= stack.bottom());
        assert!(form.debug_bounds("host-message-source-3").is_some());
        let logs = form.update(|_, cx| app.read(cx).extensions.read(cx).runtime_logs());
        let records = logs.records("publisher");
        assert_eq!(records.len(), 3);
        assert!(
            records
                .iter()
                .all(|record| record.source == "host.message" && record.level == LogLevel::Error)
        );
        assert!(records[0].message.ends_with(&"长消息\n".repeat(300)));
    });
}
