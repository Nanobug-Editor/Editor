//! All-target checks reject stale preview inputs and local document Undo before any other restore.
use super::*;

/// Disk races, native edits and close/reopen are separate sources; each stale plan performs zero effects.
#[gpui::test]
#[ignore = "opt-in frozen Package/WASM fixtures; see crates/plugin-runtime/tests/fixtures/packages/README.md"]
fn workspace_transaction_preview_rechecks_disk_native_version_and_closed_target(
    cx: &mut TestAppContext,
) {
    crate::tests::with_shortcut_editor(cx, false, Vec::new(), |visual, app, path| {
        let root = path.parent().unwrap();
        let disk = root.join("preview-disk.txt");
        std::fs::write(&disk, "before").unwrap();
        let (_profile, mut manager) = manager(root, "workspace-scaffold");
        let root_handle = workspace_root(&mut manager, "workspace-scaffold");
        for source in ["disk", "native", "closed"] {
            let before = document(visual, &app, &mut manager, "workspace-scaffold");
            let original = text(visual, &app);
            let disk_stamp = stamp(
                visual,
                &app,
                &mut manager,
                "workspace-scaffold",
                target(&root_handle, "preview-disk.txt"),
            );
            let changes = vec![
                workspace::Change::EditDocument {
                    document: before.clone(),
                    edits: vec![api::TextEdit {
                        range: api::TextRange { start: 0, end: 1 },
                        text: "P".into(),
                    }],
                    selection: api::TextRange { start: 0, end: 0 },
                },
                workspace::Change::Write {
                    target: target(&root_handle, "preview-disk.txt"),
                    expected: Some(disk_stamp),
                    bytes: b"planned".to_vec(),
                },
            ];
            let (request, _, batch_handle) =
                preview(visual, &app, &mut manager, "workspace-scaffold", changes);
            match source {
                "disk" => std::fs::write(&disk, "external change").unwrap(),
                "native" => {
                    invoke(
                        visual,
                        &app,
                        &mut manager,
                        "workspace-scaffold",
                        api::EditorOperation::ApplyEdits {
                            document: before,
                            edits: vec![api::TextEdit {
                                range: api::TextRange { start: 0, end: 1 },
                                text: "U".into(),
                            }],
                            selection: api::TextRange { start: 0, end: 0 },
                        },
                    )
                    .unwrap();
                }
                "closed" => {
                    // Save and native close leave no same-identity target; a reopened path must not inherit it.
                    let original_id = before.id.clone();
                    invoke(
                        visual,
                        &app,
                        &mut manager,
                        "workspace-scaffold",
                        api::EditorOperation::SaveDocument { document: before },
                    )
                    .unwrap();
                    let native_path =
                        visual.update(|_, cx| app.read(cx).active_path.clone().unwrap());
                    visual.update(|window, cx| {
                        app.update(cx, |app, cx| app.close_tab(native_path, window, cx))
                    });
                    visual.run_until_parked();
                    visual.update(|window, cx| {
                        app.update(cx, |app, cx| app.open_file(path.to_path_buf(), window, cx))
                    });
                    visual.run_until_parked();
                    assert_ne!(
                        document(visual, &app, &mut manager, "workspace-scaffold").id,
                        original_id,
                        "the close/reopen scenario must actually replace the native target"
                    );
                }
                _ => unreachable!(),
            }
            let changed = text(visual, &app);
            confirm(visual);
            let receipt = batch(&request);
            assert_eq!(
                receipt.state,
                workspace::BatchState::Partial,
                "stale {source}: {receipt:?}"
            );
            assert!(
                receipt.completed.is_empty(),
                "stale {source} must stop all planned steps: {receipt:?}"
            );
            assert!(!receipt.recoverable);
            assert_eq!(
                text(visual, &app),
                if source == "disk" { original } else { changed }
            );
            assert_ne!(std::fs::read(&disk).unwrap(), b"planned");
            begin(
                &mut manager,
                "workspace-scaffold",
                api::Operation::CloseResource {
                    handle: batch_handle,
                },
            )
            .unwrap();
        }
    });
}

/// A local Undo remains local; even redo to equal bytes cannot revive an earlier group revision.
#[gpui::test]
#[ignore = "opt-in frozen Package/WASM fixtures; see crates/plugin-runtime/tests/fixtures/packages/README.md"]
fn workspace_transaction_later_typing_and_local_undo_refuse_all_batch_restoration(
    cx: &mut TestAppContext,
) {
    crate::tests::with_shortcut_editor(cx, false, Vec::new(), |visual, app, path| {
        seed(visual, path);
        let root = path.parent().unwrap();
        let (_profile, mut manager) = manager(root, "workspace-scaffold");
        let request = scaffold_preview(visual, &app, &mut manager);
        confirm(visual);
        let receipt = batch(&request);
        assert_eq!(receipt.state, workspace::BatchState::Complete);
        let proposed = text(visual, &app);
        visual.simulate_keystrokes("ctrl-z");
        visual.run_until_parked();
        assert!(text(visual, &app).starts_with("旧A"));
        assert_eq!(
            std::fs::read_to_string(root.join("community-disk.txt")).unwrap(),
            "脚手架磁盘 中文🙂\r\n",
            "ordinary Undo stays in this document"
        );
        let refused = undo(
            visual,
            &app,
            &mut manager,
            "workspace-scaffold",
            receipt.batch.clone(),
        );
        assert_eq!(
            refused.failed.as_ref().unwrap().code,
            api::ErrorCode::Conflict
        );
        assert!(!refused.conflicts.is_empty());
        assert!(root.join("community-generated/final.txt").exists());
        assert!(!root.join("community-remove.txt").exists());
        visual.simulate_keystrokes("ctrl-y");
        visual.run_until_parked();
        assert_eq!(text(visual, &app), proposed);
        let refused = undo(
            visual,
            &app,
            &mut manager,
            "workspace-scaffold",
            receipt.batch.clone(),
        );
        assert_eq!(
            refused.failed.as_ref().unwrap().code,
            api::ErrorCode::Conflict,
            "matching bytes after local Redo still carry a newer revision"
        );
        visual.simulate_keystrokes("ctrl-end");
        visual.simulate_input("后续修改😀");
        visual.run_until_parked();
        let modified = text(visual, &app);
        let refused = undo(
            visual,
            &app,
            &mut manager,
            "workspace-scaffold",
            receipt.batch,
        );
        assert!(!refused.conflicts.is_empty());
        assert_eq!(text(visual, &app), modified);
        assert!(root.join("community-generated/final.txt").exists());
        assert!(!root.join("community-remove.txt").exists());
    });
}
