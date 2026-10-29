//! Actual package file selection must reveal one comparison preview without a source-file tab.
use super::*;

/// A replacement may reuse native text/layout, but a peer's old document marker never follows it.
#[gpui::test]
#[ignore = "real Git candidate and frozen bookmarks WASM; set NANOBUG_GIT_CANDIDATE and prepare Package fixtures"]
fn git_comparison_source_cache_retires_old_marker_handles(cx: &mut TestAppContext) {
    use crate::extensions::community_presentation_tests::{admit_marker, consume_marker};
    use plugin_runtime::plugin_protocol::{api, decorations as marks, ui};

    if !run_isolated_test(
        "extensions::windows_tests::comparisons::source_tabs::git_comparison_source_cache_retires_old_marker_handles",
    ) {
        return;
    }
    with_comparison(cx, "base\n", "changed\n", |manager, visual, app, file| {
        let root = file.parent().unwrap();
        let second = root.join("second.txt");
        std::fs::write(&second, "other base\n").unwrap();
        assert!(git(root, &["add", "--", "second.txt"]));
        assert!(git(root, &["commit", "-m", "second source"]));
        std::fs::write(&second, "other changed\n").unwrap();
        let package = crate::test_packages::package("bookmarks-0.1.1");
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
        // The helper opened Git before second.txt existed. Refresh its captured change list;
        // otherwise resume_comparison_selection legitimately discards this unknown file later.
        manager.invoke_command("git", "refresh", json!({})).unwrap();
        flush(manager, app, visual);
        manager
            .invoke_command("git", "diff", json!({"path":"diff.txt"}))
            .unwrap();
        flush(manager, app, visual);
        assert_sources(visual, app, "base\n", "changed\n");
        let (old_version, old_resource, original_editor, original_markers) =
            visual.update(|_, cx| {
                let owner = app.read(cx);
                let (index, tab) = owner
                    .tabs
                    .iter()
                    .enumerate()
                    .find(|(_, tab)| {
                        tab.virtual_document
                            .as_ref()
                            .is_some_and(|source| source.hidden)
                    })
                    .expect("the real Git preview owns one hidden readonly base");
                let text = tab.text.as_ref().unwrap();
                (
                    owner.plugin_document_version(index).unwrap(),
                    tab.virtual_document.as_ref().unwrap().resource.clone(),
                    text.editor.clone(),
                    text.highlighter.markers(),
                )
            });
        let operation = |operation| api::Operation::Editor {
            operation: api::EditorOperation::Decorations { operation },
            // The deliberately delayed request spans two real Git switches in a debug UI test.
            // Keep its deadline separate from the document-retirement failure being asserted.
            timeout_ms: 300_000,
        };
        let marker = |id: &str| marks::Marker {
            id: id.into(),
            anchor: marks::Anchor::Range {
                range: api::TextRange { start: 0, end: 4 },
            },
            label: ui::LocalizedText::new("Source marker", "来源标记"),
            icon: None,
            style: marks::Style::Accent,
        };
        // The independent public provider issues the actual marker handle. Its probe does not
        // retain that handle in bookmark state, so its close-event handler cannot mask host cleanup.
        let request = admit_marker(
            manager,
            "bookmarks",
            operation(marks::Operation::Create {
                document: old_version.clone(),
                markers: vec![marker("retired-source")],
            }),
        )
        .unwrap();
        let collection = request.decoration().unwrap().clone();
        assert!(matches!(
            consume_marker(visual, app, manager, "bookmarks", &request),
            api::RequestUpdate::Completed { result: Ok(_) }
        ));
        assert!(collection.is_live());
        assert!(original_markers.borrow().contains(&collection));
        assert_eq!(
            original_markers.borrow().positions(&collection).unwrap()[0].range,
            Some(api::TextRange { start: 0, end: 4 })
        );
        // Remove this accepted request from the runtime queue but withhold native consumption
        // until the same editor entity belongs to a new virtual document incarnation.
        let delayed = admit_marker(
            manager,
            "bookmarks",
            operation(marks::Operation::Create {
                document: old_version.clone(),
                markers: vec![marker("late-retired-source")],
            }),
        )
        .unwrap();
        let delayed_collection = delayed.decoration().unwrap().clone();
        assert!(delayed_collection.is_live());
        assert!(delayed_collection.document().is_none());

        manager
            .invoke_command("git", "diff", json!({"path":"second.txt"}))
            .unwrap();
        // Bridge quiescence can precede a queued selection. Wait for actual native text before
        // testing retirement, so a discarded/deferred request cannot impersonate a cache defect.
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            flush(manager, app, visual);
            if visual
                .update(|_, cx| app.read(cx).editor.read(cx).value().as_str() == "other changed\n")
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "second file comparison did not open"
            );
        }
        assert_sources(visual, app, "other base\n", "other changed\n");
        assert!(
            !old_resource.is_live(),
            "replacement retires the old source"
        );
        assert!(
            !collection.is_live(),
            "retaining native layout must still revoke old document decoration authority"
        );
        assert!(!original_markers.borrow().contains(&collection));

        manager
            .invoke_command("git", "diff", json!({"path":"diff.txt"}))
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            flush(manager, app, visual);
            if visual.update(|_, cx| app.read(cx).editor.read(cx).value().as_str() == "changed\n") {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "original file comparison did not reopen"
            );
        }
        assert_sources(visual, app, "base\n", "changed\n");
        let new_version = visual.update(|_, cx| {
            let owner = app.read(cx);
            let (index, tab) = owner
                .tabs
                .iter()
                .enumerate()
                .find(|(_, tab)| {
                    tab.virtual_document
                        .as_ref()
                        .is_some_and(|source| source.hidden)
                })
                .unwrap();
            let text = tab.text.as_ref().unwrap();
            assert_eq!(
                text.editor, original_editor,
                "exercise a cache hit, not the ordinary new-editor/drop path"
            );
            let new_version = owner.plugin_document_version(index).unwrap();
            assert_ne!(new_version.id, old_version.id);
            // Reusing native pixels cannot revive any request carrying the retired document token.
            assert_eq!(
                owner.plugin_document_target(&old_version).unwrap_err().code,
                api::ErrorCode::StaleRevision
            );
            assert_eq!(text.editor.read(cx).value().as_str(), "base\n");
            assert!(!text.highlighter.markers().borrow().contains(&collection));
            assert_eq!(
                text.highlighter
                    .markers()
                    .borrow()
                    .positions(&collection)
                    .unwrap_err()
                    .code,
                api::ErrorCode::InvalidHandle
            );
            new_version
        });
        let delayed_result = consume_marker(visual, app, manager, "bookmarks", &delayed);
        assert!(
            matches!(
                delayed_result,
                api::RequestUpdate::Completed {
                    result: Err(api::Failure {
                        code: api::ErrorCode::InvalidHandle,
                        ..
                    })
                }
            ),
            "a delayed old-version publication must fail after reuse: {delayed_result:?}"
        );
        assert!(!delayed_collection.is_live());
        assert!(delayed_collection.document().is_none());
        assert!(!original_markers.borrow().contains(&delayed_collection));
        assert_eq!(collection.document().unwrap(), old_version);
        let stale = admit_marker(
            manager,
            "bookmarks",
            operation(marks::Operation::Read {
                handle: collection.handle().clone(),
            }),
        )
        .err()
        .expect("the old handle must fail at runtime admission after native reuse");
        assert_eq!(stale.code, api::ErrorCode::InvalidHandle);

        // The new incarnation must still accept and read fresh public marker authority.
        let fresh = admit_marker(
            manager,
            "bookmarks",
            operation(marks::Operation::Create {
                document: new_version.clone(),
                markers: vec![marker("fresh-source")],
            }),
        )
        .unwrap();
        let fresh_collection = fresh.decoration().unwrap().clone();
        assert!(matches!(
            consume_marker(visual, app, manager, "bookmarks", &fresh),
            api::RequestUpdate::Completed { result: Ok(_) }
        ));
        assert!(fresh_collection.is_live());
        assert_eq!(fresh_collection.document().as_ref(), Some(&new_version));
        assert!(original_markers.borrow().contains(&fresh_collection));
        let read = admit_marker(
            manager,
            "bookmarks",
            operation(marks::Operation::Read {
                handle: fresh_collection.handle().clone(),
            }),
        )
        .unwrap();
        let api::RequestUpdate::Completed {
            result:
                Ok(api::EditorValue::Decorations {
                    document,
                    positions,
                    ..
                }),
        } = consume_marker(visual, app, manager, "bookmarks", &read)
        else {
            panic!("the reused source must serve the newly admitted collection");
        };
        assert_eq!(document, new_version);
        assert_eq!(positions.len(), 1);
        assert_eq!(
            positions[0].range,
            Some(api::TextRange { start: 0, end: 4 })
        );
    });
}

