//! The delivered Git package supplies both generic presentation capabilities through real public calls.
use super::*;
use plugin_runtime::plugin_protocol::{presentation, tree};

/// Colors, shared branch policy, independent filtering and checkout recency cross the actual package.
#[test]
#[ignore = "build independent Git candidate and set NANOBUG_GIT_CANDIDATE"]
fn git_workspace_presentation_tracks_checkout_and_shared_actions() {
    if !run_isolated_test(
        "workbench::workspace_presentation::git_workspace_presentation_tracks_checkout_and_shared_actions",
    ) {
        return;
    }
    let repository = isolated_repository();
    let root = repository.path();
    commit_file(root, "tracked.txt", "base\n", "base");
    git(root, &["branch", "topic"]);
    fs::write(root.join("tracked.txt"), "changed\n").unwrap();
    fs::write(root.join("new.txt"), "new\n").unwrap();
    let (_profile, mut manager) = activate(&candidate(), root);
    let resource = manager.live["git"].workspace_presentations().pop().unwrap();
    let snapshot = resource.snapshot();
    assert_eq!(snapshot.files["tracked.txt"].tone, tree::Tone::Informative);
    assert_eq!(snapshot.files["new.txt"].tone, tree::Tone::Positive);
    assert_eq!(snapshot.entries[0].label.en, "main");
    assert!(
        snapshot.files["tracked.txt"]
            .tooltip
            .en
            .contains("Worktree")
    );
    typed(
        &mut manager,
        "workspace-action",
        json!({"action":"branch-menu","target":"refs/heads/main"}),
    );
    let (_, main) = branch_menu::branch(&mut manager, "main");
    let entry = resource.snapshot().entries[0].clone();
    let mut head = Vec::new();
    for item in &entry.menu {
        item.visit(&mut |item| {
            if let ui::ActionMenuKind::Submenu {
                on_open: Some(command),
                items,
            } = &item.kind
            {
                if command.arguments["target"] == "refs/heads/main" && item.checked {
                    head = commands(items);
                }
            }
        });
    }
    assert_eq!(head, commands(&main.action_menu));
    assert!(
        head.iter()
            .all(|command| command.arguments["action"] != "switch"
                && command.arguments["action"] != "branch-delete")
    );
    let (tree, topic) = branch_menu::branch(&mut manager, "topic");
    send(
        &mut manager,
        "history",
        "branches",
        ui::Action::Tree(tree::Event::Selected {
            tree: tree.handle().clone(),
            epoch: tree.snapshot().epoch,
            node: topic.id,
        }),
    );
    assert_eq!(
        resource.snapshot().entries[0].label.en,
        "main",
        "filter must not change HEAD title"
    );
    assert!(
        recent(&resource.snapshot().entries[0]).is_empty(),
        "history browsing changed recent branches"
    );
    // Keep this repository clean before confirming a write; fixture writes do not bypass its review.
    git(root, &["add", "--all"]);
    git(root, &["commit", "-m", "reviewed state"]);
    click(&mut manager, "history", "refresh");
    branch_menu::context_action(&mut manager, "switch", Some("topic"));
    assert_eq!(
        git(root, &["branch", "--show-current"]).trim(),
        "main",
        "checkout preceded confirmation"
    );
    assert!(recent(&resource.snapshot().entries[0]).is_empty());
    click(&mut manager, "history", "confirm-command");
    assert_eq!(git(root, &["branch", "--show-current"]).trim(), "topic");
    assert_eq!(resource.snapshot().entries[0].label.en, "topic");
    assert_eq!(recent(&resource.snapshot().entries[0]), ["topic"]);
    // Hidden panels keep the actual background timer alive without a manual command or focus event.
    for panel in ["staging", "history"] {
        manager
            .event(
                "git",
                Some(panel.into()),
                api::Notification::PanelVisibility {
                    panel: panel.into(),
                    visible: false,
                },
            )
            .unwrap();
    }
    assert!(!resource.snapshot().files.contains_key("new.txt"));
    fs::write(root.join("new.txt"), "hidden change\n").unwrap();
    let started = Instant::now();
    let deadline = started + Duration::from_secs(75);
    while !resource.snapshot().files.contains_key("new.txt") {
        manager.poll();
        assert!(
            Instant::now() < deadline,
            "Hidden workspace presentation stopped its background observer"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    eprintln!(
        "Hidden-panel automatic filename refresh: {:?}",
        started.elapsed()
    );
    idle(&mut manager);
    assert!(resource.snapshot().files.contains_key("new.txt"));
    manager = reopen(manager, _profile.path(), root);
    assert_eq!(
        recent(
            &manager.live["git"].workspace_presentations()[0]
                .snapshot()
                .entries[0]
        ),
        ["topic"],
        "successful checkout recency was not persisted"
    );
    manager.disable("git").unwrap();
    assert!(!resource.is_live());
    assert_eq!(manager.resource_count(), 0);
}

/// Ref metadata writes require review and keep the real guest's selected history filter valid.
#[test]
#[ignore = "build independent Git candidate and set NANOBUG_GIT_CANDIDATE"]
fn git_workspace_branch_rename_and_delete_follow_the_selected_filter() {
    if !run_isolated_test(
        "workbench::workspace_presentation::git_workspace_branch_rename_and_delete_follow_the_selected_filter",
    ) {
        return;
    }
    let repository = isolated_repository();
    let root = repository.path();
    commit_file(root, "tracked.txt", "base\n", "base");
    git(root, &["branch", "topic"]);
    let (_profile, mut manager) = activate(&candidate(), root);
    let (resource, topic) = branch_menu::branch(&mut manager, "topic");
    send(
        &mut manager,
        "history",
        "branches",
        ui::Action::Tree(tree::Event::Selected {
            tree: resource.handle().clone(),
            epoch: resource.snapshot().epoch,
            node: topic.id,
        }),
    );
    typed(
        &mut manager,
        "branch-context",
        json!({"action":"branch-rename","target":"refs/heads/topic"}),
    );
    send(
        &mut manager,
        "history",
        "branch-name",
        ui::Action::Change("renamed".into()),
    );
    click(&mut manager, "history", "branch-create");
    assert_eq!(git(root, &["branch", "--list", "topic"]).trim(), "topic");
    assert!(
        manager.live["git"].views["history"]
            .active_node("confirm-command")
            .is_some()
    );
    click(&mut manager, "history", "confirm-command");
    assert_eq!(
        git(root, &["branch", "--list", "renamed"]).trim(),
        "renamed"
    );
    assert_eq!(filter(&mut manager), Some("refs/heads/renamed".into()));
    typed(
        &mut manager,
        "branch-context",
        json!({"action":"branch-delete","target":"refs/heads/renamed"}),
    );
    assert!(
        !git(root, &["branch", "--list", "renamed"])
            .trim()
            .is_empty()
    );
    click(&mut manager, "history", "confirm-command");
    assert!(
        git(root, &["branch", "--list", "renamed"])
            .trim()
            .is_empty()
    );
    assert_eq!(filter(&mut manager), None);
    assert!(manager.live["git"].error.is_none());
    manager.shutdown_checked().unwrap();
}

/// Observe the guest's declared opaque snapshot rather than borrowing its private Rust fields.
fn filter(manager: &mut Manager) -> Option<String> {
    let snapshot = manager.live.get_mut("git").unwrap().snapshot().unwrap();
    serde_json::from_str::<serde_json::Value>(&snapshot.data).unwrap()["filter"]
        .as_str()
        .map(str::to_owned)
}

/// Collect typed actions independently of their menu-specific stable namespaces.
fn commands(items: &[ui::ActionMenuItem]) -> Vec<tree::Command> {
    let mut commands = Vec::new();
    for item in items {
        item.visit(&mut |item| {
            if let ui::ActionMenuKind::Action { command } = &item.kind {
                commands.push(command.clone());
            }
        });
    }
    commands
}
/// Recent branch rows are an observable publication, not an inspection of private guest preferences.
fn recent(entry: &presentation::Entry) -> Vec<String> {
    entry
        .menu
        .iter()
        .find(|item| item.id == "recent")
        .map(|item| {
            item.children()
                .iter()
                .map(|item| item.label.en.clone())
                .collect()
        })
        .unwrap_or_default()
}
/// Await correlated public typed completion and ordinary native editor acknowledgements.
fn typed(manager: &mut Manager, command: &str, arguments: serde_json::Value) {
    let call = manager
        .invoke_typed_command("git", command, arguments, 30_000)
        .unwrap();
    idle(manager);
    assert!(
        matches!(
            call.status(),
            api::RequestUpdate::Completed { result: Ok(_) }
        ),
        "typed call: {:?}",
        call.status()
    );
}
