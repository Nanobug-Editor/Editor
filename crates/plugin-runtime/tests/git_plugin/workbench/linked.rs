//! Linked worktrees share objects while staging and detached commits remain local to their own index.
use super::*;

#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE; real linked detached worktree"]
fn git_package_stages_and_commits_a_linked_detached_worktree_without_moving_the_primary_branch() {
    // Keep factory and Manager/native Git children in one private configuration process.
    if !run_isolated_test(
        "workbench::linked::git_package_stages_and_commits_a_linked_detached_worktree_without_moving_the_primary_branch",
    ) {
        return;
    }
    let primary = repository();
    let root = primary.path();
    std::fs::write(root.join("tracked.txt"), "primary base\n").unwrap();
    git(root, &["add", "--", "tracked.txt"]);
    git(root, &["commit", "-m", "base"]);
    let original = git(root, &["rev-parse", "HEAD"]);
    let linked = tempfile::tempdir().unwrap();
    let path = linked.path();
    git(
        root,
        &[
            "worktree",
            "add",
            "--detach",
            path.to_str().unwrap(),
            "HEAD",
        ],
    );
    assert!(path.join(".git").is_file());
    std::fs::write(path.join("tracked.txt"), "linked staged 中😀\n").unwrap();
    let (_profile, mut manager) = activate(&candidate(), path);
    manager
        .invoke_command("git", "stage", json!({"path":"tracked.txt"}))
        .unwrap();
    idle(&mut manager);
    assert_eq!(git(path, &["show", ":tracked.txt"]), "linked staged 中😀\n");
    assert_eq!(git(root, &["show", ":tracked.txt"]), "primary base\n");
    // A later worktree edit is deliberately absent from the index-only commit.
    std::fs::write(path.join("tracked.txt"), "linked unstaged draft\n").unwrap();
    manager
        .invoke_command(
            "git",
            "commit",
            json!({"message":"commit on detached linked worktree"}),
        )
        .unwrap();
    idle(&mut manager);
    assert_ne!(git(path, &["rev-parse", "HEAD"]), original);
    assert_eq!(git(root, &["rev-parse", "HEAD"]), original);
    assert_eq!(git(root, &["rev-parse", "main"]), original);
    assert_eq!(
        git(path, &["show", "HEAD:tracked.txt"]),
        "linked staged 中😀\n"
    );
    assert_eq!(
        std::fs::read_to_string(path.join("tracked.txt")).unwrap(),
        "linked unstaged draft\n"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("tracked.txt")).unwrap(),
        "primary base\n"
    );
    assert_eq!(git(path, &["branch", "--show-current"]), "");
    manager.disable("git").unwrap();
    assert_eq!(manager.resource_count(), 0);
}
