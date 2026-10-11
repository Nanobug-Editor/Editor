//! Real independent SDK consumers enter the public Manager and native request publication seam.
use super::*;
use gpui_kit::{TestAppContext, VisualTestContext, gpui};
use plugin_runtime::{
    EditorRequest, Manager, Package,
    plugin_protocol::{Environment, api, ui, workspace},
};

/// Build each actual fixture with the current host --plugin-package before this test is selected.
fn manager(workspace: &Path, plugin: &str) -> (tempfile::TempDir, Manager) {
    let profile = tempfile::tempdir().unwrap();
    let package = Package::read(&Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../target/community-transactions/{plugin}-0.1.1.zip"
    )))
    .unwrap();
    let mut manager = Manager::open(
        profile.path().into(),
        Environment {
            workspace: workspace.display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    (profile, manager)
}

/// Probe output is a bounded ordinary plugin panel, not a private host testing operation.
fn begin(
    manager: &mut Manager,
    plugin: &str,
    operation: api::Operation,
) -> Result<api::Value, api::Failure> {
    super::package_ui_test_support::demand(
        manager,
        plugin,
        plugin_runtime::DemandReason::Command("probe".into()),
    )?;
    manager
        .invoke_command(plugin, "probe", serde_json::to_value(operation).unwrap())
        .unwrap();
    let ui::Kind::Text { text } = &manager.live[plugin]
        .views
        .values()
        .next()
        .unwrap()
        .root
        .kind
    else {
        panic!("probe text expected")
    };
    serde_json::from_str(text).unwrap()
}

/// Mirror the production worker publication without constructing guest authority in the UI test.
fn publish(
    visual: &mut VisualTestContext,
    app: &Entity<EditorApp>,
    manager: &mut Manager,
    plugin: &str,
) -> Vec<EditorRequest> {
    let requests = manager.live.get_mut(plugin).unwrap().take_editor_requests();
    visual.update(|_, cx| {
        app.read(cx).extensions.clone().update(cx, |panel, cx| {
            let mut state = panel.worker.state.lock().unwrap();
            state.publish_manager(manager);
            state.editor_requests.extend(
                requests
                    .iter()
                    .cloned()
                    .map(|request| (plugin.into(), request)),
            );
            drop(state);
            panel.poll(cx);
        })
    });
    visual.run_until_parked();
    requests
}

fn invoke(
    visual: &mut VisualTestContext,
    app: &Entity<EditorApp>,
    manager: &mut Manager,
    plugin: &str,
    operation: api::EditorOperation,
) -> Result<api::EditorValue, api::Failure> {
    let value = begin(
        manager,
        plugin,
        api::Operation::Editor {
            operation,
            timeout_ms: 120_000,
        },
    )?;
    assert!(matches!(value, api::Value::Accepted(_)));
    let request = publish(visual, app, manager, plugin).pop().unwrap();
    let api::RequestUpdate::Completed { result } = request.status() else {
        panic!("completion expected: {:?}", request.status())
    };
    manager.poll();
    result
}

fn text(visual: &mut VisualTestContext, app: &Entity<EditorApp>) -> String {
    visual.update(|_, cx| app.read(cx).editor.read(cx).text().to_string())
}

/// The business fixture starts with distinct disk and unsaved text, plus copy/delete sources.
pub(super) fn seed(visual: &mut VisualTestContext, path: &Path) {
    for (name, bytes) in [
        ("community-disk.txt", "磁盘旧A🙂\r\n旧B\r\n"),
        ("community-template.txt", "模板 中文🙂\r\n"),
        ("community-remove.txt", "可删除原内容\r\n"),
    ] {
        std::fs::write(path.parent().unwrap().join(name), bytes).unwrap();
    }
    visual.simulate_keystrokes("ctrl-a");
    visual.simulate_input("旧A 中文🙂\r\n中段\r\n旧B\r\n未保存 中文🙂");
    visual.run_until_parked();
}

/// A real guest state machine reaches a native preview without test-only plan construction.
pub(super) fn scaffold_preview(
    visual: &mut VisualTestContext,
    app: &Entity<EditorApp>,
    manager: &mut Manager,
) -> EditorRequest {
    super::package_ui_test_support::demand(
        manager,
        "workspace-scaffold",
        plugin_runtime::DemandReason::Command("scaffold".into()),
    )
    .unwrap();
    manager
        .invoke_command("workspace-scaffold", "scaffold", serde_json::Value::Null)
        .unwrap();
    for _ in 0..8 {
        let requests = publish(visual, app, manager, "workspace-scaffold");
        if let Some(request) = requests.iter().find(|request| {
            matches!(
                request.operation(),
                api::EditorOperation::Workspace {
                    operation: workspace::Operation::ApplyPlan { .. }
                }
            )
        }) {
            visual.update(|window, cx| window.draw(cx).clear(cx));
            assert!(
                visual.debug_bounds("workspace-plan-confirm").is_some(),
                "the real plan must wait for native user consent"
            );
            return request.clone();
        }
        for request in requests {
            assert!(
                matches!(
                    request.status(),
                    api::RequestUpdate::Completed { result: Ok(_) }
                ),
                "business input failed: {:?}",
                request.status()
            );
        }
        manager.poll();
    }
    panic!(
        "scaffolder did not produce a bounded plan: {:?}",
        manager.live["workspace-scaffold"]
            .views
            .values()
            .next()
            .unwrap()
            .root
            .kind
    )
}

pub(super) fn confirm(visual: &mut VisualTestContext) {
    let bounds = visual.debug_bounds("workspace-plan-confirm").unwrap();
    visual.simulate_click(bounds.center(), Default::default());
    visual.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear(cx));
}

