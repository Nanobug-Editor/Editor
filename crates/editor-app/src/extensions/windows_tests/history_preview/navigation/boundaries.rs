//! Real package arrows synchronize complete nested metadata and explicit content-edge landing.
use super::*;

/// Three ordered changed files expose both neighbours even when the nested folders are collapsed.
fn repository(root: &Path) {
    for args in [
        vec!["init", "-b", "main"],
        vec!["config", "core.autocrlf", "false"],
        vec!["config", "user.name", "Boundary fixture"],
        vec!["config", "user.email", "boundary@example.invalid"],
    ] {
        assert!(git(root, &args));
    }
    std::fs::create_dir_all(root.join("alpha")).unwrap();
    std::fs::create_dir_all(root.join("nested/beta")).unwrap();
    for version in [0, 2] {
        for file in ["alpha/a.txt", "nested/beta/b.txt", "nested/c.txt"] {
            std::fs::write(root.join(file), source(version)).unwrap();
        }
        assert!(git(root, &["add", "."]));
        assert!(git(
            root,
            &["commit", "-m", if version == 0 { "base" } else { "batch" }]
        ));
    }
}

/// Resolve the published resource through the actual mounted files node, never guessed guest IDs.
fn files(manager: &Manager) -> std::sync::Arc<plugin_runtime::TreeResource> {
    let ui::Kind::Tree { handle } = &manager.live["git"].views["history"]
        .active_node("files")
        .unwrap_or_else(|| {
            panic!(
                "changed-file tree missing; history={:?}",
                manager.live["git"].views["history"]
            )
        })
        .kind
    else {
        panic!("changed-file tree")
    };
    manager.live["git"]
        .tree_resources()
        .into_iter()
        .find(|tree| tree.handle() == handle)
        .unwrap()
}

/// Labels are unique in this fixture; retain the actual published ID for physical input.
fn node(manager: &Manager, label: &str) -> String {
    files(manager)
        .snapshot()
        .nodes
        .values()
        .find(|node| node.label == label)
        .unwrap()
        .id
        .clone()
}

/// Assert native selection and public action enablement, rather than trusting guest cursor state.
fn selected(
    app: &Entity<EditorApp>,
    visual: &mut VisualTestContext,
    manager: &mut Manager,
    file: &str,
    neighbours: [bool; 2],
) {
    let expected = node(manager, file);
    await_comparison(manager, app, visual, file, 2);
    let deadline = Instant::now() + Duration::from_secs(30);
    while visual.update(|_, cx| {
        app.read(cx).plugin_panels["git/history"]
            .read(cx)
            .native_ui
            .as_ref()
            .unwrap()
            .read(cx)
            .native_tree_selected_id("files", cx)
            != Some(Some(expected.clone()))
    }) {
        flush(manager, app, visual);
        assert!(Instant::now() < deadline, "native tree must select {file}");
    }
    visual.update(|window, cx| {
        let app = app.read(cx);
        let view = app
            .comparison_views
            .iter()
            .find(|view| Some(&view.id) == app.active_comparison.as_ref())
            .unwrap();
        assert!(
            view.title.ends_with(file),
            "comparison title: {}",
            view.title
        );
        let panel = app.plugin_panels["git/history"].read(cx);
        assert_eq!(
            panel
                .native_ui
                .as_ref()
                .unwrap()
                .read(cx)
                .native_tree_selected_id("files", cx),
            Some(Some(expected))
        );
        let nodes = snapshots(window);
        for (label, enabled) in [t!("editor.diff_file_previous"), t!("editor.diff_file_next")]
            .into_iter()
            .zip(neighbours)
        {
            let button = nodes
                .iter()
                .find(|node| {
                    node.role() == Some(gpui_kit::Role::Button)
                        && node.label() == Some(label.as_ref())
                })
                .expect("visible file arrow");
            assert_eq!(button.disabled().unwrap_or(false), !enabled, "{label}");
        }
    });
}

/// Content edges are measured in the same aligned viewport used by actual scroll gestures.
fn edge(app: &Entity<EditorApp>, visual: &mut VisualTestContext, end: bool) {
    frames(visual);
    let viewport = visual.debug_bounds("aligned-vertical-scrollbar-1").unwrap();
    let content = source(2);
    let byte = if end { content.len() } else { 0 };
    let bounds = visual.update(|_, cx| {
        app.read(cx)
            .comparison_alignment()
            .unwrap()
            .project_range(1, byte..byte, cx)
            .expect("requested content edge visible")
    });
    let delta = if end {
        bounds.bottom() - viewport.bottom()
    } else {
        bounds.top() - viewport.top()
    };
    assert!(
        delta.abs() <= px(25.),
        "end={end}, glyph={bounds:?}, viewport={viewport:?}"
    );
}

