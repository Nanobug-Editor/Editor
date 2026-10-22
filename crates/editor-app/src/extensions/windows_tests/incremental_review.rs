//! Actual package branch queries must keep all three populated review areas on every native frame.
use super::*;
use gpui_base::test_support::snapshots;
use gpui_kit::{ElementId, MouseButton, Role};
use plugin_runtime::plugin_protocol::{api, ui};

/// Read each source's live resource through its current admitted scene, never private guest state.
fn resource(
    manager: &Manager,
    panel: &str,
    node: &str,
) -> std::sync::Arc<plugin_runtime::TreeResource> {
    let mut handle = None;
    // Input-active lookup excludes disabled nodes; that says nothing about whether their cached
    // rows are still painted. Observe structural membership when auditing intermediate frames.
    manager.live["git"].views[panel].root.visit(&mut |item| {
        if item.id == node
            && let ui::Kind::Tree { handle: tree } = &item.kind
        {
            handle = Some(tree.clone());
        }
    });
    let handle = handle.unwrap_or_else(|| panic!("{panel}/{node}: tree must stay mounted"));
    manager.live["git"]
        .tree_resources()
        .into_iter()
        .find(|r| r.handle() == &handle)
        .unwrap()
}

/// Native rows are selected on their real mouse-down, with the same click count as production.
fn press(visual: &mut VisualTestContext, id: &str, count: usize) {
    let selector: &'static str = Box::leak(format!("plugin-tree-row-{id}").into_boxed_str());
    let position = visual
        .debug_bounds(selector)
        .expect("visible real provider row")
        .center();
    visual.simulate_event(gpui_kit::MouseDownEvent {
        position,
        button: MouseButton::Left,
        modifiers: Default::default(),
        click_count: count,
        first_mouse: false,
    });
    visual.simulate_event(gpui_kit::MouseUpEvent {
        position,
        button: MouseButton::Left,
        modifiers: Default::default(),
        click_count: count,
    });
    visual.run_until_parked();
}

/// Inspect both provider caches and actual native tree rows after each publication and paint.
fn populated(manager: &Manager, visual: &mut VisualTestContext) {
    for (panel, node) in [("staging", "staging"), ("history", "files")] {
        let snapshot = resource(manager, panel, node).snapshot();
        assert!(
            !snapshot.nodes.is_empty(),
            "{node}: refresh discarded its complete cached tree"
        );
        let prefix = format!("plugin-tree-{node}");
        visual.update(|window,_| {
            assert!(snapshots(window).iter().any(|row| row.visible() && row.role()==Some(Role::TreeItem)
                && row.path().iter().any(|part| matches!(part,ElementId::Name(name) if name.as_ref()==prefix || name.starts_with(&format!("{prefix}-action-"))))),
                "{node}: populated tree was blank in an intermediate native frame");
        });
    }
    let mut commit_painted = false;
    manager.live["git"].views["history"]
        .root
        .visit(&mut |item| {
            if item.id == "log"
                && let ui::Kind::Canvas(canvas) = &item.kind
            {
                commit_painted = canvas.paint.iter().any(|paint| {
                    matches!(paint, plugin_runtime::plugin_protocol::Paint::Text { text, .. }
                        if matches!(text.as_str(), "base" | "topic"))
                });
            }
        });
    assert!(
        commit_painted,
        "branch selection published a blank commit log before replacement data was ready"
    );
}

/// Inspect native paint submissions, not enabled-state lookup or merely nonempty guest arrays.
fn painted_log(app: &Entity<EditorApp>, visual: &mut VisualTestContext) {
    visual.update(|window, cx| {
        let state = app.read(cx).plugin_panels["git/history"]
            .read(cx)
            .native_ui
            .as_ref()
            .unwrap()
            .read(cx)
            .native_canvas_input_snapshot("log", window, cx)
            .unwrap();
        assert!(state["width"].as_f64().unwrap() > 0. && state["height"].as_f64().unwrap() > 0.);
        assert_eq!(
            state["paint_revision"], state["canvas_revision"],
            "new scene must have a real native paint"
        );
        assert!(
            !state["painted_text"].as_array().unwrap().is_empty(),
            "native history canvas painted a blank intermediate frame: {state}"
        );
    });
}