/// Read Base's painted accessibility properties, rather than assuming visible text names a button.
fn named_button(visual: &mut VisualTestContext, id: &str, label: &str) {
    visual.update(|window, _| {
        let buttons = gpui_base::test_support::snapshots(window);
        let button = buttons
            .iter()
            .find(|item| {
                item.path().iter().any(
                    |part| matches!(part, gpui_kit::ElementId::Name(name) if name.as_str() == id),
                )
            })
            .expect("the real workspace action must be painted");
        assert_eq!(button.role(), Some(gpui_kit::Role::Button));
        assert_eq!(button.label(), Some(label));
        assert!(button.visible());
    });
}

/// Inspect the real native batch completion, including its shared undo handle.
pub(super) fn batch(request: &EditorRequest) -> workspace::Receipt {
    let api::RequestUpdate::Completed {
        result: Ok(api::EditorValue::Workspace(workspace::Value::Batch(receipt))),
    } = request.status()
    else {
        panic!("actual receipt expected: {:?}", request.status())
    };
    receipt
}

/// Native preview and the request/batch handles all originate in the actual SDK guest.
fn preview(
    visual: &mut VisualTestContext,
    app: &Entity<EditorApp>,
    manager: &mut Manager,
    plugin: &str,
    changes: Vec<workspace::Change>,
) -> (EditorRequest, api::ResourceHandle, api::ResourceHandle) {
    let api::Value::BatchAccepted {
        request: handle,
        batch,
    } = begin(
        manager,
        plugin,
        api::Operation::Editor {
            operation: api::EditorOperation::Workspace {
                operation: workspace::Operation::ApplyPlan {
                    plan: workspace::Plan {
                        origin: None,
                        title: "Transaction boundary / 事务边界".into(),
                        changes,
                    },
                },
            },
            timeout_ms: 120_000,
        },
    )
    .unwrap()
    else {
        panic!("owned batch and wait expected")
    };
    let request = publish(visual, app, manager, plugin).pop().unwrap();
    visual.update(|window, cx| window.draw(cx).clear(cx));
    assert!(visual.debug_bounds("workspace-plan-confirm").is_some());
    (request, handle, batch)
}

fn workspace_root(manager: &mut Manager, plugin: &str) -> api::ResourceHandle {
    let api::Value::Resource(handle) =
        begin(manager, plugin, api::Operation::OpenWorkspace).unwrap()
    else {
        panic!("workspace root expected")
    };
    handle
}

