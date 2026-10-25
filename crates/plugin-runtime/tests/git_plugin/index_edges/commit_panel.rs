//! Local checkbox ranges preserve native trees; explicit submission owns index guards and buffer checks.
use super::*;
use plugin_runtime::{TreeResource, plugin_protocol::tree};
use std::sync::Arc;

/// Actual guest publications preserve the sibling's admitted revision through input and index writes.
#[test]
#[ignore = "build independent Git candidate with ui.canvas 1.3; set NANOBUG_GIT_CANDIDATE"]
fn git_package_panels_keep_sibling_scene_during_draft_history_and_index_operations() {
    if !run_isolated_test(
        "index_edges::commit_panel::git_package_panels_keep_sibling_scene_during_draft_history_and_index_operations",
    ) {
        return;
    }
    let repository = fixture(false);
    let root = repository.path();
    std::fs::write(root.join("tracked.txt"), "changed\n").unwrap();
    let (_profile, mut manager) = activate(&candidate(), root);
    let history = manager.live["git"].views["history"].revision;
    let staging = manager.live["git"].views["staging"].revision;
    manager
        .event(
            "git",
            Some("staging".into()),
            api::Notification::Ui(ui::UiEvent {
                revision: staging,
                node: "commit-message".into(),
                action: ui::Action::Change("isolated 中文 draft".into()),
            }),
        )
        .unwrap();
    assert_eq!(manager.live["git"].views["history"].revision, history);
    let staging = manager.live["git"].views["staging"].revision;
    manager
        .event(
            "git",
            Some("history".into()),
            api::Notification::Ui(ui::UiEvent {
                revision: history,
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
    idle_with_documents(&mut manager, &[]);
    assert_eq!(manager.live["git"].views["staging"].revision, staging);
    let history = manager.live["git"].views["history"].revision;
    let resource = load_tree(&mut manager);
    let cached = resource.snapshot();
    let node = resource
        .snapshot()
        .nodes
        .values()
        .find(|node| node.label == "tracked.txt")
        .unwrap()
        .id
        .clone();
    check(&mut manager, &resource, &node);
    assert_eq!(
        manager.live["git"].views["history"].revision, history,
        "starting stage replaced history"
    );
    idle_with_documents(&mut manager, &[]);
    assert_eq!(
        manager.live["git"].views["history"].revision, history,
        "stage completion reloaded history"
    );
    assert_eq!(git(root, &["show", ":tracked.txt"]), "base\n");
    check(&mut manager, &resource, &node);
    idle_with_documents(&mut manager, &[]);
    assert_eq!(
        manager.live["git"].views["history"].revision, history,
        "unstage reloaded history"
    );
    assert_eq!(git(root, &["show", ":tracked.txt"]), "base\n");
    let retained = resource.snapshot();
    assert_eq!(retained.expanded, cached.expanded);
    for (parent, before) in &cached.branches {
        let after = &retained.branches[parent];
        assert_eq!(
            after.children, before.children,
            "checkbox replaced a loaded directory"
        );
        assert!(
            after.loaded && !after.loading,
            "checkbox reloaded a cached page"
        );
    }
    assert!(
        matches!(&manager.live["git"].views["staging"].active_node("commit-message").unwrap().kind, ui::Kind::Textarea(input) if input.value == "isolated 中文 draft")
    );
    manager.disable("git").unwrap();
    assert_eq!(manager.resource_count(), 0);
}

/// Passive status refreshes may be canceled and must not mutate index stat data or take its lock.
#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE"]
fn git_package_passive_refresh_does_not_write_the_index() {
    if !run_isolated_test(
        "index_edges::commit_panel::git_package_passive_refresh_does_not_write_the_index",
    ) {
        return;
    }
    let repository = fixture(false);
    let root = repository.path();
    let before = std::fs::read(root.join(".git/index")).unwrap();
    std::fs::File::options()
        .write(true)
        .open(root.join("tracked.txt"))
        .unwrap()
        .set_modified(std::time::SystemTime::now() + Duration::from_secs(5))
        .unwrap();
    let (_profile, mut manager) = activate(&candidate(), root);
    idle_with_documents(&mut manager, &[]);
    assert_eq!(
        std::fs::read(root.join(".git/index")).unwrap(),
        before,
        "passive observation rewrote the index"
    );
    manager.disable("git").unwrap();
}

/// A competing lock does not delay local choices; explicit submit fails visibly and can retry the range.
#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE"]
fn git_package_directory_checkbox_recovers_after_index_lock() {
    if !run_isolated_test(
        "index_edges::commit_panel::git_package_directory_checkbox_recovers_after_index_lock",
    ) {
        return;
    }
    let repository = fixture(false);
    let root = repository.path();
    for path in [
        "src/a.txt",
        "src/deep/b.txt",
        "src/deep/deeper/c.txt",
        "sibling/d.txt",
    ] {
        std::fs::create_dir_all(root.join(path).parent().unwrap()).unwrap();
        std::fs::write(root.join(path), "base\n").unwrap();
    }
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "tree"]);
    for path in [
        "src/a.txt",
        "src/deep/b.txt",
        "src/deep/deeper/c.txt",
        "sibling/d.txt",
    ] {
        std::fs::write(root.join(path), "changed\n").unwrap();
    }
    let (_profile, mut manager) = activate(&candidate(), root);
    let resource = load_tree(&mut manager);
    let folder = resource
        .snapshot()
        .nodes
        .values()
        .find(|node| node.label == "src")
        .unwrap()
        .id
        .clone();
    let lock = root.join(".git/index.lock");
    std::fs::write(&lock, b"owned by another operation").unwrap();
    commit_submit::draft(&mut manager, "selected directory");
    check(&mut manager, &resource, &folder);
    assert_eq!(idle_with_documents(&mut manager, &[]), 0);
    assert_eq!(std::fs::read(&lock).unwrap(), b"owned by another operation");
    assert!(git(root, &["diff", "--cached", "--name-only"]).is_empty());
    assert_eq!(
        resource.snapshot().nodes[&folder].checked,
        Some(tree::CheckState::Checked)
    );
    click(&mut manager, "staging", "commit");
    assert!(manager.live["git"].views["staging"].dialog.is_some());
    assert!(git(root, &["diff", "--cached", "--name-only"]).is_empty());
    click(&mut manager, "staging", "commit-submit-error-close");
    let scene = &manager.live["git"].views["staging"];
    scene.root.visit(&mut |node| {
        if let ui::Kind::Text { text } = &node.kind {
            assert!(
                !text.contains("index.lock"),
                "raw lock stderr expanded the composer"
            );
        }
    });
    assert!(
        manager.live["git"].views["staging"]
            .active_node("status")
            .is_none()
    );
    // Only this test owns this lock. Production code must never remove another process's lock.
    std::fs::remove_file(&lock).unwrap();
    click(&mut manager, "staging", "commit");
    assert_eq!(
        git(
            root,
            &["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"]
        ),
        "src/a.txt\nsrc/deep/b.txt\nsrc/deep/deeper/c.txt\n"
    );
    assert!(git(root, &["diff", "--cached", "--name-only"]).is_empty());
    assert_eq!(git(root, &["show", "HEAD:sibling/d.txt"]), "base\n");
    assert_eq!(
        std::fs::read_to_string(root.join("src/deep/deeper/c.txt")).unwrap(),
        "changed\n"
    );
    manager.disable("git").unwrap();
    assert_eq!(manager.resource_count(), 0);
}

/// Drive ordinary bounded lazy requests as a native tree would; no test-only guest API is involved.
pub(super) fn load_tree(manager: &mut Manager) -> Arc<TreeResource> {
    let view = &manager.live["git"].views["staging"];
    let ui::Kind::Tree { handle } = &view.active_node("staging").unwrap().kind else {
        panic!("tree missing")
    };
    let resource = manager.live["git"]
        .tree_resources()
        .into_iter()
        .find(|tree| tree.handle() == handle)
        .unwrap();
    loop {
        let next = resource
            .snapshot()
            .branches
            .into_iter()
            .find(|(_, branch)| !branch.loaded && !branch.loading);
        let Some((parent, _)) = next else {
            break;
        };
        let request = resource.request(parent, false).unwrap().unwrap();
        let revision = manager.live["git"].views["staging"].revision;
        manager
            .event(
                "git",
                Some("staging".into()),
                api::Notification::Ui(ui::UiEvent {
                    revision,
                    node: "staging".into(),
                    action: ui::Action::Tree(tree::Event::Request(request)),
                }),
            )
            .unwrap();
    }
    resource
}

/// Use current metadata, just like a rendered checkbox press; native observations finish separately.
pub(super) fn check(manager: &mut Manager, tree: &TreeResource, node: &str) {
    let snapshot = tree.snapshot();
    let previous = snapshot.nodes[node].checked.unwrap();
    let revision = manager.live["git"].views["staging"].revision;
    manager
        .event(
            "git",
            Some("staging".into()),
            api::Notification::Ui(ui::UiEvent {
                revision,
                node: "staging".into(),
                action: ui::Action::Tree(tree::Event::Checked {
                    tree: tree.handle().clone(),
                    epoch: snapshot.epoch,
                    node: node.into(),
                    previous,
                    checked: previous != tree::CheckState::Checked,
                }),
            }),
        )
        .unwrap();
}

/// Intent-to-add choices are local; explicit submit checks selected buffers and supports unborn HEAD.
#[test]
#[ignore = "build independent Git candidate with ui.tree 1.7; set NANOBUG_GIT_CANDIDATE"]
fn git_package_commit_checkboxes_stage_literal_groups_and_preserve_dirty_buffers() {
    if !run_isolated_test(
        "index_edges::commit_panel::git_package_commit_checkboxes_stage_literal_groups_and_preserve_dirty_buffers",
    ) {
        return;
    }
    for unborn in [true, false] {
        let repository = fixture(unborn);
        let root = repository.path();
        std::fs::create_dir(root.join("src")).unwrap();
        for name in ["src/[new].txt", "src/other.txt"] {
            std::fs::write(root.join(name), "worktree\n").unwrap();
        }
        git(root, &["add", "-N", "--", "src/[new].txt", "src/other.txt"]);
        let (_profile, mut manager) = activate(&candidate(), root);
        let resource = load_tree(&mut manager);
        let folder = resource
            .snapshot()
            .nodes
            .values()
            .find(|node| node.label == "src")
            .unwrap()
            .id
            .clone();
        commit_submit::draft(&mut manager, "intent range");
        check(&mut manager, &resource, &folder);
        assert_eq!(
            idle_with_documents(&mut manager, &[dirty_document("src/[new].txt")]),
            0
        );
        assert!(git(root, &["diff", "--cached", "--name-only"]).is_empty());
        commit_submit::start_submit(&mut manager, "commit");
        assert!(idle_with_documents(&mut manager, &[dirty_document("src/[new].txt")]) > 0);
        assert!(manager.live["git"].views["staging"].dialog.is_some());
        assert!(git(root, &["diff", "--cached", "--name-only"]).is_empty());
        click(&mut manager, "staging", "commit-submit-error-close");
        commit_submit::start_submit(&mut manager, "commit");
        idle_with_documents(&mut manager, &[]);
        assert_eq!(git(root, &["show", "HEAD:src/[new].txt"]), "worktree\n");
        assert_eq!(git(root, &["show", "HEAD:src/other.txt"]), "worktree\n");
        assert!(git(root, &["diff", "--cached", "--name-only"]).is_empty());
        assert_eq!(
            std::fs::read_to_string(root.join("src/[new].txt")).unwrap(),
            "worktree\n"
        );
        manager.disable("git").unwrap();
        assert_eq!(manager.resource_count(), 0);
    }
}

/// Local inclusion stays separate from partial staging; an external mutation after submit cancels it.
#[test]
#[ignore = "build independent Git candidate with ui.tree 1.7; set NANOBUG_GIT_CANDIDATE"]
fn git_package_commit_checkbox_rechecks_partial_index_before_writing() {
    if !run_isolated_test(
        "index_edges::commit_panel::git_package_commit_checkbox_rechecks_partial_index_before_writing",
    ) {
        return;
    }
    let repository = fixture(false);
    let root = repository.path();
    std::fs::write(root.join("tracked.txt"), "partial\n").unwrap();
    git(root, &["add", "--", "tracked.txt"]);
    std::fs::write(root.join("tracked.txt"), "remaining\n").unwrap();
    let (_profile, mut manager) = activate(&candidate(), root);
    let resource = load_tree(&mut manager);
    let row = resource
        .snapshot()
        .nodes
        .values()
        .find(|node| node.label == "tracked.txt")
        .unwrap()
        .clone();
    assert_eq!(row.checked, Some(tree::CheckState::Unchecked));
    commit_submit::draft(&mut manager, "guard range");
    check(&mut manager, &resource, &row.id);
    assert_eq!(manager.live["git"].process_count(), 0);
    commit_submit::start_submit(&mut manager, "commit");
    // The pending ListDocuments response separates the accepted submit from its Git preflight.
    git(root, &["restore", "--staged", "--", "tracked.txt"]);
    idle_with_documents(&mut manager, &[]);
    assert!(manager.live["git"].views["staging"].dialog.is_some());
    assert_eq!(git(root, &["show", ":tracked.txt"]), "base\n");
    assert_eq!(
        std::fs::read_to_string(root.join("tracked.txt")).unwrap(),
        "remaining\n"
    );
    manager.disable("git").unwrap();
}
