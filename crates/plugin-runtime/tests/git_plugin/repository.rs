//! The independent package owns stash policy; public manager/UI events exercise actual Git effects.
use super::*;

fn candidate() -> Package {
    Package::read(Path::new(
        &std::env::var_os("NANOBUG_GIT_CANDIDATE").expect("candidate ZIP required"),
    ))
    .unwrap()
}

/// Every test confines identity, line endings and Git objects to its own temporary working tree.
fn fixture() -> (tempfile::TempDir, Package) {
    let package = candidate();
    let repository = tempfile::tempdir().unwrap();
    let root = repository.path();
    git(root, &["init", "-b", "main"]);
    git(root, &["config", "core.autocrlf", "false"]);
    git(root, &["config", "user.name", "Repository fixture"]);
    git(
        root,
        &["config", "user.email", "repository@example.invalid"],
    );
    std::fs::write(root.join("tracked.txt"), "base\n").unwrap();
    std::fs::write(root.join(".gitignore"), "ignored.tmp\n").unwrap();
    git(root, &["add", "--", "."]);
    git(root, &["commit", "-m", "base"]);
    (repository, package)
}

/// A named upstream different from origin catches assumptions hidden by a default clone setup.
fn tracking_fixture() -> (
    tempfile::TempDir,
    std::path::PathBuf,
    std::path::PathBuf,
    Package,
) {
    let repository = tempfile::tempdir().unwrap();
    let root = repository.path();
    let remote = root.join("remote.git");
    let local = root.join("local");
    let writer = root.join("writer");
    git(
        root,
        &["init", "--bare", "-b", "main", remote.to_str().unwrap()],
    );
    git(
        root,
        &[
            "clone",
            "-o",
            "upstream-name",
            remote.to_str().unwrap(),
            local.to_str().unwrap(),
        ],
    );
    configure(&local);
    write_commit(&local, "base.txt", "base\n", "base");
    git(&local, &["push", "-u", "upstream-name", "main"]);
    git(
        root,
        &["clone", remote.to_str().unwrap(), writer.to_str().unwrap()],
    );
    configure(&writer);
    (repository, local, writer, candidate())
}

fn configure(root: &Path) {
    git(root, &["config", "core.autocrlf", "false"]);
    git(root, &["config", "user.name", "Repository fixture"]);
    git(
        root,
        &["config", "user.email", "repository@example.invalid"],
    );
}

fn write_commit(root: &Path, name: &str, text: &str, title: &str) {
    std::fs::write(root.join(name), text).unwrap();
    git(root, &["add", "--", name]);
    git(root, &["commit", "-m", title]);
}

fn text(manager: &Manager, node: &str) -> String {
    let scene = &manager.live["git"].views["history"];
    let ui::Kind::Text { text } = &scene.active_node(node).unwrap().kind else {
        panic!("expected native text {node}");
    };
    text.clone()
}