fn target(root: &api::ResourceHandle, path: &str) -> workspace::FileTarget {
    workspace::FileTarget {
        root: root.clone(),
        path: path.into(),
    }
}

fn stamp(
    visual: &mut VisualTestContext,
    app: &Entity<EditorApp>,
    manager: &mut Manager,
    plugin: &str,
    target: workspace::FileTarget,
) -> workspace::FileStamp {
    let api::EditorValue::Workspace(workspace::Value::Metadata(metadata)) = invoke(
        visual,
        app,
        manager,
        plugin,
        api::EditorOperation::Workspace {
            operation: workspace::Operation::Stat { target },
        },
    )
    .unwrap() else {
        panic!("metadata expected")
    };
    metadata.stamp
}

fn document(
    visual: &mut VisualTestContext,
    app: &Entity<EditorApp>,
    manager: &mut Manager,
    plugin: &str,
) -> api::DocumentVersion {
    let api::EditorValue::Documents { documents, .. } = invoke(
        visual,
        app,
        manager,
        plugin,
        api::EditorOperation::ListDocuments,
    )
    .unwrap() else {
        panic!("documents expected")
    };
    documents[0].document.clone()
}

/// Undo the accepted workspace batch through its owning guest and native document session.
pub(super) fn undo(
    visual: &mut VisualTestContext,
    app: &Entity<EditorApp>,
    manager: &mut Manager,
    plugin: &str,
    batch: api::ResourceHandle,
) -> workspace::Receipt {
    let api::EditorValue::Workspace(workspace::Value::Batch(receipt)) = invoke(
        visual,
        app,
        manager,
        plugin,
        api::EditorOperation::Workspace {
            operation: workspace::Operation::UndoBatch { batch },
        },
    )
    .unwrap() else {
        panic!("actual recovery receipt expected")
    };
    receipt
}

mod conflicts;
mod converter;
mod limits;
mod recovery;
mod selection;

/// One declared scaffolding command previews and commits eight mixed operations, then restores all.
#[gpui::test]
#[ignore = "build workspace-scaffold using the current host --plugin-package first"]
fn workspace_transaction_scaffolder_preserves_unsaved_text_and_recovers_mixed_batch(
    cx: &mut TestAppContext,
) {
    crate::tests::with_shortcut_editor(cx, false, Vec::new(), |visual, app, path| {
        seed(visual, path);
        let original = text(visual, &app);
        let root = path.parent().unwrap();
        let (_profile, mut manager) = manager(root, "workspace-scaffold");
        let request = scaffold_preview(visual, &app, &mut manager);
        named_button(
            visual,
            "workspace-plan-cancel",
            &t!("interaction.cancel").to_string(),
        );
        named_button(
            visual,
            "workspace-plan-confirm",
            &t!("workspace_plan.apply").to_string(),
        );
        visual.update(|window, _| {
            assert!(
                gpui_base::test_support::snapshots(window)
                    .iter()
                    .any(|item| item.role() == Some(gpui_kit::Role::Dialog)),
                "preview retains Base's native modal role"
            );
        });
        assert_eq!(text(visual, &app), original);
        assert_eq!(
            std::fs::read_to_string(root.join("community-disk.txt")).unwrap(),
            "磁盘旧A🙂\r\n旧B\r\n"
        );
        assert!(!root.join("community-generated").exists());
        confirm(visual);
        let receipt = batch(&request);
        assert_eq!(
            receipt.state,
            workspace::BatchState::Complete,
            "{receipt:?}"
        );
        assert_eq!(receipt.completed, (0..8).collect::<Vec<_>>());
        assert!(receipt.recoverable);
        assert_eq!(
            text(visual, &app),
            "生成A 中文🙂\r\n中段\r\n生成B\r\n未保存 中文🙂"
        );
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "original on disk",
            "opened text must remain unsaved"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("community-disk.txt")).unwrap(),
            "脚手架磁盘 中文🙂\r\n"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("community-generated/final.txt")).unwrap(),
            "模板 中文🙂\r\n"
        );
        assert!(!root.join("community-generated/copied.txt").exists());
        assert!(!root.join("community-generated/intermediate.txt").exists());
        assert!(!root.join("community-remove.txt").exists());
        named_button(
            visual,
            &format!("workspace-batch-undo-{}", receipt.batch.resource),
            &t!("workspace_plan.undo").to_string(),
        );
        let api::EditorValue::Workspace(workspace::Value::Batch(undone)) = invoke(
            visual,
            &app,
            &mut manager,
            "workspace-scaffold",
            api::EditorOperation::Workspace {
                operation: workspace::Operation::UndoBatch {
                    batch: receipt.batch,
                },
            },
        )
        .unwrap() else {
            panic!("Undo receipt expected")
        };
        assert_eq!(undone.state, workspace::BatchState::Undone, "{undone:?}");
        assert!(!undone.recoverable);
        assert_eq!(text(visual, &app), original);
        assert_eq!(std::fs::read_to_string(path).unwrap(), "original on disk");
        assert_eq!(
            std::fs::read_to_string(root.join("community-disk.txt")).unwrap(),
            "磁盘旧A🙂\r\n旧B\r\n"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("community-remove.txt")).unwrap(),
            "可删除原内容\r\n"
        );
        assert!(!root.join("community-generated").exists());
    });
}

