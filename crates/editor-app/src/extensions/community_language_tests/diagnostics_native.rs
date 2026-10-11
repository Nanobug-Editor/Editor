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
            super::super::package_ui_test_support::demand(
                &mut manager,
                id,
                plugin_runtime::DemandReason::Command("check".into()),
            )
            .unwrap();
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
        // Open from the editor's shortcut without clicking a child first: Base keeps initial
        // focus on its Dialog, so row arrows must work from that actual dispatch path too.
        visual.simulate_keystrokes("ctrl-shift-m");
        draw(visual);
        let visible_fix = |visual: &mut VisualTestContext, label: &str| {
            visual.update(|window, _| {
                gpui_base::test_support::snapshots(window)
                    .iter()
                    .any(|node| {
                        node.role() == Some(gpui_kit::Role::Button)
                            && node.visible()
                            && node.label().is_some_and(|name| name.contains(label))
                    })
            })
        };
        assert!(visible_fix(visual, "Replace with the"));
        visual.simulate_keystrokes("down");
        draw(visual);
        assert!(
            visible_fix(visual, "Create default configuration"),
            "Down from the initially focused Base Dialog must select the other source and its fix"
        );
        visual.simulate_keystrokes("up");
        draw(visual);
        assert!(visible_fix(visual, "Replace with the"));
        visual.simulate_keystrokes("end");
        draw(visual);
        assert!(visible_fix(visual, "Create default configuration"));
        visual.simulate_keystrokes("home");
        draw(visual);
        assert!(visible_fix(visual, "Replace with the"));
        visual.simulate_keystrokes("down");
        draw(visual);
        // Match a real Base keyboard click: the shortcut helper alone only emits KeyDown.
        let keystroke = gpui_kit::Keystroke::parse("enter").unwrap();
        visual.simulate_event(gpui_kit::KeyDownEvent {
            keystroke: keystroke.clone(),
            is_held: false,
            prefer_character_input: false,
        });
        visual.simulate_event(gpui_kit::KeyUpEvent { keystroke });
        draw(visual);
        assert!(visual.debug_bounds("native-problems-list").is_none());
        assert_eq!(
            visual.update(|_, cx| app.read(cx).editor.read(cx).cursor()),
            12
        );
        assert_eq!(text(visual, &app), original);
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