/// Reactivation uses the same installed package/private profile; it is not a second clean installation.
fn reactivate(manager: &mut Manager) {
    manager.disable("git").unwrap();
    manager.enable("git").unwrap();
    let available = manager.availability("git").unwrap();
    let demand = manager
        .begin_demand(
            &available,
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
    idle(manager);
}

/// Advanced actions enter through the same declared native menu as an interactive user.
fn advanced(manager: &mut Manager, action: &str) {
    click(manager, "history", "advanced-open");
    ui_action(manager, "advanced-menu", ui::Action::Select(action.into()));
}

fn stash_oids(root: &Path) -> Vec<String> {
    git(root, &["stash", "list", "--format=%H"])
        .lines()
        .map(str::to_owned)
        .collect()
}

/// A preview is literal native code, never an instruction and never the bytes used for applying it.
fn preview(manager: &Manager) -> String {
    let scene = &manager.live["git"].views["history"];
    let dialog = scene.dialog.as_ref().expect("stash dialog");
    let mut result = String::new();
    dialog.content.visit(&mut |node| {
        if let ui::Kind::CodeBlock { text, .. } = &node.kind {
            result.push_str(text);
        }
    });
    result
}

/// `--literal-pathspecs` must not leak into stash's internal clean pathspecs: untracked files really
/// leave the worktree after creation, while ignored files and the immutable saved bytes survive.
#[test]
#[ignore = "build independent Git-for-Nanobug candidate; set NANOBUG_GIT_CANDIDATE"]
fn git_package_stash_create_previews_and_moves_untracked_files_then_apply_keeps_the_entry() {
    let (repository, package) = fixture();
    let root = repository.path();
    std::fs::write(root.join("tracked.txt"), "tracked 中😀\n").unwrap();
    std::fs::write(root.join("untracked space.txt"), "new 中😀\n").unwrap();
    std::fs::write(root.join("ignored.tmp"), "keep ignored\n").unwrap();
    let (_profile, mut manager) = activate(&package, root);
    advanced(&mut manager, "stash");
    assert!(
        manager.live["git"].views["history"]
            .active_node("repo-stash-create-confirm")
            .is_some()
    );
    assert!(stash_oids(root).is_empty(), "preview cannot create a stash");
    assert_eq!(
        std::fs::read_to_string(root.join("tracked.txt")).unwrap(),
        "tracked 中😀\n"
    );
    click(&mut manager, "history", "repo-stash-create-confirm");
    assert_eq!(git(root, &["status", "--porcelain"]), "");
    assert_eq!(
        std::fs::read_to_string(root.join("tracked.txt")).unwrap(),
        "base\n"
    );
    assert!(
        !root.join("untracked space.txt").exists(),
        "including an untracked file also removes it from the clean worktree"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("ignored.tmp")).unwrap(),
        "keep ignored\n"
    );
    let saved = stash_oids(root);
    assert_eq!(saved.len(), 1);
    let changes = preview(&manager);
    assert!(changes.contains("tracked.txt"));
    assert!(changes.contains("untracked space.txt"));
    assert!(changes.contains("new 中😀"));
    assert!(!changes.contains("keep ignored"));
    click(&mut manager, "history", "repo-stash-apply");
    assert_eq!(stash_oids(root), saved, "primary apply keeps the stash");
    assert_eq!(
        std::fs::read_to_string(root.join("tracked.txt")).unwrap(),
        "tracked 中😀\n"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("untracked space.txt")).unwrap(),
        "new 中😀\n"
    );
    manager.disable("git").unwrap();
}

/// A failed pop may already have written conflict markers/index stages. Its stash remains recoverable.
#[test]
#[ignore = "build independent Git-for-Nanobug candidate; set NANOBUG_GIT_CANDIDATE"]
fn git_package_stash_pop_conflict_retains_the_original_stash_and_real_unmerged_state() {
    let (repository, package) = fixture();
    let root = repository.path();
    std::fs::write(root.join("tracked.txt"), "stashed side\n").unwrap();
    git(root, &["stash", "push", "-m", "saved side"]);
    let saved = stash_oids(root);
    std::fs::write(root.join("tracked.txt"), "current side\n").unwrap();
    git(root, &["add", "--", "tracked.txt"]);
    git(root, &["commit", "-m", "current side"]);
    let (_profile, mut manager) = activate(&package, root);
    click(&mut manager, "history", "repo-stashes");
    assert!(preview(&manager).contains("stashed side"));
    click(&mut manager, "history", "repo-stash-pop");
    assert!(git(root, &["ls-files", "--unmerged"]).is_empty());
    assert_eq!(stash_oids(root), saved, "confirmation is still read-only");
    click(&mut manager, "history", "repo-stash-confirm");
    assert_eq!(stash_oids(root), saved);
    assert!(!git(root, &["ls-files", "--unmerged"]).is_empty());
    let text = std::fs::read_to_string(root.join("tracked.txt")).unwrap();
    assert!(text.contains("current side") && text.contains("stashed side"));
    assert!(text.contains("<<<<<<<"));
    manager.disable("git").unwrap();
}

