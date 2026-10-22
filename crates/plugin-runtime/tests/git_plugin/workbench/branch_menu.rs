//! Branch context actions enter the real package through owned tree slots, never a global target field.
use super::*;
use plugin_runtime::plugin_protocol::tree;
use std::{collections::VecDeque, sync::Arc};

/// Resolve actual published action identities through both supported public native menu shapes.
pub(super) fn menu_commands(node: &tree::Node) -> Vec<(tree::NodeAction, tree::Command)> {
    if node.action_menu.is_empty() {
        return node
            .menu
            .iter()
            .enumerate()
            .map(|(index, command)| {
                (
                    tree::NodeAction::Menu {
                        index: index as u32,
                    },
                    command.clone(),
                )
            })
            .collect();
    }
    let mut actions = Vec::new();
    for item in &node.action_menu {
        item.visit(&mut |item| {
            if let ui::ActionMenuKind::Action { command } = &item.kind {
                actions.push((
                    tree::NodeAction::MenuItem {
                        id: item.id.clone(),
                    },
                    command.clone(),
                ));
            }
        });
    }
    actions
}

/// Load one native provider page using the currently admitted history scene.
fn load(
    manager: &mut Manager,
    resource: &Arc<plugin_runtime::TreeResource>,
    parent: Option<String>,
) {
    let request = resource.request(parent, false).unwrap().unwrap();
    let revision = manager.live["git"].views["history"].revision;
    manager
        .event(
            "git",
            Some("history".into()),
            api::Notification::Ui(ui::UiEvent {
                revision,
                node: "branches".into(),
                action: ui::Action::Tree(tree::Event::Request(request)),
            }),
        )
        .unwrap();
}

/// Submit one current native menu action without waiting for network or editor preparation.
pub(crate) fn context_press(manager: &mut Manager, action: &str, input: Option<&str>) {
    let resource = manager.live["git"].tree_resources().into_iter().find(|resource| {
        matches!(&manager.live["git"].views["history"].active_node("branches").expect("branch action tree").kind,
            ui::Kind::Tree { handle } if resource.handle() == handle)
    }).unwrap();
    let mut parents = VecDeque::from([None]);
    while let Some(parent) = parents.pop_front() {
        if !resource.snapshot().branches[&parent].loaded {
            load(manager, &resource, parent.clone());
        }
        let snapshot = resource.snapshot();
        for id in &snapshot.branches[&parent].children {
            if snapshot.nodes[id].branch {
                parents.push_back(Some(id.clone()));
            }
        }
    }
    let snapshot = resource.snapshot();
    let node = snapshot
        .nodes
        .values()
        .find(|node| {
            menu_commands(node).iter().any(|(_, item)| {
                item.arguments["action"] == action
                    && !item.disabled
                    && (action != "switch"
                        || input.is_none_or(|target| {
                            item.arguments["target"].as_str().is_some_and(|reference| {
                                reference == target
                                    || reference.strip_prefix("refs/heads/") == Some(target)
                            })
                        }))
            })
        })
        .unwrap_or_else(|| panic!("missing enabled {action} context"));
    invoke(manager, &resource, node, action).unwrap();
    manager.poll();
}

/// Other public workbench scenarios use the replacement native menu, optionally editing its explicit prompt.
pub(crate) fn context_action(manager: &mut Manager, action: &str, input: Option<&str>) {
    context_press(manager, action, input);
    idle(manager);
    if let Some(input) = input
        && manager.live["git"].views["history"]
            .dialog
            .as_ref()
            .is_some_and(|dialog| dialog.id == "branch-dialog")
    {
        send(
            manager,
            "history",
            "branch-name",
            ui::Action::Change(input.into()),
        );
        click(manager, "history", "branch-create");
    }
}

/// Read on-demand repository metadata through the real menu, then restore the previous modal state.
/// An already-open information dialog stays open; other operation reviews must never be replaced.
pub(crate) fn repository_text(manager: &mut Manager, id: &str) -> String {
    let open = manager.live["git"].views["history"].dialog.as_ref();
    let already_open = open.is_some_and(|dialog| dialog.id == "repo-info-dialog");
    assert!(
        open.is_none() || already_open,
        "metadata cannot replace an operation review"
    );
    if !already_open {
        context_action(manager, "repo-info", None);
    }
    let ui::Kind::Text { text } = &manager.live["git"].views["history"]
        .active_node(id)
        .expect("visible repository information")
        .kind
    else {
        panic!("repository information is native text: {id}");
    };
    let text = text.clone();
    if !already_open {
        click(manager, "history", "repo-close");
    }
    text
}

