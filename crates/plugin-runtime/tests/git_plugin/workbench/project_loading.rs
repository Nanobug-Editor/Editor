//! Private project roots distinguish an expected non-repository diagnostic from a stalled package load.
use super::*;
use plugin_runtime::{TreeSnapshot, plugin_protocol::tree};

/// Demand only the current tree's resource-issued page; a completed empty page is still responsive.
fn page(manager: &mut Manager, panel: &str, node: &str, parent: Option<String>) -> TreeSnapshot {
    let scene = manager.live["git"].views[panel].clone();
    let ui::Kind::Tree { handle } = &scene.active_node(node).expect("enabled native tree").kind
    else {
        panic!("expected native tree {panel}/{node}");
    };
    let resource = manager.live["git"]
        .tree_resources()
        .into_iter()
        .find(|resource| resource.handle() == handle)
        .expect("instance-owned native tree");
    if let Some(request) = resource.request(parent.clone(), false).unwrap() {
        manager
            .event(
                "git",
                Some(panel.into()),
                api::Notification::Ui(ui::UiEvent {
                    revision: scene.revision,
                    node: node.into(),
                    action: ui::Action::Tree(tree::Event::Request(request)),
                }),
            )
            .unwrap();
    }
    let snapshot = resource.snapshot();
    let branch = snapshot.branches.get(&parent).expect("requested branch");
    assert!(branch.loaded, "{panel}/{node} did not return its page");
    assert!(!branch.loading);
    assert!(branch.error.is_none(), "{:?}", branch.error);
    snapshot
}

/// Busy work and user-visible timeout messages are observations of the real public scenes.
fn assert_responsive(manager: &Manager) {
    let instance = &manager.live["git"];
    assert!(instance.error.is_none(), "{:?}", instance.error);
    assert!(manager.installed["git"].error.is_none());
    assert_eq!(instance.process_count(), 0);
    assert!(instance.views["history"].active_node("branches").is_some());
    for (panel, document) in &instance.views {
        document.root.visit(&mut |node| {
            if let ui::Kind::Text { text } = &node.kind {
                let lower = text.to_ascii_lowercase();
                assert!(
                    !lower.contains("timeout") && !lower.contains("timed out"),
                    "unexpected timeout in {panel}/{}: {text}",
                    node.id
                );
            }
        });
    }
}

