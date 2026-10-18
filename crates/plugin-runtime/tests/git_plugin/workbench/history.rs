//! Actual paged Git history must survive local observations without blanking an older detail view.
use super::*;
use plugin_runtime::plugin_protocol::tree;
use std::{io::Write as _, process::Stdio};

/// Import distinct blobs in one Git process, keeping a large-history fixture inexpensive on Windows.
fn import_history(root: &Path, count: usize) {
    require_isolated_git_environment();
    let mut input = Vec::new();
    for index in 0..count {
        let text = format!("history {index}\n");
        let title = format!("history-{index:03}");
        writeln!(input, "blob\nmark :{}\ndata {}", index * 2 + 1, text.len()).unwrap();
        input.extend_from_slice(text.as_bytes());
        writeln!(input).unwrap();
        writeln!(
            input,
            "commit refs/heads/main\nmark :{}\ncommitter Workbench <workbench@example.invalid> {} +0000\ndata {}\n{}\nM 100644 :{} tracked.txt\n",
            index * 2 + 2,
            1_700_000_000 + index,
            title.len(),
            title,
            index * 2 + 1,
        )
        .unwrap();
    }
    let mut child = Command::new("git")
        .current_dir(root)
        // Fast-import writes refs and objects; inherit tool settings, never another repository.
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .args(["fast-import", "--quiet"])
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&input).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    git(root, &["reset", "--hard", "HEAD"]);
}

/// The public canvas declares one shared scroll range for both topology and table rows.
fn log_range(manager: &Manager) -> ui::ScrollRange {
    let history = manager.live["git"].views.get("history").unwrap_or_else(|| {
        panic!(
            "history scene missing: {:?}",
            manager.installed["git"].error
        )
    });
    let ui::Kind::Canvas(canvas) = &history.active_node("log").expect("history canvas").kind else {
        panic!("expected native history canvas")
    };
    canvas.scroll.clone().expect("history declares its range")
}

/// Request and select the current files-tree row through its owned opaque resource and epoch.
fn select_file(manager: &mut Manager, path: &str) {
    let scene = manager.live["git"].views["commit-details"].clone();
    let mut declared = None;
    scene.root.visit(&mut |node| {
        if let ui::Kind::Tree { handle } = &node.kind {
            declared = Some((node.id.clone(), handle.clone()));
        }
    });
    let (node, handle) = declared.expect("history files tree");
    let resource = manager.live["git"]
        .tree_resources()
        .into_iter()
        .find(|resource| resource.handle() == &handle)
        .unwrap();
    if let Some(request) = resource.request(None, false).unwrap() {
        send(
            manager,
            "commit-details",
            &node,
            ui::Action::Tree(tree::Event::Request(request)),
        );
    }
    let snapshot = resource.snapshot();
    let file = snapshot
        .nodes
        .values()
        .find(|node| node.label == path)
        .expect("selected commit has its actual changed file");
    send(
        manager,
        "commit-details",
        &node,
        ui::Action::Tree(tree::Event::Selected {
            tree: handle,
            epoch: snapshot.epoch,
            node: file.id.clone(),
        }),
    );
}

