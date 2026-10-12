//! Actual Windows sharing failures and native cancellation retain truthful partial recovery receipts.
use super::*;

/// A deny-delete handle is a real platform fault, not a special host test operation.
#[cfg(windows)]
fn lock_file(path: &Path) -> std::fs::File {
    use std::os::windows::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(path)
        .unwrap()
}

#[gpui::test]
#[cfg(windows)]
#[ignore = "opt-in frozen Package/WASM fixtures; see crates/plugin-runtime/tests/fixtures/packages/README.md"]
fn workspace_transaction_io_failure_during_commit_and_undo_retains_actual_recovery(
    cx: &mut TestAppContext,
) {
    crate::tests::with_shortcut_editor(cx, false, Vec::new(), |visual, app, path| {
        let root = path.parent().unwrap();
        let disk_a = root.join("first.txt");
        let disk_b = root.join("second.txt");
        std::fs::write(&disk_a, "first before").unwrap();
        std::fs::write(&disk_b, "second before").unwrap();
        let (_profile, mut manager) = manager(root, "workspace-scaffold");
        let root_handle = workspace_root(&mut manager, "workspace-scaffold");
        let document = document(visual, &app, &mut manager, "workspace-scaffold");
        let changes = vec![
            workspace::Change::EditDocument {
                document,
                edits: vec![api::TextEdit {
                    range: api::TextRange { start: 0, end: 1 },
                    text: " changed 中😀 ".into(),
                }],
                selection: api::TextRange { start: 0, end: 0 },
            },
            workspace::Change::Write {
                target: target(&root_handle, "first.txt"),
                expected: Some(stamp(
                    visual,
                    &app,
                    &mut manager,
                    "workspace-scaffold",
                    target(&root_handle, "first.txt"),
                )),
                bytes: b"first after".to_vec(),
            },
            workspace::Change::Write {
                target: target(&root_handle, "second.txt"),
                expected: Some(stamp(
                    visual,
                    &app,
                    &mut manager,
                    "workspace-scaffold",
                    target(&root_handle, "second.txt"),
                )),
                bytes: b"second after".to_vec(),
            },
        ];
        let (request, _, batch_handle) =
            preview(visual, &app, &mut manager, "workspace-scaffold", changes);
        let locked = lock_file(&disk_b);
        confirm(visual);
        let receipt = batch(&request);
        assert_eq!(receipt.state, workspace::BatchState::Partial);
        assert_eq!(receipt.completed, [0, 1]);
        assert_eq!(receipt.stopped_at, Some(2));
        assert_eq!(std::fs::read_to_string(&disk_a).unwrap(), "first after");
        assert_eq!(std::fs::read_to_string(&disk_b).unwrap(), "second before");
        assert!(text(visual, &app).starts_with(" changed 中😀 "));
        assert_eq!(std::fs::read_to_string(path).unwrap(), "original on disk");
        assert!(receipt.recoverable);
        assert!(receipt.failed.is_some());
        drop(locked);
        let locked = lock_file(&disk_b);
        let partial = undo(
            visual,
            &app,
            &mut manager,
            "workspace-scaffold",
            batch_handle.clone(),
        );
        assert_eq!(
            partial.state,
            workspace::BatchState::UndoPartial,
            "{partial:?}"
        );
        assert!(partial.recoverable);
        assert_eq!(
            std::fs::read_to_string(&disk_a).unwrap(),
            "first before",
            "a preceding restoration is an actual effect"
        );
        assert!(
            text(visual, &app).starts_with(" changed 中😀 "),
            "native text waits for disk restoration result"
        );
        drop(locked);
        let restored = undo(
            visual,
            &app,
            &mut manager,
            "workspace-scaffold",
            batch_handle,
        );
        assert_eq!(
            restored.state,
            workspace::BatchState::Undone,
            "{restored:?}"
        );
        assert_eq!(text(visual, &app), "original on disk");
        assert_eq!(std::fs::read_to_string(&disk_a).unwrap(), "first before");
        assert_eq!(std::fs::read_to_string(&disk_b).unwrap(), "second before");
    });
}

/// Native cancellation after the first document Change stops later IO, without rolling back text.
#[gpui::test]
#[ignore = "opt-in frozen Package/WASM fixtures; see crates/plugin-runtime/tests/fixtures/packages/README.md"]
fn workspace_transaction_cancel_after_native_change_preserves_receipt_and_explicit_undo(
    cx: &mut TestAppContext,
) {
    crate::tests::with_shortcut_editor(cx, false, Vec::new(), |visual, app, path| {
        let root = path.parent().unwrap();
        std::fs::write(root.join("cancel.txt"), "disk before").unwrap();
        let (_profile, mut manager) = manager(root, "workspace-scaffold");
        let root_handle = workspace_root(&mut manager, "workspace-scaffold");
        let before = document(visual, &app, &mut manager, "workspace-scaffold");
        let disk_stamp = stamp(
            visual,
            &app,
            &mut manager,
            "workspace-scaffold",
            target(&root_handle, "cancel.txt"),
        );
        let changes = vec![
            workspace::Change::EditDocument {
                document: before,
                edits: vec![api::TextEdit {
                    range: api::TextRange { start: 0, end: 1 },
                    text: "中😀".into(),
                }],
                selection: api::TextRange { start: 0, end: 0 },
            },
            workspace::Change::Write {
                target: target(&root_handle, "cancel.txt"),
                expected: Some(disk_stamp),
                bytes: b"disk after".to_vec(),
            },
        ];
        let (request, _, batch_handle) =
            preview(visual, &app, &mut manager, "workspace-scaffold", changes);
        let stop = request.clone();
        let subscription = visual.update(|_, cx| {
            app.update(cx, |app, cx| {
                cx.subscribe(&app.editor, move |_, _, event, _| {
                    if matches!(event, gpui_base::input::InputEvent::Change) {
                        stop.cancel_from_host(api::CancelMode::TryTerminate);
                    }
                })
            })
        });
        confirm(visual);
        drop(subscription);
        assert!(matches!(
            request.status(),
            api::RequestUpdate::Cancelled {
                effect: api::CancellationEffect::WaitingStopped,
                ..
            }
        ));
        let api::EditorValue::Workspace(workspace::Value::Batch(receipt)) = invoke(
            visual,
            &app,
            &mut manager,
            "workspace-scaffold",
            api::EditorOperation::Workspace {
                operation: workspace::Operation::BatchStatus {
                    batch: batch_handle.clone(),
                },
            },
        )
        .unwrap() else {
            panic!("retained receipt expected")
        };
        assert_eq!(receipt.state, workspace::BatchState::Partial);
        assert_eq!(receipt.completed, [0]);
        assert_eq!(
            receipt.failed.as_ref().unwrap().code,
            api::ErrorCode::Cancelled
        );
        assert!(receipt.recoverable);
        assert!(text(visual, &app).starts_with("中😀"));
        assert_eq!(
            std::fs::read_to_string(root.join("cancel.txt")).unwrap(),
            "disk before"
        );
        let restored = undo(
            visual,
            &app,
            &mut manager,
            "workspace-scaffold",
            batch_handle,
        );
        assert_eq!(restored.state, workspace::BatchState::Undone);
        assert_eq!(text(visual, &app), "original on disk");
    });
}
