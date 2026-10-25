//! The installed Git package projects real commit paths into a compact, counted native tree.
use super::*;
use plugin_runtime::{TreeResource, TreeSnapshot};
use std::sync::Arc;

/// Resolve the tree through its published scene, without reaching into the guest's model.
fn resource(manager: &Manager) -> Arc<TreeResource> {
    let ui::Kind::Tree { handle } = &manager.live["git"].views["history"]
        .active_node("files")
        .unwrap()
        .kind
    else {
        panic!("history must publish a native file tree");
    };
    manager.live["git"]
        .tree_resources()
        .into_iter()
        .find(|resource| resource.handle() == handle)
        .unwrap()
}

/// Request a real bounded page and check its successful admission through the manager.
fn page(
    manager: &mut Manager,
    resource: &Arc<TreeResource>,
    parent: Option<String>,
) -> TreeSnapshot {
    if let Some(request) = resource.request(parent.clone(), false).unwrap() {
        send(
            manager,
            "history",
            "files",
            ui::Action::Tree(tree::Event::Request(request)),
        );
    }
    let snapshot = resource.snapshot();
    let branch = &snapshot.branches[&parent];
    assert!(
        branch.loaded && branch.error.is_none(),
        "{:?}",
        branch.error
    );
    snapshot
}

/// The root replaces the heading, while compressed paths and counts retain the real commit's files.
#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE"]
fn git_package_history_files_has_one_counted_expandable_root() {
    if !run_isolated_test(
        "workbench::history::files::git_package_history_files_has_one_counted_expandable_root",
    ) {
        return;
    }
    let repository = repository();
    let root = repository.path();
    for path in [
        "crates/editor-app/src/a.rs",
        "crates/editor-app/src/b.rs",
        "crates/editor-ui/theme.rs",
        "docs/plugins/verification/readme.md",
        "Cargo.toml",
    ] {
        let file = root.join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, format!("committed {path}\n")).unwrap();
    }
    git(root, &["add", "--all"]);
    git(root, &["commit", "-m", "compact history files"]);
    let (_profile, mut manager) = activate(&candidate(), root);
    let resource = resource(&manager);
    let snapshot = page(&mut manager, &resource, None);
    let roots = &snapshot.branches[&None].children;
    assert_eq!(
        roots.len(),
        1,
        "all changed files belong below one expandable total"
    );
    let total = &snapshot.nodes[&roots[0]];
    assert!(total.branch);
    assert_eq!(total.label, "5 files");
    assert!(total.muted);
    assert!(resource.proportional_labels());
    assert!(
        snapshot.expanded.contains(&total.id),
        "the first root is open"
    );
    let total_id = total.id.clone();
    let snapshot = page(&mut manager, &resource, Some(total_id.clone()));
    let children = &snapshot.branches[&Some(total_id.clone())].children;
    assert_eq!(children.len(), 3);
    let crates = children
        .iter()
        .map(|id| &snapshot.nodes[id])
        .find(|node| node.label == "crates")
        .unwrap();
    assert_eq!(crates.suffix, "3 files");
    let crates_id = crates.id.clone();
    let docs = children
        .iter()
        .map(|id| &snapshot.nodes[id])
        .find(|node| node.label == "docs\\plugins\\verification")
        .unwrap();
    assert_eq!(docs.suffix, "1 file");
    let snapshot = page(&mut manager, &resource, Some(crates_id.clone()));
    let compressed = snapshot
        .nodes
        .values()
        .find(|node| node.label == "editor-app\\src")
        .unwrap();
    assert_eq!(compressed.suffix, "2 files");
    let compressed_id = compressed.id.clone();
    let snapshot = page(&mut manager, &resource, Some(compressed_id.clone()));
    let a = snapshot
        .nodes
        .values()
        .find(|node| node.label == "a.rs")
        .unwrap();
    assert_eq!(a.tone, Some(tree::Tone::Positive));
    let a_id = a.id.clone();
    // A directory has no business action, even though its visible label contains a path separator.
    let revision = manager.live["git"].views["history"].revision;
    select(&mut manager, &resource, &compressed_id);
    assert_eq!(manager.live["git"].views["history"].revision, revision);
    assert_eq!(manager.live["git"].process_count(), 0);
    resource.collapse(&total_id);
    let before = resource.snapshot();
    click(&mut manager, "history", "refresh");
    let after = resource.snapshot();
    assert_eq!(after.epoch, before.epoch);
    assert!(
        !after.expanded.contains(&total_id),
        "refresh must preserve manual collapse"
    );
    assert_eq!(after.branches, before.branches);
    assert_eq!(after.nodes, before.nodes);
    // Ordinary native requests, rather than provider-only APIs, reopen cached directories.
    for id in [total_id.clone(), crates_id, compressed_id] {
        let _ = resource.request(Some(id), false).unwrap();
    }
    // The next commit changes one existing file: its root/count/path projection must replace the old set.
    super::super::commit_file(
        root,
        "crates/editor-app/src/b.rs",
        "second version\n",
        "one changed file",
    );
    click(&mut manager, "history", "refresh");
    select_latest_commit(&mut manager);
    let snapshot = resource.snapshot();
    assert_eq!(snapshot.nodes[&total_id].label, "1 file");
    assert!(!snapshot.nodes.contains_key(&a_id));
    let root_children = &snapshot.branches[&Some(total_id.clone())].children;
    assert_eq!(root_children.len(), 1);
    let directory = &snapshot.nodes[&root_children[0]];
    assert_eq!(directory.label, "crates\\editor-app\\src");
    assert_eq!(directory.suffix, "1 file");

    git(
        root,
        &["commit", "--allow-empty", "-m", "empty changed files"],
    );
    click(&mut manager, "history", "refresh");
    select_latest_commit(&mut manager);
    let snapshot = resource.snapshot();
    assert_eq!(snapshot.nodes[&total_id].label, "0 files");
    assert!(
        snapshot.branches[&Some(total_id.clone())]
            .children
            .is_empty()
    );
    assert_eq!(
        snapshot.nodes.len(),
        1,
        "empty commits retire every former file/directory"
    );

    // A count describes the commit, rather than the first bounded page exposed by a lazy directory.
    fs::create_dir_all(root.join("bulk/nested")).unwrap();
    for index in 0..140 {
        fs::write(
            root.join(format!("bulk/nested/file-{index:03}.txt")),
            format!("{index}\n"),
        )
        .unwrap();
    }
    git(root, &["add", "--all"]);
    git(root, &["commit", "-m", "paged changed files"]);
    click(&mut manager, "history", "refresh");
    select_latest_commit(&mut manager);
    let snapshot = resource.snapshot();
    assert_eq!(snapshot.nodes[&total_id].label, "140 files");
    let directory = snapshot
        .nodes
        .values()
        .find(|node| node.label == "bulk\\nested")
        .unwrap();
    assert_eq!(directory.suffix, "140 files");
    let parent = Some(directory.id.clone());
    let snapshot = page(&mut manager, &resource, parent.clone());
    assert_eq!(snapshot.branches[&parent].children.len(), 128);
    assert_eq!(snapshot.branches[&parent].next.as_deref(), Some("128"));
    let request = resource.request(parent.clone(), true).unwrap().unwrap();
    send(
        &mut manager,
        "history",
        "files",
        ui::Action::Tree(tree::Event::Request(request)),
    );
    let snapshot = resource.snapshot();
    assert_eq!(snapshot.branches[&parent].children.len(), 140);
    assert!(snapshot.branches[&parent].next.is_none());
    assert_eq!(snapshot.nodes[&total_id].label, "140 files");
    // This fixture declines native UI; inspect the source last so that refusal cannot block later navigation.
    let file = snapshot
        .nodes
        .values()
        .find(|node| node.label == "file-000.txt")
        .unwrap();
    select(&mut manager, &resource, &file.id);
    let (title, text) = comparison_source(&mut manager);
    assert!(title.starts_with("file-000.txt · "), "{title}");
    assert_eq!(text, "", "the selected added file has no parent source");
    let details = &manager.live["git"].views["commit-details"];
    assert!(
        matches!(&details.active_node("details-path").unwrap().kind, ui::Kind::Text { text } if text == "bulk/nested/file-000.txt")
    );
    assert!(
        manager.live["git"].views["history"]
            .active_node("history-files-title")
            .is_none()
    );
    manager.disable("git").unwrap();
    assert_eq!(manager.resource_count(), 0);
}

