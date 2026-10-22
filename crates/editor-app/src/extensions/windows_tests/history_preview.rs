//! The independent Git ZIP drives inline commit selection and immutable history tabs through native UI.
use super::*;
use gpui_base::test_support::snapshots;
use plugin_runtime::plugin_protocol::{api, ui};
mod navigation;

/// Deliver one native log row press through its current public scene, then drain real Git/editor receipts.
fn select_row(manager: &mut Manager, row: usize) {
    let revision = manager.live["git"].views["history"].revision;
    manager
        .event(
            "git",
            Some("history".into()),
            api::Notification::Ui(ui::UiEvent {
                revision,
                node: "log".into(),
                action: ui::Action::Canvas(ui::CanvasEvent::Pointer {
                    phase: ui::PointerPhase::Down,
                    x: 100.,
                    y: row as f32 * 28. + 14.,
                    button: 0,
                    clicks: 1,
                    shift: false,
                }),
            }),
        )
        .unwrap();
}

/// Click a real native tree/tab target; selectors are bounded to this disposable test process.
fn press(visual: &mut VisualTestContext, selector: String, count: usize) {
    let selector: &'static str = Box::leak(selector.into_boxed_str());
    let position = visual
        .debug_bounds(selector)
        .unwrap_or_else(|| {
            panic!(
                "Missing {selector}; files={:?}, upper={:?}, lower={:?}, preview={:?}",
                visual.debug_bounds("plugin-ui-files"),
                visual.debug_bounds("plugin-ui-history-files-pane"),
                visual.debug_bounds("plugin-ui-history-details-scroll"),
                visual.debug_bounds("plugin-ui-history-preview")
            )
        })
        .center();
    visual.simulate_event(gpui_kit::MouseDownEvent {
        position,
        button: gpui_kit::MouseButton::Left,
        modifiers: Default::default(),
        click_count: count,
        first_mouse: false,
    });
    visual.simulate_event(gpui_kit::MouseUpEvent {
        position,
        button: gpui_kit::MouseButton::Left,
        modifiers: Default::default(),
        click_count: count,
    });
    visual.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear(cx));
}

/// Resolve the changed file from its actual lazy tree page, without reproducing guest node identities.
fn changed_file(manager: &Manager) -> String {
    let ui::Kind::Tree { handle } = &manager.live["git"].views["history"]
        .active_node("files")
        .unwrap()
        .kind
    else {
        panic!("inline files tree")
    };
    manager.live["git"]
        .tree_resources()
        .into_iter()
        .find(|tree| tree.handle() == handle)
        .unwrap()
        .snapshot()
        .nodes
        .values()
        .find(|node| node.label == "shared.txt" && !node.branch)
        .unwrap()
        .id
        .clone()
}

