//! An independent Git package exercises the public manager, real pipes and repository effects.
use plugin_runtime::{
    Manager, Package,
    plugin_protocol::{Environment, api, ui},
};
use serde_json::json;
use std::{
    path::Path,
    process::Command,
    time::{Duration, Instant},
};
#[path = "git_plugin/environment.rs"]
mod environment;
#[path = "git_plugin/index_edges.rs"]
mod index_edges;
#[path = "git_plugin/rebase_resume.rs"]
mod rebase_resume;
#[path = "git_plugin/repository.rs"]
mod repository;
#[path = "git_plugin/workbench.rs"]
mod workbench;
use environment::{require_isolated_git_environment, run_isolated_test};

/// Git identity is configured only in this isolated temporary repository, never in the user's profile.
fn git(root: &Path, args: &[&str]) -> String {
    require_isolated_git_environment();
    let result = Command::new("git")
        .current_dir(root)
        // An agent's temporary index must never redirect a disposable repository's Git writes.
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .args(args)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap()
}

/// Poll the actual WASM guest until all sequential native commands have exited.
fn idle(manager: &mut Manager) {
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut settled = 0;
    loop {
        manager.poll();
        for (_, request) in manager.take_editor_requests() {
            match request.operation() {
                api::EditorOperation::ListDocuments => {
                    request.finish(Ok(api::EditorValue::Documents {
                        documents: vec![],
                        active: None,
                    }))
                }
                api::EditorOperation::SetPanelVisibility { panel, visible } => {
                    request.finish(Ok(api::EditorValue::PanelVisibility {
                        panel: panel.clone(),
                        visible: *visible,
                    }))
                }
                _ => request.finish(Err(api::Failure::new(
                    api::ErrorCode::UnsupportedOperation,
                    "No text editor mounted in this runtime fixture",
                ))),
            }
        }
        manager.poll();
        if let Some(instance) = manager.live.get("git") {
            settled = if instance.process_count() == 0 {
                settled + 1
            } else {
                0
            };
            if settled >= 8 {
                return;
            }
        }
        assert!(
            Instant::now() < deadline,
            "Git guest did not complete its native command"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Exercise an enabled native node with its currently approved scene revision.
fn click(manager: &mut Manager, panel: &str, node: &str) {
    if panel == "history"
        && matches!(
            node,
            "refresh"
                | "all"
                | "more"
                | "repo-info"
                | "stash-list"
                | "fetch"
                | "pull"
                | "push"
                | "cancel-process"
        )
    {
        workbench::branch_menu::context_action(manager, node, None);
        return;
    }
    let revision = manager.live["git"].views[panel].revision;
    manager
        .event(
            "git",
            Some(panel.into()),
            api::Notification::Ui(ui::UiEvent {
                revision,
                node: node.into(),
                action: ui::Action::Click,
            }),
        )
        .unwrap();
    idle(manager);
}

/// Typed native controls carry current scene revisions for editable rebase plans.
fn ui_action(manager: &mut Manager, node: &str, action: ui::Action) {
    let revision = manager.live["git"].views["history"].revision;
    manager
        .event(
            "git",
            Some("history".into()),
            api::Notification::Ui(ui::UiEvent {
                revision,
                node: node.into(),
                action,
            }),
        )
        .unwrap();
    idle(manager);
}

/// Reordering, rewording and fixup rewrite only the explicitly previewed local commit range.
#[test]
#[ignore = "build Git-for-Nanobug with its declared native rebase dependency; set NANOBUG_GIT_CANDIDATE"]
fn git_package_interactive_rebase_reorders_rewords_and_fixups_the_previewed_commits() {
    // Keep factory and Manager/native Git children in one private configuration process.
    if !run_isolated_test(
        "git_package_interactive_rebase_reorders_rewords_and_fixups_the_previewed_commits",
    ) {
        return;
    }
    let package = Package::read(Path::new(
        &std::env::var_os("NANOBUG_GIT_CANDIDATE").expect("candidate required"),
    ))
    .unwrap();
    let repository = tempfile::tempdir().unwrap();
    let root = repository.path();
    git(root, &["init", "-b", "main"]);
    git(root, &["config", "core.autocrlf", "false"]);
    git(root, &["config", "user.name", "fixture"]);
    git(root, &["config", "user.email", "fixture@example.invalid"]);
    git(root, &["commit", "--allow-empty", "-m", "base"]);
    let base = git(root, &["rev-parse", "HEAD"]).trim().to_owned();
    for name in ["A", "B", "C"] {
        std::fs::write(root.join(format!("{name}.txt")), format!("{name}\n")).unwrap();
        git(root, &["add", "--", &format!("{name}.txt")]);
        git(root, &["commit", "-m", name]);
    }
    let before = git(root, &["rev-parse", "HEAD"]);
    git(root, &["config", "rebase.updateRefs", "true"]);
    git(root, &["branch", "retained", "HEAD~2"]);
    let retained = git(root, &["rev-parse", "retained"]);
    let (_profile, mut manager) = activate(&package, root);
    manager
        .invoke_command("git", "rebase-plan", json!({"target":base}))
        .unwrap();
    idle(&mut manager);
    click(&mut manager, "history", "rebase-up-1");
    let b_message = format!("B rewritten\n\n{}\n", "b".repeat(40000));
    let a_message = format!("A rewritten\n\n{}😀\n", "文".repeat(14000));
    ui_action(
        &mut manager,
        "rebase-action-0",
        ui::Action::Select("reword".into()),
    );
    ui_action(
        &mut manager,
        "rebase-message-0",
        ui::Action::Change(b_message.clone()),
    );
    ui_action(
        &mut manager,
        "rebase-action-1",
        ui::Action::Select("reword".into()),
    );
    ui_action(
        &mut manager,
        "rebase-message-1",
        ui::Action::Change(a_message.clone()),
    );
    ui_action(
        &mut manager,
        "rebase-action-2",
        ui::Action::Select("fixup".into()),
    );
    click(&mut manager, "history", "rebase-run");
    // A visible idle-period refresh must not retire the approved but not-yet-started plan.
    manager.invoke_command("git", "refresh", json!({})).unwrap();
    idle(&mut manager);
    assert_eq!(
        git(root, &["rev-parse", "HEAD"]),
        before,
        "a plan preview cannot rewrite refs"
    );
    click(&mut manager, "history", "confirm-command");
    assert_eq!(
        git(
            root,
            &["log", "--reverse", "--format=%s", &format!("{base}..HEAD")]
        ),
        "B rewritten\nA rewritten\n"
    );
    assert_eq!(
        git(root, &["log", "-1", "--format=%B"]),
        format!("{a_message}\n")
    );
    assert_eq!(
        git(root, &["show", "--no-patch", "--format=%B", "HEAD~"]),
        format!("{b_message}\n")
    );
    assert_eq!(
        git(root, &["rev-parse", "retained"]),
        retained,
        "unapproved refs must not be rewritten by repository configuration"
    );
    for name in ["A", "B", "C"] {
        assert_eq!(
            std::fs::read_to_string(root.join(format!("{name}.txt"))).unwrap(),
            format!("{name}\n")
        );
    }
    assert!(git(root, &["status", "--porcelain"]).is_empty());
    // Identical HEAD OIDs do not make two branch identities interchangeable.
    let rewritten = git(root, &["rev-parse", "HEAD"]);
    manager
        .invoke_command("git", "rebase-plan", json!({"target":base}))
        .unwrap();
    idle(&mut manager);
    click(&mut manager, "history", "rebase-run");
    git(root, &["switch", "-c", "outside"]);
    click(&mut manager, "history", "confirm-command");
    assert_eq!(git(root, &["rev-parse", "HEAD"]), rewritten);
    assert_eq!(git(root, &["rev-parse", "main"]), rewritten);
    assert!(
        !root.join(".git/rebase-merge").exists(),
        "a stale branch target must be rejected before starting rebase"
    );
    manager.disable("git").unwrap();
}

/// An add/add binary conflict has no base stage and must preserve the selected side byte for byte.
#[test]
#[ignore = "build independent Git-for-Nanobug candidate; set NANOBUG_GIT_CANDIDATE to its ZIP"]
fn git_package_resolves_a_binary_add_add_conflict_with_an_explicit_complete_version() {
    // Keep factory and Manager/native Git children in one private configuration process.
    if !run_isolated_test(
        "git_package_resolves_a_binary_add_add_conflict_with_an_explicit_complete_version",
    ) {
        return;
    }
    let package = Package::read(Path::new(
        &std::env::var_os("NANOBUG_GIT_CANDIDATE").expect("candidate required"),
    ))
    .unwrap();
    let repository = tempfile::tempdir().unwrap();
    let root = repository.path();
    git(root, &["init", "-b", "main"]);
    git(root, &["config", "core.autocrlf", "false"]);
    git(root, &["config", "user.name", "fixture"]);
    git(root, &["config", "user.email", "fixture@example.invalid"]);
    git(root, &["commit", "--allow-empty", "-m", "base"]);
    git(root, &["switch", "-c", "feature"]);
    std::fs::write(root.join("binary.dat"), [0, 2, 0, 255]).unwrap();
    git(root, &["add", "--", "binary.dat"]);
    git(root, &["commit", "-m", "local"]);
    git(root, &["switch", "main"]);
    std::fs::write(root.join("binary.dat"), [0, 3, 0, 254]).unwrap();
    git(root, &["add", "--", "binary.dat"]);
    git(root, &["commit", "-m", "incoming"]);
    git(root, &["switch", "feature"]);
    assert!(
        !Command::new("git")
            .current_dir(root)
            .args(["merge", "main"])
            .output()
            .unwrap()
            .status
            .success()
    );
    let (_profile, mut manager) = activate(&package, root);
    manager
        .invoke_command("git", "resolve-file", json!({"path":"binary.dat"}))
        .unwrap();
    idle(&mut manager);
    click(&mut manager, "conflicts", "conflict-stage-3");
    assert!(
        !git(root, &["ls-files", "--unmerged"]).is_empty(),
        "preview alone does not resolve a conflict"
    );
    click(&mut manager, "conflicts", "confirm-command");
    assert!(git(root, &["ls-files", "--unmerged"]).is_empty());
    assert_eq!(
        std::fs::read(root.join("binary.dat")).unwrap(),
        [0, 3, 0, 254]
    );
    assert!(
        root.join(".git/MERGE_HEAD").exists(),
        "resolving one file never continues the whole merge"
    );
    manager.disable("git").unwrap();
}

/// Install and demand-activate the independently built package through the public manager.
fn activate(package: &Package, root: &Path) -> (tempfile::TempDir, Manager) {
    let profile = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(
        profile.path().join("plugins"),
        Environment {
            workspace: root.display().to_string(),
            os: "windows".into(),
            locale: "en".into(),
            ..Default::default()
        },
    )
    .unwrap();
    manager
        .install(package, package.manifest.permissions.clone())
        .unwrap();
    let availability = manager.availability("git").unwrap();
    let demand = manager
        .begin_demand(
            &availability,
            plugin_runtime::DemandReason::Command("refresh".into()),
            60_000,
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        manager.poll();
        if let api::RequestUpdate::Completed { result } = demand.status() {
            result.unwrap();
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    idle(&mut manager);
    (profile, manager)
}

/// Before the first commit an index entry can be removed without requiring a nonexistent HEAD.
#[test]
#[ignore = "build independent Git-for-Nanobug candidate; set NANOBUG_GIT_CANDIDATE to its ZIP"]
fn git_package_unstages_an_unborn_file_without_deleting_worktree_content() {
    // Keep factory and Manager/native Git children in one private configuration process.
    if !run_isolated_test("git_package_unstages_an_unborn_file_without_deleting_worktree_content") {
        return;
    }
    let package = Package::read(Path::new(
        &std::env::var_os("NANOBUG_GIT_CANDIDATE").expect("candidate required"),
    ))
    .unwrap();
    let repository = tempfile::tempdir().unwrap();
    let root = repository.path();
    git(root, &["init", "-b", "main"]);
    std::fs::write(root.join("new.txt"), "new 中😀\n").unwrap();
    let (_profile, mut manager) = activate(&package, root);
    manager
        .invoke_command("git", "stage", json!({"path":"new.txt"}))
        .unwrap();
    idle(&mut manager);
    assert_eq!(git(root, &["ls-files"]), "new.txt\n");
    std::fs::write(root.join("new.txt"), "later worktree edit\n").unwrap();
    manager
        .invoke_command("git", "unstage", json!({"path":"new.txt"}))
        .unwrap();
    idle(&mut manager);
    assert_eq!(git(root, &["ls-files"]), "");
    assert_eq!(
        std::fs::read_to_string(root.join("new.txt")).unwrap(),
        "later worktree edit\n"
    );
    manager.disable("git").unwrap();
}

/// stdin EOF must commit the exact multiline message; another unstaged edit stays out of the commit.
#[test]
#[ignore = "build independent Git-for-Nanobug through --plugin-package; set NANOBUG_GIT_CANDIDATE to its ZIP"]
fn git_package_stages_one_path_and_commits_only_the_index_with_multiline_stdin() {
    // Keep factory and Manager/native Git children in one private configuration process.
    if !run_isolated_test(
        "git_package_stages_one_path_and_commits_only_the_index_with_multiline_stdin",
    ) {
        return;
    }
    let candidate = std::env::var_os("NANOBUG_GIT_CANDIDATE").expect("candidate ZIP required");
    let package = Package::read(Path::new(&candidate)).unwrap();
    let repository = tempfile::tempdir().unwrap();
    let root = repository.path();
    git(root, &["init", "-b", "main"]);
    git(root, &["config", "core.autocrlf", "false"]);
    git(root, &["config", "user.name", "Git fixture"]);
    git(root, &["config", "user.email", "fixture@example.invalid"]);
    std::fs::write(root.join("selected space.txt"), "base\n").unwrap();
    std::fs::write(root.join("other.txt"), "base\n").unwrap();
    git(root, &["add", "--", "selected space.txt", "other.txt"]);
    git(root, &["commit", "-m", "initial"]);
    std::fs::write(root.join("selected space.txt"), "staged 中😀\n").unwrap();
    std::fs::write(root.join("other.txt"), "keep unstaged\n").unwrap();
    let profile = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(
        profile.path().join("plugins"),
        Environment {
            workspace: root.display().to_string(),
            os: "windows".into(),
            locale: "en".into(),
            ..Default::default()
        },
    )
    .unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    let availability = manager.availability("git").unwrap();
    let demand = manager
        .begin_demand(
            &availability,
            plugin_runtime::DemandReason::Command("refresh".into()),
            60_000,
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        manager.poll();
        if let api::RequestUpdate::Completed { result } = demand.status() {
            result.unwrap();
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    manager.invoke_command("git", "refresh", json!({})).unwrap();
    idle(&mut manager);
    manager
        .invoke_command("git", "stage", json!({"path":"selected space.txt"}))
        .unwrap();
    idle(&mut manager);
    assert_eq!(
        git(root, &["diff", "--cached", "--name-only"]),
        "selected space.txt\n"
    );
    // A later disk edit must not be included by the commit action itself.
    std::fs::write(root.join("selected space.txt"), "later disk edit\n").unwrap();
    manager
        .invoke_command("git", "commit", json!({"message":"subject\n\nbody 中😀\n"}))
        .unwrap();
    idle(&mut manager);
    assert_eq!(
        git(root, &["log", "-1", "--format=%B"]),
        "subject\n\nbody 中😀\n\n"
    );
    assert_eq!(
        git(root, &["show", "HEAD:selected space.txt"]),
        "staged 中😀\n"
    );
    assert_eq!(git(root, &["show", "HEAD:other.txt"]), "base\n");
    assert_eq!(
        std::fs::read_to_string(root.join("selected space.txt")).unwrap(),
        "later disk edit\n"
    );
    // Separate distant edits form independently applicable hunks. The real manager validates the
    // current UI publication before Git rechecks the patch and writes only the selected index hunk.
    let baseline = (0..40)
        .map(|index| format!("line {index}\n"))
        .collect::<String>();
    std::fs::write(root.join("selected space.txt"), &baseline).unwrap();
    git(root, &["add", "--", "selected space.txt"]);
    git(root, &["commit", "-m", "hunk baseline"]);
    let modified = baseline
        .replace("line 1\n", "first change\n")
        .replace("line 35\n", "last change\n");
    std::fs::write(root.join("selected space.txt"), &modified).unwrap();
    manager.invoke_command("git", "refresh", json!({})).unwrap();
    idle(&mut manager);
    manager
        .invoke_command("git", "refresh-hunks", json!({"path":"selected space.txt"}))
        .unwrap();
    idle(&mut manager);
    let scene = &manager.live["git"].views["staging"];
    assert!(
        scene.active_node("hunk-1").is_some(),
        "Git must produce two independent hunks"
    );
    let revision = scene.revision;
    manager
        .event(
            "git",
            Some("staging".into()),
            api::Notification::Ui(ui::UiEvent {
                revision,
                node: "hunk-0".into(),
                action: ui::Action::Click,
            }),
        )
        .unwrap();
    idle(&mut manager);
    assert_eq!(
        git(root, &["show", ":selected space.txt"]),
        baseline.replace("line 1\n", "first change\n")
    );
    assert_eq!(
        std::fs::read_to_string(root.join("selected space.txt")).unwrap(),
        modified
    );
    // Git's `--` stops options, but bracket names still need --literal-pathspecs.
    // Only the exact selected file may enter the index, even with matching sibling names.
    for name in ["[ab].txt", "a.txt", "b.txt"] {
        std::fs::write(root.join(name), name).unwrap();
    }
    manager.invoke_command("git", "refresh", json!({})).unwrap();
    idle(&mut manager);
    manager
        .invoke_command("git", "stage", json!({"path":"[ab].txt"}))
        .unwrap();
    idle(&mut manager);
    assert_eq!(git(root, &["show", ":[ab].txt"]), "[ab].txt");
    assert_eq!(git(root, &["ls-files", "--", "a.txt", "b.txt"]), "");
    // More than sixteen visible rows must still fit the native vector-paint quota.
    for index in 0..20 {
        git(
            root,
            &["commit", "--allow-empty", "-m", &format!("history {index}")],
        );
    }
    manager.invoke_command("git", "refresh", json!({})).unwrap();
    idle(&mut manager);
    let history = &manager.live["git"].views["history"];
    let ui::Kind::Canvas(canvas) = &history.active_node("log").unwrap().kind else {
        panic!("history canvas")
    };
    canvas.validate().unwrap();
    assert!(canvas.paint.iter().any(|paint|matches!(paint,plugin_runtime::plugin_protocol::Paint::Text{text,..} if text=="history 0")));
    // Cancelling a staged rename restores both index paths; the renamed worktree file is retained.
    std::fs::write(root.join("old.txt"), "rename bytes\n").unwrap();
    git(root, &["add", "--", "old.txt"]);
    git(root, &["commit", "-m", "rename baseline"]);
    git(root, &["mv", "--", "old.txt", "renamed.txt"]);
    manager.invoke_command("git", "refresh", json!({})).unwrap();
    idle(&mut manager);
    manager
        .invoke_command("git", "unstage", json!({"path":"renamed.txt"}))
        .unwrap();
    idle(&mut manager);
    assert_eq!(git(root, &["diff", "--cached", "--name-only"]), "");
    assert_eq!(
        std::fs::read_to_string(root.join("renamed.txt")).unwrap(),
        "rename bytes\n"
    );
    let staging = &manager.live["git"].views["staging"];
    assert!(matches!(
        staging.active_node("staging").unwrap().kind,
        ui::Kind::Tree { .. }
    ));
    assert!(manager.live["git"].views.contains_key("conflicts"));
    manager.disable("git").unwrap();
    assert!(!manager.live.contains_key("git"));
}
