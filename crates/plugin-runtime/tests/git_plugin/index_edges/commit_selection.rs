//! Public installed-package events keep every checkbox local; only explicit submit writes Git.
use super::commit_panel::{check, load_tree};
use super::*;
use plugin_runtime::plugin_protocol::tree;

/// Collapsed and partially loaded groups select locally, preserving index bytes and both panel scenes.
#[test]
#[ignore = "build independent Git candidate with ui.native 1.2; set NANOBUG_GIT_CANDIDATE"]
fn git_package_untracked_checks_keep_files_unversioned_without_git_or_editor_requests() {
    if !run_isolated_test(
        "index_edges::commit_selection::git_package_untracked_checks_keep_files_unversioned_without_git_or_editor_requests",
    ) {
        return;
    }
    let mut timings = vec![];
    for unborn in [false, true] {
        let repository = fixture(unborn);
        let root = repository.path();
        std::fs::create_dir(root.join("new")).unwrap();
        for index in 0..590 {
            std::fs::write(
                root.join(format!("new/file-{index:03}.txt")),
                "unversioned\n",
            )
            .unwrap();
        }
        let (_profile, mut manager) = activate(&candidate(), root);
        commit_submit::draft(&mut manager, "unversioned range");
        let resource = load_tree(&mut manager);
        let group = resource
            .snapshot()
            .nodes
            .values()
            .find(|node| node.label == "Unversioned Files")
            .unwrap()
            .id
            .clone();
        let folder = resource
            .snapshot()
            .nodes
            .values()
            .find(|node| node.label == "new")
            .unwrap()
            .id
            .clone();
        let before_index = std::fs::read(root.join(".git/index")).ok();
        // These are the two admitted workbench panels. Unopened comparison windows may still
        // publish their initial selected OID after history-only startup receipts.
        let staging = manager.live["git"].views["staging"].clone();
        let history = manager.live["git"].views["history"].clone();
        let start = Instant::now();
        check(&mut manager, &resource, &group);
        let group_ms = start.elapsed().as_millis();
        for action in ["commit", "commit-push"] {
            assert!(
                manager.live["git"].views["staging"]
                    .active_node(action)
                    .is_some(),
                "large range admission waited beyond its receipt"
            );
        }
        assert_eq!(
            resource.snapshot().nodes[&folder].checked,
            Some(tree::CheckState::Checked)
        );
        assert_eq!(
            idle_with_documents(&mut manager, &[dirty_document("new/file-000.txt")]),
            0
        );
        assert_eq!(std::fs::read(root.join(".git/index")).ok(), before_index);
        assert_eq!(
            manager.live["git"].views["staging"].active_node("staging"),
            staging.active_node("staging")
        );
        assert_eq!(
            manager.live["git"].views["staging"].active_node("commit-message"),
            staging.active_node("commit-message")
        );
        assert!(
            manager.live["git"].views["history"] == history,
            "a local checkbox replaced the history document"
        );
        assert_eq!(
            git(root, &["ls-files", "--others", "--exclude-standard"])
                .lines()
                .count(),
            590
        );
        let leaf = resource
            .snapshot()
            .nodes
            .values()
            .find(|node| node.label == "file-000.txt")
            .unwrap()
            .id
            .clone();
        check(&mut manager, &resource, &leaf);
        assert_eq!(
            resource.snapshot().nodes[&folder].checked,
            Some(tree::CheckState::Mixed)
        );
        check(&mut manager, &resource, &group);
        check(&mut manager, &resource, &group);
        assert_eq!(
            resource.snapshot().nodes[&folder].checked,
            Some(tree::CheckState::Unchecked)
        );
        assert_eq!(idle_with_documents(&mut manager, &[]), 0);
        assert_eq!(std::fs::read(root.join(".git/index")).ok(), before_index);
        // An unversioned file must also enable both submit actions in this same receipt, including
        // an unborn HEAD. An ordinary click then commits only that file, preserving all siblings.
        commit_submit::draft(&mut manager, "unversioned range");
        let start = Instant::now();
        check(&mut manager, &resource, &leaf);
        for action in ["commit", "commit-push"] {
            assert!(
                manager.live["git"].views["staging"]
                    .active_node(action)
                    .is_some()
            );
        }
        let elapsed = start.elapsed().as_millis();
        timings.push(json!({"unborn": unborn, "untracked_files": 590, "group_to_enabled_ms": group_ms, "leaf_to_enabled_ms": elapsed}));
        assert_eq!(manager.live["git"].process_count(), 0);
        assert!(manager.take_editor_requests().is_empty());
        eprintln!("590 unversioned files; checkbox-to-enabled milliseconds: {elapsed}");
        click(&mut manager, "staging", "commit");
        assert_eq!(
            git(root, &["show", "HEAD:new/file-000.txt"]),
            "unversioned\n"
        );
        assert_eq!(
            git(root, &["ls-files", "--others", "--exclude-standard"])
                .lines()
                .count(),
            589
        );
        manager.disable("git").unwrap();
        assert_eq!(manager.resource_count(), 0);
    }
    if let Some(path) = std::env::var_os("NANOBUG_GIT_RANGE_REPORT") {
        let path = std::path::PathBuf::from(path);
        assert!(path.is_absolute());
        std::fs::write(path, serde_json::to_vec_pretty(&timings).unwrap()).unwrap();
    }
}

