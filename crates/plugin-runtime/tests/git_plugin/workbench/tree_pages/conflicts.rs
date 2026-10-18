//! A live conflict and its already opened native draft must publish one repository-relative leaf.
use super::*;
use plugin_runtime::{TreeResource, TreeSnapshot};
use std::sync::Arc;

/// Serve only the existing draft through public editor receipts while real Git reads complete.
/// The fixture never edits or saves: opening or selecting the file must retain this exact version.
fn settle_conflict_draft(manager: &mut Manager, info: &api::DocumentInfo, text: &str) {
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut settled = 0;
    loop {
        manager.poll();
        let requests = manager.take_editor_requests();
        let had_requests = !requests.is_empty();
        for (_, request) in requests {
            match request.operation().clone() {
                api::EditorOperation::ListDocuments => {
                    request.finish(Ok(api::EditorValue::Documents {
                        documents: vec![info.clone()],
                        active: Some(info.document.clone()),
                    }));
                }
                api::EditorOperation::SetPanelVisibility { panel, visible } => {
                    request.finish(Ok(api::EditorValue::PanelVisibility { panel, visible }));
                }
                api::EditorOperation::OpenDocument { resource } => {
                    assert_eq!(resource, info.resource);
                    request.finish(Ok(api::EditorValue::DocumentOpened(info.clone())));
                }
                api::EditorOperation::ReadDocument { document, range } => {
                    assert_eq!(
                        document, info.document,
                        "draft reads require the exact version"
                    );
                    assert!(range.is_none());
                    request.finish(Ok(api::EditorValue::DocumentSnapshot(
                        api::DocumentSnapshot {
                            info: info.clone(),
                            range: api::TextRange {
                                start: 0,
                                end: text.len(),
                            },
                            text: text.into(),
                        },
                    )));
                }
                operation => panic!("unexpected conflict-tree editor operation: {operation:?}"),
            }
        }
        manager.poll();
        assert!(
            manager.live["git"].error.is_none(),
            "{:?}",
            manager.diagnostics("git")
        );
        settled = if manager.live["git"].process_count() == 0 && !had_requests {
            settled + 1
        } else {
            0
        };
        if settled >= 8 {
            return;
        }
        assert!(Instant::now() < deadline, "conflict draft did not settle");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Load each fresh root and directory exactly once, retaining the resource-issued request identities.
fn conflict_directory(manager: &mut Manager, resource: &Arc<TreeResource>) -> TreeSnapshot {
    use plugin_runtime::plugin_protocol::tree;

    let mut page = |parent| {
        let request = resource.request(parent, false).unwrap().unwrap();
        let revision = manager.live["git"].views["conflicts"].revision;
        manager
            .event(
                "git",
                Some("conflicts".into()),
                api::Notification::Ui(ui::UiEvent {
                    revision,
                    node: "conflicts".into(),
                    action: ui::Action::Tree(tree::Event::Request(request)),
                }),
            )
            .unwrap();
    };
    page(None);
    let root = resource.snapshot();
    assert!(root.branches[&None].loaded);
    assert_eq!(root.branches[&None].children.len(), 1);
    let directory = root.branches[&None].children[0].clone();
    assert!(root.nodes[&directory].branch);
    page(Some(directory));
    resource.snapshot()
}

/// An accepted directory page must expose exactly one leaf; its source must remain the real draft.
fn assert_unique_conflict_leaf(manager: &Manager, snapshot: &TreeSnapshot) -> String {
    let directory = &snapshot.branches[&None].children[0];
    let branch = &snapshot.branches[&Some(directory.clone())];
    let mut diagnostic = String::new();
    manager.live["git"].views["conflicts"]
        .root
        .visit(&mut |node| {
            if node.id == "window-status" {
                if let ui::Kind::Text { text } = &node.kind {
                    diagnostic = text.clone();
                }
            }
        });
    assert!(
        branch.loaded && !branch.loading && branch.error.is_none(),
        "current conflict directory must publish its unique leaf: {diagnostic}"
    );
    assert_eq!(branch.children.len(), 1, "one path must produce one leaf");
    let leaf = &snapshot.nodes[&branch.children[0]];
    assert_eq!(leaf.label, "merge.txt");
    assert!(!leaf.branch);
    leaf.id.clone()
}

/// The old package duplicates the same live path once its native result draft is opened.
#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE; conflict tree with an open native draft"]
fn git_package_conflict_tree_has_one_leaf_for_a_live_path_with_an_open_draft() {
    if !run_isolated_test(
        "workbench::tree_pages::conflicts::git_package_conflict_tree_has_one_leaf_for_a_live_path_with_an_open_draft",
    ) {
        return;
    }
    let repository = repository();
    let root = repository.path();
    let path = "conflicts/merge.txt";
    std::fs::create_dir(root.join("conflicts")).unwrap();
    commit_file(root, path, "base\n", "base");
    git(root, &["switch", "-c", "topic"]);
    commit_file(root, path, "incoming\n", "incoming");
    git(root, &["switch", "main"]);
    commit_file(root, path, "current\n", "current");
    let merge = Command::new("git")
        .current_dir(root)
        .args(["merge", "--no-edit", "topic"])
        .output()
        .unwrap();
    assert!(
        !merge.status.success(),
        "the fixture must enter a real conflict"
    );
    assert!(root.join(".git/MERGE_HEAD").is_file());
    assert!(!git(root, &["ls-files", "--unmerged", "--", path]).is_empty());

    let (_profile, mut manager) = activate(&candidate(), root);
    let ui::Kind::Tree { handle } = &manager.live["git"].views["conflicts"]
        .active_node("conflicts")
        .unwrap()
        .kind
    else {
        panic!("the conflict window declares its owned native tree");
    };
    let resource = manager.live["git"]
        .tree_resources()
        .into_iter()
        .find(|resource| resource.handle() == handle)
        .unwrap();
    // Control: the same real conflict path is unique before a result draft is registered.
    let before = conflict_directory(&mut manager, &resource);
    let first = assert_unique_conflict_leaf(&manager, &before);
    let disk = std::fs::read_to_string(root.join(path)).unwrap();
    let text = format!("{disk}unsaved native draft\n");
    let info = api::DocumentInfo {
        document: api::DocumentVersion {
            id: "conflict-tree-result-session".into(),
            path: path.into(),
            revision: 7,
        },
        resource: api::ResourceIdentity::Local { path: path.into() },
        title: "merge.txt".into(),
        access: api::DocumentAccess {
            read: true,
            edit: true,
            save: true,
        },
        dirty: true,
        language: "text".into(),
        encoding: api::DocumentEncoding::Utf8,
        eol: api::DocumentEol::of(&text),
        byte_len: text.len(),
        selection: api::TextRange { start: 0, end: 0 },
        visible_rows: None,
    };
    manager
        .invoke_command("git", "resolve-file", json!({"path": path}))
        .unwrap();
    settle_conflict_draft(&mut manager, &info, &text);
    assert_eq!(
        manager.live["git"].views["conflicts"].source.as_ref(),
        Some(&info.document)
    );
    // The ordinary refresh merges live status and retained draft metadata in a new tree epoch.
    manager
        .invoke_command("git", "refresh", serde_json::Value::Null)
        .unwrap();
    settle_conflict_draft(&mut manager, &info, &text);
    let after = conflict_directory(&mut manager, &resource);
    let leaf = assert_unique_conflict_leaf(&manager, &after);
    assert_eq!(
        leaf, first,
        "the repository path keeps its stable native identity"
    );
    let revision = manager.live["git"].views["conflicts"].revision;
    manager
        .event(
            "git",
            Some("conflicts".into()),
            api::Notification::Ui(ui::UiEvent {
                revision,
                node: "conflicts".into(),
                action: ui::Action::Tree(plugin_runtime::plugin_protocol::tree::Event::Selected {
                    tree: resource.handle().clone(),
                    epoch: after.epoch,
                    node: leaf,
                }),
            }),
        )
        .unwrap();
    settle_conflict_draft(&mut manager, &info, &text);
    assert_eq!(
        manager.live["git"].views["conflicts"].source.as_ref(),
        Some(&info.document)
    );
    assert_eq!(std::fs::read_to_string(root.join(path)).unwrap(), disk);
    manager.disable("git").unwrap();
    assert_eq!(manager.resource_count(), 0);
}