/// A current public canvas pointer selects the first refreshed commit, just as the native log does.
fn select_latest_commit(manager: &mut Manager) {
    send(
        manager,
        "history",
        "log",
        ui::Action::Canvas(ui::CanvasEvent::Resize {
            width: 1000.,
            height: 280.,
            grid: None,
        }),
    );
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

/// Deliver a selection with the current public epoch and current scene; directories must remain inert.
fn select(manager: &mut Manager, resource: &Arc<TreeResource>, id: &str) {
    let revision = manager.live["git"].views["history"].revision;
    manager
        .event(
            "git",
            Some("history".into()),
            api::Notification::Ui(ui::UiEvent {
                revision,
                node: "files".into(),
                action: ui::Action::Tree(tree::Event::Selected {
                    tree: resource.handle().clone(),
                    epoch: resource.snapshot().epoch,
                    node: id.into(),
                }),
            }),
        )
        .unwrap();
}

/// Observe the real immutable source request; a runtime-only fixture explicitly declines native UI.
fn comparison_source(manager: &mut Manager) -> (String, String) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        manager.poll();
        let mut observed = None;
        for (_, request) in manager.take_editor_requests() {
            match request.operation().clone() {
                api::EditorOperation::ListDocuments => {
                    request.finish(Ok(api::EditorValue::Documents {
                        documents: vec![],
                        active: None,
                    }))
                }
                api::EditorOperation::SetPanelVisibility { panel, visible } => {
                    request.finish(Ok(api::EditorValue::PanelVisibility { panel, visible }))
                }
                api::EditorOperation::OpenVirtualSource { title, text, .. } => {
                    observed = Some((title, text));
                    request.finish(Err(api::Failure::new(
                        api::ErrorCode::UnsupportedOperation,
                        "No native comparison in this tree projection fixture",
                    )));
                }
                operation => panic!("unexpected history source request: {operation:?}"),
            }
        }
        if let Some(source) = observed {
            idle(manager);
            return source;
        }
        assert!(
            Instant::now() < deadline,
            "selected original file never requested a comparison"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
