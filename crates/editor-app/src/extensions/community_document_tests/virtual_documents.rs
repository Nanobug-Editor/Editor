//! Readonly virtual input, replacement and native/runtime lifecycle acceptance.
use super::*;

/// Readonly comparison snapshots use the same selected plugin grammar as ordinary native documents.
/// The frozen consumer supports visible virtual opens; both variants share `open_plugin_virtual`.
#[gpui::test]
#[ignore = "opt-in frozen Package/WASM fixtures; see crates/plugin-runtime/tests/fixtures/packages/README.md"]
fn community_virtual_comparison_sources_apply_selected_language(cx: &mut TestAppContext) {
    crate::tests::with_shortcut_editor(cx, false, Vec::new(), |visual, app, path| {
        let workspace = path.parent().unwrap();
        let mut manager = Manager::open(
            workspace.join(".runtime-plugin-test"),
            Environment {
                workspace: workspace.display().to_string(),
                ..Default::default()
            },
        )
        .unwrap();
        let package = crate::test_packages::package("history-preview-0.1.2");
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
        let language =
            crate::extensions::language_tests::packages::language_package("snapshot-language");
        manager.install(&language, Default::default()).unwrap();
        crate::extensions::language_tests::publish_languages(&app, &manager, visual);
        let mut sources = Vec::new();
        for (title, text) in [
            ("left snapshot", "answer = 41\n"),
            ("right snapshot", "answer = 42\n"),
        ] {
            let api::EditorValue::DocumentOpened(info) = invoke(
                visual,
                &app,
                &mut manager,
                json!({"kind":"open_virtual_document", "title":title, "language":"novel", "text":text}),
            ) else {
                panic!("readonly source expected")
            };
            visual.update(|_, cx| {
                let owner = app.read(cx);
                let editor = owner
                    .tabs
                    .iter()
                    .find(|tab| tab.title() == title)
                    .unwrap()
                    .text
                    .as_ref()
                    .unwrap()
                    .editor
                    .read(cx);
                assert_eq!(
                    editor.language_name().as_ref(),
                    "novel",
                    "virtual sources must enable their declared grammar before mounting"
                );
                assert_eq!(editor.value().as_ref(), text);
            });
            sources.push(info.document);
        }
        invoke(
            visual,
            &app,
            &mut manager,
            json!({"kind":"compare_documents", "left":sources[0], "right":sources[1]}),
        );
        visual.update(|window, cx| window.draw(cx).clear(cx));
        assert!(visual.debug_bounds("document-diff-left").is_some());
        assert!(visual.debug_bounds("document-diff-right").is_some());
        let mut palettes = Vec::new();
        for dark in [false, true] {
            visual.update(|window, cx| {
                crate::theme::apply_theme(crate::theme::builtin_theme(dark), cx);
                window.draw(cx).clear(cx);
                let owner = app.read(cx);
                let mut colors = Vec::new();
                for source in &sources {
                    let index = (0..owner.tabs.len())
                        .find(|&index| owner.plugin_document_version(index).unwrap() == *source)
                        .unwrap();
                    let editor = owner.tabs[index].text.as_ref().unwrap().editor.read(cx);
                    let mut syntax = crate::language::highlighter::SyntaxHighlighter::new(
                        &owner.providers.parser_key(&editor.language_name()),
                    );
                    assert!(syntax.update(None, editor.text(), None));
                    let styles = syntax.styles(
                        &(0..editor.text().len()),
                        cx.theme().highlight_theme.as_ref(),
                    );
                    colors.push(
                        styles
                            .iter()
                            .find(|(range, style)| range.contains(&9) && style.color.is_some())
                            .expect("number capture must use the selected plugin theme")
                            .1
                            .color
                            .unwrap(),
                    );
                    assert!(editor.presentation().is_readonly());
                }
                palettes.push(colors);
            });
        }
        assert_ne!(
            palettes[0], palettes[1],
            "both source palettes follow theme changes"
        );
        for enabled in [false, true] {
            if enabled {
                manager.enable("snapshot-language").unwrap();
            } else {
                manager.disable("snapshot-language").unwrap();
            }
            crate::extensions::language_tests::publish_languages(&app, &manager, visual);
            visual.update(|_, cx| {
                let owner = app.read(cx);
                assert_eq!(owner.providers.grammars().is_empty(), !enabled);
                for source in &sources {
                    let index = (0..owner.tabs.len())
                        .find(|&index| owner.plugin_document_version(index).unwrap() == *source)
                        .unwrap();
                    let editor = owner.tabs[index].text.as_ref().unwrap().editor.read(cx);
                    assert_eq!(
                        editor.language_name().as_ref(),
                        "novel",
                        "explicit source language survives provider withdrawal/reinstall"
                    );
                    let mut syntax = crate::language::highlighter::SyntaxHighlighter::new(
                        &owner.providers.parser_key(&editor.language_name()),
                    );
                    assert!(syntax.update(None, editor.text(), None));
                    assert_eq!(
                        syntax
                            .styles(
                                &(0..editor.text().len()),
                                cx.theme().highlight_theme.as_ref()
                            )
                            .iter()
                            .any(|(range, style)| range.contains(&9) && style.color.is_some()),
                        enabled,
                        "provider withdrawal removes token styles and reenabling restores them"
                    );
                }
            });
        }
    });
}

