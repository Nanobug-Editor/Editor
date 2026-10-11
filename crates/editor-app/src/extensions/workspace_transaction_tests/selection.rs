//! Actual SDK selection grants retain their native picker access intent through confirmed disk work.
use super::*;
use plugin_runtime::plugin_protocol::interaction::{self, SelectionAccess, SelectionMode};

/// Native dialog results enter the production result boundary; guests never issue a native path.
fn select(
    manager: &mut Manager,
    plugin: &str,
    path: &Path,
    operation: interaction::Operation,
) -> api::ResourceHandle {
    let value = begin(
        manager,
        plugin,
        api::Operation::Editor {
            operation: api::EditorOperation::Interaction { operation },
            timeout_ms: 30_000,
        },
    )
    .unwrap();
    assert!(matches!(value, api::Value::Accepted(_)));
    let request = manager
        .live
        .get_mut(plugin)
        .unwrap()
        .take_editor_requests()
        .pop()
        .unwrap();
    assert!(request.begin());
    request.finish_selection(vec![path.to_path_buf()]);
    // The worker validates native paths and issues grants when polling, after the temporary Unit receipt.
    manager.poll();
    let ui::Kind::Text { text } = &manager.live[plugin]
        .views
        .values()
        .next()
        .unwrap()
        .root
        .kind
    else {
        panic!("selected result must reach the real guest panel")
    };
    let result: api::RequestUpdate = serde_json::from_str(text).unwrap();
    let api::RequestUpdate::Completed {
        result: Ok(api::EditorValue::Interaction(interaction::Value::Selected(selected))),
    } = result
    else {
        panic!("selection failed: {text}")
    };
    selected[0].handle.clone()
}

fn access(mode: SelectionMode, access: SelectionAccess) -> interaction::Operation {
    interaction::Operation::SelectWithAccess {
        title: "Chosen external resource / 外部资源".into(),
        mode,
        multiple: false,
        suggested_name: None,
        access,
    }
}

fn plan(changes: Vec<workspace::Change>) -> api::Operation {
    api::Operation::Editor {
        operation: api::EditorOperation::Workspace {
            operation: workspace::Operation::ApplyPlan {
                plan: workspace::Plan {
                    title: "External plan / 外部计划".into(),
                    changes,
                },
            },
        },
        timeout_ms: 30_000,
    }
}

