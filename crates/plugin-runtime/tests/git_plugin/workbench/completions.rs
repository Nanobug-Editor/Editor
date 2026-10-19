//! Positive completion branches complement rejection tests through the installed package and real Git.
use super::*;
use std::{fs, path::PathBuf};

/// Observe an exact object identity independently from the guest's visible operation result.
fn oid(root: &Path, reference: &str) -> String {
    git(root, &["rev-parse", "--verify", reference])
        .trim()
        .into()
}

/// The public manager owns private-data identity; no test reaches into a guest's in-memory plan.
fn plan_file(profile: &Path, package: &Package, root: &Path) -> PathBuf {
    Manager::persisted_data_directory(
        &profile.join("plugins"),
        &package.manifest,
        &root.display().to_string(),
    )
    .unwrap()
    .join("rebase-plan.json")
}

/// Finishing either lifecycle action revokes the persisted approval instead of reviving it later.
fn assert_plan_cleared(path: &Path) {
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&fs::read(path).unwrap()).unwrap(),
        json!(null)
    );
}

/// Squash uses Git's message editor as well as the sequence editor. Merge-squash or fixup alone
/// cannot prove that the reviewed combined message and both original file changes are retained.
#[test]
#[ignore = "build independent Git candidate with native rebase dependency; set NANOBUG_GIT_CANDIDATE"]
fn git_package_interactive_squash_commits_the_reviewed_message_and_combined_tree() {
    // Keep factory and Manager/native Git children in one private configuration process.
    if !run_isolated_test(
        "workbench::completions::git_package_interactive_squash_commits_the_reviewed_message_and_combined_tree",
    ) {
        return;
    }
    let repository = isolated_repository();
    let root = repository.path();
    let base = commit_file(root, "base.txt", "base\n", "base");
    let first = commit_file(root, "first.txt", "first bytes\n", "first subject");
    commit_file(root, "second.txt", "second bytes 中😀\n", "second subject");
    let original_head = oid(root, "HEAD");
    let original_tree = oid(root, "HEAD^{tree}");
    let original_index = git(root, &["ls-files", "--stage"]);
    git(root, &["branch", "retained", &first]);
    let package = candidate();
    let (profile, mut manager) = activate(&package, root);
    let persisted = plan_file(profile.path(), &package, root);
    manager
        .invoke_command("git", "rebase-plan", json!({"target":base}))
        .unwrap();
    idle(&mut manager);
    ui_action(
        &mut manager,
        "rebase-action-1",
        ui::Action::Select("squash".into()),
    );
    let message = "Combined subject 中文😀\n\nReviewed first and second changes\n";
    ui_action(
        &mut manager,
        "rebase-message-1",
        ui::Action::Change(message.into()),
    );
    click(&mut manager, "history", "rebase-run");
    assert_eq!(
        oid(root, "HEAD"),
        original_head,
        "review cannot squash commits"
    );
    assert_eq!(git(root, &["ls-files", "--stage"]), original_index);
    assert!(!root.join(".git/rebase-merge").exists());
    click(&mut manager, "history", "confirm-command");

    assert_ne!(oid(root, "HEAD"), original_head);
    assert_eq!(
        git(root, &["rev-list", "--count", &format!("{base}..HEAD")]),
        "1\n"
    );
    assert_eq!(oid(root, "HEAD^"), base);
    assert_eq!(oid(root, "HEAD^{tree}"), original_tree);
    assert_eq!(
        git(root, &["log", "-1", "--format=%B"]),
        format!("{message}\n")
    );
    assert_eq!(git(root, &["show", "HEAD:first.txt"]), "first bytes\n");
    assert_eq!(
        git(root, &["show", "HEAD:second.txt"]),
        "second bytes 中😀\n"
    );
    assert_eq!(oid(root, "retained"), first);
    assert!(git(root, &["status", "--porcelain"]).is_empty());
    assert_plan_cleared(&persisted);
    manager.disable("git").unwrap();
    assert_eq!(manager.resource_count(), 0);
}