/// Inspect the actual mounted native sessions rather than the plugin's patch summary or cache.
fn assert_sources(
    visual: &mut VisualTestContext,
    app: &Entity<EditorApp>,
    left: &str,
    right: &str,
) {
    paint_comparison(visual);
    let left_point = visual.update(|_, cx| {
        let bounds = app
            .read(cx)
            .comparison_alignment()
            .unwrap()
            .project_range(0, 0..0, cx)
            .expect("visible left comparison text");
        point(bounds.left() + px(0.4), bounds.center().y)
    });
    visual.simulate_click(left_point, Default::default());
    visual.update(|window, cx| {
        let app = app.read(cx);
        eprintln!(
            "displayed comparison {:?}: {:?} -> {:?}",
            app.comparison_views
                .iter()
                .map(|view| (&view.title, view.preview))
                .collect::<Vec<_>>(),
            left,
            right
        );
        let left_editor = app
            .tabs
            .iter()
            .filter_map(|tab| tab.text.as_ref())
            .find(|text| text.editor.focus_handle(cx).is_focused(window))
            .expect("readonly comparison base");
        assert_eq!(
            left_editor.editor.read(cx).value().as_str(),
            left,
            "wrong comparison base"
        );
        assert_eq!(
            app.editor.read(cx).value().as_str(),
            right,
            "wrong comparison target"
        );
    });
}

