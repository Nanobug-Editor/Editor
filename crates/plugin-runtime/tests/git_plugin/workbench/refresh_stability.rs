//! Quiet local observation must preserve loaded native trees when repository data is unchanged.
use super::*;
use plugin_runtime::plugin_protocol::tree;

/// Load the same provider page requested by a mounted native tree, through the public manager.
fn load(
    manager: &mut Manager,
    panel: &str,
    node: &str,
    resource: &plugin_runtime::TreeResource,
    parent: Option<String>,
) {
    let request = resource.request(parent, false).unwrap().unwrap();
    let revision = manager.live["git"].views[panel].revision;
    manager
        .event(
            "git",
            Some(panel.into()),
            api::Notification::Ui(ui::UiEvent {
                revision,
                node: node.into(),
                action: ui::Action::Tree(tree::Event::Request(request)),
            }),
        )
        .unwrap();
}

/// Real timers must leave expanded pages usable across two quiet intervals without suppressing
/// observation itself. Both visible trees participate, as they share one Git refresh round.
#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE; real quiet timer and expanded native trees"]
fn git_package_unchanged_timer_preserves_loaded_staging_and_branch_pages() {
    if !run_isolated_test(
        "workbench::refresh_stability::git_package_unchanged_timer_preserves_loaded_staging_and_branch_pages",
    ) {
        return;
    }
    let repository = repository();
    let root = repository.path();
    fs::create_dir(root.join("src")).unwrap();
    commit_file(root, "src/tracked.txt", "base\n", "base");
    git(root, &["branch", "feature/review"]);
    fs::write(root.join("src/tracked.txt"), "worktree change\n").unwrap();
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
    let mut trees = Vec::new();
    for (panel, node, group) in [
        ("staging", "staging", "Unstaged"),
        ("history", "branches", "Local"),
    ] {
        let ui::Kind::Tree { handle } = &manager.live["git"].views[panel]
            .active_node(node)
            .unwrap()
            .kind
        else {
            panic!("native tree required");
        };
        let resource = manager.live["git"]
            .tree_resources()
            .into_iter()
            .find(|resource| resource.handle() == handle)
            .unwrap();
        load(&mut manager, panel, node, &resource, None);
        let parent = resource
            .snapshot()
            .nodes
            .values()
            .find(|entry| entry.branch && entry.label == group)
            .unwrap_or_else(|| panic!("missing {group} directory"))
            .id
            .clone();
        load(&mut manager, panel, node, &resource, Some(parent.clone()));
        let before = resource.snapshot();
        assert!(before.branches[&Some(parent.clone())].loaded);
        trees.push((panel, parent, resource, before));
    }
    let started = Instant::now();
    let scenes =
        ["staging", "history"].map(|panel| (panel, manager.live["git"].views[panel].revision));
    let mut observed_query = false;
    while started.elapsed() < Duration::from_secs(8) {
        manager.poll();
        observed_query |= manager.live["git"].process_count() > 0;
        for (_, request) in manager.take_editor_requests() {
            match request.operation().clone() {
                api::EditorOperation::ListDocuments => {
                    request.finish(Ok(api::EditorValue::Documents {
                        documents: vec![],
                        active: None,
                    }))
                }
                api::EditorOperation::SetPanelVisibility { panel, visible } => {
                    request.finish(Ok(api::EditorValue::PanelVisibility { panel, visible }))
                }
                operation => panic!("unexpected local refresh request: {operation:?}"),
            }
        }
        for (panel, parent, resource, before) in &trees {
            let current = resource.snapshot();
            assert_eq!(
                current.epoch,
                before.epoch,
                "unchanged local refresh invalidated the expanded {panel} tree after {:?}",
                started.elapsed()
            );
            assert!(
                current
                    .branches
                    .get(&Some(parent.clone()))
                    .is_some_and(|branch| branch.loaded),
                "unchanged local refresh unloaded the expanded {panel} page"
            );
        }
        for (panel, revision) in scenes {
            assert_eq!(
                manager.live["git"].views[panel].revision,
                revision,
                "unchanged background observation replaced the usable {panel} controls after {:?}",
                started.elapsed()
            );
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        observed_query,
        "the real visible timer must still observe local Git state"
    );
    // Interrupt the next real read with one admitted action. A busy guard must not silently
    // swallow this click; the read-only child yields before the repository dialog is opened.
    let deadline = Instant::now() + Duration::from_secs(15);
    while manager.live["git"].process_count() == 0 {
        manager.poll();
        assert!(
            Instant::now() < deadline,
            "next local observation did not start"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let revision = manager.live["git"].views["history"].revision;
    manager
        .event(
            "git",
            Some("history".into()),
            api::Notification::Ui(ui::UiEvent {
                revision,
                node: "repo-info".into(),
                action: ui::Action::Click,
            }),
        )
        .expect("one current click can interrupt optional read-only observation");
    assert!(
        manager.live["git"].views["history"].dialog.is_some(),
        "background Git reads swallowed the repository dialog action"
    );
    assert_eq!(
        manager.live["git"].process_count(),
        0,
        "the optional child must be retired before user intent"
    );
    idle(&mut manager);
    assert!(
        manager.live["git"].views["history"].dialog.is_some(),
        "late cancelled process output must not replace current user intent"
    );
    // This is observation, not a disabled timer: independently changed status and ref membership
    // must still retire their old provider pages without requiring save, focus or manual refresh.
    fs::write(root.join("src/external.txt"), "external file\n").unwrap();
    git(root, &["branch", "feature/external"]);
    let deadline = Instant::now() + Duration::from_secs(15);
    while trees
        .iter()
        .any(|(_, _, resource, before)| resource.snapshot().epoch == before.epoch)
    {
        idle(&mut manager);
        assert!(
            Instant::now() < deadline,
            "real external status/ref changes did not invalidate their tree pages"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    manager.disable("git").unwrap();
    assert_eq!(manager.resource_count(), 0);
}