/// A page-sized main branch and a short topic exercise both large-to-small and small-to-large paints.
fn extend_main_history(root: &Path) {
    use std::io::Write as _;
    let tip = std::process::Command::new("git")
        .current_dir(root)
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    assert!(tip.status.success());
    let mut parent = String::from_utf8(tip.stdout).unwrap().trim().to_owned();
    let mut input = String::new();
    for index in 0..144 {
        let mark = 1000 + index;
        let text = format!("main {index}\n");
        input.push_str(&format!("commit refs/heads/main\nmark :{mark}\ncommitter Incremental Test <incremental@example.invalid> {} +0800\ndata 4\nbase\nfrom {parent}\nM 100644 inline shared.txt\ndata {}\n{text}\n", 1792000000 + index, text.len()));
        parent = format!(":{mark}");
    }
    let mut child = std::process::Command::new("git")
        .current_dir(root)
        .args(["fast-import", "--quiet"])
        .stdin(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(
        result.status.success(),
        "private fast-import: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    // Only this test's generated tracked file is reset; the fixture owns the temporary repository.
    assert!(git(root, &["reset", "--hard", "HEAD"]));
}

/// Two nonempty branches and a retained dirty file make an empty intermediate view unambiguously a bug.
#[gpui::test]
#[ignore = "real independent Git ZIP and native intermediate frames; set NANOBUG_GIT_CANDIDATE"]
fn git_branch_queries_never_blank_populated_review_and_patch_only_changed_files(
    cx: &mut TestAppContext,
) {
    if !run_isolated_test(
        "extensions::windows_tests::incremental_review::git_branch_queries_never_blank_populated_review_and_patch_only_changed_files",
    ) {
        return;
    }
    crate::tests::with_shortcut_editor(cx, false, Vec::new(), |visual, app, path| {
        let root = path.parent().unwrap();
        assert!(git(root, &["init", "-b", "main"]));
        for (key, value) in [
            ("core.autocrlf", "false"),
            ("user.name", "Incremental Test"),
            ("user.email", "incremental@example.invalid"),
        ] {
            assert!(git(root, &["config", key, value]));
        }
        std::fs::write(root.join("shared.txt"), "base\n").unwrap();
        assert!(git(root, &["add", "shared.txt"]));
        assert!(git(root, &["commit", "-m", "base"]));
        assert!(git(root, &["switch", "-c", "topic"]));
        std::fs::write(root.join("shared.txt"), "topic\n").unwrap();
        std::fs::write(root.join("topic.txt"), "topic\n").unwrap();
        assert!(git(root, &["add", "shared.txt", "topic.txt"]));
        assert!(git(root, &["commit", "-m", "topic"]));
        assert!(git(root, &["switch", "main"]));
        extend_main_history(root);
        std::fs::write(root.join("draft.txt"), "retained draft\n").unwrap();
        let package = Package::read(Path::new(
            &std::env::var_os("NANOBUG_GIT_CANDIDATE").unwrap(),
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
        if visual.debug_bounds("plugin-ui-staging-root").is_none() {
            let toggle = visual
                .debug_bounds("plugin-window-git/staging")
                .expect("staging window toggle")
                .center();
            visual.simulate_click(toggle, Default::default());
            flush(&mut manager, &app, visual);
        }
        let branches = resource(&manager, "history", "branches");
        let root_branch = branches
            .snapshot()
            .nodes
            .values()
            .find(|node| node.branch && matches!(node.label.as_str(), "本地" | "Local"))
            .unwrap()
            .id
            .clone();
        // Initial path expansion usually already opened Local; expand it only if necessary.
        if !branches.snapshot().expanded.contains(&root_branch) {
            press(visual, &root_branch, 2);
            flush(&mut manager, &app, visual);
        }
        let topic = branches
            .snapshot()
            .nodes
            .values()
            .find(|node| !node.branch && node.label == "topic")
            .unwrap()
            .id
            .clone();
        populated(&manager, visual);
        let staging = resource(&manager, "staging", "staging");
        let unstaged = staging
            .snapshot()
            .nodes
            .values()
            .find(|node| matches!(node.label.as_str(), "未暂存" | "Unstaged"))
            .unwrap()
            .id
            .clone();
        if !staging.snapshot().expanded.contains(&unstaged) {
            press(visual, &unstaged, 2);
            flush(&mut manager, &app, visual);
        }
        let before = staging.snapshot();
        let draft = before
            .nodes
            .values()
            .find(|node| node.label == "draft.txt")
            .unwrap()
            .clone();
        press(visual, &topic, 2);
        let mut frames = 0;
        flush_checked(&mut manager, &app, visual, |manager, visual| {
            populated(manager, visual);
            painted_log(&app, visual);
            frames += 1;
        });
        assert!(frames >= 2);
        assert_eq!(
            staging.snapshot().nodes,
            before.nodes,
            "filter-only queries cannot discard staging membership"
        );
        assert_eq!(
            staging.snapshot().branches,
            before.branches,
            "filter-only queries cannot discard cached staging pages"
        );
        let local = branches
            .snapshot()
            .nodes
            .values()
            .find(|node| !node.branch && node.label == "main")
            .unwrap()
            .id
            .clone();
        press(visual, &local, 2);
        flush_checked(&mut manager, &app, visual, |manager, visual| {
            populated(manager, visual);
            painted_log(&app, visual);
        });
        // Both branches and their complete previews are now admitted. The native tree mouse
        // event must publish each target immediately, before a process poll can supply Git data.
        let mut measurements = Vec::new();
        for (node, title) in [
            (&topic, "topic"),
            (&local, "base"),
            (&topic, "topic"),
            (&local, "base"),
        ] {
            let started = Instant::now();
            let staging_revision = manager.live["git"].views["staging"].revision;
            press(visual, node, 2);
            composable_tests::pump(&mut manager, &app, visual);
            let mut titles = Vec::new();
            manager.live["git"].views["history"]
                .root
                .visit(&mut |node| {
                    if node.id == "log"
                        && let ui::Kind::Canvas(canvas) = &node.kind
                    {
                        titles.extend(canvas.paint.iter().filter_map(|paint| match paint {
                            plugin_runtime::plugin_protocol::Paint::Text { text, .. } => {
                                Some(text.clone())
                            }
                            _ => None,
                        }));
                    }
                });
            assert!(
                titles.iter().any(|text| text == title),
                "visited branch did not publish immediately: {titles:?}"
            );
            assert_eq!(
                manager.live["git"].process_count(),
                0,
                "cached branch must not launch any Git child"
            );
            let published = started.elapsed();
            assert_eq!(
                manager.live["git"].views["staging"].revision, staging_revision,
                "branch filtering must not replace the unrelated staging scene"
            );
            flush_checked(&mut manager, &app, visual, |manager, visual| {
                populated(manager, visual);
                painted_log(&app, visual);
            });
            eprintln!(
                "native cached branch {title}: publication={published:?}, complete frame checks={:?}, child queries=0",
                started.elapsed()
            );
            measurements.push(serde_json::json!({"target":title, "publication_us":published.as_micros(), "child_queries":0}));
        }
        if let Some(output) = std::env::var_os("NANOBUG_GIT_NATIVE_OUTPUT") {
            let output = PathBuf::from(output);
            std::fs::create_dir_all(&output).unwrap();
            std::fs::write(output.join("branch-fast-switch.json"), serde_json::to_vec_pretty(&serde_json::json!({
                "fixture_commits":146, "warm_switches":measurements, "native_paint_nonempty":true,
                "timing_scope":"mouse event and synchronous publication, before process polling; excludes artificial frame settling"
            })).unwrap()).unwrap();
        }
        // A real external checkout and new dirty path update through the ordinary focus refresh.
        assert!(git(root, &["switch", "topic"]));
        std::fs::write(root.join("new-draft.txt"), "new draft\n").unwrap();
        manager
            .event("git", None, api::Notification::Focus(true))
            .unwrap();
        flush_checked(&mut manager, &app, visual, populated);
        let after = staging.snapshot();
        assert_eq!(
            after.nodes[&draft.id], draft,
            "unchanged cached file retains its identity"
        );
        assert!(
            after
                .nodes
                .values()
                .any(|node| node.label == "new-draft.txt")
        );
        assert!(after.expanded.contains(&unstaged));
        assert_eq!(after.branches[&None], before.branches[&None]);
        std::fs::remove_file(root.join("new-draft.txt")).unwrap();
        manager
            .event("git", None, api::Notification::Focus(true))
            .unwrap();
        flush_checked(&mut manager, &app, visual, populated);
        let removed = staging.snapshot();
        assert_eq!(removed.nodes[&draft.id], draft);
        assert!(
            !removed
                .nodes
                .values()
                .any(|node| node.label == "new-draft.txt")
        );
        assert!(removed.expanded.contains(&unstaged));
        manager.disable("git").unwrap();
    });
}
