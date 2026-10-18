//! Remaining workbench scenarios enter the published package, never guest implementation details.
use super::*;
use std::fs;

#[path = "workbench/completions.rs"]
mod completions;
#[path = "workbench/history.rs"]
mod history;
#[path = "workbench/linked.rs"]
mod linked;
#[path = "workbench/native_guards.rs"]
mod native_guards;
#[path = "workbench/network.rs"]
mod network;
#[path = "workbench/operations.rs"]
mod operations;
#[path = "workbench/package_lifecycle.rs"]
mod package_lifecycle;
#[path = "workbench/recovery.rs"]
mod recovery;
#[path = "workbench/scene_activity.rs"]
mod scene_activity;
#[path = "workbench/scene_receipts.rs"]
mod scene_receipts;
#[path = "workbench/tree_collisions.rs"]
mod tree_collisions;
#[path = "workbench/tree_pages.rs"]
mod tree_pages;

/// One exact environment-selected ZIP is shared with the native window and final delivery gates.
fn candidate() -> Package {
    Package::read(Path::new(
        &std::env::var_os("NANOBUG_GIT_CANDIDATE").expect("candidate ZIP required"),
    ))
    .unwrap()
}

/// Keep every object, branch and Git setting in a disposable worktree, with no user configuration.
fn repository() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    git(root, &["init", "-b", "main"]);
    git(root, &["config", "core.autocrlf", "false"]);
    git(root, &["config", "user.name", "Workbench fixture"]);
    git(root, &["config", "user.email", "workbench@example.invalid"]);
    git(root, &["config", "commit.gpgsign", "false"]);
    directory
}

/// An inherited Git override must not redirect an operation fixture outside its owned temp worktree.
/// Advanced and completion scenarios share exactly the same local hooks and automatic-policy inputs.
fn isolated_repository() -> tempfile::TempDir {
    require_isolated_git_environment();
    let repository = repository();
    let root = repository.path();
    let hooks = root.join(".git/fixture-hooks");
    fs::create_dir(&hooks).unwrap();
    git(
        root,
        &[
            "config",
            "--local",
            "core.hooksPath",
            hooks.to_str().unwrap(),
        ],
    );
    for (key, value) in [
        ("rerere.enabled", "false"),
        ("merge.verifySignatures", "false"),
        ("merge.autoStash", "false"),
        ("rebase.autoStash", "false"),
    ] {
        git(root, &["config", "--local", key, value]);
    }
    repository
}

/// Files are literal fixture paths; every mutation under test still belongs to the actual guest.
fn commit_file(root: &Path, path: &str, text: &str, title: &str) -> String {
    fs::write(root.join(path), text).unwrap();
    git(root, &["add", "--", path]);
    git(root, &["commit", "-m", title]);
    git(root, &["rev-parse", "HEAD"]).trim().into()
}

/// Restart the public manager against the same installation, without recreating a clean profile.
/// Shared recovery fixtures retain their native document owner separately from this plugin actor.
pub(super) fn reopen(mut old: Manager, profile: &Path, root: &Path) -> Manager {
    old.shutdown_checked().unwrap();
    assert_eq!(old.resource_count(), 0);
    drop(old);
    let mut manager = Manager::open(
        profile.join("plugins"),
        Environment {
            workspace: root.display().to_string(),
            os: "windows".into(),
            locale: "en".into(),
            ..Default::default()
        },
    )
    .unwrap();
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
        assert!(
            Instant::now() < deadline,
            "reopened installation did not activate"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    idle(&mut manager);
    manager
}

/// Use the current scene and declared panel for the same typed interaction emitted by native UI.
fn send(manager: &mut Manager, panel: &str, node: &str, action: ui::Action) {
    let revision = manager.live["git"].views[panel].revision;
    manager
        .event(
            "git",
            Some(panel.into()),
            api::Notification::Ui(ui::UiEvent {
                revision,
                node: node.into(),
                action,
            }),
        )
        .unwrap();
    idle(manager);
}

/// Read visible readonly code, so a retained selection cannot pass merely by keeping an OID string.
fn snapshot(manager: &Manager, column: usize) -> String {
    let id = format!("snapshot-{column}");
    match &manager.live["git"].views["commit-details"]
        .active_node(&id)
        .unwrap_or_else(|| panic!("missing visible history source {id}"))
        .kind
    {
        ui::Kind::ReadonlyCode { text, .. } => text.clone(),
        _ => panic!("history source is not readonly code"),
    }
}