/// The notice is centered in the content and does not change file until a second native press.
fn confirm(visual: &mut VisualTestContext, next: bool) {
    let selector = if next { "diff-next" } else { "diff-previous" };
    press(visual, selector.into(), 1);
    let notice = visual
        .debug_bounds("centered-notice-card")
        .expect("first press displays notice");
    let viewport = visual.debug_bounds("centered-notice").unwrap();
    assert!((notice.center().y - viewport.center().y).abs() < px(1.));
    assert!((notice.center().x - viewport.center().x).abs() < px(1.));
    press(visual, selector.into(), 1);
    assert!(visual.debug_bounds("centered-notice").is_none());
}

/// All navigation starts in the real published scene and retains native tree/viewport ownership.
#[gpui::test]
#[ignore = "real independent Git ZIP and native comparison; set NANOBUG_GIT_CANDIDATE"]
fn git_nested_file_navigation_confirms_boundaries_and_reveals_content_edges(
    cx: &mut TestAppContext,
) {
    if !run_isolated_test(
        "extensions::windows_tests::history_preview::navigation::boundaries::git_nested_file_navigation_confirms_boundaries_and_reveals_content_edges",
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
        select_subject(&mut manager, "batch");
        await_history(&mut manager, &app, visual, "batch");
        let nested = node(&manager, "nested");
        if files(&manager).snapshot().expanded.contains(&nested) {
            press(visual, format!("plugin-tree-row-{nested}"), 2);
            flush(&mut manager, &app, visual);
        }
        let alpha = node(&manager, "alpha");
        if !files(&manager).snapshot().expanded.contains(&alpha) {
            press(visual, format!("plugin-tree-row-{alpha}"), 2);
            flush(&mut manager, &app, visual);
        }
        let first = node(&manager, "a.txt");
        press(visual, format!("plugin-tree-row-{first}"), 1);
        flush(&mut manager, &app, visual);
        selected(&app, visual, &mut manager, "a.txt", [false, true]);
        // Complete metadata supplies b even before nested has ever been expanded.
        press(visual, "diff-file-next".into(), 1);
        flush(&mut manager, &app, visual);
        selected(&app, visual, &mut manager, "b.txt", [true, true]);
        assert!(files(&manager).snapshot().expanded.contains(&nested));
        assert!(
            files(&manager)
                .snapshot()
                .expanded
                .contains(&node(&manager, "beta"))
        );
        press(visual, format!("plugin-tree-row-{nested}"), 2);
        flush(&mut manager, &app, visual);
        assert!(!files(&manager).snapshot().expanded.contains(&nested));
        press(visual, "diff-file-next".into(), 1);
        flush(&mut manager, &app, visual);
        selected(&app, visual, &mut manager, "c.txt", [true, false]);
        let snapshot = files(&manager).snapshot();
        assert!(snapshot.expanded.contains(&nested));
        // c is nested's direct child: opening it must not undo the user's collapse of beta.
        assert!(!snapshot.expanded.contains(&node(&manager, "beta")));
        press(visual, "diff-file-previous".into(), 1);
        flush(&mut manager, &app, visual);
        selected(&app, visual, &mut manager, "b.txt", [true, true]);
        centered(&app, visual, 2, "main first", 2);
        // Expiry cancels the confirmation; a later click displays a fresh prompt in this file.
        press(visual, "diff-previous".into(), 1);
        visual.executor().advance_clock(Duration::from_secs(2));
        frames(visual);
        assert!(visual.debug_bounds("centered-notice").is_none());
        confirm(visual, false);
        flush(&mut manager, &app, visual);
        selected(&app, visual, &mut manager, "a.txt", [false, true]);
        edge(&app, visual, true);
        // Entering from an edge visits its nearest difference, rather than skipping it.
        press(visual, "diff-previous".into(), 1);
        centered(&app, visual, 2, "main third", 2);
        confirm(visual, true);
        flush(&mut manager, &app, visual);
        selected(&app, visual, &mut manager, "b.txt", [true, true]);
        edge(&app, visual, false);
        press(visual, "diff-next".into(), 1);
        centered(&app, visual, 2, "main first", 2);
        press(visual, "diff-next".into(), 1);
        centered(&app, visual, 2, "main third", 2);
        confirm(visual, true);
        flush(&mut manager, &app, visual);
        selected(&app, visual, &mut manager, "c.txt", [true, false]);
        edge(&app, visual, false);
        for _ in 0..2 {
            press(visual, "diff-next".into(), 1);
        }
        centered(&app, visual, 2, "main third", 2);
        confirm(visual, true);
        flush(&mut manager, &app, visual);
        selected(&app, visual, &mut manager, "c.txt", [true, false]);
        centered(&app, visual, 2, "main third", 2);
    });
}
