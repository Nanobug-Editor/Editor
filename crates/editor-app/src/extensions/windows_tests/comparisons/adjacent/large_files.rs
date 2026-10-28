//! A real independent Git package crosses Rust, YAML and lockfile previews above the old line cap.
use super::*;

/// Preserve actual project content while marking the fixture's exact committed revision visibly.
fn content(name: &str, version: &str) -> String {
    let body = match name {
        "a.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/run/tests.rs")),
        "b.yml" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../editor-ui/locales/app.yml"
        )),
        "c.lock" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.lock")),
        _ => unreachable!(),
    };
    format!("# {name} {version}\n{body}")
}

/// Commit exact large-file versions in the disposable fixture, never touching the user's index.
fn large_repository(root: &Path) {
    repository(root);
    for version in ["base", "changed"] {
        for name in ["a.rs", "b.yml", "c.lock"] {
            std::fs::write(root.join(name), content(name, version)).unwrap();
        }
        assert!(git(root, &["add", "--", "a.rs", "b.yml", "c.lock"]));
        assert!(git(root, &["commit", "-m", version]));
    }
}

/// Drive pointer file arrows through the package, public manager and real host comparison admission.
#[gpui::test]
#[ignore = "real independent Git ZIP and native comparison; set NANOBUG_GIT_CANDIDATE"]
fn git_adjacent_large_rs_yaml_lockfiles_keep_navigation_and_alignment(cx: &mut TestAppContext) {
    if !run_isolated_test(
        "extensions::windows_tests::comparisons::adjacent::large_files::git_adjacent_large_rs_yaml_lockfiles_keep_navigation_and_alignment",
    ) {
        return;
    }
    crate::tests::with_shortcut_editor(cx, false, Vec::new(), |visual, app, path| {
        let root = path.parent().unwrap();
        large_repository(root);
        let package = Package::read(Path::new(
            &std::env::var_os("NANOBUG_GIT_CANDIDATE").expect("candidate ZIP"),
        ))
        .unwrap();
        let env = visual.update(|_, cx| environment(root, cx));
        let mut manager = Manager::open(root.join(".runtime-plugin-test"), env).unwrap();
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
        let deadline = Instant::now() + Duration::from_secs(30);
        while manager.live["git"].views["history"]
            .active_node("files")
            .is_none()
        {
            flush(&mut manager, &app, visual);
            assert!(Instant::now() < deadline, "large file tree did not arrive");
        }
        let tree = files(&manager);
        let node = tree
            .snapshot()
            .nodes
            .into_values()
            .find(|node| node.label == "a.rs" && !node.branch)
            .unwrap();
        click_count(visual, &format!("plugin-tree-row-{}", node.id), 1);
        for (arrow, name) in [
            (None, "a.rs"),
            (Some("diff-file-next"), "b.yml"),
            (Some("diff-file-next"), "c.lock"),
            (Some("diff-file-previous"), "b.yml"),
            (Some("diff-file-previous"), "a.rs"),
            (Some("diff-file-next"), "b.yml"),
        ] {
            if let Some(arrow) = arrow {
                click_count(visual, arrow, 1);
            }
            let deadline = Instant::now() + Duration::from_secs(30);
            loop {
                flush(&mut manager, &app, visual);
                let ready = file_is_visible(&app, visual, name)
                    && visual.update(|_, cx| {
                        app.read(cx).comparison_alignment().is_some_and(|group| {
                            group.status(cx) == editor_ui::aligned_editor::AlignmentStatus::Ready
                        })
                    });
                if ready {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "{name} did not become visible and aligned: {:?}",
                    visual.update(|_, cx| {
                        let host = app.read(cx);
                        (
                            host.editor.read(cx).line_height(),
                            host.comparison_alignment().map(|group| group.status(cx)),
                        )
                    })
                );
            }
            visual.update(|window, cx| {
                assert_eq!(
                    app.read(cx).editor.read(cx).value().as_ref(),
                    content(name, "changed")
                );
                assert!(app.read(cx).editor.read(cx).selected_range().is_empty());
                window.draw(cx).clear(cx);
            });
            assert!(visual.debug_bounds("document-diff-left").is_some());
            assert!(visual.debug_bounds("document-diff-right").is_some());
            eprintln!(
                "large comparison passed: {name}, {} lines",
                content(name, "changed").lines().count()
            );
        }
        manager.disable("git").unwrap();
        flush(&mut manager, &app, visual);
        assert!(visual.update(|_, cx| app.read(cx).comparison_views.is_empty()));
    });
}