/// An external push shifts stash@{0}; pop still applies the previewed OID and refuses to delete either
/// entry after its selector identity changed, rather than applying/deleting the newly inserted stash.
#[test]
#[ignore = "build independent Git-for-Nanobug candidate; set NANOBUG_GIT_CANDIDATE"]
fn git_package_stash_pop_uses_the_previewed_oid_and_keeps_entries_after_a_selector_shift() {
    let (repository, package) = fixture();
    let root = repository.path();
    std::fs::write(root.join("tracked.txt"), "approved original\n").unwrap();
    git(root, &["stash", "push", "-m", "original"]);
    let original = stash_oids(root)[0].clone();
    let (_profile, mut manager) = activate(&package, root);
    click(&mut manager, "history", "repo-stashes");
    assert!(preview(&manager).contains("approved original"));
    click(&mut manager, "history", "repo-stash-pop");
    std::fs::write(root.join("tracked.txt"), "external newer stash\n").unwrap();
    git(root, &["stash", "push", "-m", "external"]);
    let before = stash_oids(root);
    assert_eq!(before.len(), 2);
    assert_eq!(before[1], original);
    click(&mut manager, "history", "repo-stash-confirm");
    assert_eq!(
        stash_oids(root),
        before,
        "neither shifted entry may be deleted"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("tracked.txt")).unwrap(),
        "approved original\n"
    );
    assert!(git(root, &["ls-files", "--unmerged"]).is_empty());
    manager.disable("git").unwrap();
}

/// Standalone deletion has the same selector guard and cannot remove a newly inserted stash.
#[test]
#[ignore = "build independent Git-for-Nanobug candidate; set NANOBUG_GIT_CANDIDATE"]
fn git_package_stash_drop_refuses_a_selector_that_changed_after_confirmation_preview() {
    let (repository, package) = fixture();
    let root = repository.path();
    std::fs::write(root.join("tracked.txt"), "original stash\n").unwrap();
    git(root, &["stash", "push", "-m", "original"]);
    let (_profile, mut manager) = activate(&package, root);
    click(&mut manager, "history", "repo-stashes");
    click(&mut manager, "history", "repo-stash-drop");
    std::fs::write(root.join("tracked.txt"), "newer stash\n").unwrap();
    git(root, &["stash", "push", "-m", "newer"]);
    let before = stash_oids(root);
    click(&mut manager, "history", "repo-stash-confirm");
    assert_eq!(stash_oids(root), before);
    assert_eq!(git(root, &["status", "--porcelain"]), "");
    assert_eq!(
        std::fs::read_to_string(root.join("tracked.txt")).unwrap(),
        "base\n"
    );
    manager.disable("git").unwrap();
}

/// A local refresh never performs the network part of pull; manual Pull uses the configured remote.
#[test]
#[ignore = "build independent Git-for-Nanobug candidate; set NANOBUG_GIT_CANDIDATE"]
fn git_package_pull_fast_forwards_a_non_origin_upstream_and_local_refresh_does_not_fetch() {
    let (_repository, local, writer, package) = tracking_fixture();
    write_commit(&writer, "remote.txt", "remote update\n", "remote update");
    git(&writer, &["push"]);
    let remote_head = git(&writer, &["rev-parse", "HEAD"]);
    let (_profile, mut manager) = activate(&package, &local);
    assert!(text(&manager, "repo-fetch-time").contains("Not fetched yet"));
    let before = git(&local, &["rev-parse", "HEAD"]);
    manager.invoke_command("git", "refresh", json!({})).unwrap();
    idle(&mut manager);
    assert_eq!(git(&local, &["rev-parse", "HEAD"]), before);
    assert_eq!(git(&local, &["rev-parse", "@{upstream}"]), before);
    click(&mut manager, "history", "pull");
    assert_eq!(git(&local, &["rev-parse", "HEAD"]), remote_head);
    assert_eq!(
        std::fs::read_to_string(local.join("remote.txt")).unwrap(),
        "remote update\n"
    );
    assert!(manager.live["git"].views["history"].dialog.is_none());
    let fetched = text(&manager, "repo-fetch-time");
    assert!(fetched.contains("UTC") && !fetched.contains("Not fetched yet"));
    write_commit(
        &writer,
        "remote2.txt",
        "later remote update\n",
        "later remote update",
    );
    git(&writer, &["push"]);
    manager.invoke_command("git", "refresh", json!({})).unwrap();
    idle(&mut manager);
    assert_eq!(git(&local, &["rev-parse", "@{upstream}"]), remote_head);
    assert_eq!(text(&manager, "repo-fetch-time"), fetched);
    manager.disable("git").unwrap();
}