/// Moving B before its prerequisite A enters a real conflict. Dropping A leaves C as the next
/// operation, so Skip must actually resume the approved helper rather than merely clear a marker.
fn start_conflicting_plan(
    package: &Package,
    root: &Path,
    base: &str,
) -> (tempfile::TempDir, Manager) {
    let (profile, mut manager) = activate(package, root);
    manager
        .invoke_command("git", "rebase-plan", json!({"target":base}))
        .unwrap();
    idle(&mut manager);
    click(&mut manager, "history", "rebase-up-1");
    ui_action(
        &mut manager,
        "rebase-action-1",
        ui::Action::Select("drop".into()),
    );
    click(&mut manager, "history", "rebase-run");
    click(&mut manager, "history", "confirm-command");
    assert!(root.join(".git/rebase-merge").is_dir());
    assert!(!git(root, &["ls-files", "--unmerged", "--", "merge.txt"]).is_empty());
    assert!(!root.join("tail.txt").exists());
    (profile, manager)
}

/// A reopened installation can skip its real conflicted interactive step or abort the whole
/// operation. Both commands require their own confirmation and leave no stale rebase approval.
#[test]
#[ignore = "build independent Git candidate with native rebase dependency; set NANOBUG_GIT_CANDIDATE"]
fn git_package_reopened_interactive_rebase_can_skip_or_abort_its_actual_conflict() {
    // Keep factory and Manager/native Git children in one private configuration process.
    if !run_isolated_test(
        "workbench::completions::git_package_reopened_interactive_rebase_can_skip_or_abort_its_actual_conflict",
    ) {
        return;
    }
    let package = candidate();
    for action in ["skip", "abort"] {
        let repository = isolated_repository();
        let root = repository.path();
        let base = commit_file(root, "merge.txt", "base\n", "base");
        commit_file(root, "merge.txt", "A\n", "A");
        commit_file(root, "merge.txt", "B 中😀\n", "B");
        let original = commit_file(root, "tail.txt", "C\n", "C");
        let tree = oid(root, "HEAD^{tree}");
        let index = git(root, &["ls-files", "--stage"]);
        let (profile, mut manager) = start_conflicting_plan(&package, root, &base);
        let persisted = plan_file(profile.path(), &package, root);
        let plan = fs::read(&persisted).unwrap();
        manager = reopen(manager, profile.path(), root);
        assert_eq!(fs::read(&persisted).unwrap(), plan);
        let paused_head = oid(root, "HEAD");
        let paused_index = git(root, &["ls-files", "--stage"]);
        click(&mut manager, "conflicts", action);
        assert!(
            manager.live["git"].views["conflicts"]
                .active_node("confirm-command")
                .is_some()
        );
        assert_eq!(oid(root, "HEAD"), paused_head, "preview cannot {action}");
        assert_eq!(git(root, &["ls-files", "--stage"]), paused_index);
        assert!(root.join(".git/rebase-merge").is_dir());
        click(&mut manager, "conflicts", "confirm-command");

        assert!(
            !root.join(".git/rebase-merge").exists(),
            "{action} did not finish"
        );
        assert_eq!(git(root, &["symbolic-ref", "HEAD"]), "refs/heads/main\n");
        assert!(git(root, &["ls-files", "--unmerged"]).is_empty());
        assert!(git(root, &["status", "--porcelain"]).is_empty());
        assert_eq!(fs::read(root.join("tail.txt")).unwrap(), b"C\n");
        if action == "skip" {
            assert_ne!(oid(root, "HEAD"), original);
            assert_eq!(oid(root, "HEAD^"), base);
            assert_eq!(
                git(
                    root,
                    &["log", "--reverse", "--format=%s", &format!("{base}..HEAD")]
                ),
                "C\n"
            );
            assert_eq!(git(root, &["show", "HEAD:merge.txt"]), "base\n");
        } else {
            assert_eq!(oid(root, "HEAD"), original);
            assert_eq!(oid(root, "HEAD^{tree}"), tree);
            assert_eq!(git(root, &["ls-files", "--stage"]), index);
            assert_eq!(
                fs::read(root.join("merge.txt")).unwrap(),
                "B 中😀\n".as_bytes()
            );
        }
        assert_plan_cleared(&persisted);
        manager.disable("git").unwrap();
        assert_eq!(manager.resource_count(), 0);
    }
}