/// Readonly picks cannot write; explicit exact-file writes and the converter's Save intent can recover.
#[gpui::test]
#[ignore = "build workspace-converter using the current host --plugin-package first"]
fn workspace_transaction_external_selection_and_save_keep_exact_access_and_restore_disk(
    cx: &mut TestAppContext,
) {
    crate::tests::with_shortcut_editor(cx, false, Vec::new(), |visual, app, path| {
        let external = tempfile::tempdir().unwrap();
        let file = external.path().join("external.txt");
        std::fs::write(&file, "external before 中😀\r\n").unwrap();
        let (_profile, mut manager) = manager(path.parent().unwrap(), "workspace-converter");
        let readonly = select(
            &mut manager,
            "workspace-converter",
            &file,
            interaction::Operation::Select {
                title: "Read file".into(),
                mode: SelectionMode::File,
                multiple: false,
                suggested_name: None,
            },
        );
        assert_eq!(
            begin(
                &mut manager,
                "workspace-converter",
                plan(vec![workspace::Change::Write {
                    target: target(&readonly, ""),
                    expected: Some(workspace::FileStamp::Missing),
                    bytes: b"denied".to_vec()
                }])
            )
            .unwrap_err()
            .code,
            api::ErrorCode::PermissionDenied
        );
        begin(
            &mut manager,
            "workspace-converter",
            api::Operation::CloseResource { handle: readonly },
        )
        .unwrap();
        let writable = select(
            &mut manager,
            "workspace-converter",
            &file,
            access(SelectionMode::File, SelectionAccess::ReadWrite),
        );
        let version = stamp(
            visual,
            &app,
            &mut manager,
            "workspace-converter",
            target(&writable, ""),
        );
        let (request, _, batch_handle) = preview(
            visual,
            &app,
            &mut manager,
            "workspace-converter",
            vec![workspace::Change::Write {
                target: target(&writable, ""),
                expected: Some(version),
                bytes: "external after 中😀\r\n".as_bytes().to_vec(),
            }],
        );
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            "external before 中😀\r\n"
        );
        confirm(visual);
        assert_eq!(batch(&request).state, workspace::BatchState::Complete);
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            "external after 中😀\r\n"
        );
        assert_eq!(
            undo(
                visual,
                &app,
                &mut manager,
                "workspace-converter",
                batch_handle
            )
            .state,
            workspace::BatchState::Undone
        );
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            "external before 中😀\r\n"
        );
        // The converter's genuine export command acquires a Save intent and submits the same native plan.
        manager
            .invoke_command("workspace-converter", "export", serde_json::Value::Null)
            .unwrap();
        let selection = manager
            .live
            .get_mut("workspace-converter")
            .unwrap()
            .take_editor_requests()
            .pop()
            .unwrap();
        assert!(selection.begin());
        selection.finish_selection(vec![file.clone()]);
        manager.poll();
        let request = publish(visual, &app, &mut manager, "workspace-converter")
            .pop()
            .unwrap();
        visual.update(|window, cx| window.draw(cx).clear(cx));
        assert!(visual.debug_bounds("workspace-plan-confirm").is_some());
        let api::EditorOperation::Workspace {
            operation: workspace::Operation::ApplyPlan { plan: proposal },
        } = request.operation()
        else {
            panic!("export must use the public plan")
        };
        let workspace::Change::Write { target: save, .. } = &proposal.changes[0] else {
            panic!("Save replacement expected")
        };
        let save = save.clone();
        let workspace_root = workspace_root(&mut manager, "workspace-converter");
        for denied in [
            api::Operation::Editor {
                operation: api::EditorOperation::Workspace {
                    operation: workspace::Operation::Read {
                        target: save.clone(),
                    },
                },
                timeout_ms: 30_000,
            },
            api::Operation::ReadFile {
                handle: save.root.clone(),
                path: String::new(),
            },
            plan(vec![workspace::Change::Delete {
                target: save.clone(),
                expected: workspace::FileStamp::Missing,
            }]),
            plan(vec![workspace::Change::Copy {
                source: save.clone(),
                target: target(&workspace_root, "copied-export.txt"),
                expected_source: workspace::FileStamp::Missing,
            }]),
            plan(vec![workspace::Change::Rename {
                source: save.clone(),
                target: target(&workspace_root, "renamed-export.txt"),
                expected_source: workspace::FileStamp::Missing,
            }]),
            plan(vec![workspace::Change::Write {
                target: target(&save.root, "descendant.txt"),
                expected: None,
                bytes: b"denied".to_vec(),
            }]),
        ] {
            assert!(
                begin(&mut manager, "workspace-converter", denied).is_err(),
                "Save intent must stay exact and write-only"
            );
        }
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            "external before 中😀\r\n"
        );
        confirm(visual);
        let receipt = batch(&request);
        assert_eq!(
            receipt.state,
            workspace::BatchState::Complete,
            "{receipt:?}"
        );
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            "Conversion report 中文🙂\r\n"
        );
        assert_eq!(
            undo(
                visual,
                &app,
                &mut manager,
                "workspace-converter",
                receipt.batch
            )
            .state,
            workspace::BatchState::Undone
        );
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            "external before 中😀\r\n"
        );
        assert_eq!(std::fs::read_to_string(path).unwrap(), "original on disk");
        assert!(!path.parent().unwrap().join("copied-export.txt").exists());
    });
}

/// Windows case aliases must resolve to the unique dirty session instead of becoming disk-only writes.
#[gpui::test]
#[cfg(windows)]
#[ignore = "build workspace-scaffold using the current host --plugin-package first"]
fn workspace_transaction_case_alias_reads_and_writes_the_opened_native_buffer(
    cx: &mut TestAppContext,
) {
    crate::tests::with_shortcut_editor(cx, false, Vec::new(), |visual, app, path| {
        visual.simulate_keystrokes("ctrl-a");
        visual.simulate_input("dirty 中😀\r\n");
        visual.run_until_parked();
        let (_profile, mut manager) = manager(path.parent().unwrap(), "workspace-scaffold");
        let root = workspace_root(&mut manager, "workspace-scaffold");
        let alias = target(&root, "SHORTCUTS.TXT");
        let api::EditorValue::Workspace(workspace::Value::Read { metadata, bytes }) = invoke(
            visual,
            &app,
            &mut manager,
            "workspace-scaffold",
            api::EditorOperation::Workspace {
                operation: workspace::Operation::Read {
                    target: alias.clone(),
                },
            },
        )
        .unwrap() else {
            panic!("read expected")
        };
        assert_eq!(String::from_utf8(bytes).unwrap(), "dirty 中😀\r\n");
        assert!(metadata.document.is_some());
        let (request, _, batch_handle) = preview(
            visual,
            &app,
            &mut manager,
            "workspace-scaffold",
            vec![workspace::Change::Write {
                target: alias,
                expected: Some(metadata.stamp),
                bytes: "changed 中😀\r\n".as_bytes().to_vec(),
            }],
        );
        confirm(visual);
        assert_eq!(batch(&request).state, workspace::BatchState::Complete);
        assert_eq!(text(visual, &app), "changed 中😀\r\n");
        assert_eq!(std::fs::read_to_string(path).unwrap(), "original on disk");
        assert_eq!(
            undo(
                visual,
                &app,
                &mut manager,
                "workspace-scaffold",
                batch_handle
            )
            .state,
            workspace::BatchState::Undone
        );
        assert_eq!(text(visual, &app), "dirty 中😀\r\n");
    });
}
