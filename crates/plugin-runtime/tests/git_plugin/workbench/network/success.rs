//! Local bare remotes prove successful publication uses separate local-rewrite and lease approvals.
//! Every repository, identity and hook override belongs to the disposable fixture directory.
use super::*;

const REMOTE: &str = "upstream-name";
const BRANCH: &str = "refs/heads/main";

/// Even local Git peers must not run hooks, signing helpers or credential programs from a profile.
fn configure(root: &Path, hooks: &Path) {
    for (key, value) in [
        ("user.name", "Network success fixture"),
        ("user.email", "network-success@example.invalid"),
        ("core.autocrlf", "false"),
        ("commit.gpgsign", "false"),
        ("credential.helper", ""),
        ("credential.interactive", "false"),
        ("core.askPass", ""),
        ("protocol.file.allow", "always"),
    ] {
        git(root, &["config", "--local", key, value]);
    }
    git(
        root,
        &[
            "config",
            "--local",
            "core.hooksPath",
            hooks.to_str().unwrap(),
        ],
    );
}

fn write_commit(root: &Path, path: &str, text: &str, title: &str) {
    std::fs::write(root.join(path), text).unwrap();
    git(root, &["add", "--", path]);
    git(root, &["commit", "--no-gpg-sign", "-m", title]);
}

fn oid(root: &Path, reference: &str) -> String {
    git(root, &["rev-parse", "--verify", reference])
        .trim()
        .into()
}

/// The local clone has one non-origin remote, while a separate writer makes fetch observable.
fn fixture() -> (
    tempfile::TempDir,
    std::path::PathBuf,
    std::path::PathBuf,
    std::path::PathBuf,
) {
    require_isolated_git_environment();
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let hooks = root.join("empty-hooks");
    std::fs::create_dir(&hooks).unwrap();
    let remote = root.join("central.git");
    let local = root.join("local");
    let writer = root.join("writer");
    git(
        root,
        &["init", "--bare", "-b", "main", remote.to_str().unwrap()],
    );
    configure(&remote, &hooks);
    git(
        &remote,
        &["config", "--local", "receive.denyNonFastForwards", "false"],
    );
    git(
        root,
        &[
            "clone",
            "-o",
            REMOTE,
            remote.to_str().unwrap(),
            local.to_str().unwrap(),
        ],
    );
    configure(&local, &hooks);
    write_commit(&local, "tracked.txt", "base\n", "base");
    git(&local, &["push", "-u", REMOTE, "main"]);
    git(
        root,
        &["clone", remote.to_str().unwrap(), writer.to_str().unwrap()],
    );
    configure(&writer, &hooks);
    (directory, local, writer, remote)
}

/// Read the published native confirmation rather than reconstructing the guest's execution argv.
fn review(manager: &Manager) -> String {
    let dialog = manager.live["git"].views["history"]
        .dialog
        .as_ref()
        .expect("an explicit confirmation must be visible");
    assert_eq!(dialog.id, "confirm");
    let mut result = String::new();
    dialog.content.visit(&mut |node| match &node.kind {
        ui::Kind::Text { text } | ui::Kind::CodeBlock { text, .. } => {
            result.push_str(text);
            result.push('\n');
        }
        _ => {}
    });
    result
}

fn advanced(manager: &mut Manager, action: &str) {
    click(manager, "history", "advanced-open");
    send(
        manager,
        "history",
        "advanced-menu",
        ui::Action::Select(action.into()),
    );
}

/// Preparing a remote update is an independent manual action, with exact old/new OIDs visible.
/// The caller verifies the remote before and after the later confirmation, never inside this helper.
fn lease_review(manager: &mut Manager, local: &str, expected: &str) {
    send(
        manager,
        "history",
        "argument",
        ui::Action::Change(format!("{REMOTE} {BRANCH}")),
    );
    advanced(manager, "push-lease");
    let preview = review(manager);
    assert!(preview.contains(&format!("--force-with-lease={BRANCH}:{expected}")));
    assert!(preview.contains(&format!("{local}:{BRANCH}")));
    assert!(preview.contains(REMOTE));
}