/// Read reflog object identities, not positions that shift after deletion.
fn stash_oids(root: &Path) -> Vec<String> {
    git(root, &["stash", "list", "--format=%H"])
        .lines()
        .map(str::to_owned)
        .collect()
}

/// The positive pop/delete branch must remove only the explicitly reviewed object. A later
/// standalone drop must preserve the applied worktree, index and HEAD without applying its entry.
#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE; positive stash completion"]
fn git_package_successful_stash_pop_and_drop_remove_only_the_confirmed_entries() {
    // Keep factory and Manager/native Git children in one private configuration process.
    if !run_isolated_test(
        "workbench::completions::git_package_successful_stash_pop_and_drop_remove_only_the_confirmed_entries",
    ) {
        return;
    }
    let repository = isolated_repository();
    let root = repository.path();
    let head = commit_file(root, "tracked.txt", "base\n", "base");
    let index = git(root, &["ls-files", "--stage"]);
    for (title, text) in [
        ("older entry", "older bytes\n"),
        ("newest entry", "newest bytes 中😀\n"),
    ] {
        fs::write(root.join("tracked.txt"), text).unwrap();
        git(root, &["stash", "push", "-m", title]);
    }
    let entries = stash_oids(root);
    assert_eq!(entries.len(), 2);
    let (_profile, mut manager) = activate(&candidate(), root);
    click(&mut manager, "history", "stash-list");
    click(&mut manager, "history", "repo-stash-pop");
    assert_eq!(
        stash_oids(root),
        entries,
        "pop preview cannot remove an entry"
    );
    assert_eq!(fs::read(root.join("tracked.txt")).unwrap(), b"base\n");
    click(&mut manager, "history", "repo-stash-confirm");
    assert_eq!(stash_oids(root), [entries[1].clone()]);
    let applied = "newest bytes 中😀\n".as_bytes();
    assert_eq!(fs::read(root.join("tracked.txt")).unwrap(), applied);
    assert_eq!(oid(root, "HEAD"), head);
    assert_eq!(git(root, &["ls-files", "--stage"]), index);

    click(&mut manager, "history", "repo-close");
    click(&mut manager, "history", "stash-list");
    click(&mut manager, "history", "repo-stash-drop");
    assert_eq!(
        stash_oids(root),
        [entries[1].clone()],
        "drop preview cannot remove an entry"
    );
    assert_eq!(fs::read(root.join("tracked.txt")).unwrap(), applied);
    click(&mut manager, "history", "repo-stash-confirm");
    assert!(stash_oids(root).is_empty());
    assert_eq!(fs::read(root.join("tracked.txt")).unwrap(), applied);
    assert_eq!(oid(root, "HEAD"), head);
    assert_eq!(git(root, &["ls-files", "--stage"]), index);
    assert_eq!(git(root, &["diff", "--name-only"]), "tracked.txt\n");
    manager.disable("git").unwrap();
    assert_eq!(manager.resource_count(), 0);
}

