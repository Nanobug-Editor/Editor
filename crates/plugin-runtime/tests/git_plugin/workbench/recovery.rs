//! Opaque conflicts retain complete bytes and recover their authority from real Git operation state.
use super::*;

/// Git must actually stop at a conflict; a failed setup never counts as a successful recovery test.
fn merge_conflict(root: &Path) {
    require_isolated_git_environment();
    let output = Command::new("git")
        .current_dir(root)
        // Expected merge failure still writes an index and must stay in the private repository.
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .args(["merge", "--no-edit", "topic"])
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "the fixture must enter a real conflict"
    );
    assert!(root.join(".git/MERGE_HEAD").is_file());
    assert!(!git(root, &["ls-files", "--unmerged"]).is_empty());
}

/// No text decoding or truncated patch can participate in the whole-version confirmation.
fn choose_complete(manager: &mut Manager, path: &str) {
    manager
        .invoke_command("git", "resolve-file", json!({"path":path}))
        .unwrap();
    idle(manager);
    let scene = &manager.live["git"].views["conflicts"];
    assert!(scene.active_node("opaque-conflict").is_some());
    assert!(
        scene.source.is_none(),
        "opaque sources never open an editable truncated result"
    );
    click(manager, "conflicts", "conflict-stage-3");
    assert!(
        !git(
            Path::new(manager.workspace()),
            &["ls-files", "--unmerged", "--", path]
        )
        .is_empty()
    );
    click(manager, "conflicts", "confirm-command");
}

/// Disabled controls are intentionally absent from `active_node`; inspect the declared scene to
/// distinguish a painted unavailable action from a missing button, then leave admission to Manager.
fn disabled(manager: &Manager, id: &str) -> bool {
    let mut state = None;
    manager.live["git"].views["conflicts"]
        .root
        .visit(&mut |node| {
            if node.id == id {
                state = Some(node.disabled);
            }
        });
    state.expect("the conflict window declares its recovery action")
}

#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE; opaque multi-file conflicts and restart"]
fn git_package_opaque_conflicts_restart_abort_and_reappear_without_reusing_old_approval() {
    // Keep factory and Manager/native Git children in one private configuration process.
    if !run_isolated_test(
        "workbench::recovery::git_package_opaque_conflicts_restart_abort_and_reappear_without_reusing_old_approval",
    ) {
        return;
    }
    let repository = repository();
    let root = repository.path();
    git(root, &["commit", "--allow-empty", "-m", "base"]);
    let non_utf8 = vec![0xff, 0xfe, b'\n', b't', b'o', b'p', b'i', b'c'];
    let large = format!("topic\n{}", "complete large source\n".repeat(14_000)).into_bytes();
    assert!(large.len() > 256 * 1024);
    git(root, &["switch", "-c", "topic"]);
    std::fs::write(root.join("non-utf8.dat"), &non_utf8).unwrap();
    std::fs::write(root.join("large.txt"), &large).unwrap();
    git(root, &["add", "--", "."]);
    git(root, &["commit", "-m", "topic opaque sources"]);
    git(root, &["switch", "main"]);
    std::fs::write(root.join("non-utf8.dat"), [0xfd, 0xfc, b'\n', b'm']).unwrap();
    std::fs::write(
        root.join("large.txt"),
        format!("main\n{}", "complete large source\n".repeat(14_000)),
    )
    .unwrap();
    git(root, &["add", "--", "."]);
    git(root, &["commit", "-m", "main opaque sources"]);
    let head = git(root, &["rev-parse", "HEAD"]);
    merge_conflict(root);
    let (profile, mut manager) = activate(&candidate(), root);
    choose_complete(&mut manager, "non-utf8.dat");
    assert_eq!(std::fs::read(root.join("non-utf8.dat")).unwrap(), non_utf8);
    assert!(!git(root, &["ls-files", "--unmerged", "--", "large.txt"]).is_empty());
    assert!(disabled(&manager, "continue"));
    assert_eq!(git(root, &["rev-parse", "HEAD"]), head);

    manager = reopen(manager, profile.path(), root);
    manager
        .invoke_command("git", "resolve-file", json!({"path":"large.txt"}))
        .unwrap();
    idle(&mut manager);
    assert!(
        manager.live["git"].views["conflicts"]
            .active_node("opaque-conflict")
            .is_some()
    );
    let stale = ui::UiEvent {
        revision: manager.live["git"].views["conflicts"].revision,
        node: "conflict-stage-3".into(),
        action: ui::Action::Click,
    };
    git(root, &["merge", "--abort"]);
    click(&mut manager, "history", "refresh");
    assert!(disabled(&manager, "abort"));
    assert_eq!(git(root, &["rev-parse", "HEAD"]), head);
    merge_conflict(root);
    click(&mut manager, "history", "refresh");
    manager
        .invoke_command("git", "resolve-file", json!({"path":"large.txt"}))
        .unwrap();
    idle(&mut manager);
    assert!(
        manager
            .event(
                "git",
                Some("conflicts".into()),
                api::Notification::Ui(stale)
            )
            .is_err(),
        "a former operation's source action cannot approve a new conflict at the same path"
    );
    choose_complete(&mut manager, "large.txt");
    assert_eq!(std::fs::read(root.join("large.txt")).unwrap(), large);
    assert!(root.join(".git/MERGE_HEAD").exists());
    choose_complete(&mut manager, "non-utf8.dat");
    assert_eq!(std::fs::read(root.join("non-utf8.dat")).unwrap(), non_utf8);
    assert!(git(root, &["ls-files", "--unmerged"]).is_empty());
    assert!(root.join(".git/MERGE_HEAD").exists());
    click(&mut manager, "conflicts", "continue");
    assert!(!root.join(".git/MERGE_HEAD").exists());
    assert_eq!(
        git(root, &["show", "-s", "--format=%P", "HEAD"])
            .split_whitespace()
            .count(),
        2
    );
    assert!(git(root, &["status", "--porcelain"]).is_empty());
    manager.disable("git").unwrap();
    assert_eq!(manager.resource_count(), 0);
}