/// Resolve a visible local leaf from real lazy pages, preserving its exact epoch and menu slot.
pub(super) fn branch(
    manager: &mut Manager,
    label: &str,
) -> (Arc<plugin_runtime::TreeResource>, tree::Node) {
    let resource = manager.live["git"]
        .tree_resources()
        .into_iter()
        .find(|resource| {
            matches!(&manager.live["git"].views["history"].active_node("branches").unwrap().kind,
            ui::Kind::Tree { handle } if resource.handle() == handle)
        })
        .unwrap();
    if !resource.snapshot().branches[&None].loaded {
        load(manager, &resource, None);
    }
    let local = resource
        .snapshot()
        .nodes
        .values()
        .find(|node| node.label == "Local")
        .unwrap()
        .id
        .clone();
    if !resource.snapshot().branches[&Some(local.clone())].loaded {
        load(manager, &resource, Some(local));
    }
    let node = resource
        .snapshot()
        .nodes
        .values()
        .find(|node| !node.branch && node.label == label)
        .unwrap()
        .clone();
    (resource, node)
}

/// A real package removes chrome without removing pagination, Stash or inspectable diagnostics.
#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE; real history layout and menu pagination"]
fn git_package_headerless_history_preserves_pagination_and_on_demand_information() {
    if !run_isolated_test(
        "workbench::branch_menu::git_package_headerless_history_preserves_pagination_and_on_demand_information",
    ) {
        return;
    }
    let repo = isolated_repository();
    let root = repo.path();
    // One private fast-import creates enough actual commits to exercise the second bounded page.
    let history = (0..130).map(|index| {
        let title = format!("Layout {index}");
        format!("commit refs/heads/main\ncommitter Layout <layout@example.invalid> 1700000000 +0000\ndata {}\n{title}\n\n", title.len())
    }).collect::<String>();
    let mut importer = Command::new("git")
        .current_dir(root)
        .args(["fast-import", "--quiet"])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    std::io::Write::write_all(importer.stdin.take().as_mut().unwrap(), history.as_bytes()).unwrap();
    assert!(importer.wait().unwrap().success());
    let (_profile, mut manager) = activate(&candidate(), root);
    let scene = &manager.live["git"].views["history"];
    for absent in [
        "toolbar",
        "argument",
        "log-headers",
        "repo-controls",
        "status",
        "repo-fetch-time",
        "more",
    ] {
        assert!(
            scene.active_node(absent).is_none(),
            "removed history chrome: {absent}"
        );
    }
    let ui::Kind::Canvas(canvas) = &scene.active_node("log").unwrap().kind else {
        panic!("headerless native history canvas");
    };
    assert_eq!(canvas.scroll.as_ref().unwrap().content, 128. * 28.);
    assert!(canvas.paint.iter().any(|paint| matches!(paint,
        plugin_runtime::plugin_protocol::Paint::Svg { rect, clip, .. } if rect.x == 16. && rect == clip)));
    context_action(&mut manager, "more", None);
    let ui::Kind::Canvas(canvas) = &manager.live["git"].views["history"]
        .active_node("log")
        .unwrap()
        .kind
    else {
        panic!("paged history canvas");
    };
    assert_eq!(canvas.scroll.as_ref().unwrap().content, 130. * 28.);
    assert!(repository_text(&mut manager, "repo-fetch-time").contains("Not fetched yet"));
    assert!(manager.live["git"].views["history"].dialog.is_none());
    context_action(&mut manager, "stash-list", None);
    assert_eq!(
        manager.live["git"].views["history"]
            .dialog
            .as_ref()
            .unwrap()
            .id,
        "repo-stash-window"
    );
    click(&mut manager, "history", "repo-close");
    assert_eq!(git(root, &["rev-list", "--count", "HEAD"]).trim(), "130");
    manager.disable("git").unwrap();
    assert_eq!(manager.resource_count(), 0);
}