/// A sequenced pick has a different recovery marker from rebase. After reopening, Skip must
/// discard only its conflicted first step and apply the already reviewed second commit once.
#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE; sequenced cherry-pick recovery"]
fn git_package_reopened_cherry_pick_skips_only_its_conflicted_first_commit() {
    // Keep factory and Manager/native Git children in one private configuration process.
    if !run_isolated_test(
        "workbench::completions::git_package_reopened_cherry_pick_skips_only_its_conflicted_first_commit",
    ) {
        return;
    }
    let repository = isolated_repository();
    let root = repository.path();
    commit_file(root, "merge.txt", "base\n", "base");
    git(root, &["switch", "-c", "source"]);
    let first = commit_file(root, "merge.txt", "incoming first\n", "conflicted first");
    let second = commit_file(
        root,
        "tail.txt",
        "reviewed second 中😀\n",
        "reviewed second",
    );
    git(root, &["switch", "main"]);
    let original = commit_file(root, "merge.txt", "retained local\n", "local target");
    let original_index = git(root, &["ls-files", "--stage"]);
    let (profile, mut manager) = activate(&candidate(), root);
    branch_menu::context_action(
        &mut manager,
        "cherry-pick",
        Some(&format!("{first} {second}")),
    );
    assert!(
        manager.live["git"].views["history"]
            .active_node("confirm-command")
            .is_some()
    );
    assert_eq!(
        oid(root, "HEAD"),
        original,
        "the batch preview must not pick"
    );
    assert_eq!(git(root, &["ls-files", "--stage"]), original_index);
    click(&mut manager, "history", "confirm-command");
    assert_eq!(oid(root, "CHERRY_PICK_HEAD"), first);
    assert!(root.join(".git/sequencer").is_dir());
    assert!(!git(root, &["ls-files", "--unmerged", "--", "merge.txt"]).is_empty());
    assert!(!root.join("tail.txt").exists());

    manager = reopen(manager, profile.path(), root);
    assert_eq!(oid(root, "HEAD"), original);
    assert_eq!(oid(root, "CHERRY_PICK_HEAD"), first);
    let conflicted_index = git(root, &["ls-files", "--stage"]);
    click(&mut manager, "conflicts", "skip");
    assert!(
        manager.live["git"].views["conflicts"]
            .active_node("confirm-command")
            .is_some()
    );
    assert_eq!(
        oid(root, "HEAD"),
        original,
        "Skip preview cannot advance the batch"
    );
    assert_eq!(oid(root, "CHERRY_PICK_HEAD"), first);
    assert_eq!(git(root, &["ls-files", "--stage"]), conflicted_index);
    click(&mut manager, "conflicts", "confirm-command");

    assert_ne!(oid(root, "HEAD"), original);
    assert_eq!(oid(root, "HEAD^"), original);
    assert_eq!(
        git(
            root,
            &[
                "log",
                "--reverse",
                "--format=%s",
                &format!("{original}..HEAD")
            ]
        ),
        "reviewed second\n"
    );
    assert_eq!(git(root, &["show", "HEAD:merge.txt"]), "retained local\n");
    assert_eq!(
        fs::read(root.join("merge.txt")).unwrap(),
        b"retained local\n"
    );
    assert_eq!(
        git(root, &["show", "HEAD:tail.txt"]),
        "reviewed second 中😀\n"
    );
    assert_eq!(
        fs::read(root.join("tail.txt")).unwrap(),
        "reviewed second 中😀\n".as_bytes()
    );
    assert_eq!(oid(root, "source"), second);
    assert_eq!(git(root, &["symbolic-ref", "HEAD"]), "refs/heads/main\n");
    assert!(!root.join(".git/CHERRY_PICK_HEAD").exists());
    assert!(!root.join(".git/sequencer").exists());
    assert!(git(root, &["ls-files", "--unmerged"]).is_empty());
    assert!(git(root, &["status", "--porcelain"]).is_empty());
    manager.disable("git").unwrap();
    assert_eq!(manager.resource_count(), 0);
}