/// Virtual content is opened through the real guest and remains readonly under native keyboard input.
#[gpui::test]
#[ignore = "opt-in frozen Package/WASM fixtures; see crates/plugin-runtime/tests/fixtures/packages/README.md"]
fn community_virtual_document_rejects_native_input(cx: &mut TestAppContext) {
    crate::tests::with_shortcut_editor(cx, false, Vec::new(), |visual, app, path| {
        let (_runtime, mut manager) = manager(path.parent().unwrap());
        let providers = visual.update(|_, cx| {
            serde_json::to_value(&app.read(cx).session_state.file_view_providers).unwrap()
        });
        let value = invoke(
            visual,
            &app,
            &mut manager,
            json!({"kind":"open_virtual_document", "title":"History.txt", "text":"历史😀\r\nreadonly"}),
        );
        let value = serde_json::to_value(value).unwrap();
        assert_eq!(value["DocumentOpened"]["resource"]["kind"], "virtual");
        assert_eq!(value["DocumentOpened"]["access"]["edit"], false);
        assert_eq!(value["DocumentOpened"]["access"]["save"], false);
        visual.update(|window, cx| {
            app.update(cx, |owner, cx| {
                owner.extensions.update(cx, |panel, _| {
                    // Clear earlier document publications before checking the virtual document's own
                    // public contribution projection; registry discovery never inspects local bundles.
                    let _ = panel
                        .worker
                        .recorded
                        .lock()
                        .unwrap()
                        .try_iter()
                        .collect::<Vec<_>>();
                });
                owner.sync_runtime_contributions(window, cx);
                assert_eq!(
                    serde_json::to_value(&owner.session_state.file_view_providers).unwrap(),
                    providers,
                    "virtual activation cannot replace remembered local providers"
                );
                assert_eq!(
                    owner.plugin_file_context(1).unwrap_err().code,
                    api::ErrorCode::UnsupportedOperation
                );
                let panel = owner.extensions.read(cx);
                assert!(
                    !panel
                        .worker
                        .recorded
                        .lock()
                        .unwrap()
                        .try_iter()
                        .any(|work| matches!(
                            work,
                            Work::Event(_, _, _, PluginEvent::FilePreview { file: Some(_) })
                        )),
                    "virtual identities must not publish local filesystem preview contexts"
                );
            })
        });
        visual.simulate_keystrokes("ctrl-a");
        visual.simulate_input("forbidden");
        visual.simulate_keystrokes("ctrl-s");
        visual.run_until_parked();
        visual.update(|_, cx| {
            let owner = app.read(cx);
            assert_eq!(
                owner.editor.read(cx).value().to_string(),
                "历史😀\r\nreadonly"
            );
            assert_eq!(owner.tabs.len(), 2);
            assert_eq!(
                owner.session_state.open_tabs.len(),
                1,
                "virtual tabs never enter persisted filesystem restoration"
            );
            assert!(!owner.tabs[1].is_dirty());
        });
        assert_eq!(std::fs::read_to_string(path).unwrap(), "original on disk");
        assert_eq!(
            std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
            1,
            "no temporary backing document"
        );
        let initial: api::DocumentInfo =
            serde_json::from_value(value["DocumentOpened"].clone()).unwrap();
        let api::EditorValue::DocumentOpened(refreshed) = invoke(
            visual,
            &app,
            &mut manager,
            json!({"kind":"refresh_virtual_document", "document":initial.document, "text":"新内容\r\nline two😀"}),
        ) else {
            panic!("refresh expected")
        };
        assert_eq!(refreshed.document.id, initial.document.id);
        assert!(refreshed.document.revision > initial.document.revision);
        assert!(!refreshed.dirty);
        assert_eq!(
            invoke_result(
                visual,
                &app,
                &mut manager,
                json!({"kind":"read_document", "document":initial.document})
            )
            .unwrap_err()
            .code,
            api::ErrorCode::StaleRevision
        );
        invoke(
            visual,
            &app,
            &mut manager,
            json!({"kind":"locate_document", "document":refreshed.document, "position":{"line":1, "character":8}}),
        );
        visual.update(|_, cx| assert_eq!(app.read(cx).editor.read(cx).selected_range(), 19..19));
        assert_eq!(invoke_result(visual, &app, &mut manager, json!({"kind":"locate_document", "document":refreshed.document, "position":{"line":1, "character":9}})).unwrap_err().code, api::ErrorCode::InvalidRequest);
        assert_eq!(
            invoke_result(visual, &app, &mut manager, json!({"kind":"read_selection"}))
                .unwrap_err()
                .code,
            api::ErrorCode::UnsupportedOperation
        );
        invoke(
            visual,
            &app,
            &mut manager,
            json!({"kind":"open_document", "resource":refreshed.resource}),
        );
        let virtual_path = visual.update(|_, cx| app.read(cx).tabs[1].path().to_path_buf());
        visual
            .update(|window, cx| app.update(cx, |app, cx| app.close_tab(virtual_path, window, cx)));
        visual.run_until_parked();
        assert_eq!(
            invoke_result(
                visual,
                &app,
                &mut manager,
                json!({"kind":"open_document", "resource":refreshed.resource})
            )
            .unwrap_err()
            .code,
            api::ErrorCode::InvalidHandle
        );
        invoke(
            visual,
            &app,
            &mut manager,
            json!({"kind":"open_virtual_document", "title":"Second history", "text":"next"}),
        );
        manager.disable("history-preview").unwrap();
        visual.update(|_, cx| {
            app.update(cx, |app, cx| {
                cx.notify();
                app.editor_panel.update(cx, |_, cx| cx.notify());
            })
        });
        visual.run_until_parked();
        visual.update(|_, cx| {
            assert_eq!(
                app.read(cx).tabs.len(),
                1,
                "retirement removes native tabs along with authority"
            )
        });
    });
}