/// A partially staged file compares index/worktree; the staged action compares HEAD/index.
#[gpui::test]
#[ignore = "requires a built Git-for-Nanobug ZIP in NANOBUG_GIT_CANDIDATE"]
fn git_commit_comparisons_use_exact_index_and_worktree_versions(cx: &mut TestAppContext) {
    if !run_isolated_test(
        "extensions::windows_tests::comparisons::source_tabs::git_commit_comparisons_use_exact_index_and_worktree_versions",
    ) {
        return;
    }
    with_comparison(
        cx,
        "HEAD version\ncontext\n",
        "INDEX version\ncontext\n",
        |manager, visual, app, file| {
            let root = file.parent().unwrap();
            assert_sources(
                visual,
                app,
                "HEAD version\ncontext\n",
                "INDEX version\ncontext\n",
            );
            assert!(git(root, &["add", "--", "diff.txt"]));
            std::fs::write(file, "WORKTREE version\ncontext\n").unwrap();
            manager.invoke_command("git", "refresh", json!({})).unwrap();
            flush(manager, app, visual);
            manager
                .invoke_command("git", "diff", json!({"path":"diff.txt"}))
                .unwrap();
            flush(manager, app, visual);
            paint_comparison(visual);
            assert_sources(
                visual,
                app,
                "INDEX version\ncontext\n",
                "WORKTREE version\ncontext\n",
            );
            manager
                .invoke_command("git", "diff-index", json!({"path":"diff.txt"}))
                .unwrap();
            flush(manager, app, visual);
            paint_comparison(visual);
            // Provider titles are presentation; exact HEAD/index bytes identify the staged comparison.
            assert_sources(
                visual,
                app,
                "HEAD version\ncontext\n",
                "INDEX version\ncontext\n",
            );
            visual.update(|_, cx| {
                let app = app.read(cx);
                assert_eq!(app.comparison_views.len(), 1);
                assert_eq!(
                    app.tabs
                        .iter()
                        .filter(|tab| tab.source_only.is_some())
                        .count(),
                    0
                );
                assert_eq!(
                    app.tabs
                        .iter()
                        .filter(|tab| tab.virtual_document.is_some())
                        .count(),
                    2
                );
            });
        },
    );
}