/// Two original-revision byte ranges form exactly one local Undo action and preserve CRLF/non-BMP.
#[gpui::test]
#[ignore = "build workspace-scaffold using the current host --plugin-package first"]
fn workspace_transaction_sdk_multiple_ranges_are_one_native_undo(cx: &mut TestAppContext) {
    crate::tests::with_shortcut_editor(cx, false, Vec::new(), |visual, app, path| {
        let (_profile, mut manager) = manager(path.parent().unwrap(), "workspace-scaffold");
        visual.simulate_keystrokes("ctrl-a");
        visual.simulate_input("旧A 中文🙂\r\n中段\r\n旧B\r\n");
        visual.run_until_parked();
        let original = text(visual, &app);
        let api::EditorValue::Documents { documents, .. } = invoke(
            visual,
            &app,
            &mut manager,
            "workspace-scaffold",
            api::EditorOperation::ListDocuments,
        )
        .unwrap() else {
            panic!("documents expected")
        };
        let before = documents[0].document.clone();
        let api::EditorValue::Edited { document, .. } = invoke(
            visual,
            &app,
            &mut manager,
            "workspace-scaffold",
            api::EditorOperation::ApplyEdits {
                document: before.clone(),
                edits: vec![
                    api::TextEdit {
                        range: api::TextRange { start: 0, end: 4 },
                        text: "生成A".into(),
                    },
                    api::TextEdit {
                        range: api::TextRange { start: 25, end: 29 },
                        text: "生成B".into(),
                    },
                ],
                selection: api::TextRange { start: 0, end: 0 },
            },
        )
        .unwrap() else {
            panic!("edit completion expected")
        };
        assert_eq!(document.revision, before.revision + 1);
        assert_eq!(text(visual, &app), "生成A 中文🙂\r\n中段\r\n生成B\r\n");
        assert_eq!(std::fs::read_to_string(path).unwrap(), "original on disk");
        visual.simulate_keystrokes("ctrl-end");
        visual.simulate_input("后续中文😀");
        visual.run_until_parked();
        visual.simulate_keystrokes("ctrl-z");
        visual.run_until_parked();
        assert_eq!(text(visual, &app), "生成A 中文🙂\r\n中段\r\n生成B\r\n");
        visual.simulate_keystrokes("ctrl-z");
        visual.run_until_parked();
        assert_eq!(
            text(visual, &app),
            original,
            "one Undo must restore both separated ranges"
        );
        visual.simulate_keystrokes("ctrl-y");
        visual.run_until_parked();
        assert_eq!(text(visual, &app), "生成A 中文🙂\r\n中段\r\n生成B\r\n");
        visual.simulate_keystrokes("ctrl-y");
        visual.run_until_parked();
        assert!(text(visual, &app).ends_with("后续中文😀"));
    });
}
