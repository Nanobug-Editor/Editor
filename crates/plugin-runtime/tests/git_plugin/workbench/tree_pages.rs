//! Independent native trees can request their current owned pages from one published scene revision.
use super::*;

#[path = "tree_pages/conflicts.rs"]
mod conflicts;

#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE; concurrent owned native tree pages"]
fn git_package_parallel_tree_pages_do_not_invalidate_each_others_scene_revision() {
    // Keep factory and Manager/native Git children in one private configuration process.
    if !run_isolated_test(
        "workbench::tree_pages::git_package_parallel_tree_pages_do_not_invalidate_each_others_scene_revision",
    ) {
        return;
    }
    let repository = repository();
    let root = repository.path();
    std::fs::create_dir(root.join("src")).unwrap();
    std::fs::write(root.join("src/tracked.txt"), "base\n").unwrap();
    git(root, &["add", "--", "."]);
    git(root, &["commit", "-m", "base"]);
    std::fs::write(root.join("src/tracked.txt"), "worktree change\n").unwrap();
    let (_profile, mut manager) = activate(&candidate(), root);
    // Native reactivation can repeat a valid measurement. A metadata no-op must not create a
    // fresh scene before its independent trees can finish their current page requests.
    let dimensions = ui::Action::Canvas(ui::CanvasEvent::Resize {
        width: 800.,
        height: 180.,
        grid: None,
    });
    send(&mut manager, "history", "log", dimensions.clone());
    let measured_revision = manager.live["git"].views["history"].revision;
    send(&mut manager, "history", "log", dimensions);
    assert_eq!(
        manager.live["git"].views["history"].revision, measured_revision,
        "repeating native geometry preserves the admitted scene"
    );
    let mut pages = Vec::new();
    for (panel, node) in [("staging", "staging"), ("history", "branches")] {
        let scene = &manager.live["git"].views[panel];
        let ui::Kind::Tree { handle } = &scene.active_node(node).unwrap().kind else {
            panic!("workbench declares its native tree");
        };
        let resource = manager.live["git"]
            .tree_resources()
            .into_iter()
            .find(|resource| resource.handle() == handle)
            .unwrap();
        let before = resource.snapshot().revision;
        let request = resource
            .request(None, false)
            .unwrap()
            .expect("initial page requested");
        pages.push((panel, node, scene.revision, resource, before, request));
    }
    // This one-row history fits its viewport. A real wheel at its boundary must leave both
    // already allocated tree pages admitted under the same immutable scene revision.
    send(
        &mut manager,
        "history",
        "log",
        ui::Action::Canvas(ui::CanvasEvent::Wheel {
            x: 100.,
            y: 14.,
            delta_x: 0.,
            delta_y: -160.,
            shift: false,
        }),
    );
    assert_eq!(
        manager.live["git"].views["history"].revision, measured_revision,
        "a boundary wheel preserves independent pending tree pages"
    );
    // Both requests were allocated by the painted native trees before either reply. The public
    // Manager must still reject genuinely stale UI; the guest avoids replacing an unchanged scene.
    for (panel, node, revision, _, _, request) in &pages {
        manager
            .event(
                "git",
                Some((*panel).into()),
                api::Notification::Ui(ui::UiEvent {
                    revision: *revision,
                    node: (*node).into(),
                    action: ui::Action::Tree(
                        plugin_runtime::plugin_protocol::tree::Event::Request(request.clone()),
                    ),
                }),
            )
            .unwrap_or_else(|error| {
                panic!("independent {node} page was invalidated by another tree reply: {error}")
            });
    }
    for (panel, _, revision, resource, before, _) in pages {
        let snapshot = resource.snapshot();
        assert!(snapshot.revision > before);
        assert!(snapshot.branches[&None].loaded);
        assert!(!snapshot.branches[&None].loading);
        assert!(snapshot.branches[&None].error.is_none());
        assert!(!snapshot.nodes.is_empty());
        assert_eq!(
            manager.live["git"].views[panel].revision, revision,
            "a data-page reply does not change the native scene's controls"
        );
    }
    manager.disable("git").unwrap();
    assert_eq!(manager.resource_count(), 0);
}