/// Creating a ref is distinct from checking it out. Switching and discarding require their own
/// explicit confirmations; a selected file's discard cannot consume another file's index/draft.
#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE; branch and discard completion"]
fn git_package_creates_a_branch_switches_explicitly_and_confirms_only_the_selected_discard() {
    // Keep factory and Manager/native Git children in one private configuration process.
    if !run_isolated_test(
        "workbench::completions::git_package_creates_a_branch_switches_explicitly_and_confirms_only_the_selected_discard",
    ) {
        return;
    }
    let repository = isolated_repository();
    let root = repository.path();
    commit_file(root, "tracked.txt", "reviewed base\n", "base");
    let original = commit_file(root, "other.txt", "other base\n", "other baseline");
    let original_index = git(root, &["ls-files", "--stage"]);
    let (_profile, mut manager) = activate(&candidate(), root);
    let branch = "topic/from-plugin";
    let reference = format!("refs/heads/{branch}");
    branch_menu::context_action(&mut manager, "branch", None);
    send(
        &mut manager,
        "history",
        "branch-name",
        ui::Action::Change(branch.into()),
    );
    assert!(
        git(
            root,
            &["for-each-ref", "--format=%(objectname)", &reference]
        )
        .is_empty()
    );
    assert_eq!(
        oid(root, "HEAD"),
        original,
        "entering a name cannot create or switch refs"
    );
    click(&mut manager, "history", "branch-create");
    assert_eq!(oid(root, &reference), original);
    assert_eq!(oid(root, "main"), original);
    assert_eq!(git(root, &["symbolic-ref", "HEAD"]), "refs/heads/main\n");
    assert_eq!(git(root, &["ls-files", "--stage"]), original_index);
    assert_eq!(
        fs::read(root.join("tracked.txt")).unwrap(),
        b"reviewed base\n"
    );
    branch_menu::context_action(&mut manager, "switch", Some(branch));
    assert!(
        manager.live["git"].views["history"]
            .active_node("confirm-command")
            .is_some()
    );
    assert_eq!(git(root, &["symbolic-ref", "HEAD"]), "refs/heads/main\n");
    assert_eq!(oid(root, "HEAD"), original);
    assert_eq!(git(root, &["ls-files", "--stage"]), original_index);
    assert_eq!(
        fs::read(root.join("tracked.txt")).unwrap(),
        b"reviewed base\n"
    );
    click(&mut manager, "history", "confirm-command");
    assert_eq!(
        git(root, &["symbolic-ref", "HEAD"]),
        format!("{reference}\n")
    );
    assert_eq!(oid(root, "HEAD"), original);
    assert!(git(root, &["status", "--porcelain"]).is_empty());

    fs::write(root.join("tracked.txt"), "discard only these bytes\n").unwrap();
    fs::write(root.join("other.txt"), "keep this staged\n").unwrap();
    git(root, &["add", "--", "other.txt"]);
    fs::write(root.join("other.txt"), "keep this unstaged 中😀\n").unwrap();
    let staged_index = git(root, &["ls-files", "--stage"]);
    manager.invoke_command("git", "refresh", json!({})).unwrap();
    idle(&mut manager);
    // The declared command selects the file and samples its patch; no text editor is substituted.
    manager
        .invoke_command("git", "refresh-hunks", json!({"path":"tracked.txt"}))
        .unwrap();
    idle(&mut manager);
    click(&mut manager, "staging", "discard");
    assert!(
        manager.live["git"].views["staging"]
            .active_node("confirm-command")
            .is_some()
    );
    assert_eq!(
        fs::read(root.join("tracked.txt")).unwrap(),
        b"discard only these bytes\n"
    );
    assert_eq!(git(root, &["ls-files", "--stage"]), staged_index);
    click(&mut manager, "staging", "cancel-command");
    assert_eq!(
        fs::read(root.join("tracked.txt")).unwrap(),
        b"discard only these bytes\n"
    );
    click(&mut manager, "staging", "discard");
    assert_eq!(
        fs::read(root.join("tracked.txt")).unwrap(),
        b"discard only these bytes\n"
    );
    click(&mut manager, "staging", "confirm-command");

    assert_eq!(
        fs::read(root.join("tracked.txt")).unwrap(),
        b"reviewed base\n"
    );
    assert_eq!(
        fs::read(root.join("other.txt")).unwrap(),
        "keep this unstaged 中😀\n".as_bytes()
    );
    assert_eq!(git(root, &["show", ":other.txt"]), "keep this staged\n");
    assert_eq!(git(root, &["ls-files", "--stage"]), staged_index);
    assert_eq!(git(root, &["diff", "--name-only"]), "other.txt\n");
    assert_eq!(
        git(root, &["diff", "--cached", "--name-only"]),
        "other.txt\n"
    );
    assert_eq!(oid(root, "HEAD"), original);
    assert_eq!(oid(root, "main"), original);
    assert_eq!(oid(root, &reference), original);
    assert_eq!(
        git(root, &["symbolic-ref", "HEAD"]),
        format!("{reference}\n")
    );
    manager.disable("git").unwrap();
    assert_eq!(manager.resource_count(), 0);
}
