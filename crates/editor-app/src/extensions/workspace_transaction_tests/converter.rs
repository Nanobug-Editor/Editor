//! The second independent guest drives two-entry discovery pages and native confirmation/recovery.
use super::*;

/// Discovery, reads and submission are the converter's business state machine, not host test policy.
fn converter_preview(
    visual: &mut VisualTestContext,
    app: &Entity<EditorApp>,
    manager: &mut Manager,
) -> EditorRequest {
    super::super::package_ui_test_support::demand(
        manager,
        "workspace-converter",
        plugin_runtime::DemandReason::Command("convert".into()),
    )
    .unwrap();
    manager
        .invoke_command("workspace-converter", "convert", serde_json::Value::Null)
        .unwrap();
    for _ in 0..24 {
        let requests = publish(visual, app, manager, "workspace-converter");
        if let Some(request) = requests.iter().find(|request| {
            matches!(
                request.operation(),
                api::EditorOperation::Workspace {
                    operation: workspace::Operation::ApplyPlan { .. }
                }
            )
        }) {
            visual.update(|window, cx| window.draw(cx).clear(cx));
            assert!(visual.debug_bounds("workspace-plan-confirm").is_some());
            return request.clone();
        }
        for request in requests {
            assert!(
                matches!(
                    request.status(),
                    api::RequestUpdate::Completed { result: Ok(_) }
                ),
                "conversion input failed: {:?}",
                request.status()
            );
        }
        manager.poll();
    }
    panic!(
        "converter did not finish bounded paged discovery: {:?}",
        manager.live["workspace-converter"]
            .views
            .values()
            .next()
            .unwrap()
            .root
            .kind
    )
}

/// Escape causes no effects; confirmation converts dirty text and unopened disk through the same API.
#[gpui::test]
#[ignore = "build workspace-converter using the current host --plugin-package first"]
fn workspace_transaction_converter_pages_preserves_dirty_buffer_and_restores_complete_batch(
    cx: &mut TestAppContext,
) {
    crate::tests::with_shortcut_editor(cx, true, Vec::new(), |visual, app, path| {
        let root = path.parent().unwrap();
        let live = root.join("community-live.txt");
        std::fs::write(&live, "旧A 中文🙂\r\n中段\r\n旧B\r\n").unwrap();
        visual
            .update(|window, cx| app.update(cx, |app, cx| app.open_file(live.clone(), window, cx)));
        visual.run_until_parked();
        seed(visual, &live);
        let original = text(visual, &app);
        let (_profile, mut manager) = manager(root, "workspace-converter");
        let cancelled = converter_preview(visual, &app, &mut manager);
        visual.simulate_keystrokes("escape");
        visual.run_until_parked();
        let cancelled = batch(&cancelled);
        assert_eq!(cancelled.state, workspace::BatchState::Cancelled);
        assert!(cancelled.completed.is_empty());
        assert!(!cancelled.recoverable);
        assert_eq!(text(visual, &app), original);
        assert_eq!(
            std::fs::read_to_string(root.join("community-disk.txt")).unwrap(),
            "磁盘旧A🙂\r\n旧B\r\n"
        );
        manager.poll();
        // The guest's own Cancel command must stop the native preview, not merely its local task.
        let stopped = converter_preview(visual, &app, &mut manager);
        manager
            .invoke_command("workspace-converter", "cancel", serde_json::Value::Null)
            .unwrap();
        publish(visual, &app, &mut manager, "workspace-converter");
        visual.update(|window, cx| window.draw(cx).clear(cx));
        visual.run_until_parked();
        assert!(matches!(
            stopped.status(),
            api::RequestUpdate::Cancelled { .. }
        ));
        assert!(visual.debug_bounds("workspace-plan-confirm").is_none());
        let stopped = stopped
            .workspace_request()
            .unwrap()
            .batch()
            .unwrap()
            .receipt();
        assert_eq!(stopped.state, workspace::BatchState::Cancelled);
        assert!(stopped.completed.is_empty());
        assert_eq!(text(visual, &app), original);
        manager.poll();
        let request = converter_preview(visual, &app, &mut manager);
        assert_eq!(text(visual, &app), original);
        confirm(visual);
        let receipt = batch(&request);
        assert_eq!(
            receipt.state,
            workspace::BatchState::Complete,
            "{receipt:?}"
        );
        assert_eq!(receipt.completed.len(), 2);
        assert_eq!(
            text(visual, &app),
            "转换A 中文🙂\r\n中段\r\n转换B\r\n未保存 中文🙂"
        );
        assert_eq!(
            std::fs::read_to_string(&live).unwrap(),
            "旧A 中文🙂\r\n中段\r\n旧B\r\n",
            "conversion must preserve unsaved native text"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("community-disk.txt")).unwrap(),
            "磁盘转换A🙂\r\n转换B\r\n"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("community-template.txt")).unwrap(),
            "模板 中文🙂\r\n"
        );
        let restored = undo(
            visual,
            &app,
            &mut manager,
            "workspace-converter",
            receipt.batch,
        );
        assert_eq!(
            restored.state,
            workspace::BatchState::Undone,
            "{restored:?}"
        );
        assert_eq!(text(visual, &app), original);
        assert_eq!(
            std::fs::read_to_string(root.join("community-disk.txt")).unwrap(),
            "磁盘旧A🙂\r\n旧B\r\n"
        );
        assert_eq!(
            std::fs::read_to_string(&live).unwrap(),
            "旧A 中文🙂\r\n中段\r\n旧B\r\n"
        );
    });
}