/// Actual lazy pages admit both folder theme variants and distinguish every ref from its directories.
#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE; real branch tree artwork"]
fn git_package_branch_tree_uses_explorer_folders_and_supplied_branch_artwork() {
    if !run_isolated_test(
        "workbench::branch_menu::git_package_branch_tree_uses_explorer_folders_and_supplied_branch_artwork",
    ) {
        return;
    }
    let repo = isolated_repository();
    commit_file(repo.path(), "base.txt", "base\n", "base");
    git(repo.path(), &["branch", "feature/topic"]);
    git(
        repo.path(),
        &["update-ref", "refs/remotes/origin/main", "HEAD"],
    );
    let package = candidate();
    let (_profile, mut manager) = activate(&package, repo.path());
    let resource = manager.live["git"]
        .tree_resources()
        .into_iter()
        .find(|resource| {
            matches!(&manager.live["git"].views["history"].active_node("branches").unwrap().kind,
            ui::Kind::Tree { handle } if resource.handle() == handle)
        })
        .unwrap();
    let mut parents = VecDeque::from([None]);
    while let Some(parent) = parents.pop_front() {
        load(&mut manager, &resource, parent.clone());
        let page = resource.snapshot();
        for id in &page.branches[&parent].children {
            if page.nodes[id].branch {
                parents.push_back(Some(id.clone()));
            }
        }
    }
    let snapshot = resource.snapshot();
    let mut directories = 0;
    let mut refs = 0;
    for node in snapshot.nodes.values() {
        let icon = node.icon.as_ref().expect("published branch-tree artwork");
        if node.branch {
            directories += 1;
            assert_eq!(
                icon,
                &ui::ToolIcon::new("icons/folder.svg", "icons/folder-dark.svg")
            );
        } else {
            refs += 1;
            assert_eq!(icon, &ui::ToolIcon::new("icons/git.svg", "icons/git.svg"));
        }
    }
    assert_eq!((directories, refs), (4, 3));
    for (owned, original) in [
        ("icons/folder.svg", "folder.svg"),
        ("icons/folder-dark.svg", "folder_dark.svg"),
    ] {
        let expected = std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../editor-ui/assets/file-icons")
                .join(original),
        )
        .unwrap();
        assert_eq!(
            snapshot.icons[owned].as_ref(),
            expected.as_slice(),
            "folder art differs from Explorer: {owned}"
        );
    }
    assert_eq!(
        snapshot.icons["icons/git.svg"].as_ref(),
        package.files["icons/git.svg"].as_slice()
    );
    assert_eq!(
        snapshot.icons.len(),
        3,
        "repeated rows share admitted artwork"
    );
    manager.shutdown_checked().unwrap();
    assert_eq!(manager.resource_count(), 0);
}

/// Invoke only the slot published on this node; caller-supplied command arguments cannot replace it.
fn invoke(
    manager: &mut Manager,
    resource: &Arc<plugin_runtime::TreeResource>,
    node: &tree::Node,
    action: &str,
) -> anyhow::Result<()> {
    let action = menu_commands(node)
        .into_iter()
        .find(|(_, item)| item.arguments["action"] == action)
        .map(|(action, _)| action)
        .ok_or_else(|| anyhow::anyhow!("Branch action is not published"))?;
    let revision = manager.live["git"].views["history"].revision;
    manager.event(
        "git",
        Some("history".into()),
        api::Notification::Ui(ui::UiEvent {
            revision,
            node: "branches".into(),
            action: ui::Action::Tree(tree::Event::Invoke {
                tree: resource.handle().clone(),
                epoch: resource.snapshot().epoch,
                node: node.id.clone(),
                action,
            }),
        }),
    )
}