/// Merge and ordinary commits preview in the dock, open readonly tabs, retain pinned identities and clean up.
#[gpui::test]
#[ignore = "real independent Git ZIP, native GPUI, editor.diff 1.3; set NANOBUG_GIT_CANDIDATE"]
fn git_inline_history_preview_opens_readonly_two_and_three_pane_tabs(cx: &mut TestAppContext) {
    if !run_isolated_test(
        "extensions::windows_tests::history_preview::git_inline_history_preview_opens_readonly_two_and_three_pane_tabs",
    ) {
        return;
    }
    crate::tests::with_shortcut_editor(cx, false, Vec::new(), |visual, app, path| {
        let root = path.parent().unwrap();
        assert!(git(root, &["init", "-b", "main"]));
        for (key, value) in [
            ("core.autocrlf", "false"),
            ("user.name", "Preview 作者"),
            ("user.email", "preview@example.invalid"),
        ] {
            assert!(git(root, &["config", key, value]));
        }
        let file = root.join("shared.txt");
        std::fs::write(&file, "base\n").unwrap();
        assert!(git(root, &["add", "shared.txt"]));
        assert!(git(root, &["commit", "-m", "base"]));
        assert!(git(root, &["switch", "-c", "feature"]));
        std::fs::write(&file, "feature 中文\n").unwrap();
        assert!(git(root, &["commit", "-am", "feature"]));
        assert!(git(root, &["switch", "main"]));
        std::fs::write(&file, "main 中文\n").unwrap();
        assert!(git(root, &["commit", "-am", "main"]));
        assert!(!git(root, &["merge", "--no-ff", "--no-commit", "feature"]));
        std::fs::write(&file, "merge result 中文\n").unwrap();
        std::fs::create_dir(root.join("docs")).unwrap();
        std::fs::write(root.join("docs/settings.json"), "{}\n").unwrap();
        std::fs::write(root.join("docs/notes.md"), "# Preview\n").unwrap();
        assert!(git(root, &["add", "shared.txt", "docs"]));
        assert!(git(
            root,
            &[
                "commit",
                "-m",
                "merge preview",
                "-m",
                "完整提交说明\n\n第二段 😀"
            ]
        ));
        let package = Package::read(Path::new(
            &std::env::var_os("NANOBUG_GIT_CANDIDATE").expect("candidate ZIP"),
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
            plugin_runtime::DemandReason::View("history".into()),
        )
        .unwrap();
        flush(&mut manager, &app, visual);
        select_row(&mut manager, 0);
        flush(&mut manager, &app, visual);
        let history = &manager.live["git"].views["history"];
        assert!(history.active_node("history-preview").is_some());
        assert!(
            matches!(&history.active_node("history-subject").unwrap().kind, ui::Kind::Text { text } if text == "merge preview")
        );
        assert!(
            matches!(&history.active_node("history-message-0").unwrap().kind, ui::Kind::Text { text } if text.contains("完整提交说明\n\n第二段 😀"))
        );
        assert!(history.active_node("history-loading").is_none());
        assert!(visual.update(|_, cx| {
            app.read(cx)
                .plugin_windows
                .get("git/commit-details")
                .is_none()
        }));
        assert!(visual.update(|_, cx| app.read(cx).comparison_views.is_empty()));
        let files = visual
            .debug_bounds("plugin-ui-history-files-pane")
            .expect("visible upper preview");
        let details = visual
            .debug_bounds("plugin-ui-history-details-scroll")
            .expect("visible lower preview");
        assert!(files.size.height > gpui_kit::px(20.) && details.size.height > gpui_kit::px(20.));
        assert!(details.top() >= files.bottom());
        let folder = manager.live["git"]
            .tree_resources()
            .into_iter()
            .flat_map(|tree| tree.snapshot().nodes.into_values())
            .find(|node| node.branch && node.label == "docs")
            .expect("real changed-file directory");
        press(visual, format!("plugin-tree-row-{}", folder.id), 2);
        flush(&mut manager, &app, visual);
        // Inspect the rendered panes rather than just the guest's numeric layout declarations.
        for dark in [false, true] {
            visual.update(|_, cx| {
                crate::ui::theme::apply_theme(crate::ui::theme::builtin_theme(dark), cx)
            });
            visual.update(|window, cx| window.draw(cx).clear(cx));
            let title = visual
                .debug_bounds("plugin-ui-history-files-title")
                .unwrap();
            let subject = visual.debug_bounds("plugin-ui-history-subject").unwrap();
            assert_eq!(title.left() - files.left(), gpui_kit::px(8.));
            assert_eq!(title.top() - files.top(), gpui_kit::px(8.));
            assert_eq!(subject.left() - details.left(), gpui_kit::px(8.));
            assert_eq!(subject.top() - details.top(), gpui_kit::px(8.));
            assert!(
                visual.debug_bounds("tree-row-icon-file").is_some(),
                "preview files use Explorer's file-type icon in either theme"
            );
            visual.update(|window, _| {
                assert!(
                    !snapshots(window).iter().any(|node| node.path().iter().any(|part| {
                        matches!(part, gpui_kit::ElementId::Name(name) if name == "plugin-command-menu")
                    })),
                    "a package with no header menu commands must not show its empty menu button"
                );
            });
        }
        // Return the root file to the bounded viewport before testing its physical comparison click.
        press(visual, format!("plugin-tree-row-{}", folder.id), 2);
        flush(&mut manager, &app, visual);
        press(
            visual,
            format!("plugin-tree-row-{}", changed_file(&manager)),
            1,
        );
        flush(&mut manager, &app, visual);
        assert!(
            visual.debug_bounds("document-diff-third").is_some(),
            "merge opens three native panes in the main tab"
        );
        // All four actions are compact native icon buttons in the requested left/right/up/down order.
        for dark in [false, true] {
            visual.update(|window, cx| {
                crate::ui::theme::apply_theme(crate::ui::theme::builtin_theme(dark), cx);
                window.draw(cx).clear(cx);
            });
            let mut right = gpui_kit::px(0.);
            for selector in [
                "diff-file-previous",
                "diff-file-next",
                "diff-previous",
                "diff-next",
            ] {
                let bounds = visual
                    .debug_bounds(selector)
                    .expect("native comparison arrow");
                assert!(bounds.left() >= right && bounds.size.width <= gpui_kit::px(32.));
                right = bounds.right();
            }
            assert!(visual.debug_bounds("diff-unified").is_none());
            assert!(visual.debug_bounds("diff-sync").is_none());
            super::comparison_chrome::assert_comparison_chrome(visual, 3);
        }
        visual.update(|window, _| {
            assert!(
                !snapshots(window).iter().any(|node| {
                    node.role() == Some(gpui_kit::Role::Button)
                        && node.label() == Some(t!("editor.diff_sync").as_ref())
                }),
                "comparison scrolling is synchronized by default without a toggle button"
            );
        });
        let before = visual.update(|_, cx| {
            let owner = app.read(cx);
            let sources = owner
                .tabs
                .iter()
                .filter(|tab| tab.virtual_document.is_some())
                .map(|tab| {
                    let editor = &tab.text.as_ref().unwrap().editor;
                    assert!(editor.read(cx).presentation().is_readonly());
                    editor.read(cx).value().to_string()
                })
                .collect::<Vec<_>>();
            assert_eq!(sources.len(), 3);
            assert!(sources.iter().any(|s| s == "main 中文\n"));
            assert!(sources.iter().any(|s| s == "merge result 中文\n"));
            assert!(sources.iter().any(|s| s == "feature 中文\n"));
            sources
        });
        let normal = visual.update(|_, cx| {
            app.read(cx)
                .tabs
                .iter()
                .filter(|tab| tab.virtual_document.as_ref().is_none_or(|s| !s.hidden))
                .count()
        });
        press(visual, format!("comparison-tab-{normal}"), 2);
        let third = visual.debug_bounds("document-diff-third").unwrap().center();
        visual.simulate_click(third, Default::default());
        visual.simulate_input("must not edit");
        flush(&mut manager, &app, visual);
        let after = visual.update(|_, cx| {
            app.read(cx)
                .tabs
                .iter()
                .filter(|t| t.virtual_document.is_some())
                .map(|t| t.text.as_ref().unwrap().editor.read(cx).value().to_string())
                .collect::<Vec<_>>()
        });
        assert_eq!(
            after, before,
            "all historical panes remain readonly under real text input"
        );
        select_row(&mut manager, 1);
        flush(&mut manager, &app, visual);
        assert_eq!(
            visual.update(|_, cx| app.read(cx).comparison_views.len()),
            1,
            "preview selection cannot retarget the pinned comparison"
        );
        // These real arrow clicks still navigate the pinned merge's list after selecting another
        // commit. First/last disable rather than wrap; the log preview must stay on that other OID.
        let preview_subject = serde_json::to_value(
            &manager.live["git"].views["history"]
                .active_node("history-subject")
                .unwrap()
                .kind,
        )
        .unwrap();
        for (arrow, title, previous_disabled, next_disabled) in [
            ("diff-file-previous", "settings.json", false, false),
            ("diff-file-previous", "notes.md", true, false),
            ("diff-file-next", "settings.json", false, false),
            ("diff-file-next", "shared.txt", false, true),
        ] {
            press(visual, arrow.into(), 1);
            flush(&mut manager, &app, visual);
            let guest_error = manager.live["git"].views["history"]
                .active_node("history-error")
                .map(|node| serde_json::to_value(&node.kind).unwrap());
            visual.update(|window, cx| {
                let state = app.read(cx);
                let worker = state.extensions.read(cx).worker.state.lock().unwrap();
                let active = state
                    .comparison_views
                    .iter()
                    .find(|view| Some(&view.id) == state.active_comparison.as_ref())
                    .unwrap();
                assert!(
                    active.title.starts_with(title),
                    "actual neighboring file: {}; host={}; worker={:?}; guest={guest_error:?}",
                    active.title,
                    state.status,
                    worker.status.as_ref().map(|status| &status.message)
                );
                let controls = snapshots(window);
                for (label, disabled) in [
                    (t!("editor.diff_file_previous"), previous_disabled),
                    (t!("editor.diff_file_next"), next_disabled),
                ] {
                    let button = controls
                        .iter()
                        .find(|node| {
                            node.role() == Some(gpui_kit::Role::Button)
                                && node.label() == Some(label.as_ref())
                        })
                        .unwrap();
                    // Base buttons may omit a native disabled flag. Absence says nothing about
                    // activation; boundary presses below verify the actual no-wrap behavior.
                    if let Some(exposed) = button.disabled() {
                        assert_eq!(exposed, disabled);
                    }
                }
            });
            assert!(
                visual.debug_bounds("document-diff-third").is_some(),
                "merge source count stays three"
            );
            assert_eq!(
                serde_json::to_value(
                    &manager.live["git"].views["history"]
                        .active_node("history-subject")
                        .unwrap()
                        .kind
                )
                .unwrap(),
                preview_subject
            );
            if previous_disabled {
                let before = visual.update(|_, cx| app.read(cx).active_comparison.clone());
                press(visual, "diff-file-previous".into(), 1);
                flush(&mut manager, &app, visual);
                assert_eq!(
                    visual.update(|_, cx| app.read(cx).active_comparison.clone()),
                    before,
                    "the first file rejects a previous-file press instead of wrapping"
                );
            }
        }
        press(visual, "diff-file-next".into(), 1);
        flush(&mut manager, &app, visual);
        assert_eq!(
            visual.update(|_, cx| app.read(cx).comparison_views.len()),
            1,
            "last file remains the original pinned tab"
        );
        press(
            visual,
            format!("plugin-tree-row-{}", changed_file(&manager)),
            1,
        );
        flush(&mut manager, &app, visual);
        assert!(visual.debug_bounds("document-diff-left").is_some());
        assert!(
            visual.debug_bounds("document-diff-third").is_none(),
            "ordinary commit opens two panes"
        );
        assert_eq!(
            visual.update(|_, cx| app.read(cx).comparison_views.len()),
            2
        );
        press(visual, format!("comparison-tab-{normal}"), 1);
        assert!(
            visual.debug_bounds("document-diff-third").is_some(),
            "pinned merge retains its three sources across tab switches"
        );
        select_row(&mut manager, 3);
        flush(&mut manager, &app, visual);
        press(
            visual,
            format!("plugin-tree-row-{}", changed_file(&manager)),
            1,
        );
        flush(&mut manager, &app, visual);
        assert!(visual.debug_bounds("document-diff-third").is_none());
        let root_sources = visual.update(|_, cx| {
            app.read(cx)
                .tabs
                .iter()
                .filter(|tab| tab.virtual_document.is_some())
                .map(|tab| {
                    tab.text
                        .as_ref()
                        .unwrap()
                        .editor
                        .read(cx)
                        .value()
                        .to_string()
                })
                .collect::<Vec<_>>()
        });
        assert!(
            root_sources.iter().any(String::is_empty),
            "root commit has an explicit empty parent source"
        );
        assert!(root_sources.iter().any(|text| text == "base\n"));
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            "merge result 中文\n"
        );
        manager.disable("git").unwrap();
        flush(&mut manager, &app, visual);
        visual.update(|_, cx| {
            assert!(app.read(cx).comparison_views.is_empty());
            assert!(
                app.read(cx)
                    .tabs
                    .iter()
                    .all(|tab| tab.virtual_document.is_none())
            );
        });
    });
}
