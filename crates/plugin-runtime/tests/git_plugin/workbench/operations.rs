//! Advanced operations enter the published menu and confirmation before mutating disposable Git state.
//! Git CLI constructs fixture inputs and independently observes refs, index entries and disk bytes.
use super::*;
use std::fs;

/// A reviewed action must leave every independently observed repository layer unchanged until Confirm.
#[derive(Debug, PartialEq, Eq)]
struct Observation {
    head: String,
    index: String,
    files: Vec<Option<Vec<u8>>>,
}

fn observe(root: &Path, paths: &[&str]) -> Observation {
    Observation {
        head: git(root, &["rev-parse", "HEAD"]),
        index: git(root, &["ls-files", "--stage"]),
        files: paths
            .iter()
            .map(|path| fs::read(root.join(path)).ok())
            .collect(),
    }
}

/// The action is selected from its current native menu, never injected as an undeclared command.
fn review(manager: &mut Manager, action: &str, target: &str) -> String {
    branch_menu::context_action(manager, action, Some(target));
    let scene = &manager.live["git"].views["history"];
    assert!(
        !scene
            .active_node("confirm-command")
            .expect("explicit confirmation")
            .disabled
    );
    let mut text = String::new();
    scene
        .dialog
        .as_ref()
        .expect("impact review")
        .content
        .visit(&mut |node| {
            if let ui::Kind::Text { text: part } = &node.kind {
                text.push_str(part);
                text.push('\n');
            }
        });
    text
}

/// Confirm the same currently published review; success must retire its modal and native child.
fn confirm(manager: &mut Manager) {
    let scene = &manager.live["git"].views["history"];
    assert!(!scene.active_node("confirm-command").unwrap().disabled);
    click(manager, "history", "confirm-command");
    assert!(manager.live["git"].error.is_none());
    assert!(manager.live["git"].views["history"].dialog.is_none());
    assert_eq!(manager.live["git"].process_count(), 0);
}

/// Ordinary merge may fast-forward, no-ff creates two parents, and squash stages without moving HEAD.
#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE; real advanced merge modes"]
fn git_package_merge_modes_confirm_their_distinct_head_index_and_worktree_effects() {
    // Keep factory and Manager/native Git children in one private configuration process.
    if !run_isolated_test(
        "workbench::operations::git_package_merge_modes_confirm_their_distinct_head_index_and_worktree_effects",
    ) {
        return;
    }
    let package = candidate();
    for action in ["merge", "merge-no-ff", "merge-squash"] {
        let repository = isolated_repository();
        let root = repository.path();
        let base = commit_file(root, "base.txt", "base\n", "base");
        git(root, &["switch", "-c", "source"]);
        let source = commit_file(root, "feature.txt", "feature content\n", "feature");
        git(root, &["switch", "main"]);
        let (_profile, mut manager) = activate(&package, root);
        let before = observe(root, &["base.txt", "feature.txt"]);
        let preview = review(&mut manager, action, "source");
        assert!(
            preview.contains(&source),
            "review must freeze the incoming source: {preview}"
        );
        assert_eq!(
            observe(root, &["base.txt", "feature.txt"]),
            before,
            "{action} review mutated the repository"
        );
        confirm(&mut manager);
        let head = git(root, &["rev-parse", "HEAD"]).trim().to_owned();
        match action {
            "merge" => assert_eq!(
                head, source,
                "ordinary merge should fast-forward this fixture"
            ),
            "merge-no-ff" => {
                let parents = git(root, &["show", "-s", "--format=%P", "HEAD"]);
                assert_eq!(
                    parents.split_whitespace().collect::<Vec<_>>(),
                    [base.as_str(), source.as_str()]
                );
                assert_ne!(head, source);
            }
            _ => {
                assert_eq!(head, base, "squash cannot create or move to a merge commit");
                assert_eq!(
                    git(root, &["diff", "--cached", "--name-only"]),
                    "feature.txt\n"
                );
                assert!(!root.join(".git/MERGE_HEAD").exists());
            }
        }
        assert_eq!(git(root, &["show", ":feature.txt"]), "feature content\n");
        assert_eq!(
            fs::read(root.join("feature.txt")).unwrap(),
            b"feature content\n"
        );
        assert_eq!(fs::read(root.join("base.txt")).unwrap(), b"base\n");
        if action != "merge-squash" {
            assert!(git(root, &["status", "--porcelain"]).is_empty());
        }
        assert_eq!(git(root, &["rev-parse", "source"]).trim(), source);
        manager.disable("git").unwrap();
    }
}