/// Native expansion can restore selection while its child page is already allocated. Selecting
/// that directory is not a business action, so the page keeps its exact original scene authority.
#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE; native directory selection and child page"]
fn git_package_directory_selection_keeps_an_already_allocated_child_page_current() {
    // Keep factory and Manager/native Git children in one private configuration process.
    if !run_isolated_test(
        "workbench::tree_pages::git_package_directory_selection_keeps_an_already_allocated_child_page_current",
    ) {
        return;
    }
    use plugin_runtime::plugin_protocol::tree;

    let repository = repository();
    let root = repository.path();
    std::fs::create_dir(root.join("src")).unwrap();
    std::fs::write(root.join("src/a-review.txt"), "base\n").unwrap();
    git(root, &["add", "--", "."]);
    git(root, &["commit", "-m", "base"]);
    std::fs::write(root.join("src/a-review.txt"), "worktree change\n").unwrap();
    let (_profile, mut manager) = activate(&candidate(), root);
    let scene = manager.live["git"].views["staging"].clone();
    let ui::Kind::Tree { handle } = &scene.active_node("staging").unwrap().kind else {
        panic!("staging declares its native tree");
    };
    let resource = manager.live["git"]
        .tree_resources()
        .into_iter()
        .find(|resource| resource.handle() == handle)
        .unwrap();

    // Prepare actual provider pages through Manager before reproducing the native ordering.
    for parent in [None, Some("Unstaged")] {
        let parent = parent.map(|label| {
            resource
                .snapshot()
                .nodes
                .values()
                .find(|node| node.label == label && node.branch)
                .expect("the worktree change has its status directory")
                .id
                .clone()
        });
        let request = resource.request(parent, false).unwrap().unwrap();
        let revision = manager.live["git"].views["staging"].revision;
        manager
            .event(
                "git",
                Some("staging".into()),
                api::Notification::Ui(ui::UiEvent {
                    revision,
                    node: "staging".into(),
                    action: ui::Action::Tree(tree::Event::Request(request)),
                }),
            )
            .unwrap();
    }
    let snapshot = resource.snapshot();
    let directory = snapshot
        .nodes
        .values()
        .find(|node| node.label == "src" && node.branch)
        .expect("nested source directory is visible")
        .id
        .clone();
    let revision = manager.live["git"].views["staging"].revision;
    let request = resource
        .request(Some(directory.clone()), false)
        .unwrap()
        .unwrap();
    assert!(resource.snapshot().branches[&Some(directory.clone())].loading);

    // Both envelopes originate in one painted frame. Do not resample the revision between
    // selection and delivery: doing so would hide the native page's StaleRevision failure.
    manager
        .event(
            "git",
            Some("staging".into()),
            api::Notification::Ui(ui::UiEvent {
                revision,
                node: "staging".into(),
                action: ui::Action::Tree(tree::Event::Selected {
                    tree: handle.clone(),
                    epoch: snapshot.epoch,
                    node: directory.clone(),
                }),
            }),
        )
        .unwrap();
    manager
        .event(
            "git",
            Some("staging".into()),
            api::Notification::Ui(ui::UiEvent {
                revision,
                node: "staging".into(),
                action: ui::Action::Tree(tree::Event::Request(request)),
            }),
        )
        .unwrap_or_else(|error| {
            panic!("directory selection invalidated its allocated child page: {error}")
        });
    let snapshot = resource.snapshot();
    let branch = &snapshot.branches[&Some(directory)];
    assert!(branch.loaded && !branch.loading && branch.error.is_none());
    let file = snapshot
        .nodes
        .values()
        .find(|node| node.label == "a-review.txt" && !node.branch)
        .expect("the originally requested file becomes visible");
    assert_eq!(manager.live["git"].views["staging"].revision, revision);

    // A real file selection must still publish its changed path and native comparison state.
    manager
        .event(
            "git",
            Some("staging".into()),
            api::Notification::Ui(ui::UiEvent {
                revision,
                node: "staging".into(),
                action: ui::Action::Tree(tree::Event::Selected {
                    tree: handle.clone(),
                    epoch: snapshot.epoch,
                    node: file.id.clone(),
                }),
            }),
        )
        .unwrap();
    let updated = &manager.live["git"].views["staging"];
    assert!(updated.revision > revision);
    assert!(matches!(
        &updated.active_node("selected-path").unwrap().kind,
        ui::Kind::Text { text } if text == "src/a-review.txt"
    ));
    manager.disable("git").unwrap();
    assert_eq!(manager.resource_count(), 0);
}
