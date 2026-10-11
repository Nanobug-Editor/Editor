//! Two business checkers share the native model, own source sets and reuse previewed transactions.
use super::*;

/// Real business fixes cover local text, an outside-only target, cancellation, stale source and Undo.
#[gpui::test]
#[ignore = "package spell-check and config-check through the current host --plugin-package first"]
fn native_community_language_diagnostics_fix_preview_source_revision_and_undo(
    cx: &mut TestAppContext,
) {
    crate::tests::with_shortcut_editor(cx, false, Vec::new(), |visual, app, path| {
        visual.simulate_keystrokes("ctrl-a");
        visual.simulate_input("teh\ninclude=generated.conf");
        draw(visual);
        let original = text(visual, &app);
        let mut manager = packages(
            visual,
            &app,
            path.parent().unwrap(),
            &["spell-check", "config-check"],
        );
        for id in ["spell-check", "config-check"] {
            manager
                .invoke_command(id, "check", serde_json::Value::Null)
                .unwrap();
        }
        pump(visual, &app, &mut manager);
        assert_eq!(
            visual.update(
                |_, cx| app.read(cx).tabs[app.read(cx).active_text_tab_index().unwrap()]
                    .text
                    .as_ref()
                    .unwrap()
                    .diagnostics
                    .direct
                    .len()
            ),
            2
        );
        select_problem(visual, &app, "teh");
        assert!(
            visual.update(|window, _| gpui_base::test_support::snapshots(window)
                .iter()
                .any(|node| node.role() == Some(gpui_kit::Role::Dialog))),
            "Problems preserves Base modal semantics"
        );
        click(visual, "problem-related-0");
        assert_eq!(
            visual.update(|_, cx| app.read(cx).editor.read(cx).cursor()),
            0
        );
        let spell = fix_preview(visual, &app, &mut manager, "teh");
        assert_eq!(text(visual, &app), original);
        click(visual, "workspace-plan-confirm");
        pump(visual, &app, &mut manager);
        let spell_receipt = receipt(&spell);
        assert_eq!(spell_receipt.state, workspace::BatchState::Complete);
        assert_eq!(text(visual, &app), "the\ninclude=generated.conf");
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "original on disk",
            "A single-document fix stays unsaved"
        );
        assert!(
            matches!(invoke(visual, &app, &mut manager, "spell-check", api::EditorOperation::Workspace { operation: workspace::Operation::UndoBatch { batch: spell_receipt.batch } }).unwrap(), api::EditorValue::Workspace(workspace::Value::Batch(restored)) if restored.state == workspace::BatchState::Undone)
        );
        assert_eq!(text(visual, &app), original);
        pump(visual, &app, &mut manager);

        let cancelled = fix_preview(visual, &app, &mut manager, "generated.conf");
        click(visual, "workspace-plan-cancel");
        pump(visual, &app, &mut manager);
        assert_eq!(receipt(&cancelled).state, workspace::BatchState::Cancelled);
        assert_eq!(rejected(&cancelled).code, api::ErrorCode::Cancelled);
        assert!(!path.parent().unwrap().join("generated.conf").exists());
        let outside = fix_preview(visual, &app, &mut manager, "generated.conf");
        click(visual, "workspace-plan-confirm");
        pump(visual, &app, &mut manager);
        let outside_receipt = receipt(&outside);
        assert_eq!(outside_receipt.state, workspace::BatchState::Complete);
        assert_eq!(
            std::fs::read_to_string(path.parent().unwrap().join("generated.conf")).unwrap(),
            "port=8080\n"
        );
        assert_eq!(
            text(visual, &app),
            original,
            "An origin is readonly and adds no source edit"
        );
        invoke(
            visual,
            &app,
            &mut manager,
            "config-check",
            api::EditorOperation::Workspace {
                operation: workspace::Operation::UndoBatch {
                    batch: outside_receipt.batch,
                },
            },
        )
        .unwrap();
        assert!(!path.parent().unwrap().join("generated.conf").exists());
        manager
            .invoke_command("config-check", "check", serde_json::Value::Null)
            .unwrap();
        pump(visual, &app, &mut manager);

        let stale = fix_preview(visual, &app, &mut manager, "generated.conf");
        // Real native input commits a new capability revision while the outside-only plan is open.
        visual.update(|window, cx| app.read(cx).editor.focus_handle(cx).focus(window, cx));
        visual.simulate_keystrokes("ctrl-end");
        visual.simulate_input("\nchanged");
        draw(visual);
        if visual.debug_bounds("workspace-plan-confirm").is_some() {
            click(visual, "workspace-plan-confirm");
        }
        pump(visual, &app, &mut manager);
        assert_eq!(
            rejected(&stale).code,
            api::ErrorCode::StaleRevision,
            "Readonly origin must reject edits made during preview"
        );
        assert!(!path.parent().unwrap().join("generated.conf").exists());
        manager
            .invoke_command("config-check", "check", serde_json::Value::Null)
            .unwrap();
        pump(visual, &app, &mut manager);
        let reopened = fix_preview(visual, &app, &mut manager, "generated.conf");
        let native = path.canonicalize().unwrap();
        visual.update(|window, cx| {
            app.update(cx, |app, cx| {
                app.discard_tab(native.clone(), window, cx);
                app.open_file(native, window, cx);
            })
        });
        draw(visual);
        if visual.debug_bounds("workspace-plan-confirm").is_some() {
            click(visual, "workspace-plan-confirm");
        }
        pump(visual, &app, &mut manager);
        assert_eq!(
            rejected(&reopened).code,
            api::ErrorCode::StaleRevision,
            "Reopening the path creates a new native source identity"
        );
        assert!(!path.parent().unwrap().join("generated.conf").exists());
    });
}

