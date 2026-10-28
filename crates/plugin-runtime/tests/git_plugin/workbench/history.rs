//! Actual paged Git history retains its complete inline commit preview across local observations.
use super::*;
use plugin_runtime::plugin_protocol::tree;
use std::{io::Write as _, process::Stdio};
#[path = "history/files.rs"]
mod files;
#[path = "history/pagination.rs"]
mod pagination;
#[path = "history/performance.rs"]
mod performance;
#[path = "history/startup.rs"]
mod startup;

/// Import distinct blobs in one Git process, keeping a large-history fixture inexpensive on Windows.
fn import_history(root: &Path, count: usize) {
    require_isolated_git_environment();
    let mut input = Vec::new();
    for index in 0..count {
        let text = format!("history {index}\n");
        let message = format!(
            "history-{index:03}\n\nComplete history body {index} 中文😀\n\nReviewed-by: Workbench\n"
        );
        writeln!(input, "blob\nmark :{}\ndata {}", index * 2 + 1, text.len()).unwrap();
        input.extend_from_slice(text.as_bytes());
        writeln!(input).unwrap();
        writeln!(
            input,
            "commit refs/heads/main\nmark :{}\ncommitter Workbench <workbench@example.invalid> {} +0000\ndata {}\n{}\nM 100644 :{} tracked.txt\n",
            index * 2 + 2,
            1_700_000_000 + index,
            message.len(),
            message,
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

/// One displayed frame can emit many deltas before the next paint; every pixel must be retained.
#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE; real scroll stream"]
fn git_package_scroll_burst_keeps_every_delta_and_rejects_old_clicks() {
    if !run_isolated_test(
        "workbench::history::git_package_scroll_burst_keeps_every_delta_and_rejects_old_clicks",
    ) {
        return;
    }
    let repository = repository();
    import_history(repository.path(), 128);
    let (_profile, mut manager) = activate(&candidate(), repository.path());
    send(
        &mut manager,
        "history",
        "log",
        ui::Action::Canvas(ui::CanvasEvent::Resize {
            width: 1000.,
            height: 280.,
            grid: None,
        }),
    );
    let captured = manager.live["git"].views["history"].clone();
    let staging_before = manager.live["git"].views["staging"].clone();
    let ui::Kind::Canvas(canvas) = &captured.active_node("log").unwrap().kind else {
        panic!("history canvas")
    };
    let epoch = canvas.scroll_epoch.expect("content-owned range input");
    let started = Instant::now();
    for _ in 0..64 {
        manager
            .event(
                "git",
                Some("history".into()),
                api::Notification::Ui(ui::UiEvent {
                    revision: captured.revision,
                    node: "log".into(),
                    action: ui::Action::Canvas(ui::CanvasEvent::ScrollInput {
                        epoch,
                        change: ui::ScrollChange::Delta { delta_y: -7. },
                    }),
                }),
            )
            .expect("painting does not revoke the remaining wheel deltas");
    }
    assert_eq!(log_range(&manager).offset, 448.);
    assert!(
        std::sync::Arc::ptr_eq(&staging_before, &manager.live["git"].views["staging"]),
        "scrolling history must not replace unrelated panel input authority"
    );
    eprintln!(
        "64 real WASM scroll deltas retained 448px in {:?}",
        started.elapsed()
    );
    let error = manager
        .event(
            "git",
            Some("history".into()),
            api::Notification::Ui(ui::UiEvent {
                revision: captured.revision,
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
        .unwrap_err();
    assert_eq!(
        error.downcast_ref::<api::Failure>().unwrap().code,
        api::ErrorCode::StaleRevision
    );
    manager.disable("git").unwrap();
    assert_eq!(manager.resource_count(), 0);
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

/// Load the preview's public file page without opening a comparison in this runtime-only fixture.
fn preview_files(manager: &mut Manager) -> Vec<String> {
    let scene = &manager.live["git"].views["history"];
    let ui::Kind::Tree { handle } = &scene
        .active_node("files")
        .expect("inline history files")
        .kind
    else {
        panic!("history preview must declare its owned tree");
    };
    let handle = handle.clone();
    let resource = manager.live["git"]
        .tree_resources()
        .into_iter()
        .find(|resource| resource.handle() == &handle)
        .unwrap();
    if let Some(request) = resource.request(None, false).unwrap() {
        send(
            manager,
            "history",
            "files",
            ui::Action::Tree(tree::Event::Request(request)),
        );
    }
    let snapshot = resource.snapshot();
    let total = snapshot.branches[&None].children[0].clone();
    if let Some(request) = resource.request(Some(total.clone()), false).unwrap() {
        send(
            manager,
            "history",
            "files",
            ui::Action::Tree(tree::Event::Request(request)),
        );
    }
    let snapshot = resource.snapshot();
    let root = &snapshot.branches[&Some(total)];
    assert!(root.loaded && !root.loading && root.error.is_none());
    let mut files = root
        .children
        .iter()
        .map(|id| {
            let file = &snapshot.nodes[id];
            assert!(
                !file.branch,
                "these bounded fixtures contain only root files"
            );
            file.label.clone()
        })
        .collect::<Vec<_>>();
    files.sort();
    files
}

/// Read exact native preview metadata, including all bounded chunks of the full commit message.
fn preview_text(manager: &Manager, id: &str) -> String {
    let ui::Kind::Text { text } = &manager.live["git"].views["history"]
        .active_node(id)
        .unwrap_or_else(|| panic!("missing inline metadata {id}"))
        .kind
    else {
        panic!("inline metadata must be native text");
    };
    text.clone()
}

/// Capture the whole selected message and metadata so refresh cannot pass by retaining only its OID.
fn preview_contents(manager: &Manager) -> Vec<(String, String)> {
    let mut contents = Vec::new();
    manager.live["git"].views["history"]
        .active_node("history-preview")
        .unwrap()
        .visit(&mut |node| {
            if let ui::Kind::Text { text } = &node.kind {
                contents.push((node.id.clone(), text.clone()));
            }
        });
    contents
}

/// Compare published files, subject, author, timestamp, parents and complete body with actual Git OIDs.
fn assert_preview(manager: &mut Manager, root: &Path, oid: &str, files: &[&str]) {
    assert_eq!(preview_text(manager, "history-oid"), oid);
    assert_eq!(
        preview_text(manager, "history-subject"),
        git(root, &["show", "-s", "--format=%s", oid]).trim_end()
    );
    assert_eq!(
        preview_text(manager, "history-author"),
        format!(
            "Author: {}",
            git(root, &["show", "-s", "--format=%an", oid]).trim_end()
        )
    );
    assert_eq!(
        preview_text(manager, "history-time"),
        format!(
            "Commit time: {}",
            git(root, &["show", "-s", "--format=%aI", oid]).trim_end()
        )
    );
    let parents = git(root, &["show", "-s", "--format=%P", oid])
        .split_whitespace()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    assert_eq!(
        preview_text(manager, "history-parents"),
        format!(
            "Parents: {}",
            if parents.is_empty() {
                "Empty tree".into()
            } else {
                parents.join("\n")
            }
        )
    );
    let message = preview_contents(manager)
        .into_iter()
        .filter(|(id, _)| id.starts_with("history-message-"))
        .map(|(_, text)| text)
        .collect::<String>();
    let complete = git(root, &["show", "-s", "--format=%B", oid]);
    let heading = format!("{}\n", preview_text(manager, "history-subject"));
    assert_eq!(
        message,
        complete.strip_prefix(&heading).unwrap_or(&complete)
    );
    assert_eq!(preview_files(manager), files);
    assert!(
        manager.live["git"].views["history"]
            .active_node("history-loading")
            .is_none()
    );
    assert!(
        manager.live["git"].views["history"]
            .active_node("history-error")
            .is_none()
    );
}

#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE; real 385-commit repository"]
fn git_package_paged_history_keeps_complete_inline_preview_across_focus_save_and_new_commits() {
    // Keep factory and Manager/native Git children in one private configuration process.
    if !run_isolated_test(
        "workbench::history::git_package_paged_history_keeps_complete_inline_preview_across_focus_save_and_new_commits",
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
    let selected = git(root, &["rev-list", "--skip=260", "--max-count=1", "HEAD"])
        .trim()
        .to_owned();
    assert_preview(&mut manager, root, &selected, &["tracked.txt"]);
    let preview = preview_contents(&manager);
    let offset = log_range(&manager).offset;
    // The real 30-second quiet timer is covered by refresh_stability. Unchanged observations
    // preserve their scene, so scene-revision changes cannot be used as proof of a timer tick.
    manager
        .event("git", None, api::Notification::Focus(true))
        .unwrap();
    idle(&mut manager);
    assert_eq!(log_range(&manager).content, 384. * 28.);
    assert_eq!(preview_contents(&manager), preview);
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
    assert_eq!(preview_contents(&manager), preview);
    git(root, &["commit", "--allow-empty", "-m", "fresh-top"]);
    click(&mut manager, "history", "refresh");
    assert_eq!(log_range(&manager).offset, offset);
    assert_eq!(preview_contents(&manager), preview);
    click(&mut manager, "history", "more");
    assert_eq!(log_range(&manager).content, 386. * 28.);
    assert_eq!(preview_contents(&manager), preview);
    assert_preview(&mut manager, root, &selected, &["tracked.txt"]);
    manager.disable("git").unwrap();
    assert_eq!(manager.resource_count(), 0);
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

/// Inspect selected parent IDs and disabled peers from the inline native Choice control.
fn parent_choice(manager: &Manager, side: usize) -> (String, Vec<(String, bool)>) {
    let id = format!("history-parent-{side}");
    match &manager.live["git"].views["history"]
        .active_node(&id)
        .unwrap()
        .kind
    {
        ui::Kind::Choice { options, selected } => (
            selected.clone().unwrap(),
            options
                .iter()
                .map(|option| (option.id.clone(), option.disabled))
                .collect(),
        ),
        _ => panic!("inline parent choice required"),
    }
}

/// Create a real multiline commit body so preview assertions cover more than the subject.
fn commit_file(root: &Path, file: &str, text: &str) {
    std::fs::write(root.join(file), text).unwrap();
    git(root, &["add", "--", file]);
    git(
        root,
        &[
            "commit",
            "-m",
            &format!("{file}\n\n完整提交说明😀\n\nReviewed-by: Workbench"),
        ],
    );
}

#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE; real root/ordinary/octopus history"]
fn git_package_inline_root_ordinary_and_octopus_history_keep_metadata_files_and_distinct_parents() {
    // Keep factory and Manager/native Git children in one private configuration process.
    if !run_isolated_test(
        "workbench::history::git_package_inline_root_ordinary_and_octopus_history_keep_metadata_files_and_distinct_parents",
    ) {
        return;
    }
    let repository = repository();
    let root = repository.path();
    commit_file(root, "tracked.txt", "root version\n");
    let (_profile, mut manager) = activate(&candidate(), root);
    first_commit(&mut manager);
    let root_commit = git(root, &["rev-parse", "HEAD"]).trim().to_owned();
    assert_preview(&mut manager, root, &root_commit, &["tracked.txt"]);
    assert!(
        manager.live["git"].views["history"]
            .active_node("history-parent-0")
            .is_none()
    );

    commit_file(root, "tracked.txt", "ordinary version\n");
    click(&mut manager, "history", "refresh");
    first_commit(&mut manager);
    let ordinary = git(root, &["rev-parse", "HEAD"]).trim().to_owned();
    assert_preview(&mut manager, root, &ordinary, &["tracked.txt"]);
    assert!(
        manager.live["git"].views["history"]
            .active_node("history-parent-0")
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
    let merge = git(root, &["rev-parse", "HEAD"]).trim().to_owned();
    assert_preview(&mut manager, root, &merge, &["first.txt", "second.txt"]);
    assert_eq!(
        parent_choice(&manager, 0),
        (
            "0".into(),
            vec![("0".into(), false), ("1".into(), true), ("2".into(), false)]
        )
    );
    assert_eq!(parent_choice(&manager, 1).0, "1");
    send(
        &mut manager,
        "history",
        "history-parent-0",
        ui::Action::Select("2".into()),
    );
    assert_eq!(parent_choice(&manager, 0).0, "2");
    assert_eq!(
        parent_choice(&manager, 1),
        (
            "1".into(),
            vec![("0".into(), false), ("1".into(), false), ("2".into(), true)]
        )
    );
    send(
        &mut manager,
        "history",
        "history-parent-1",
        ui::Action::Select("0".into()),
    );
    let left = parent_choice(&manager, 0);
    let right = parent_choice(&manager, 1);
    assert_eq!((&left.0, &right.0), (&"2".to_owned(), &"0".to_owned()));
    click(&mut manager, "history", "refresh");
    assert_eq!(parent_choice(&manager, 0), left);
    assert_eq!(parent_choice(&manager, 1), right);
    assert_preview(&mut manager, root, &merge, &["first.txt", "second.txt"]);
    manager.disable("git").unwrap();
    assert_eq!(manager.resource_count(), 0);
}