/// Visibility follows the public host notification and never activates a user profile or OS window.
fn make_visible(manager: &mut Manager) {
    for panel in ["history", "staging"] {
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
    idle(manager);
}

/// Rejected metadata admission cancels only its exact pending page, without accepting stale input.
/// A real visibility publication replaces the scene while retaining the same tree authority.
fn assert_stale_page_is_released(manager: &mut Manager) {
    let panel = "commit-details";
    let node = "files";
    let scene = manager.live["git"].views[panel].clone();
    let ui::Kind::Tree { handle } = &scene.active_node(node).expect("visible files tree").kind
    else {
        panic!("expected native files tree");
    };
    let resource = manager.live["git"]
        .tree_resources()
        .into_iter()
        .find(|resource| resource.handle() == handle)
        .expect("instance-owned files tree");
    let epoch = resource.snapshot().epoch;
    let old_request = resource.request(None, false).unwrap().unwrap();
    let pending_revision = resource.snapshot().revision;
    let old_event = ui::UiEvent {
        revision: scene.revision,
        node: node.into(),
        action: ui::Action::Tree(tree::Event::Request(old_request.clone())),
    };
    manager
        .event(
            "git",
            Some("history".into()),
            api::Notification::PanelVisibility {
                panel: "history".into(),
                visible: false,
            },
        )
        .unwrap();
    let current = manager.live["git"].views[panel].clone();
    assert!(
        current.revision > scene.revision,
        "visibility must replace the scene"
    );
    assert!(matches!(
        &current.active_node(node).expect("retained files tree").kind,
        ui::Kind::Tree { handle: current_handle } if current_handle == handle
    ));
    assert_eq!(resource.snapshot().epoch, epoch);
    let error = manager
        .event(
            "git",
            Some(panel.into()),
            api::Notification::Ui(old_event.clone()),
        )
        .unwrap_err();
    assert_eq!(
        error.downcast_ref::<api::Failure>().unwrap().code,
        api::ErrorCode::StaleRevision
    );
    let cancelled = resource.snapshot();
    let root = &cancelled.branches[&None];
    assert!(
        !root.loading,
        "rejected stale page must release its exact pending loading before the 15-second timeout"
    );
    assert!(!root.loaded);
    assert!(
        root.error.is_none(),
        "host cancellation is not a provider timeout"
    );
    assert_eq!(
        cancelled.epoch, epoch,
        "one rejection must not refresh the tree"
    );
    assert!(cancelled.revision > pending_revision);
    assert_eq!(manager.live["git"].views[panel].revision, current.revision);

    // The current scene issues fresh metadata. Repeated old events, with either old or current
    // scene revision, cannot cancel a newer pending sequence or reach the provider as valid input.
    let next = resource.request(None, false).unwrap().unwrap();
    assert!(next.sequence > old_request.sequence);
    assert_eq!(next.epoch, epoch);
    let next_revision = resource.snapshot().revision;
    for revision in [scene.revision, current.revision] {
        let mut obsolete = old_event.clone();
        obsolete.revision = revision;
        let error = manager
            .event("git", Some(panel.into()), api::Notification::Ui(obsolete))
            .unwrap_err();
        assert_eq!(
            error.downcast_ref::<api::Failure>().unwrap().code,
            api::ErrorCode::StaleRevision
        );
        let retained = resource.snapshot();
        assert_eq!(retained.epoch, epoch);
        assert_eq!(retained.revision, next_revision);
        assert!(retained.branches[&None].loading);
        assert!(retained.branches[&None].error.is_none());
    }
    manager
        .event(
            "git",
            Some(panel.into()),
            api::Notification::Ui(ui::UiEvent {
                revision: current.revision,
                node: node.into(),
                action: ui::Action::Tree(tree::Event::Request(next)),
            }),
        )
        .unwrap();
    let loaded = resource.snapshot();
    assert_eq!(loaded.epoch, epoch);
    assert!(loaded.nodes.is_empty());
    assert!(loaded.branches[&None].loaded);
    assert!(!loaded.branches[&None].loading);
    assert!(loaded.branches[&None].error.is_none());
}

/// A non-Git project must finish with an actionable diagnostic, while an independent valid root
/// must load its actual history, dirty file and exact detail identity through the same package.
#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE; private non-Git and real repository startup"]
fn git_package_project_loading_distinguishes_non_git_roots_from_ready_workbenches() {
    if !run_isolated_test(
        "workbench::project_loading::git_package_project_loading_distinguishes_non_git_roots_from_ready_workbenches",
    ) {
        return;
    }
    let package = candidate();
    let empty = tempfile::tempdir().unwrap();
    let (_empty_profile, mut empty_manager) = activate(&package, empty.path());
    make_visible(&mut empty_manager);
    assert_responsive(&empty_manager);
    let text = branch_menu::repository_text(&mut empty_manager, "status");
    assert!(
        text.contains("not a Git worktree") && text.contains("refresh"),
        "non-Git startup must explain which root to open: {text}"
    );
    // A restored detail window may be visible without any selected commit. Its current
    // files tree must answer a real empty page rather than waiting for a nonexistent OID.
    empty_manager
        .event(
            "git",
            Some("commit-details".into()),
            api::Notification::PanelVisibility {
                panel: "commit-details".into(),
                visible: true,
            },
        )
        .unwrap();
    idle(&mut empty_manager);
    assert_stale_page_is_released(&mut empty_manager);
    for (panel, node) in [
        ("history", "branches"),
        ("staging", "staging"),
        ("commit-details", "files"),
    ] {
        let snapshot = page(&mut empty_manager, panel, node, None);
        assert!(
            snapshot
                .nodes
                .values()
                .all(|node| panel == "history" && node.branch),
            "an empty root has no Git leaves; branch directories retain repository actions"
        );
    }
    empty_manager.disable("git").unwrap();
    assert_eq!(empty_manager.resource_count(), 0);
    drop(empty_manager);

    // A separate root/profile starts afresh; the negative project cannot donate cached UI or refs.
    let repository = repository();
    let root = repository.path();
    commit_file(root, "tracked.txt", "first\n", "loading-first");
    commit_file(root, "tracked.txt", "second\n", "loading-second");
    let newest = commit_file(root, "tracked.txt", "third\n", "loading-third");
    fs::write(root.join("tracked.txt"), "private unstaged edit\n").unwrap();
    let (_profile, mut manager) = activate(&package, root);
    make_visible(&mut manager);
    send(
        &mut manager,
        "history",
        "log",
        ui::Action::Canvas(ui::CanvasEvent::Resize {
            width: 900.,
            height: 240.,
            grid: None,
        }),
    );
    assert_responsive(&manager);
    let ui::Kind::Canvas(canvas) = &manager.live["git"].views["history"]
        .active_node("log")
        .expect("ready history canvas")
        .kind
    else {
        panic!("history must expose its native canvas");
    };
    for title in ["loading-first", "loading-second", "loading-third"] {
        assert!(canvas.paint.iter().any(|paint| {
            matches!(paint, plugin_runtime::plugin_protocol::Paint::Text { text, .. } if text == title)
        }), "missing actual history title {title}");
    }
    let staging = page(&mut manager, "staging", "staging", None);
    let unstaged = staging
        .nodes
        .values()
        .find(|node| node.branch && node.label == "Unstaged")
        .expect("actual dirty file has its native group")
        .id
        .clone();
    let files = page(&mut manager, "staging", "staging", Some(unstaged));
    assert!(
        files
            .nodes
            .values()
            .any(|node| !node.branch && node.label == "tracked.txt")
    );

    for key in ["home", "enter"] {
        send(
            &mut manager,
            "history",
            "log",
            ui::Action::Canvas(ui::CanvasEvent::Key {
                key: key.into(),
                ctrl: false,
                alt: false,
                shift: false,
            }),
        );
    }
    assert_responsive(&manager);
    assert!(matches!(
        &manager.live["git"].views["commit-details"].active_node("commit-identity")
            .expect("actual detail identity").kind,
        ui::Kind::Text { text } if text == &newest
    ));
    let details = page(&mut manager, "commit-details", "files", None);
    assert!(
        details
            .nodes
            .values()
            .any(|node| !node.branch && node.label == "tracked.txt")
    );
    assert_eq!(
        fs::read_to_string(root.join("tracked.txt")).unwrap(),
        "private unstaged edit\n"
    );
    manager.disable("git").unwrap();
    assert_eq!(manager.resource_count(), 0);
}
