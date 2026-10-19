//! File-to-directory Git changes keep equally named branch and leaf identities independently usable.
use super::*;
use plugin_runtime::{TreeResource, TreeSnapshot, plugin_protocol::tree};
use std::{collections::BTreeMap, sync::Arc};

/// Stage a real file removal and a different nested addition, without rename similarity.
fn file_to_directory() -> tempfile::TempDir {
    let repository = repository();
    let root = repository.path();
    commit_file(root, "foo", "deleted foo contents\n", "original file");
    fs::remove_file(root.join("foo")).unwrap();
    fs::create_dir(root.join("foo")).unwrap();
    fs::write(root.join("foo/bar"), "new nested bar contents\n").unwrap();
    git(root, &["add", "--all"]);
    assert_eq!(
        git(
            root,
            &["diff", "--cached", "--name-status", "--no-renames", "--"]
        ),
        "D\tfoo\nA\tfoo/bar\n",
        "both original Git paths belong to the same real staged change"
    );
    repository
}

/// Resolve the native tree declared by `node` in `panel`, retaining its public owned resource.
fn resource(manager: &Manager, panel: &str, node: &str) -> Arc<TreeResource> {
    let ui::Kind::Tree { handle } = &manager.live["git"].views[panel]
        .active_node(node)
        .expect("the workbench declares its native tree")
        .kind
    else {
        panic!("expected a native tree");
    };
    manager.live["git"]
        .tree_resources()
        .into_iter()
        .find(|resource| resource.handle() == handle)
        .unwrap()
}

/// Deliver the resource-issued page for `parent`; duplicate IDs must fail public admission.
fn page(
    manager: &mut Manager,
    panel: &str,
    node: &str,
    resource: &Arc<TreeResource>,
    parent: Option<String>,
) -> TreeSnapshot {
    let request = resource.request(parent.clone(), false).unwrap().unwrap();
    let revision = manager.live["git"].views[panel].revision;
    manager
        .event(
            "git",
            Some(panel.into()),
            api::Notification::Ui(ui::UiEvent {
                revision,
                node: node.into(),
                action: ui::Action::Tree(tree::Event::Request(request)),
            }),
        )
        .unwrap();
    let snapshot = resource.snapshot();
    let branch = &snapshot.branches[&parent];
    let mut status = String::new();
    for view in manager.live["git"].views.values() {
        view.root.visit(&mut |node| {
            if let ui::Kind::Text { text } = &node.kind {
                if node.id == "window-status" || node.id == "status" {
                    status.push_str(text);
                }
            }
        });
    }
    assert!(
        branch.loaded && !branch.loading && branch.error.is_none(),
        "{panel} page {parent:?} must admit both legitimate Git entries: error={:?}; {status}",
        branch.error
    );
    snapshot
}

/// Return the equally labelled directory and deleted file from one accepted two-entry page.
fn collision_nodes(snapshot: &TreeSnapshot, parent: Option<String>) -> (String, String) {
    let children = &snapshot.branches[&parent].children;
    assert_eq!(
        children.len(),
        2,
        "neither the directory nor deleted file may be discarded"
    );
    let mut directory = None;
    let mut file = None;
    for id in children {
        let node = &snapshot.nodes[id];
        assert_eq!(node.label, "foo");
        let slot = if node.branch {
            &mut directory
        } else {
            &mut file
        };
        assert!(
            slot.replace(id.clone()).is_none(),
            "each node kind occurs once"
        );
    }
    let directory = directory.expect("foo remains an expandable directory");
    let file = file.expect("deleted foo remains a selectable file");
    assert_ne!(directory, file, "node kind is part of business identity");
    (directory, file)
}

/// Send `id` with the tree's current epoch and the panel's exact published scene revision.
fn select(manager: &mut Manager, panel: &str, node: &str, resource: &Arc<TreeResource>, id: &str) {
    let revision = manager.live["git"].views[panel].revision;
    manager
        .event(
            "git",
            Some(panel.into()),
            api::Notification::Ui(ui::UiEvent {
                revision,
                node: node.into(),
                action: ui::Action::Tree(tree::Event::Selected {
                    tree: resource.handle().clone(),
                    epoch: resource.snapshot().epoch,
                    node: id.into(),
                }),
            }),
        )
        .unwrap();
}

/// Record every visible scene so branch selection cannot silently open or replace another panel.
fn revisions(manager: &Manager) -> BTreeMap<String, u64> {
    manager.live["git"]
        .views
        .iter()
        .map(|(name, view)| (name.clone(), view.revision))
        .collect()
}

/// Expand only `directory` to bar, then prove selecting it schedules no native business work.
fn assert_directory_only(
    manager: &mut Manager,
    panel: &str,
    node: &str,
    resource: &Arc<TreeResource>,
    directory: &str,
) {
    let snapshot = page(manager, panel, node, resource, Some(directory.into()));
    let children = &snapshot.branches[&Some(directory.into())].children;
    assert_eq!(children.len(), 1);
    let child = &snapshot.nodes[&children[0]];
    assert_eq!(child.label, "bar");
    assert!(!child.branch);
    assert_eq!(manager.live["git"].process_count(), 0);
    assert!(manager.take_editor_requests().is_empty());

    let before = revisions(manager);
    select(manager, panel, node, resource, directory);
    assert_eq!(
        revisions(manager),
        before,
        "a directory cannot be mistaken for its equally named deleted file"
    );
    assert_eq!(
        manager.live["git"].process_count(),
        0,
        "directory selection cannot queue a Git diff or history read"
    );
    assert!(
        manager.take_editor_requests().is_empty(),
        "directory selection cannot open any native editor source"
    );
}

