//! Two independent formal languages use the same native request, reference and document lifetime.
use super::*;
use plugin_runtime::plugin_protocol::direct;
use std::time::Instant;

/// Dedicated pure/stdio work must paint a real Base row before the user can select its result.
pub(super) fn wait_for_control(
    visual: &mut VisualTestContext,
    app: &Entity<EditorApp>,
    selector: &'static str,
) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        draw(visual);
        if visual.debug_bounds(selector).is_some() {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "No native language result {selector}: {}",
            visual.update(|_, cx| app.read(cx).status.clone())
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Cancelled work gets an opportunity to finish, proving that an old overlay never reappears.
fn no_late_row(visual: &mut VisualTestContext, stage: &str, name: &str) {
    let deadline = Instant::now() + Duration::from_millis(100);
    loop {
        draw(visual);
        assert!(
            visual.debug_bounds("language-feature-row-0").is_none(),
            "A cancelled {name} reference reappeared after {stage}"
        );
        if Instant::now() >= deadline {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Native caret/focus mutation uses the existing editor state and paints its actual input handler.
pub(super) fn place_caret(visual: &mut VisualTestContext, app: &Entity<EditorApp>, offset: usize) {
    visual.update(|window, cx| {
        let editor = app.read(cx).editor.clone();
        editor.update(cx, |state, cx| {
            state.set_cursor_position(state.text().offset_to_position(offset), window, cx);
        });
        editor.focus_handle(cx).focus(window, cx);
    });
    draw(visual);
}

/// Direct and standard LSP reference selection reveal unsaved native text and retire tab lifetimes.
#[gpui::test]
#[ignore = "package expression-language and keyvalue-language through the current public SDK first"]
fn native_community_languages_share_reference_navigation_and_document_lifetime(
    cx: &mut TestAppContext,
) {
    crate::tests::with_shortcut_editor(cx, false, Vec::new(), |visual, app, original| {
        let root = original.parent().unwrap();
        let mut manager = packages(
            visual,
            &app,
            root,
            &["expression-language", "keyvalue-language"],
        );
        for (name, source, caret) in [
            ("sample.nbexpr", "sum(中文, 2)\n中文 + 中文\nadd(1, 2)", 4),
            (
                "sample.nbkv",
                "port=oops\n中文=1\nother=中文\nsetting(中文, ",
                10,
            ),
        ] {
            eprintln!("Formal native language {name}: references");
            let path = root.join(name);
            std::fs::write(&path, "disk-only value").unwrap();
            // Native tabs store canonical Windows paths; closure must target that exact identity,
            // including its verbatim prefix, rather than a tempfile's unnormalized spelling.
            let path = path.canonicalize().unwrap();
            visual.update(|window, cx| {
                app.update(cx, |app, cx| app.open_file(path.clone(), window, cx));
            });
            draw(visual);
            visual.simulate_keystrokes("ctrl-a");
            visual.simulate_input(source);
            draw(visual);
            // The source comes only from real native input; caret placement uses its public editor seam.
            place_caret(visual, &app, caret);
            if name.ends_with("nbkv") {
                let server =
                    visual.update(|_, cx| app.read(cx).language_servers["keyvalue"].clone());
                server.prepare_until_ready().unwrap();
                visual.update(|_, cx| {
                    let app = app.read(cx);
                    let uri = crate::language::navigation::file_uri(&path).unwrap();
                    eprintln!("KV actual selection={:?}, feature-grant={}, lease={}, language={}, status={}",
                        crate::language::providers::language_servers(), server.supports_language_features(),
                        server.document(&uri).is_some(), editor::language_for_path(&path), app.status);
                });
            }
            visual.simulate_keystrokes("shift-f12");
            wait_for_control(visual, &app, "language-feature-row-1");
            assert!(visual.update(|window, _| {
                gpui_base::test_support::snapshots(window)
                    .iter()
                    .any(|node| {
                        node.role() == Some(gpui_kit::Role::Button)
                            && node.visible()
                            && node.label().is_some_and(|label| label.contains(name))
                    })
            }));
            click(visual, "language-feature-row-1");
            assert_eq!(
                text(visual, &app),
                source,
                "Navigation never writes a result into source"
            );
            assert_eq!(
                visual.update(|_, cx| app.read(cx).editor.read(cx).cursor()),
                source.match_indices("中文").nth(1).unwrap().0,
                "Both providers resolve UTF-16 targets against the unsaved native source"
            );
            assert_eq!(std::fs::read_to_string(&path).unwrap(), "disk-only value");
            assert!(
                visual.update(|window, cx| {
                    app.read(cx).editor.focus_handle(cx).is_focused(window)
                }),
                "Reference navigation returns focus to the native source before Ctrl+."
            );

            // Both real providers propose an ordinary revision-bound edit. A named native row
            // enters the same public actor and preview before text changes; group Undo restores it.
            eprintln!("Formal native language {name}: actions and group Undo");
            visual.simulate_keystrokes("ctrl-.");
            wait_for_control(visual, &app, "language-feature-row-0");
            click(visual, "language-feature-row-0");
            let request = pump(visual, &app, &mut manager)
                .into_iter()
                .find(|request| {
                    matches!(
                        request.operation(),
                        api::EditorOperation::Workspace {
                            operation: workspace::Operation::ApplyPlan { .. }
                        }
                    )
                })
                .expect("A code action must enter the original owner's ordinary plan queue");
            assert_eq!(
                text(visual, &app),
                source,
                "A proposal cannot edit before consent"
            );
            click(visual, "workspace-plan-confirm");
            pump(visual, &app, &mut manager);
            assert_eq!(receipt(&request).state, workspace::BatchState::Complete);
            let changed = if name.ends_with("nbexpr") {
                source.replacen("add(", "sum(", 1)
            } else {
                source.replacen("port=oops", "port=8080", 1)
            };
            assert_eq!(text(visual, &app), changed);
            assert_eq!(std::fs::read_to_string(&path).unwrap(), "disk-only value");
            click(visual, "workspace-batch-undo");
            pump(visual, &app, &mut manager);
            assert_eq!(
                text(visual, &app),
                source,
                "Group Undo restores the native source"
            );

            // Signatures use the same native card and active-parameter name. Unicode source is
            // synchronized through the original direct callback or the already-open stdio lease.
            eprintln!("Formal native language {name}: signatures");
            let (signature_caret, parameter) = if name.ends_with("nbexpr") {
                (source.find(',').unwrap() + 1, "right")
            } else {
                (source.len(), "value")
            };
            place_caret(visual, &app, signature_caret);
            visual.simulate_keystrokes("ctrl-shift-space");
            wait_for_control(visual, &app, "language-signature-card-0");
            visual.update(|window, _| {
                let nodes = gpui_base::test_support::snapshots(window);
                let label = rust_i18n::t!("language_features.parameter", parameter = parameter);
                assert!(
                    nodes
                        .iter()
                        .any(|node| node.label() == Some(label.as_ref()) && node.visible())
                );
            });
            visual.simulate_keystrokes("escape");
            draw(visual);
            assert_eq!(text(visual, &app), source);

            // An actual owner retirement removes its delayed native contribution. Re-enabling
            // creates a new runtime authority; it cannot revive a previously cancelled selection.
            eprintln!("Formal native language {name}: provider disable/replacement");
            place_caret(visual, &app, caret);
            let owner = if name.ends_with("nbexpr") {
                "expression-language"
            } else {
                "keyvalue-language"
            };
            visual.update(|window, cx| {
                app.update(cx, |app, cx| {
                    app.request_language_feature(direct::Feature::References, window, cx);
                });
            });
            manager.disable(owner).unwrap();
            publish_providers(visual, &app, &mut manager);
            no_late_row(visual, "provider disable", name);
            manager.enable(owner).unwrap();
            publish_providers(visual, &app, &mut manager);
            no_late_row(visual, "provider replacement", name);
            if name.ends_with("nbkv") {
                visual
                    .update(|_, cx| app.read(cx).language_servers["keyvalue"].clone())
                    .prepare_until_ready()
                    .unwrap();
            }
            place_caret(visual, &app, caret);

            // All switches occur before a dedicated result can be delivered. Returning to A does
            // not revive its cancelled request, even though its native document still exists.
            eprintln!("Formal native language {name}: A→B→A");
            visual.update(|window, cx| {
                app.update(cx, |app, cx| {
                    app.request_language_feature(direct::Feature::References, window, cx);
                    app.open_file(original.to_path_buf(), window, cx);
                    app.open_file(path.clone(), window, cx);
                });
            });
            no_late_row(visual, "A→B→A", name);

            place_caret(visual, &app, caret);

            // A reopened path has a different native incarnation, not merely the same pathname.
            eprintln!("Formal native language {name}: close/reopen");
            visual.update(|window, cx| {
                app.update(cx, |app, cx| {
                    app.request_language_feature(direct::Feature::References, window, cx);
                    let previous = app
                        .plugin_document_version(app.active_text_tab_index().unwrap())
                        .unwrap();
                    app.discard_tab(path.clone(), window, cx);
                    assert!(app.tabs.iter().all(|tab| tab.path() != path));
                    app.open_file(path.clone(), window, cx);
                    let reopened = app
                        .plugin_document_version(app.active_text_tab_index().unwrap())
                        .unwrap();
                    assert_ne!(
                        previous.id, reopened.id,
                        "The native source was really reopened"
                    );
                });
            });
            no_late_row(visual, "close/reopen", name);
            pump(visual, &app, &mut manager);
        }
    });
}