/// The same position/message from two real installed owners remains two native markers and rows.
#[gpui::test]
#[ignore = "package spell-check and config-check through the current host --plugin-package first"]
fn native_community_language_diagnostics_preserve_independent_owners_and_cleanup(
    cx: &mut TestAppContext,
) {
    crate::tests::with_shortcut_editor(cx, true, Vec::new(), |visual, app, path| {
        let mut manager = packages(
            visual,
            &app,
            path.parent().unwrap(),
            &["spell-check", "config-check"],
        );
        let document = visual.update(|_, cx| app.read(cx).plugin_document_version(0).unwrap());
        let mut handles = Vec::new();
        for id in ["spell-check", "config-check"] {
            let api::Value::Resource(collection) = begin(
                &mut manager,
                id,
                api::Operation::Diagnostics {
                    operation: diagnostics::Operation::Create {
                        label: "Shared label / 同名来源".into(),
                    },
                },
            )
            .unwrap() else {
                panic!("Owned collection expected")
            };
            invoke(
                visual,
                &app,
                &mut manager,
                id,
                api::EditorOperation::Diagnostics {
                    publication: diagnostics::Publication::Publish {
                        collection: collection.clone(),
                        document: document.clone(),
                        diagnostics: vec![diagnostics::Diagnostic {
                            id: "same".into(),
                            severity: diagnostics::Severity::Warning,
                            range: api::TextRange { start: 0, end: 1 },
                            message: "Same position and message / 同位置同消息".into(),
                            code: None,
                            related: Vec::new(),
                            fixes: Vec::new(),
                        }],
                    },
                },
            )
            .unwrap();
            handles.push(collection);
        }
        let sources = visual.update(|_, cx| {
            app.read(cx)
                .editor
                .read(cx)
                .diagnostics()
                .unwrap()
                .iter()
                .filter(|item| item.message.as_str().contains("Same position"))
                .map(|item| item.source.clone())
                .collect::<Vec<_>>()
        });
        assert_eq!(sources.len(), 2);
        assert_ne!(
            sources[0], sources[1],
            "Native source attribution retains the original owner even for a shared label"
        );
        select_problem(visual, &app, "Same position");
        assert!(visual.debug_bounds("native-problem-0").is_some());
        assert!(visual.debug_bounds("native-problem-1").is_some());
        visual.simulate_keystrokes("end enter");
        draw(visual);
        assert_eq!(
            visual.update(|_, cx| app.read(cx).editor.read(cx).cursor()),
            0
        );
        assert_eq!(
            begin(
                &mut manager,
                "config-check",
                api::Operation::Editor {
                    operation: api::EditorOperation::Diagnostics {
                        publication: diagnostics::Publication::Clear {
                            collection: handles[0].clone(),
                            document: document.clone()
                        }
                    },
                    timeout_ms: 30_000
                }
            )
            .unwrap_err()
            .code,
            api::ErrorCode::InvalidHandle
        );
        invoke(
            visual,
            &app,
            &mut manager,
            "spell-check",
            api::EditorOperation::Diagnostics {
                publication: diagnostics::Publication::Clear {
                    collection: handles[0].clone(),
                    document,
                },
            },
        )
        .unwrap();
        assert_eq!(
            visual.update(|_, cx| app
                .read(cx)
                .editor
                .read(cx)
                .diagnostics()
                .unwrap()
                .iter()
                .filter(|item| item.message.as_str().contains("Same position"))
                .count()),
            1
        );
        manager.disable("config-check").unwrap();
        pump(visual, &app, &mut manager);
        draw(visual);
        assert_eq!(
            visual.update(|_, cx| app
                .read(cx)
                .editor
                .read(cx)
                .diagnostics()
                .map_or(0, |set| set
                    .iter()
                    .filter(|item| item.message.as_str().contains("Same position"))
                    .count())),
            0
        );
    });
}
