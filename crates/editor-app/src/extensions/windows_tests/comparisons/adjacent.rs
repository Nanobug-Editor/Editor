//! The real Git package navigates immutable files without replacing its history dock or tree pages.
use super::*;
use plugin_runtime::plugin_protocol::{api, ui};

/// Prepare two exact text versions for every file without using the user's repository or index.
fn repository(root: &Path) {
    for args in [
        vec!["init", "-b", "main"],
        vec!["config", "core.autocrlf", "false"],
        vec!["config", "user.name", "Adjacent fixture"],
        vec!["config", "user.email", "adjacent@example.invalid"],
        vec!["config", "commit.gpgsign", "false"],
    ] {
        assert!(git(root, &args));
    }
    // The app's disposable language profile is not part of the repository state under test.
    std::fs::create_dir_all(root.join(".git/info")).unwrap();
    std::fs::write(root.join(".git/info/exclude"), ".runtime-plugin-test/\n").unwrap();
    for (version, number) in [("base", 41), ("changed", 42)] {
        for name in ["a.novel", "b.novel", "c.novel"] {
            std::fs::write(
                root.join(name),
                format!("# {name} {version}\nanswer = {number}\n"),
            )
            .unwrap();
        }
        assert!(git(root, &["add", "--", "a.novel", "b.novel", "c.novel"]));
        assert!(git(root, &["commit", "-m", version]));
    }
}

/// Current public history scene supplies the same retained tree used by a native row selection.
fn files(manager: &Manager) -> std::sync::Arc<plugin_runtime::TreeResource> {
    let plugin_runtime::plugin_protocol::ui::Kind::Tree { handle } =
        &manager.live["git"].views["history"]
            .active_node("files")
            .unwrap()
            .kind
    else {
        panic!("history file tree");
    };
    manager.live["git"]
        .tree_resources()
        .into_iter()
        .find(|tree| tree.handle() == handle)
        .unwrap()
}

/// Idle dock scenes do not imply source admission; wait on the actual active native editor.
fn file_is_visible(app: &Entity<EditorApp>, visual: &mut VisualTestContext, name: &str) -> bool {
    visual.update(|_, cx| {
        app.read(cx)
            .editor
            .read(cx)
            .value()
            .starts_with(&format!("# {name} changed"))
    })
}

