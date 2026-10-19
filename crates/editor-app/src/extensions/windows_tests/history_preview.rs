//! The independent Git ZIP drives inline commit selection and immutable history tabs through native UI.
use super::*;
use gpui_base::test_support::snapshots;
use plugin_runtime::plugin_protocol::{api, ui};

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
#[ignore = "real independent Git ZIP, native GPUI, editor.diff 1.2; set NANOBUG_GIT_CANDIDATE"]
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