/// A cold native filter can be superseded by an explicit menu without consuming the reviewed write.
#[test]
#[ignore = "real independent Git candidate; set NANOBUG_GIT_CANDIDATE; typed command after cold filters"]
fn git_package_branch_menu_after_cold_filters_keeps_switch_review() {
    if !run_isolated_test(
        "workbench::branch_menu::git_package_branch_menu_after_cold_filters_keeps_switch_review",
    ) {
        return;
    }
    let repo = isolated_repository();
    let root = repo.path();
    commit_file(root, "base.txt", "base\n", "base");
    git(root, &["branch", "topic"]);
    let (_profile, mut manager) = activate(&candidate(), root);
    for label in ["topic", "main"] {
        let (resource, node) = branch(&mut manager, label);
        let revision = manager.live["git"].views["history"].revision;
        manager
            .event(
                "git",
                Some("history".into()),
                api::Notification::Ui(ui::UiEvent {
                    revision,
                    node: "branches".into(),
                    action: ui::Action::Tree(tree::Event::Selected {
                        tree: resource.handle().clone(),
                        epoch: resource.snapshot().epoch,
                        node: node.id,
                    }),
                }),
            )
            .unwrap();
    }
    let call = manager
        .invoke_typed_command(
            "git",
            "branch-context",
            serde_json::json!({"action":"switch","target":"refs/heads/topic"}),
            30_000,
        )
        .unwrap();
    idle(&mut manager);
    if let api::RequestUpdate::Completed { result } = call.status() {
        result.expect("the exact typed menu call must succeed after canceling history reads");
    } else {
        panic!("typed switch still pending: {:?}", call.status());
    }
    assert!(
        manager.live["git"].views["history"]
            .active_node("confirm-command")
            .is_some(),
        "switch review must remain visible after late history replies"
    );
    assert_eq!(git(root, &["branch", "--show-current"]).trim(), "main");
    manager.shutdown_checked().unwrap();
}

/// A clicked branch supplies switch/merge/create targets; unavailable actions cannot bypass a grey menu.
#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE; real native tree menu commands"]
fn git_package_branch_context_owns_targets_and_rejects_disabled_actions() {
    if !run_isolated_test(
        "workbench::branch_menu::git_package_branch_context_owns_targets_and_rejects_disabled_actions",
    ) {
        return;
    }
    let repo = isolated_repository();
    let root = repo.path();
    commit_file(root, "base.txt", "base\n", "base");
    git(root, &["branch", "topic"]);
    let (_profile, mut manager) = activate(&candidate(), root);
    assert!(
        manager.live["git"].views["history"]
            .active_node("toolbar")
            .is_none(),
        "Git toolbar must be replaced by branch context actions"
    );
    assert!(
        manager.live["git"].views["history"]
            .active_node("argument")
            .is_none(),
        "context actions must not borrow a mutable global target field"
    );
    let (resource, topic) = branch(&mut manager, "topic");
    assert!(
        menu_commands(&topic)
            .iter()
            .any(|(_, item)| item.arguments["action"] == "switch")
    );
    assert!(
        invoke(&mut manager, &resource, &topic, "pull").is_err(),
        "a non-current branch cannot pull into the current checkout"
    );
    invoke(&mut manager, &resource, &topic, "switch").unwrap();
    idle(&mut manager);
    assert!(
        manager.live["git"].views["history"]
            .active_node("confirm-command")
            .is_some()
    );
    click(&mut manager, "history", "confirm-command");
    assert_eq!(git(root, &["branch", "--show-current"]).trim(), "topic");
    let (resource, main) = branch(&mut manager, "main");
    let epoch = resource.snapshot().epoch;
    invoke(&mut manager, &resource, &main, "branch").unwrap();
    idle(&mut manager);
    assert!(!candidate().manifest.permissions.contains("files.select"));
    assert!(
        resource.snapshot().epoch == epoch,
        "a branch prompt updates cached menus without reloading provider pages or requiring a picker grant"
    );
    send(
        &mut manager,
        "history",
        "branch-name",
        ui::Action::Change("from-main".into()),
    );
    click(&mut manager, "history", "branch-create");
    assert_eq!(
        git(root, &["rev-parse", "from-main"]),
        git(root, &["rev-parse", "main"])
    );
    assert_eq!(git(root, &["branch", "--show-current"]).trim(), "topic");
    let (resource, main) = branch(&mut manager, "main");
    invoke(&mut manager, &resource, &main, "merge").unwrap();
    idle(&mut manager);
    click(&mut manager, "history", "branch-create");
    let scene = &manager.live["git"].views["history"];
    assert!(
        scene.active_node("confirm-command").is_some(),
        "merge still requires its impact review"
    );
    let mut review = String::new();
    scene.dialog.as_ref().unwrap().content.visit(&mut |node| {
        if let ui::Kind::Text { text } = &node.kind {
            review.push_str(text);
        }
    });
    assert!(review.contains("refs/heads/main"));
    click(&mut manager, "history", "cancel-command");
    // Hold a real native-buffer enumeration open: cancellation must be reachable from the
    // branch menu even before a Git child exists, and its late receipt cannot execute the switch.
    let (resource, main) = branch(&mut manager, "main");
    invoke(&mut manager, &resource, &main, "switch").unwrap();
    idle(&mut manager);
    let revision = manager.live["git"].views["history"].revision;
    manager
        .event(
            "git",
            Some("history".into()),
            api::Notification::Ui(ui::UiEvent {
                revision,
                node: "confirm-command".into(),
                action: ui::Action::Click,
            }),
        )
        .unwrap();
    let pending = manager
        .take_editor_requests()
        .into_iter()
        .find_map(|(_, request)| {
            matches!(request.operation(), api::EditorOperation::ListDocuments).then_some(request)
        })
        .expect("switch waits for native buffers");
    for parent in [None, Some("Local")] {
        let parent = parent.map(|label| {
            resource
                .snapshot()
                .nodes
                .values()
                .find(|node| node.label == label)
                .unwrap()
                .id
                .clone()
        });
        // Cached pages are already current after a dismissed review; they need no provider call.
        if let Some(request) = resource.request(parent, false).unwrap() {
            let revision = manager.live["git"].views["history"].revision;
            manager
                .event(
                    "git",
                    Some("history".into()),
                    api::Notification::Ui(ui::UiEvent {
                        revision,
                        node: "branches".into(),
                        action: ui::Action::Tree(tree::Event::Request(request)),
                    }),
                )
                .unwrap();
        }
    }
    let main = resource
        .snapshot()
        .nodes
        .values()
        .find(|node| node.label == "main")
        .unwrap()
        .clone();
    invoke(&mut manager, &resource, &main, "cancel-process").unwrap();
    pending.finish(Ok(api::EditorValue::Documents {
        documents: vec![],
        active: None,
    }));
    idle(&mut manager);
    assert_eq!(
        git(root, &["branch", "--show-current"]).trim(),
        "topic",
        "cancelled native preparation executed after its late reply"
    );
    manager.shutdown_checked().unwrap();
    assert_eq!(manager.resource_count(), 0);
}