/// A real missing-file publication must point at the trimmed Unicode path's complete UTF-8 bytes.
#[gpui::test]
#[ignore = "package config-check through the current host --plugin-package first"]
fn native_community_language_config_trimmed_unicode_path_keeps_exact_diagnostic_span(
    cx: &mut TestAppContext,
) {
    crate::tests::with_shortcut_editor(cx, false, Vec::new(), |visual, app, path| {
        let mut manager = packages(visual, &app, path.parent().unwrap(), &["config-check"]);
        for (source, expected_path, start, end) in [
            ("include=中", "中", 8, 11),
            ("include= 中 ", "中", 9, 12),
            ("include=\t 文件🙂.conf  \t", "文件🙂.conf", 10, 25),
        ] {
            visual.simulate_keystrokes("ctrl-a");
            visual.simulate_input(source);
            draw(visual);
            super::super::package_ui_test_support::demand(
                &mut manager,
                "config-check",
                plugin_runtime::DemandReason::Command("check".into()),
            )
            .unwrap();
            manager
                .invoke_command("config-check", "check", serde_json::Value::Null)
                .unwrap();
            pump(visual, &app, &mut manager);
            let diagnostics = visual.update(|_, cx| {
                let app = app.read(cx);
                app.tabs[app.active_text_tab_index().unwrap()]
                    .text
                    .as_ref()
                    .unwrap()
                    .diagnostics
                    .direct
                    .values()
                    .filter(|publication| publication.collection.owner() == "config-check")
                    .flat_map(|publication| publication.items.clone())
                    .collect::<Vec<_>>()
            });
            assert_eq!(
                diagnostics.len(),
                1,
                "A trimmed Unicode path lost its missing-file diagnostic"
            );
            assert_eq!(diagnostics[0].range, api::TextRange { start, end });
            assert_eq!(&source[start..end], expected_path);
            assert!(matches!(&diagnostics[0].fixes[0].plan.changes[0],
                workspace::Change::Write { target, expected: Some(workspace::FileStamp::Missing), .. }
                if target.path == expected_path));
            assert_eq!(text(visual, &app), source);
            assert!(!path.parent().unwrap().join(expected_path).exists());
        }
        manager.shutdown();
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
                    document: document.clone(),
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
        let retained = visual.update(|_, cx| {
            app.read(cx).tabs[0]
                .text
                .as_ref()
                .unwrap()
                .diagnostics
                .direct
                .values()
                .find(|publication| publication.collection.owner() == "config-check")
                .unwrap()
                .collection
                .clone()
        });
        // Retiring A and reconciling actual contributions must preserve B's exact-current
        // publication. No document change is sent and B is not asked to publish it again.
        manager.disable("spell-check").unwrap();
        assert!(
            retained.is_live(),
            "Retiring A revoked independent B's collection seal"
        );
        publish_providers(visual, &app, &mut manager);
        assert_eq!(
            visual.update(|_, cx| app.read(cx).plugin_document_version(0).unwrap()),
            document
        );
        assert_eq!(
            matching_sources(visual, &app).len(),
            1,
            "Disabling A erased live unchanged B"
        );
        assert!(
            matching_sources(visual, &app)[0]
                .as_deref()
                .is_some_and(|owner| owner.contains("config-check"))
        );

        // A genuine independent language install and provider preference refresh use the
        // same parser-reset seam. Their grammar policy grants no authority over B's collection.
        let language = Package::read(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/community-language/expression-language-0.1.0.zip"),
        )
        .unwrap();
        manager
            .install(&language, language.manifest.permissions.clone())
            .unwrap();
        publish_providers(visual, &app, &mut manager);
        crate::language::providers::choose(
            protocol::settings::Scope::User,
            "references:expression",
            Some("expression-language/expressions"),
        )
        .unwrap();
        visual.update(|_, cx| app.update(cx, |app, cx| app.sync_dynamic_languages(cx)));
        draw(visual);
        assert_eq!(
            visual.update(|_, cx| app.read(cx).plugin_document_version(0).unwrap()),
            document
        );
        assert_eq!(
            matching_sources(visual, &app).len(),
            1,
            "A language selection erased live unchanged B"
        );

        // A real edit does invalidate B. Re-publication at the new native version remains legal,
        // and then retiring B removes the final marker without clearing another owner's collection.
        visual.simulate_keystrokes("escape");
        visual.update(|window, cx| app.read(cx).editor.focus_handle(cx).focus(window, cx));
        visual.simulate_keystrokes("ctrl-end");
        visual.simulate_input("\nchanged");
        draw(visual);
        assert!(
            matching_sources(visual, &app).is_empty(),
            "An edited source retained obsolete markers"
        );
        let updated = visual.update(|_, cx| app.read(cx).plugin_document_version(0).unwrap());
        assert_ne!(updated.revision, document.revision);
        invoke(
            visual,
            &app,
            &mut manager,
            "config-check",
            api::EditorOperation::Diagnostics {
                publication: diagnostics::Publication::Publish {
                    collection: handles[1].clone(),
                    document: updated,
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
        assert_eq!(matching_sources(visual, &app).len(), 1);
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

        // Unlike the direct-provider declaration's mandatory read authority, diagnostics are
        // an ordinary operation: an admitted checker without the permission receives typed denial.
        let spell = Package::read(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/community-language/spell-check-0.1.0.zip"),
        )
        .unwrap();
        let mut files = spell.files;
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&files["manifest.json"]).unwrap();
        manifest["permissions"]
            .as_array_mut()
            .unwrap()
            .retain(|permission| permission != "editor.diagnostics");
        files.insert(
            "manifest.json".into(),
            serde_json::to_vec(&manifest).unwrap(),
        );
        let denied = Package::from_files(files).unwrap();
        manager
            .install(&denied, denied.manifest.permissions.clone())
            .unwrap();
        // Replacement deliberately preserves A's prior disabled choice; enable the admitted
        // checker before testing its operation permission, rather than calling a stopped guest.
        manager.enable("spell-check").unwrap();
        publish_providers(visual, &app, &mut manager);
        pump(visual, &app, &mut manager);
        let original = text(visual, &app);
        assert_eq!(
            begin(
                &mut manager,
                "spell-check",
                api::Operation::Diagnostics {
                    operation: diagnostics::Operation::Create {
                        label: "Denied checker / 未授权来源".into()
                    },
                }
            )
            .unwrap_err()
            .code,
            api::ErrorCode::PermissionDenied
        );
        assert!(manager.take_editor_requests().is_empty());
        assert!(matching_sources(visual, &app).is_empty());
        assert_eq!(text(visual, &app), original);
        manager.shutdown();
    });
}

/// Count actual Base markers rather than keeping a second diagnostic cache in this fixture.
fn matching_sources(
    visual: &mut VisualTestContext,
    app: &Entity<EditorApp>,
) -> Vec<Option<String>> {
    visual.update(|_, cx| {
        app.read(cx)
            .editor
            .read(cx)
            .diagnostics()
            .map_or_else(Vec::new, |set| {
                set.iter()
                    .filter(|item| item.message.as_str().contains("Same position"))
                    .map(|item| item.source.as_ref().map(|source| source.to_string()))
                    .collect()
            })
    })
}
