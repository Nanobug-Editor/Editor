//! Ownerless manager failures cross the real one-shot worker publication into the editor's message controls.

use super::*;

/// Identical new failures are distinct receipts; unchanged status and clearing history cannot replay them.
#[gpui::test]
fn unified_messages_unowned_manager_errors_are_once_per_publication(cx: &mut TestAppContext) {
    with_manager(cx, "en", false, |manager, owner| {
        let app = manager.update(|_, cx| owner.read(cx).parent.upgrade().unwrap());
        let editor_window = manager.update(|window, cx| {
            cx.windows()
                .into_iter()
                .find(|other| *other != window.window_handle())
                .unwrap()
        });
        let logs = manager.update(|_, cx| owner.read(cx).runtime_logs());
        let original = logs.records("manager-first");
        // The initial fixture's two owned receipts are already delivered through RuntimeLogs.
        assert_eq!(original.len(), 2);
        let publish = |form: &mut VisualTestContext| {
            form.update(|_, cx| {
                owner.update(cx, |owner, cx| {
                    owner.worker.state.lock().unwrap().status = Some(worker::OperationStatus {
                        plugin: None,
                        message: "Manager could not read its registry".into(),
                    });
                    owner.poll(cx);
                })
            });
            draw(form);
        };
        publish(manager);
        assert!(
            manager
                .debug_bounds("plugin-manager-operation-error")
                .is_some()
        );
        {
            let editor = VisualTestContext::from_window(editor_window, manager).into_mut();
            draw(editor);
            click(editor, "host-messages-toggle");
            assert!(
                editor.debug_bounds("host-message-3").is_some(),
                "ownerless manager failure must reach unified history"
            );
            assert!(
                editor.debug_bounds("host-message-source-3").is_none(),
                "manager failure must not fabricate a plugin owner"
            );
            assert_eq!(
                editor.update(|_, cx| app.read(cx).message_delivery.notifications(cx).count()),
                2
            );
            assert!(editor.debug_bounds("host-message-toast").is_some());
            click(editor, "host-messages-clear");
            for _ in 0..2 {
                editor.update(|_, cx| owner.update(cx, |owner, cx| owner.poll(cx)));
                draw(editor);
            }
            assert!(editor.debug_bounds("host-messages-empty").is_some());
            assert!(editor.debug_bounds("host-message-4").is_none());
            publish(editor);
            assert!(editor.debug_bounds("host-message-4").is_some());
            assert!(editor.debug_bounds("host-message-5").is_none());
            assert_eq!(
                editor.update(|_, cx| app.read(cx).message_delivery.notifications(cx).count()),
                2,
                "identical visible floating cards coalesce while history gets a new receipt"
            );
        }
        assert_eq!(logs.records("manager-first"), original);
        assert_eq!(
            logs.records_since(
                0,
                &manager.update(|_, cx| owner.read(cx).workspace.to_string_lossy().into_owned())
            )
            .1
            .len(),
            2
        );
    });
}