/// A newer host must not silently accept extended menus from a package declaring the 1.0 floor.
#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE; real SDK tree publication admission"]
fn git_package_extended_branch_menu_requires_an_explicit_tree_minor_floor() {
    if !run_isolated_test(
        "workbench::branch_menu::git_package_extended_branch_menu_requires_an_explicit_tree_minor_floor",
    ) {
        return;
    }
    use std::io::{Cursor, Write as _};
    let original = candidate();
    let mut files = original.files.clone();
    let mut manifest = original.manifest.clone();
    manifest
        .api
        .as_mut()
        .unwrap()
        .required
        .insert("ui.tree".into(), "^1".parse().unwrap());
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (path, bytes) in files {
        archive
            .start_file(path, zip::write::SimpleFileOptions::default())
            .unwrap();
        archive.write_all(&bytes).unwrap();
    }
    let package = Package::from_bytes(&archive.finish().unwrap().into_inner()).unwrap();
    let repo = isolated_repository();
    commit_file(repo.path(), "base.txt", "base\n", "base");
    let (_profile, mut manager) = activate(&package, repo.path());
    let resource = manager.live["git"]
        .tree_resources()
        .into_iter()
        .find(|resource| {
            matches!(&manager.live["git"].views["history"].active_node("branches").unwrap().kind,
            ui::Kind::Tree { handle } if resource.handle() == handle)
        })
        .unwrap();
    load(&mut manager, &resource, None);
    assert!(
        resource.snapshot().nodes.is_empty(),
        "extended metadata was partially admitted under ui.tree 1.0"
    );
    // A deliberately incompatible tree cannot supply its context menu. The guest still publishes
    // the same diagnostic in its independent review window, without a permanent history footer.
    let ui::Kind::Text { text: status } = &manager.live["git"].views["commit-details"]
        .active_node("window-status")
        .expect("published capability diagnostic")
        .kind
    else {
        panic!("native capability diagnostic");
    };
    assert!(
        status.contains("ui.tree >=1.3") || status.contains("ui.tree >=1.1"),
        "missing explicit capability diagnostic: {status}"
    );
    manager.shutdown_checked().unwrap();
    assert_eq!(manager.resource_count(), 0);
}