/// Observe the deleted file's actual base snapshot through the public native editor request.
fn deleted_diff(manager: &mut Manager) -> (String, String) {
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
                    }));
                }
                api::EditorOperation::SetPanelVisibility { panel, visible } => {
                    request.finish(Ok(api::EditorValue::PanelVisibility { panel, visible }));
                }
                api::EditorOperation::OpenVirtualSource { title, text, .. } => {
                    assert!(observed.replace((title, text)).is_none());
                    // The public receipt proves the requested immutable diff source; this runtime
                    // fixture has no GPUI editor and does not impersonate native document ownership.
                    request.finish(Err(api::Failure::new(
                        api::ErrorCode::UnsupportedOperation,
                        "No native comparison window in this tree-identity fixture",
                    )));
                }
                operation => panic!("unexpected deleted-file editor request: {operation:?}"),
            }
        }
        if let Some(observed) = observed {
            idle(manager);
            return observed;
        }
        assert!(
            Instant::now() < deadline,
            "deleted foo never requested its actual base diff"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Staged deletion foo and addition foo/bar must retain two independently actionable native nodes.
#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE; staged file-to-directory identity"]
fn git_package_staging_tree_distinguishes_a_deleted_file_from_its_replacement_directory() {
    if !run_isolated_test(
        "workbench::tree_collisions::git_package_staging_tree_distinguishes_a_deleted_file_from_its_replacement_directory",
    ) {
        return;
    }
    let repository = file_to_directory();
    let (_profile, mut manager) = activate(&candidate(), repository.path());
    let resource = resource(&manager, "staging", "staging");
    let root = page(&mut manager, "staging", "staging", &resource, None);
    let group = root
        .nodes
        .values()
        .find(|node| node.label == "Staged" && node.branch)
        .expect("the replacement is staged")
        .id
        .clone();
    let snapshot = page(
        &mut manager,
        "staging",
        "staging",
        &resource,
        Some(group.clone()),
    );
    let (directory, file) = collision_nodes(&snapshot, Some(group));
    assert_directory_only(&mut manager, "staging", "staging", &resource, &directory);

    let before = manager.live["git"].views["staging"].revision;
    select(&mut manager, "staging", "staging", &resource, &file);
    assert!(manager.live["git"].views["staging"].revision > before);
    let (title, text) = deleted_diff(&mut manager);
    assert!(title.starts_with("foo"));
    assert_eq!(text, "deleted foo contents\n");
    assert!(matches!(
        &manager.live["git"].views["staging"].active_node("selected-path").unwrap().kind,
        ui::Kind::Text { text } if text == "foo"
    ));
    manager.disable("git").unwrap();
    assert_eq!(manager.resource_count(), 0);
}

/// The inline preview exposes both root rows, and file selection requests the exact deleted source.
#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE; history file-to-directory identity"]
fn git_package_history_tree_distinguishes_a_deleted_file_from_its_replacement_directory() {
    if !run_isolated_test(
        "workbench::tree_collisions::git_package_history_tree_distinguishes_a_deleted_file_from_its_replacement_directory",
    ) {
        return;
    }
    let repository = file_to_directory();
    let root = repository.path();
    let old_commit = git(root, &["rev-parse", "HEAD"]).trim().to_owned();
    git(root, &["commit", "-m", "replace file with directory"]);
    let replacement_commit = git(root, &["rev-parse", "HEAD"]).trim().to_owned();
    assert_eq!(
        git(
            root,
            &[
                "diff-tree",
                "--root",
                "--no-commit-id",
                "--name-only",
                "-r",
                "-m",
                "HEAD",
                "--"
            ]
        ),
        "foo\nfoo/bar\n"
    );
    let (_profile, mut manager) = activate(&candidate(), root);
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
    let resource = resource(&manager, "history", "files");
    let tree_snapshot = page(&mut manager, "history", "files", &resource, None);
    let (directory, file) = collision_nodes(&tree_snapshot, None);
    assert_directory_only(&mut manager, "history", "files", &resource, &directory);
    select(&mut manager, "history", "files", &resource, &file);
    // Observe the actual immutable base request before rejecting native rendering explicitly.
    // The native history-tab test verifies the resulting panes; this runtime fixture owns no editor.
    let (title, text) = deleted_diff(&mut manager);
    assert!(title.contains("foo"));
    assert_eq!(text, "deleted foo contents\n");
    assert!(matches!(
        &manager.live["git"].views["history"].active_node("history-oid").unwrap().kind,
        ui::Kind::Text { text } if text == &replacement_commit
    ));
    assert!(matches!(
        &manager.live["git"].views["history"].active_node("history-parents").unwrap().kind,
        ui::Kind::Text { text } if text == &format!("Parents: {old_commit}")
    ));
    assert!(matches!(
        &manager.live["git"].views["history"].active_node("history-error").unwrap().kind,
        ui::Kind::Text { text } if text.contains("No native comparison window in this tree-identity fixture")
    ));
    assert!(matches!(
        &manager.live["git"].views["commit-details"].active_node("details-path").unwrap().kind,
        ui::Kind::Text { text } if text == "foo"
    ));
    assert_eq!(snapshot(&manager, 0), "deleted foo contents\n");
    assert_eq!(
        snapshot(&manager, 1),
        "",
        "the replaced file has an empty deleted side"
    );
    for (column, commit) in [(0, old_commit), (1, replacement_commit)] {
        let id = format!("pane-title-{column}");
        assert!(
            matches!(
                &manager.live["git"].views["commit-details"].active_node(&id).unwrap().kind,
                ui::Kind::Text { text } if text == &commit
            ),
            "each readonly side retains its exact selected commit identity"
        );
    }
    manager.disable("git").unwrap();
    assert_eq!(manager.resource_count(), 0);
}
