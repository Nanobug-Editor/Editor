//! Real branch selection and panel visibility retain loaded public provider pages.
use super::*;
use plugin_runtime::{TreeResource, TreeSnapshot, plugin_protocol::tree};
use std::{collections::VecDeque, sync::Arc};

/// Expand every real prefix through admitted lazy-page requests, as a mounted native tree does.
fn expanded_branches(manager: &mut Manager) -> Arc<TreeResource> {
    let ui::Kind::Tree { handle } = &manager.live["git"].views["history"]
        .active_node("branches")
        .unwrap()
        .kind
    else {
        panic!("branch tree")
    };
    let resource = manager.live["git"]
        .tree_resources()
        .into_iter()
        .find(|resource| resource.handle() == handle)
        .unwrap();
    let mut pending = VecDeque::from([None]);
    while let Some(parent) = pending.pop_front() {
        if let Some(request) = resource.request(parent.clone(), false).unwrap() {
            send(
                manager,
                "history",
                "branches",
                ui::Action::Tree(tree::Event::Request(request)),
            );
        }
        let snapshot = resource.snapshot();
        for id in &snapshot.branches[&parent].children {
            if snapshot.nodes[id].branch {
                pending.push_back(Some(id.clone()));
            }
        }
    }
    resource
}

/// Detect the user's reload itself, rather than allowing a later page request to hide it.
fn retained(resource: &TreeResource, before: &TreeSnapshot, trigger: &str) {
    let current = resource.snapshot();
    assert_eq!(
        current.epoch, before.epoch,
        "{trigger} reloaded the unchanged branch tree"
    );
    assert_eq!(
        current.expanded, before.expanded,
        "{trigger} lost expanded prefixes"
    );
    assert_eq!(
        current.nodes.keys().collect::<Vec<_>>(),
        before.nodes.keys().collect::<Vec<_>>()
    );
    for (parent, branch) in &before.branches {
        assert!(branch.loaded);
        let now = current
            .branches
            .get(parent)
            .expect("retained provider page");
        assert!(
            now.loaded && !now.loading && now.error.is_none(),
            "{trigger}: {parent:?} unloaded"
        );
        assert_eq!(now.children, branch.children);
    }
}

/// Only repository changes require new branch data; selecting a filter or hiding a dock does not.
#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE; real branch-page retention"]
fn git_package_branch_selection_and_panel_visibility_keep_loaded_branch_pages() {
    if !run_isolated_test(
        "workbench::branch_retention::git_package_branch_selection_and_panel_visibility_keep_loaded_branch_pages",
    ) {
        return;
    }
    let repository = isolated_repository();
    let root = repository.path();
    commit_file(root, "tracked.txt", "base\n", "base");
    git(root, &["branch", "feature/review"]);
    let (_profile, mut manager) = activate(&candidate(), root);
    for panel in ["staging", "history"] {
        manager
            .event(
                "git",
                Some(panel.into()),
                api::Notification::PanelVisibility {
                    panel: panel.into(),
                    visible: true,
                },
            )
            .unwrap();
    }
    idle(&mut manager);
    let resource = expanded_branches(&mut manager);
    let before = resource.snapshot();
    assert!(!before.expanded.is_empty());
    let review = before
        .nodes
        .values()
        .find(|node| !node.branch && node.label == "review")
        .unwrap();
    send(
        &mut manager,
        "history",
        "branches",
        ui::Action::Tree(tree::Event::Selected {
            tree: resource.handle().clone(),
            epoch: before.epoch,
            node: review.id.clone(),
        }),
    );
    retained(&resource, &before, "branch selection");
    for (panel, visible) in [
        ("staging", false),
        ("staging", true),
        ("history", false),
        ("history", true),
    ] {
        manager
            .event(
                "git",
                Some(panel.into()),
                api::Notification::PanelVisibility {
                    panel: panel.into(),
                    visible,
                },
            )
            .unwrap();
        retained(
            &resource,
            &before,
            &format!("{panel} visible={visible} immediately"),
        );
        idle(&mut manager);
        retained(
            &resource,
            &before,
            &format!("{panel} visible={visible} completed"),
        );
    }
    // Status affects Stash availability, not branch membership. Update its cached menu without
    // invalidating any page, while retaining the provider's live command-side policy check.
    fs::write(root.join("tracked.txt"), "changed\n").unwrap();
    branch_menu::context_action(&mut manager, "refresh", None);
    retained(&resource, &before, "worktree-only change");
    assert!(resource.snapshot().nodes.values().all(|node| {
        node.menu
            .iter()
            .any(|action| action.arguments["action"] == "stash" && !action.disabled)
    }));
    // A real ref addition must remain observable; cache retention cannot hide repository changes.
    git(root, &["branch", "feature/added"]);
    branch_menu::context_action(&mut manager, "refresh", None);
    assert!(
        resource.snapshot().epoch > before.epoch,
        "real branch membership changes must refresh data"
    );
    let refreshed = expanded_branches(&mut manager);
    assert_eq!(refreshed.handle(), resource.handle());
    assert!(
        refreshed
            .snapshot()
            .nodes
            .values()
            .any(|node| !node.branch && node.label == "added")
    );
    manager.disable("git").unwrap();
    assert_eq!(manager.resource_count(), 0);
}