/// A timer is driven by the live package's real subscription, rather than a forged timer handle.
fn next_timer_refresh(manager: &mut Manager) {
    let previous = manager.live["git"].views["history"].revision;
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        manager.poll();
        if manager.live["git"].views["history"].revision != previous {
            idle(manager);
            return;
        }
        assert!(
            Instant::now() < deadline,
            "live local refresh timer did not tick"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE; real 385-commit repository"]
fn git_package_loaded_history_keeps_its_older_detail_across_timer_focus_save_and_new_commits() {
    // Keep factory and Manager/native Git children in one private configuration process.
    if !run_isolated_test(
        "workbench::history::git_package_loaded_history_keeps_its_older_detail_across_timer_focus_save_and_new_commits",
    ) {
        return;
    }
    let repository = repository();
    let root = repository.path();
    import_history(root, 385);
    let (_profile, mut manager) = activate(&candidate(), root);
    manager
        .event(
            "git",
            Some("history".into()),
            api::Notification::PanelVisibility {
                panel: "history".into(),
                visible: true,
            },
        )
        .unwrap();
    idle(&mut manager);
    assert_eq!(log_range(&manager).content, 128. * 28.);
    click(&mut manager, "history", "more");
    click(&mut manager, "history", "more");
    assert_eq!(log_range(&manager).content, 384. * 28.);
    send(
        &mut manager,
        "history",
        "log",
        // Window wheel input supplies a pixel delta; a scrollbar's absolute Scroll receipt
        // would conceal a guest which does not handle the native Wheel event at all.
        ui::Action::Canvas(ui::CanvasEvent::Wheel {
            x: 100.,
            y: 14.,
            delta_x: 0.,
            delta_y: -260. * 28.,
            shift: false,
        }),
    );
    assert_eq!(log_range(&manager).offset, 260. * 28.);
    let pointer_revision = manager.live["git"].views["history"].revision;
    // Native Down first focuses its canvas. Resource observations must leave that same
    // admitted revision available for the following immutable-history selection.
    for event in [
        ui::CanvasEvent::Focus { focused: true },
        ui::CanvasEvent::Pointer {
            phase: ui::PointerPhase::Up,
            x: 100.,
            y: 14.,
            button: 0,
            clicks: 1,
            shift: false,
        },
    ] {
        send(&mut manager, "history", "log", ui::Action::Canvas(event));
        assert_eq!(
            manager.live["git"].views["history"].revision,
            pointer_revision
        );
    }
    send(
        &mut manager,
        "history",
        "log",
        ui::Action::Canvas(ui::CanvasEvent::Pointer {
            phase: ui::PointerPhase::Down,
            x: 100.,
            y: 14.,
            button: 0,
            clicks: 1,
            shift: false,
        }),
    );
    select_file(&mut manager, "tracked.txt");
    assert_eq!(snapshot(&manager, 1), "history 124\n");
    let offset = log_range(&manager).offset;
    next_timer_refresh(&mut manager);
    assert_eq!(log_range(&manager).content, 384. * 28.);
    assert_eq!(log_range(&manager).offset, offset);
    assert_eq!(snapshot(&manager, 1), "history 124\n");
    manager
        .event("git", None, api::Notification::Focus(true))
        .unwrap();
    idle(&mut manager);
    assert_eq!(log_range(&manager).content, 384. * 28.);
    assert_eq!(snapshot(&manager, 1), "history 124\n");
    manager.document_event(api::DocumentEvent {
        sequence: 1,
        resource: Some(api::ResourceIdentity::Local {
            path: "tracked.txt".into(),
        }),
        kind: api::DocumentEventKind::DidSave {
            document: api::DocumentVersion {
                id: "native-history-workbench".into(),
                path: "tracked.txt".into(),
                revision: 10,
            },
            result: Ok(()),
        },
    });
    idle(&mut manager);
    assert_eq!(log_range(&manager).content, 384. * 28.);
    assert_eq!(snapshot(&manager, 1), "history 124\n");
    git(root, &["commit", "--allow-empty", "-m", "fresh-top"]);
    click(&mut manager, "history", "refresh");
    assert_eq!(log_range(&manager).offset, offset);
    assert_eq!(snapshot(&manager, 1), "history 124\n");
    click(&mut manager, "history", "more");
    assert_eq!(log_range(&manager).content, 386. * 28.);
    assert_eq!(snapshot(&manager, 1), "history 124\n");
    manager.disable("git").unwrap();
}

/// Open the first visible topology row through the same canvas pointer event as the native table.
fn first_commit(manager: &mut Manager) {
    send(
        manager,
        "history",
        "log",
        ui::Action::Canvas(ui::CanvasEvent::Pointer {
            phase: ui::PointerPhase::Down,
            x: 100.,
            y: 14.,
            button: 0,
            clicks: 1,
            shift: false,
        }),
    );
}

fn title(manager: &Manager, column: usize) -> String {
    let id = format!("pane-title-{column}");
    match &manager.live["git"].views["commit-details"]
        .active_node(&id)
        .unwrap()
        .kind
    {
        ui::Kind::Text { text } => text.clone(),
        _ => panic!("visible source title required"),
    }
}

fn commit_file(root: &Path, file: &str, text: &str) {
    std::fs::write(root.join(file), text).unwrap();
    git(root, &["add", "--", file]);
    git(root, &["commit", "-m", file]);
}

#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE; real root/ordinary/octopus history"]
fn git_package_root_ordinary_and_octopus_history_keep_readonly_sources_and_distinct_parents() {
    // Keep factory and Manager/native Git children in one private configuration process.
    if !run_isolated_test(
        "workbench::history::git_package_root_ordinary_and_octopus_history_keep_readonly_sources_and_distinct_parents",
    ) {
        return;
    }
    let repository = repository();
    let root = repository.path();
    commit_file(root, "tracked.txt", "root version\n");
    let (_profile, mut manager) = activate(&candidate(), root);
    first_commit(&mut manager);
    select_file(&mut manager, "tracked.txt");
    assert_eq!(snapshot(&manager, 0), "");
    assert_eq!(snapshot(&manager, 1), "root version\n");
    assert_eq!(title(&manager, 0), "Empty tree · Read only");

    commit_file(root, "tracked.txt", "ordinary version\n");
    click(&mut manager, "history", "refresh");
    first_commit(&mut manager);
    select_file(&mut manager, "tracked.txt");
    assert_eq!(snapshot(&manager, 0), "root version\n");
    assert_eq!(snapshot(&manager, 1), "ordinary version\n");
    assert!(
        manager.live["git"].views["commit-details"]
            .active_node("snapshot-2")
            .is_none()
    );

    git(root, &["switch", "-c", "first-parent"]);
    commit_file(root, "first.txt", "first branch\n");
    git(root, &["switch", "main"]);
    git(root, &["switch", "-c", "second-parent"]);
    commit_file(root, "second.txt", "second branch\n");
    git(root, &["switch", "main"]);
    git(
        root,
        &[
            "merge",
            "--no-ff",
            "--no-edit",
            "first-parent",
            "second-parent",
        ],
    );
    let parents = git(root, &["show", "-s", "--format=%P", "HEAD"])
        .split_whitespace()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    assert_eq!(parents.len(), 3);
    click(&mut manager, "history", "refresh");
    first_commit(&mut manager);
    select_file(&mut manager, "first.txt");
    for column in 0..3 {
        let _ = snapshot(&manager, column);
    }
    assert_eq!(snapshot(&manager, 1), "first branch\n");
    let original_left = title(&manager, 0);
    send(
        &mut manager,
        "commit-details",
        "history-parent-0",
        ui::Action::Select("2".into()),
    );
    assert_ne!(title(&manager, 0), original_left);
    assert_ne!(title(&manager, 0), title(&manager, 2));
    assert!(title(&manager, 0).contains(&parents[2]));
    let left = title(&manager, 0);
    let right = title(&manager, 2);
    click(&mut manager, "history", "refresh");
    assert_eq!(title(&manager, 0), left);
    assert_eq!(title(&manager, 2), right);
    assert_eq!(snapshot(&manager, 1), "first branch\n");
    manager.disable("git").unwrap();
}