/// All reset modes move HEAD, while only mixed/hard reset the index and only hard resets disk bytes.
#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE; real reset mode effects"]
fn git_package_reset_modes_confirm_their_distinct_head_index_and_worktree_effects() {
    // Keep factory and Manager/native Git children in one private configuration process.
    if !run_isolated_test(
        "workbench::operations::git_package_reset_modes_confirm_their_distinct_head_index_and_worktree_effects",
    ) {
        return;
    }
    let package = candidate();
    for action in ["reset-soft", "reset-mixed", "reset-hard"] {
        let repository = isolated_repository();
        let root = repository.path();
        let target = commit_file(root, "tracked.txt", "base\n", "base");
        fs::write(root.join("tracked.txt"), "committed\n").unwrap();
        fs::write(root.join("committed-only.txt"), "later committed file\n").unwrap();
        git(root, &["add", "--", "tracked.txt", "committed-only.txt"]);
        git(root, &["commit", "-m", "later commit"]);
        fs::write(root.join("tracked.txt"), "staged\n").unwrap();
        git(root, &["add", "--", "tracked.txt"]);
        fs::write(root.join("tracked.txt"), "unstaged\n").unwrap();
        fs::write(root.join("untracked.txt"), "keep untracked\n").unwrap();
        let (_profile, mut manager) = activate(&package, root);
        let paths = ["tracked.txt", "committed-only.txt", "untracked.txt"];
        let before = observe(root, &paths);
        let preview = review(&mut manager, action, &target);
        assert!(preview.contains(&target));
        assert_eq!(
            observe(root, &paths),
            before,
            "{action} review mutated the repository"
        );
        confirm(&mut manager);
        assert_eq!(git(root, &["rev-parse", "HEAD"]).trim(), target);
        assert_eq!(
            fs::read(root.join("untracked.txt")).unwrap(),
            b"keep untracked\n"
        );
        if action == "reset-soft" {
            assert_eq!(git(root, &["ls-files", "--stage"]), before.index);
            assert_eq!(git(root, &["show", ":tracked.txt"]), "staged\n");
        } else {
            assert_eq!(git(root, &["show", ":tracked.txt"]), "base\n");
            assert!(git(root, &["ls-files", "--", "committed-only.txt"]).is_empty());
        }
        if action == "reset-hard" {
            assert_eq!(fs::read(root.join("tracked.txt")).unwrap(), b"base\n");
            assert!(!root.join("committed-only.txt").exists());
        } else {
            assert_eq!(fs::read(root.join("tracked.txt")).unwrap(), b"unstaged\n");
            assert_eq!(
                fs::read(root.join("committed-only.txt")).unwrap(),
                b"later committed file\n"
            );
        }
        manager.disable("git").unwrap();
    }
}