/// The package freezes both the proposed source and exact remote OID through public UI events.
/// A second writer invalidates that lease after review; confirmation must retain its remote commit.
#[test]
#[ignore = "build independent Git-for-Nanobug candidate; set NANOBUG_GIT_CANDIDATE"]
fn git_package_force_lease_refuses_remote_advance_after_preview() {
    let (repository, local, writer, package) = tracking_fixture();
    let remote = repository.path().join("remote.git");
    let expected_remote = git(&remote, &["rev-parse", "refs/heads/main"])
        .trim()
        .to_owned();
    write_commit(
        &local,
        "local.txt",
        "approved local source\n",
        "local proposal",
    );
    let local_head = git(&local, &["rev-parse", "HEAD"]).trim().to_owned();
    let local_index = git(&local, &["ls-files", "--stage"]);
    let (_profile, mut manager) = activate(&package, &local);
    ui_action(
        &mut manager,
        "argument",
        ui::Action::Change("upstream-name refs/heads/main".into()),
    );
    advanced(&mut manager, "push-lease");
    let scene = &manager.live["git"].views["history"];
    assert!(scene.active_node("confirm-command").is_some());
    let mut review = String::new();
    scene
        .dialog
        .as_ref()
        .expect("lease confirmation")
        .content
        .visit(&mut |node| {
            if let ui::Kind::Text { text } = &node.kind {
                review.push_str(text);
                review.push('\n');
            }
        });
    assert!(review.contains(&format!(
        "--force-with-lease=refs/heads/main:{expected_remote}"
    )));
    assert!(review.contains(&format!("{local_head}:refs/heads/main")));
    assert!(review.contains("upstream-name"));
    assert_eq!(
        git(&remote, &["rev-parse", "refs/heads/main"]).trim(),
        expected_remote,
        "preparing confirmation cannot push"
    );

    write_commit(
        &writer,
        "external.txt",
        "external writer owns this\n",
        "external advance",
    );
    git(&writer, &["push"]);
    let external_head = git(&writer, &["rev-parse", "HEAD"]).trim().to_owned();
    assert_ne!(external_head, expected_remote);
    assert_ne!(external_head, local_head);
    click(&mut manager, "history", "confirm-command");

    assert_eq!(
        git(&remote, &["rev-parse", "refs/heads/main"]).trim(),
        external_head,
        "a stale lease must never overwrite the second writer's commit"
    );
    assert_eq!(git(&local, &["rev-parse", "HEAD"]).trim(), local_head);
    assert_eq!(git(&local, &["ls-files", "--stage"]), local_index);
    assert!(git(&local, &["status", "--porcelain"]).is_empty());
    assert_eq!(
        std::fs::read_to_string(local.join("local.txt")).unwrap(),
        "approved local source\n"
    );
    assert_eq!(
        git(&remote, &["show", "refs/heads/main:external.txt"]),
        "external writer owns this\n"
    );
    let diagnostic = text(&manager, "status").to_lowercase();
    assert!(
        diagnostic.contains("stale info") || diagnostic.contains("rejected"),
        "real Git rejection must remain visible: {diagnostic}"
    );
    manager.disable("git").unwrap();
}