/// Rapid tracked selection and reversal stay local; neither the index nor native buffers are touched.
#[test]
#[ignore = "build independent Git candidate with ui.native 1.2; set NANOBUG_GIT_CANDIDATE"]
fn git_package_rapid_tracked_checks_preserve_latest_intent_and_guard_every_write() {
    if !run_isolated_test(
        "index_edges::commit_selection::git_package_rapid_tracked_checks_preserve_latest_intent_and_guard_every_write",
    ) {
        return;
    }
    let repository = fixture(false);
    let root = repository.path();
    std::fs::write(root.join("other.txt"), "base\n").unwrap();
    git(root, &["add", "--", "other.txt"]);
    git(root, &["commit", "-m", "second tracked file"]);
    for name in ["tracked.txt", "other.txt"] {
        std::fs::write(root.join(name), "changed\n").unwrap();
    }
    let (_profile, mut manager) = activate(&candidate(), root);
    let resource = load_tree(&mut manager);
    let id = |name: &str| {
        resource
            .snapshot()
            .nodes
            .values()
            .find(|node| node.label == name)
            .unwrap()
            .id
            .clone()
    };
    let first = id("tracked.txt");
    let second = id("other.txt");
    check(&mut manager, &resource, &first);
    assert_eq!(
        resource.snapshot().nodes[&first].checked,
        Some(tree::CheckState::Checked),
        "feedback waited for Git"
    );
    check(&mut manager, &resource, &second);
    check(&mut manager, &resource, &first);
    assert_eq!(idle_with_documents(&mut manager, &[]), 0);
    assert_eq!(git(root, &["show", ":tracked.txt"]), "base\n");
    assert_eq!(git(root, &["show", ":other.txt"]), "base\n");
    assert_eq!(
        resource.snapshot().nodes[&first].checked,
        Some(tree::CheckState::Unchecked)
    );
    assert_eq!(
        resource.snapshot().nodes[&second].checked,
        Some(tree::CheckState::Checked)
    );
    manager.disable("git").unwrap();
    assert_eq!(manager.resource_count(), 0);
}

/// External index writes cannot turn a local range gesture into a Git write or reset local choices.
#[test]
#[ignore = "build independent Git candidate with ui.native 1.2; set NANOBUG_GIT_CANDIDATE"]
fn git_package_local_range_preserves_external_index_changes() {
    if !run_isolated_test(
        "index_edges::commit_selection::git_package_local_range_preserves_external_index_changes",
    ) {
        return;
    }
    let repository = fixture(false);
    let root = repository.path();
    std::fs::write(root.join("other.txt"), "base\n").unwrap();
    git(root, &["add", "--", "other.txt"]);
    git(root, &["commit", "-m", "second tracked file"]);
    for name in ["tracked.txt", "other.txt"] {
        std::fs::write(root.join(name), "changed\n").unwrap();
    }
    let (_profile, mut manager) = activate(&candidate(), root);
    let resource = load_tree(&mut manager);
    let id = |name: &str| {
        resource
            .snapshot()
            .nodes
            .values()
            .find(|node| node.label == name)
            .unwrap()
            .id
            .clone()
    };
    let first = id("tracked.txt");
    let second = id("other.txt");
    check(&mut manager, &resource, &first);
    check(&mut manager, &resource, &second);
    std::fs::write(root.join("other.txt"), "external staged\n").unwrap();
    git(root, &["add", "--", "other.txt"]);
    std::fs::write(root.join("other.txt"), "later disk text\n").unwrap();
    assert_eq!(idle_with_documents(&mut manager, &[]), 0);
    assert_eq!(git(root, &["show", ":other.txt"]), "external staged\n");
    assert_eq!(
        std::fs::read_to_string(root.join("other.txt")).unwrap(),
        "later disk text\n"
    );
    assert_eq!(
        resource.snapshot().nodes[&second].checked,
        Some(tree::CheckState::Checked)
    );
    manager.disable("git").unwrap();
    assert_eq!(manager.resource_count(), 0);
}