/// Ordinary rebase replays the reviewed range on the frozen upstream without rewriting another ref.
#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE; real ordinary rebase"]
fn git_package_ordinary_rebase_replays_only_the_reviewed_branch_after_confirmation() {
    // Keep factory and Manager/native Git children in one private configuration process.
    if !run_isolated_test(
        "workbench::operations::git_package_ordinary_rebase_replays_only_the_reviewed_branch_after_confirmation",
    ) {
        return;
    }
    let repository = isolated_repository();
    let root = repository.path();
    commit_file(root, "base.txt", "base\n", "base");
    git(root, &["switch", "-c", "topic"]);
    let first = commit_file(root, "topic.txt", "topic one\n", "topic one");
    let original = commit_file(root, "topic-two.txt", "topic two\n", "topic two");
    git(root, &["branch", "retained", &first]);
    git(root, &["config", "--local", "rebase.updateRefs", "true"]);
    git(root, &["switch", "main"]);
    let upstream = commit_file(root, "upstream.txt", "upstream\n", "upstream");
    git(root, &["switch", "topic"]);
    let (_profile, mut manager) = activate(&candidate(), root);
    let paths = ["base.txt", "topic.txt", "topic-two.txt", "upstream.txt"];
    let before = observe(root, &paths);
    let preview = review(&mut manager, "rebase", "main");
    assert!(preview.contains(&upstream) && preview.contains(&first) && preview.contains(&original));
    assert_eq!(observe(root, &paths), before);
    confirm(&mut manager);
    assert_ne!(git(root, &["rev-parse", "HEAD"]).trim(), original);
    assert_eq!(git(root, &["symbolic-ref", "HEAD"]), "refs/heads/topic\n");
    assert_eq!(
        git(root, &["merge-base", "--is-ancestor", &upstream, "HEAD"]),
        ""
    );
    assert_eq!(
        git(
            root,
            &[
                "log",
                "--reverse",
                "--format=%s",
                &format!("{upstream}..HEAD")
            ]
        ),
        "topic one\ntopic two\n"
    );
    assert_eq!(git(root, &["rev-parse", "retained"]).trim(), first);
    for (path, expected) in [
        ("base.txt", "base\n"),
        ("topic.txt", "topic one\n"),
        ("topic-two.txt", "topic two\n"),
        ("upstream.txt", "upstream\n"),
    ] {
        assert_eq!(fs::read(root.join(path)).unwrap(), expected.as_bytes());
    }
    assert!(git(root, &["status", "--porcelain"]).is_empty());
    assert!(!root.join(".git/rebase-merge").exists() && !root.join(".git/rebase-apply").exists());
    manager.disable("git").unwrap();
}

/// Explicit batch order is preserved even when it differs from the source branch's original order.
#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE; real ordered cherry-pick"]
fn git_package_cherry_pick_keeps_the_exact_reviewed_batch_order_after_confirmation() {
    // Keep factory and Manager/native Git children in one private configuration process.
    if !run_isolated_test(
        "workbench::operations::git_package_cherry_pick_keeps_the_exact_reviewed_batch_order_after_confirmation",
    ) {
        return;
    }
    let repository = isolated_repository();
    let root = repository.path();
    let base = commit_file(root, "base.txt", "base\n", "base");
    git(root, &["switch", "-c", "source"]);
    let first = commit_file(root, "first.txt", "first\n", "first");
    let second = commit_file(root, "second.txt", "second\n", "second");
    git(root, &["switch", "main"]);
    let (_profile, mut manager) = activate(&candidate(), root);
    let paths = ["base.txt", "first.txt", "second.txt"];
    let before = observe(root, &paths);
    let preview = review(&mut manager, "cherry-pick", &format!("{second} {first}"));
    assert!(preview.find(&second).unwrap() < preview.find(&first).unwrap());
    assert_eq!(observe(root, &paths), before);
    confirm(&mut manager);
    assert_eq!(
        git(
            root,
            &["log", "--reverse", "--format=%s", &format!("{base}..HEAD")]
        ),
        "second\nfirst\n"
    );
    assert_eq!(
        git(root, &["rev-list", "--count", &format!("{base}..HEAD")]),
        "2\n"
    );
    assert_eq!(git(root, &["symbolic-ref", "HEAD"]), "refs/heads/main\n");
    assert_eq!(git(root, &["rev-parse", "source"]).trim(), second);
    assert_eq!(fs::read(root.join("first.txt")).unwrap(), b"first\n");
    assert_eq!(fs::read(root.join("second.txt")).unwrap(), b"second\n");
    assert!(git(root, &["status", "--porcelain"]).is_empty());
    assert!(!root.join(".git/CHERRY_PICK_HEAD").exists() && !root.join(".git/sequencer").exists());
    manager.disable("git").unwrap();
}