/// Ordinary and warm back/forward clicks retain the same log document and loaded file hierarchy.
#[gpui::test]
#[ignore = "real independent Git ZIP and native comparison; set NANOBUG_GIT_CANDIDATE"]
fn git_adjacent_file_arrows_preserve_history_scene_and_cached_sources(cx: &mut TestAppContext) {
    if !run_isolated_test(
        "extensions::windows_tests::comparisons::adjacent::git_adjacent_file_arrows_preserve_history_scene_and_cached_sources",
    ) {
        return;
    }
    crate::tests::with_shortcut_editor(cx, false, Vec::new(), |visual, app, path| {
        let root = path.parent().unwrap();
        repository(root);
        let package = Package::read(Path::new(
            &std::env::var_os("NANOBUG_GIT_CANDIDATE").expect("candidate ZIP"),
        ))
        .unwrap();
        let env = visual.update(|_, cx| environment(root, cx));
        // Match this fixture app's private runtime root so its normal provider discovery resolves
        // the installed grammar asset by package digest instead of pointing at another profile.
        let mut manager = Manager::open(root.join(".runtime-plugin-test"), env).unwrap();
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
        // A separately installed, unfamiliar language proves that Git supplies a generic path hint.
        let language =
            crate::extensions::language_tests::packages::language_package("snapshot-language");
        manager.install(&language, Default::default()).unwrap();
        crate::extensions::language_tests::publish_languages(&app, &manager, visual);
        package_ui_test_support::demand(
            &mut manager,
            "git",
            plugin_runtime::DemandReason::View("history".into()),
        )
        .unwrap();
        flush(&mut manager, &app, visual);
        // Opening the dock does not implicitly select HEAD; admit its first commit before files.
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
                        y: 14.,
                        button: 0,
                        clicks: 1,
                        shift: false,
                    }),
                }),
            )
            .unwrap();
        flush(&mut manager, &app, visual);
        let tree = files(&manager);
        let node = tree
            .snapshot()
            .nodes
            .into_values()
            .find(|node| node.label == "a.novel" && !node.branch)
            .expect("first changed file");
        click_count(visual, &format!("plugin-tree-row-{}", node.id), 1);
        let initial_deadline = Instant::now() + Duration::from_secs(30);
        while !file_is_visible(&app, visual, "a.novel") {
            flush(&mut manager, &app, visual);
            assert!(
                Instant::now() < initial_deadline,
                "initial comparison did not open"
            );
        }
        let initial_history = manager.live["git"].views["history"].revision;
        let initial_tree = tree.snapshot();
        let mut elapsed = Vec::new();
        let mut replaced_history = false;
        let mut reread_warm_source = false;
        for (arrow, name, warm) in [
            ("diff-file-next", "b.novel", false),
            ("diff-file-next", "c.novel", false),
            ("diff-file-previous", "b.novel", true),
            ("diff-file-previous", "a.novel", true),
            ("diff-file-next", "b.novel", true),
        ] {
            let start = Instant::now();
            let mut visible = None;
            let mut saw_git = false;
            click_count(visual, arrow, 1);
            while visible.is_none() {
                assert!(
                    start.elapsed() < Duration::from_secs(30),
                    "native neighboring file did not appear"
                );
                flush_checked(&mut manager, &app, visual, |manager, visual| {
                    replaced_history |=
                        manager.live["git"].views["history"].revision != initial_history;
                    saw_git |= manager.live["git"].process_count() > 0;
                    let matches = file_is_visible(&app, visual, name);
                    if matches && visible.is_none() {
                        visible = Some(start.elapsed());
                    }
                });
            }
            let elapsed_ms = visible
                .expect("native neighboring file must appear")
                .as_secs_f64()
                * 1000.;
            reread_warm_source |= warm && saw_git;
            let after = tree.snapshot();
            assert_eq!(after.epoch, initial_tree.epoch);
            assert_eq!(after.branches, initial_tree.branches);
            assert_eq!(after.nodes, initial_tree.nodes);
            assert_eq!(after.expanded, initial_tree.expanded);
            assert_eq!(after.initial_selection, initial_tree.initial_selection);
            elapsed.push(serde_json::json!({"file":name,"warm":warm,"elapsed_ms":elapsed_ms,"git_process":saw_git}));
        }
        eprintln!(
            "adjacent native navigation: {}",
            serde_json::Value::Array(elapsed)
        );
        // Inspect the actual mounted source editors and the production plugin-backed token styles.
        // Keep this after timing so an older package still records a comparable regression baseline.
        assert!(visual.debug_bounds("document-diff-left").is_some());
        assert!(visual.debug_bounds("document-diff-right").is_some());
        let mut syntax_ok = true;
        let mut palettes = Vec::new();
        for dark in [false, true] {
            visual.update(|window, cx| {
                crate::theme::apply_theme(crate::theme::builtin_theme(dark), cx);
                window.draw(cx).clear(cx);
                let owner = app.read(cx);
                let mut colors = Vec::new();
                let mut count = 0;
                for source in &owner.tabs {
                    let Some(text) = source.text.as_ref() else {
                        continue;
                    };
                    let editor = text.editor.read(cx);
                    let value = editor.value();
                    if !value.starts_with("# b.novel ") {
                        continue;
                    }
                    count += 1;
                    syntax_ok &= editor.language_name().as_ref() == "novel";
                    syntax_ok &= editor.presentation().is_readonly();
                    let mut syntax = crate::language::highlighter::SyntaxHighlighter::new(
                        &owner.providers.parser_key(&editor.language_name()),
                    );
                    syntax_ok &= syntax.update(None, editor.text(), None);
                    let styles = syntax.styles(
                        &(0..editor.text().len()),
                        cx.theme().highlight_theme.as_ref(),
                    );
                    let number = value.find("answer = ").unwrap() + "answer = ".len();
                    colors.push(
                        styles
                            .iter()
                            .find(|(range, style)| range.contains(&number) && style.color.is_some())
                            .and_then(|(_, style)| style.color),
                    );
                }
                syntax_ok &= count == 2 && colors.iter().all(Option::is_some);
                palettes.push(colors);
            });
        }
        syntax_ok &= palettes[0] != palettes[1];
        manager.disable("git").unwrap();
        flush(&mut manager, &app, visual);
        assert!(visual.update(|_, cx| app.read(cx).comparison_views.is_empty()));
        assert!(
            !replaced_history,
            "adjacent file replaced the history canvas/tree scene"
        );
        assert!(
            !reread_warm_source,
            "warm immutable neighbor launched a Git process"
        );
        assert!(
            syntax_ok,
            "actual Git source path hints must select and theme both plugin grammars"
        );
    });
}