#[test]
#[ignore = "build independent Git candidate with native rebase dependency; set NANOBUG_GIT_CANDIDATE; local bare remote"]
fn git_package_remote_success_and_published_rewrites_require_separate_exact_lease_confirmations() {
    // Keep factory and Manager/native Git children in one private configuration process.
    if !run_isolated_test(
        "workbench::network::success::git_package_remote_success_and_published_rewrites_require_separate_exact_lease_confirmations",
    ) {
        return;
    }
    let (_directory, local, writer, remote) = fixture();
    let base = oid(&local, "HEAD");
    let initial_index = git(&local, &["ls-files", "--stage"]);
    write_commit(&writer, "incoming.txt", "writer update\n", "writer update");
    git(&writer, &["push"]);
    let incoming = oid(&writer, "HEAD");
    assert_eq!(oid(&local, "@{upstream}"), base);
    assert_eq!(git(&local, &["remote"]), format!("{REMOTE}\n"));
    let (_profile, mut manager) = activate(&candidate(), &local);
    assert!(text(&manager, "repo-fetch-time").contains("Not fetched yet"));

    // Fetch observes the remote ref without merging, changing the local index or writing its files.
    click(&mut manager, "history", "fetch");
    assert_eq!(oid(&local, "HEAD"), base);
    assert_eq!(oid(&local, "@{upstream}"), incoming);
    assert_eq!(oid(&remote, BRANCH), incoming);
    assert_eq!(git(&local, &["ls-files", "--stage"]), initial_index);
    assert!(!local.join("incoming.txt").exists());
    assert!(text(&manager, "repo-fetch-time").contains("UTC"));

    // Pull integrates only after its own manual action, even though Fetch already saw the commit.
    click(&mut manager, "history", "pull");
    assert_eq!(oid(&local, "HEAD"), incoming);
    assert_eq!(oid(&remote, BRANCH), incoming);
    assert_eq!(
        std::fs::read_to_string(local.join("incoming.txt")).unwrap(),
        "writer update\n"
    );
    assert!(git(&local, &["status", "--porcelain"]).is_empty());
    let sampled = text(&manager, "repo-fetch-time");

    write_commit(
        &local,
        "local-only.txt",
        "published local bytes\n",
        "publish original",
    );
    let published = oid(&local, "HEAD");
    manager.invoke_command("git", "refresh", json!({})).unwrap();
    idle(&mut manager);
    assert_eq!(
        oid(&remote, BRANCH),
        incoming,
        "local work cannot publish implicitly"
    );
    click(&mut manager, "history", "push");
    assert!(review(&manager).contains("push"));
    assert_eq!(
        oid(&remote, BRANCH),
        incoming,
        "push preview cannot publish"
    );
    click(&mut manager, "history", "confirm-command");
    assert_eq!(oid(&remote, BRANCH), published);
    assert_eq!(oid(&local, "HEAD"), published);
    assert_eq!(
        git(&remote, &["show", "main:local-only.txt"]),
        "published local bytes\n"
    );
    assert_eq!(
        text(&manager, "repo-fetch-time"),
        sampled,
        "push is not a fetch sample"
    );

    // Reword one already-published commit. The local plan has its own review and does not grant
    // permission to update the remote; even successful local rewriting leaves the old remote OID.
    manager
        .invoke_command("git", "rebase-plan", json!({"target":incoming}))
        .unwrap();
    idle(&mut manager);
    ui_action(
        &mut manager,
        "rebase-action-0",
        ui::Action::Select("reword".into()),
    );
    ui_action(
        &mut manager,
        "rebase-message-0",
        ui::Action::Change("published message revised\n".into()),
    );
    click(&mut manager, "history", "rebase-run");
    let plan = review(&manager);
    assert!(plan.contains(&published));
    assert!(plan.contains("published message revised"));
    assert_eq!(
        oid(&local, "HEAD"),
        published,
        "rebase preview cannot rewrite locally"
    );
    assert_eq!(oid(&remote, BRANCH), published);
    click(&mut manager, "history", "confirm-command");
    let rewritten = oid(&local, "HEAD");
    assert_ne!(rewritten, published);
    assert_eq!(
        git(&local, &["log", "-1", "--format=%s"]),
        "published message revised\n"
    );
    assert_eq!(
        git(&local, &["rev-parse", "HEAD^{tree}"]),
        git(&remote, &["rev-parse", "main^{tree}"])
    );
    assert_eq!(
        oid(&remote, BRANCH),
        published,
        "local rebase approval cannot publish a rewrite"
    );
    assert!(git(&local, &["status", "--porcelain"]).is_empty());

    lease_review(&mut manager, &rewritten, &published);
    assert_eq!(
        oid(&remote, BRANCH),
        published,
        "sampling an expected OID cannot push"
    );
    click(&mut manager, "history", "confirm-command");
    assert_eq!(oid(&remote, BRANCH), rewritten);
    assert_eq!(oid(&local, "HEAD"), rewritten);
    assert_eq!(
        git(&remote, &["log", "-1", "--format=%s"]),
        "published message revised\n"
    );

    // A soft reset is a second published-history rewrite. Its local confirmation changes only
    // HEAD, preserving the current index/worktree; removing the remote commit needs another lease.
    let retained_index = git(&local, &["ls-files", "--stage"]);
    send(
        &mut manager,
        "history",
        "argument",
        ui::Action::Change(incoming.clone()),
    );
    advanced(&mut manager, "reset-soft");
    let reset = review(&manager);
    assert!(reset.contains(&rewritten));
    assert!(reset.contains("refs/remotes/upstream-name/main"));
    assert!(reset.contains("reset --soft"));
    assert_eq!(oid(&local, "HEAD"), rewritten);
    assert_eq!(oid(&remote, BRANCH), rewritten);
    click(&mut manager, "history", "confirm-command");
    assert_eq!(oid(&local, "HEAD"), incoming);
    assert_eq!(
        oid(&remote, BRANCH),
        rewritten,
        "local reset confirmation cannot change the remote"
    );
    assert_eq!(git(&local, &["ls-files", "--stage"]), retained_index);
    assert_eq!(
        std::fs::read_to_string(local.join("local-only.txt")).unwrap(),
        "published local bytes\n"
    );

    lease_review(&mut manager, &incoming, &rewritten);
    assert_eq!(oid(&remote, BRANCH), rewritten);
    click(&mut manager, "history", "confirm-command");
    assert_eq!(oid(&remote, BRANCH), incoming);
    assert_eq!(oid(&local, "HEAD"), incoming);
    assert_eq!(git(&local, &["ls-files", "--stage"]), retained_index);
    assert_eq!(
        git(&remote, &["ls-tree", "--name-only", "main"]),
        "incoming.txt\ntracked.txt\n"
    );
    assert_eq!(text(&manager, "repo-fetch-time"), sampled);
    assert_eq!(manager.live["git"].process_count(), 0);
    manager.disable("git").unwrap();
    assert!(!manager.live.contains_key("git"));
    assert_eq!(manager.resource_count(), 0);
}