/// Divergence needs an explicit decision until a workspace/repository preference has been approved.
#[test]
#[ignore = "build independent Git-for-Nanobug candidate; set NANOBUG_GIT_CANDIDATE"]
fn git_package_pull_asks_on_divergence_and_recovers_the_remembered_choice_on_reactivation() {
    let (_repository, local, writer, package) = tracking_fixture();
    write_commit(&local, "local.txt", "local update\n", "local update");
    write_commit(&writer, "remote.txt", "remote update\n", "remote update");
    git(&writer, &["push"]);
    let before = git(&local, &["rev-parse", "HEAD"]);
    let (_profile, mut manager) = activate(&package, &local);
    click(&mut manager, "history", "pull");
    assert_eq!(
        git(&local, &["rev-parse", "HEAD"]),
        before,
        "ff-only policy cannot create a merge silently"
    );
    assert!(
        manager.live["git"].views["history"]
            .active_node("repo-pull-merge")
            .is_some()
    );
    ui_action(&mut manager, "repo-pull-remember", ui::Action::Toggle(true));
    click(&mut manager, "history", "repo-pull-merge");
    assert_eq!(
        git(&local, &["rev-list", "--parents", "-1", "HEAD"])
            .split_whitespace()
            .count(),
        3
    );
    assert!(git(&local, &["status", "--porcelain"]).is_empty());
    reactivate(&mut manager);
    assert!(text(&manager, "repo-policy").contains("Remembered: merge"));
    assert!(text(&manager, "repo-fetch-time").contains("UTC"));
    write_commit(
        &writer,
        "remote2.txt",
        "second remote update\n",
        "second remote update",
    );
    git(&writer, &["push"]);
    let next_remote = git(&writer, &["rev-parse", "HEAD"]).trim().to_owned();
    click(&mut manager, "history", "pull");
    assert!(manager.live["git"].views["history"].dialog.is_none());
    assert_eq!(
        git(
            &local,
            &["merge-base", "--is-ancestor", &next_remote, "HEAD"]
        ),
        ""
    );
    assert_eq!(
        git(&local, &["rev-list", "--parents", "-1", "HEAD"])
            .split_whitespace()
            .count(),
        3
    );
    manager.disable("git").unwrap();
}

/// Linked metadata may be outside the workspace; a project below the Git root cannot reinterpret paths.
#[test]
#[ignore = "build independent Git-for-Nanobug candidate; set NANOBUG_GIT_CANDIDATE"]
fn git_package_discovers_linked_worktrees_and_rejects_projects_below_the_repository_root() {
    let (repository, package) = fixture();
    let root = repository.path();
    let linked_parent = tempfile::tempdir().unwrap();
    let linked = linked_parent.path().join("linked");
    git(
        root,
        &[
            "worktree",
            "add",
            "-b",
            "linked-fixture",
            linked.to_str().unwrap(),
        ],
    );
    let (_profile, mut manager) = activate(&package, &linked);
    assert!(text(&manager, "repo-identity").contains("linked worktree"));
    manager.disable("git").unwrap();
    let nested = root.join("nested");
    std::fs::create_dir(&nested).unwrap();
    std::fs::write(nested.join("new.txt"), "must stay untracked\n").unwrap();
    let (_profile, mut manager) = activate(&package, &nested);
    assert!(text(&manager, "status").contains("repository root"));
    manager
        .invoke_command("git", "stage", json!({"path":"nested/new.txt"}))
        .unwrap();
    idle(&mut manager);
    assert_eq!(git(root, &["ls-files", "--", "nested/new.txt"]), "");
    assert_eq!(
        std::fs::read_to_string(nested.join("new.txt")).unwrap(),
        "must stay untracked\n"
    );
    manager.disable("git").unwrap();
}