/// Opening a previously closed worktree file keeps the native source readable/editable through
/// the comparison, without adding its ordinary file tab to the visible strip.
#[gpui::test]
#[ignore = "requires a built Git-for-Nanobug ZIP in NANOBUG_GIT_CANDIDATE"]
fn git_commit_file_diff_opens_only_comparison_preview(cx: &mut TestAppContext) {
    if !run_isolated_test(
        "extensions::windows_tests::comparisons::source_tabs::git_commit_file_diff_opens_only_comparison_preview",
    ) {
        return;
    }
    crate::tests::with_shortcut_editor(cx, true, Vec::new(), |visual, app, path| {
        let root = path.parent().unwrap();
        assert!(git(root, &["init", "-b", "main"]));
        assert!(git(root, &["config", "core.autocrlf", "false"]));
        assert!(git(root, &["config", "user.name", "fixture"]));
        assert!(git(
            root,
            &["config", "user.email", "fixture@example.invalid"]
        ));
        let file = root.join("changed.txt");
        let second = root.join("second.txt");
        std::fs::write(&file, "first\nold value\nlast\n").unwrap();
        std::fs::write(&second, "other base\n").unwrap();
        assert!(git(root, &["add", "--", "changed.txt", "second.txt"]));
        assert!(git(root, &["commit", "-m", "base"]));
        std::fs::write(&file, "first\r\nnew value\r\nlast\r\n").unwrap();
        std::fs::write(&second, "other changed\n").unwrap();
        let package = Package::read(Path::new(
            &std::env::var_os("NANOBUG_GIT_CANDIDATE").expect("candidate required"),
        ))
        .unwrap();
        let profile = tempfile::tempdir().unwrap();
        let env = visual.update(|_, cx| environment(root, cx));
        let mut manager = Manager::open(profile.path().into(), env).unwrap();
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
        package_ui_test_support::demand(
            &mut manager,
            "git",
            plugin_runtime::DemandReason::Command("refresh".into()),
        )
        .unwrap();
        flush(&mut manager, &app, visual);
        manager
            .invoke_command("git", "diff", json!({"path":"changed.txt"}))
            .unwrap();
        flush(&mut manager, &app, visual);
        paint_comparison(visual);
        assert!(visual.debug_bounds("document-diff-left").is_some());
        assert!(visual.debug_bounds("document-diff-right").is_some());
        let source_index = visual.update(|_, cx| {
            let app = app.read(cx);
            assert_eq!(app.comparison_views.len(), 1);
            assert!(app.comparison_views[0].preview);
            let left = app
                .tabs
                .iter()
                .find(|tab| tab.virtual_document.is_some())
                .unwrap();
            let left = left
                .text
                .as_ref()
                .unwrap()
                .editor
                .read(cx)
                .value()
                .to_string();
            let right = app.editor.read(cx).value().to_string();
            assert_eq!(left, "first\nold value\nlast\n");
            assert_eq!(right, "first\r\nnew value\r\nlast\r\n");
            assert_eq!(
                crate::editor::comparison::changed_byte_ranges(&left, &right).unwrap(),
                vec![[6..16, 7..18]],
                "only the changed content line should be highlighted"
            );
            app.tabs
                .iter()
                .position(|tab| tab.path() == file.canonicalize().unwrap())
        });
        if let Some(index) = source_index {
            let selector: &'static str = Box::leak(format!("editor-tab-{index}").into_boxed_str());
            assert!(
                visual.debug_bounds(selector).is_none(),
                "worktree comparison created an extra visible source-file tab"
            );
        }
        // Replacing a clean preview releases both its readonly blob and its hidden worktree source.
        manager
            .invoke_command("git", "diff", json!({"path":"second.txt"}))
            .unwrap();
        flush(&mut manager, &app, visual);
        paint_comparison(visual);
        let (source, id) = visual.update(|_, cx| {
            let app = app.read(cx);
            assert_eq!(app.comparison_views.len(), 1);
            assert!(
                !app.tabs
                    .iter()
                    .any(|tab| tab.path() == file.canonicalize().unwrap())
            );
            assert_eq!(
                app.tabs
                    .iter()
                    .filter(|tab| tab.source_only.is_some())
                    .count(),
                1
            );
            assert_eq!(
                app.tabs.iter().filter(|tab| tab.is_visible()).count(),
                1,
                "the original unrelated file remains open"
            );
            (app.editor.clone(), app.comparison_views[0].id.clone())
        });
        // Typing stays in the shared original session. Closing its view reveals rather than loses edits.
        visual.update(|window, cx| source.focus_handle(cx).focus(window, cx));
        visual.simulate_keystrokes("ctrl-end");
        visual.simulate_input("draft 中文😀\n");
        flush(&mut manager, &app, visual);
        let draft = visual.update(|_, cx| source.read(cx).value().to_string());
        assert!(draft.ends_with("draft 中文😀\n"));
        std::fs::write(&second, "external version\n").unwrap();
        manager
            .invoke_command("git", "diff", json!({"path":"second.txt"}))
            .unwrap();
        flush(&mut manager, &app, visual);
        visual.update(|_, cx| {
            assert_eq!(app.read(cx).editor, source);
            assert_eq!(source.read(cx).value().to_string(), draft,
                "reopening a comparison must preserve unsaved native text over external disk changes");
        });
        visual.update(|window, cx| {
            app.update(cx, |app, cx| app.close_comparison_view(&id, window, cx))
        });
        paint_comparison(visual);
        visual.update(|_, cx| {
            let app = app.read(cx);
            let tab = app
                .tabs
                .iter()
                .find(|tab| tab.path() == second.canonicalize().unwrap())
                .unwrap();
            assert!(tab.is_visible());
            assert!(tab.is_dirty());
            assert_eq!(tab.text.as_ref().unwrap().editor, source);
            assert_eq!(source.read(cx).value().to_string(), draft);
            assert!(!app.tabs.iter().any(|tab| tab.virtual_document.is_some()));
        });
        assert_eq!(
            std::fs::read_to_string(&second).unwrap(),
            "external version\n"
        );
        manager.disable("git").unwrap();
        flush(&mut manager, &app, visual);
    });
}
