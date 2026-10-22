//! Actual Git packages drive native right-click controls; disabled rows never reach a typed invocation.
use super::*;
use gpui_base::Root;
use gpui_kit::{TestAppContext, VisualTestContext};
use gpui_kit::{gpui, px, size};
use plugin_runtime::{
    Manager, Package,
    plugin_protocol::{api, tree, ui},
};
use std::{
    cell::RefCell,
    process::Command,
    time::{Duration, Instant},
};
mod interaction;

/// Git runs only after the shared private-environment handoff, inside this test's disposable worktree.
fn git(root: &std::path::Path, args: &[&str]) -> String {
    super::git_fixture_environment::require_private_configuration().unwrap();
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

/// Draw the complete real history scene at multiple widths in both themes, without replaying any
/// passive layout receipts. Menu business effects are exercised separately through native input.
#[gpui::test]
#[ignore = "real independent Git candidate and native GPUI; set NANOBUG_GIT_CANDIDATE"]
fn git_headerless_history_native_layout_uses_the_full_body(cx: &mut TestAppContext) {
    if !super::git_fixture_environment::run_isolated_test(
        "extensions::branch_menu_tests::git_headerless_history_native_layout_uses_the_full_body",
    ) {
        return;
    }
    let storage = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    git(workspace.path(), &["init", "-b", "main"]);
    git(
        workspace.path(),
        &[
            "-c",
            "user.name=layout",
            "-c",
            "user.email=layout@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--allow-empty",
            "-m",
            "Layout",
        ],
    );
    let package = Package::read(&std::path::PathBuf::from(
        std::env::var_os("NANOBUG_GIT_CANDIDATE").expect("candidate ZIP"),
    ))
    .unwrap();
    let environment = plugin_runtime::plugin_protocol::Environment {
        workspace: workspace.path().display().to_string(),
        locale: "en".into(),
        os: "windows".into(),
        ..Default::default()
    };
    let mut manager = Manager::open(storage.path().into(), environment.clone()).unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    super::demand_plugin_fixture(
        &mut manager,
        "git",
        plugin_runtime::DemandReason::View("history".into()),
    )
    .unwrap();
    settle(&mut manager);
    let document = manager.live["git"].views["history"].as_ref().clone();
    let resources = manager.live["git"].tree_resources();
    cx.update(|cx| {
        gpui_kit::init(cx);
        editor_ui::typography::init(cx);
        crate::ui::theme::apply_theme(crate::ui::theme::builtin_theme(false), cx);
    });
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let view = cx.new(|cx| {
            let mut view = crate::ui::plugin::PluginView::new(
                "git".into(),
                document,
                environment,
                |_, _| {},
                window,
                cx,
            );
            view.update_trees(&resources, cx);
            view
        });
        Root::new(view, window, cx)
    });
    for dark in [false, true] {
        visual.update(|_, cx| {
            crate::ui::theme::apply_theme(crate::ui::theme::builtin_theme(dark), cx)
        });
        for width in [760., 1100., 1500.] {
            visual.simulate_resize(size(px(width), px(300.)));
            visual.run_until_parked();
            visual.update(|window, cx| window.draw(cx).clear(cx));
            let body = visual
                .debug_bounds("plugin-ui-history-body")
                .expect("painted history body");
            let root = visual
                .debug_bounds("plugin-ui-history-root")
                .expect("history fills its dock body");
            assert_eq!(body.top(), root.top(), "no outer top inset");
            assert_eq!(body.bottom(), root.bottom(), "no outer bottom inset");
            let branches = visual
                .debug_bounds("plugin-ui-branches")
                .expect("painted branch tree");
            let log = visual
                .debug_bounds("plugin-ui-log")
                .expect("painted history canvas");
            assert_eq!(log.top(), body.top());
            assert_eq!(log.bottom(), body.bottom());
            assert_eq!(branches.top(), log.top());
            assert_eq!(branches.bottom(), log.bottom());
            assert!(log.left() >= branches.right());
            assert!(log.size.height >= px(280.));
            for absent in [
                "plugin-ui-argument",
                "plugin-ui-log-headers",
                "plugin-ui-repo-controls",
                "plugin-ui-status",
            ] {
                assert!(
                    visual.debug_bounds(absent).is_none(),
                    "removed native chrome: {absent}"
                );
            }
        }
    }
    manager.shutdown_checked().unwrap();
}

/// Drain real process notifications and generic document receipts without inventing a scene.
fn settle(manager: &mut Manager) {
    let deadline = Instant::now() + Duration::from_secs(45);
    let mut quiet = 0;
    loop {
        manager.poll();
        for (_, request) in manager.take_editor_requests() {
            match request.operation().clone() {
                api::EditorOperation::SetPanelVisibility { panel, visible } => {
                    request.finish(Ok(api::EditorValue::PanelVisibility { panel, visible }))
                }
                api::EditorOperation::ListDocuments => {
                    request.finish(Ok(api::EditorValue::Documents {
                        documents: vec![],
                        active: None,
                    }))
                }
                operation => panic!("unexpected menu receipt: {operation:?}"),
            }
        }
        quiet = if manager.live["git"].process_count() == 0 {
            quiet + 1
        } else {
            0
        };
        if quiet == 8 {
            return;
        }
        assert!(Instant::now() < deadline, "Git menu action did not settle");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Retain the package's actual branch node and modal without unrelated canvas layout callbacks.
fn menu_document(manager: &Manager) -> ui::Document {
    let scene = &manager.live["git"].views["history"];
    // Mount the actual branch subtree and actual dialogs. Unrelated canvas layout callbacks
    // belong to the full worker harness; no synthetic branch/menu nodes are introduced here.
    let mut root = None;
    scene.root.visit(&mut |node| {
        if node.id == "branches" {
            root = Some(node.clone());
        }
    });
    let mut document =
        ui::Document::new(root.expect("published branches")).revision(scene.revision);
    document.dialog = scene.dialog.clone();
    document
}

/// Publish the real menu subtree and any confirmation from the public manager into native controls.
fn frame(
    visual: &mut VisualTestContext,
    view: &Entity<crate::ui::plugin::PluginView>,
    manager: &Rc<RefCell<Manager>>,
) {
    let (document, resources) = {
        let manager = manager.borrow();
        (
            menu_document(&manager),
            manager.live["git"].tree_resources(),
        )
    };
    visual.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.update_document(
                document,
                plugin_runtime::plugin_protocol::Environment {
                    locale: "en".into(),
                    ..Default::default()
                },
                window,
                cx,
            );
            view.update_trees(&resources, cx);
        })
    });
    visual.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear(cx));
}

/// Both themes expose the real branch menu; a disabled pull is inert and a native switch changes Git.
#[gpui::test]
#[ignore = "real independent Git candidate and native GPUI; set NANOBUG_GIT_CANDIDATE"]
fn git_branch_menu_native_right_click_disables_unavailable_actions(cx: &mut TestAppContext) {
    if !super::git_fixture_environment::run_isolated_test(
        "extensions::branch_menu_tests::git_branch_menu_native_right_click_disables_unavailable_actions",
    ) {
        return;
    }
    let storage = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let root = workspace.path();
    git(root, &["init", "-b", "main"]);
    git(
        root,
        &[
            "-c",
            "user.name=menu",
            "-c",
            "user.email=menu@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--allow-empty",
            "-m",
            "base",
        ],
    );
    git(root, &["branch", "topic"]);
    let package = Package::read(&std::path::PathBuf::from(
        std::env::var_os("NANOBUG_GIT_CANDIDATE").expect("candidate ZIP"),
    ))
    .unwrap();
    let environment = plugin_runtime::plugin_protocol::Environment {
        workspace: root.display().to_string(),
        locale: "en".into(),
        os: "windows".into(),
        ..Default::default()
    };
    let mut manager = Manager::open(storage.path().into(), environment.clone()).unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    super::demand_plugin_fixture(
        &mut manager,
        "git",
        plugin_runtime::DemandReason::View("history".into()),
    )
    .unwrap();
    settle(&mut manager);
    let handle = match &manager.live["git"].views["history"]
        .active_node("branches")
        .unwrap()
        .kind
    {
        ui::Kind::Tree { handle } => handle.clone(),
        _ => panic!("native branch tree"),
    };
    let resource = manager.live["git"]
        .tree_resources()
        .into_iter()
        .find(|tree| *tree.handle() == handle)
        .unwrap();
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
    let topic = resource
        .snapshot()
        .nodes
        .values()
        .find(|node| node.label == "topic")
        .unwrap()
        .clone();
    let mut actions = Vec::new();
    for item in &topic.action_menu {
        item.visit(&mut |item| {
            if let ui::ActionMenuKind::Action { command } = &item.kind {
                actions.push((item.id.clone(), command.clone()));
            }
        });
    }
    let pull = actions
        .iter()
        .find(|(_, command)| command.arguments["action"] == "pull")
        .unwrap();
    let switch = actions
        .iter()
        .find(|(_, command)| command.arguments["action"] == "switch")
        .unwrap();
    assert!(pull.1.disabled);
    assert!(!switch.1.disabled);
    let pull = pull.0.clone();
    let switch = switch.0.clone();
    cx.update(|cx| {
        gpui_kit::init(cx);
        editor_ui::typography::init(cx);
        crate::ui::theme::apply_theme(crate::ui::theme::builtin_theme(false), cx);
    });
    let manager = Rc::new(RefCell::new(manager));
    let source = manager.clone();
    let slot = Rc::new(RefCell::new(None));
    let capture = slot.clone();
    let events = Rc::new(RefCell::new(Vec::new()));
    let output = events.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let document = menu_document(&source.borrow());
        let trees = source.borrow().live["git"].tree_resources();
        let sink = source.clone();
        let view = cx.new(|cx| {
            let mut view = crate::ui::plugin::PluginView::new(
                "git".into(),
                document,
                environment,
                move |event, _| {
                    output.borrow_mut().push(event.clone());
                    sink.borrow_mut()
                        .event("git", Some("history".into()), api::Notification::Ui(event))
                        .unwrap();
                },
                window,
                cx,
            );
            view.update_trees(&trees, cx);
            view
        });
        *capture.borrow_mut() = Some(view.clone());
        Root::new(view, window, cx)
    });
    visual.simulate_resize(size(px(1100.), px(760.)));
    let view = slot.borrow_mut().take().unwrap();
    // GPUI's selector API requires static strings; these three bounded test selectors live to exit.
    let row_selector: &'static str =
        Box::leak(format!("plugin-tree-row-{}", topic.id).into_boxed_str());
    let pull_selector: &'static str = Box::leak(format!("action-menu-{pull}").into_boxed_str());
    let switch_selector: &'static str = Box::leak(format!("action-menu-{switch}").into_boxed_str());
    frame(visual, &view, &manager);
    // A first press only focuses the branch tree. Filtering and the blue selection require
    // the native second press, so inspecting another branch never starts a history query.
    let selected = visual.update(|_, cx| view.read(cx).native_tree_selected_id("branches", cx));
    let before = events.borrow().len();
    let row = visual.debug_bounds(row_selector).unwrap().center();
    visual.simulate_click(row, Default::default());
    frame(visual, &view, &manager);
    assert_eq!(
        visual.update(|_, cx| view.read(cx).native_tree_selected_id("branches", cx)),
        selected,
        "single click must retain the selected branch"
    );
    assert_eq!(
        events.borrow().len(),
        before,
        "single click must not filter history"
    );
    interaction::second_press(visual, row_selector);
    frame(visual, &view, &manager);
    assert_eq!(
        visual.update(|_, cx| view.read(cx).native_tree_selected_id("branches", cx)),
        Some(Some(topic.id.clone())),
        "double click must select the target branch"
    );
    let selected_events = events.borrow()[before..].iter().filter(|event| {
        matches!(&event.action, ui::Action::Tree(tree::Event::Selected { node, .. }) if node == &topic.id)
    }).count();
    assert_eq!(
        selected_events, 1,
        "one double click emits one filter event"
    );
    // Return to the initially selected branch before checking gray hover on another menu target.
    let main_selector: &'static str = Box::leak(
        format!("plugin-tree-row-{}", selected.clone().unwrap().unwrap()).into_boxed_str(),
    );
    let main_position = visual.debug_bounds(main_selector).unwrap().center();
    visual.simulate_click(main_position, Default::default());
    interaction::second_press(visual, main_selector);
    frame(visual, &view, &manager);
    for dark in [false, true] {
        visual.update(|_, cx| {
            crate::ui::theme::apply_theme(crate::ui::theme::builtin_theme(dark), cx)
        });
        frame(visual, &view, &manager);
        let row = visual.debug_bounds(row_selector).unwrap().center();
        let main_color = interaction::background(visual, main_selector);
        let plain_color = interaction::background(visual, row_selector);
        let hover_color: gpui_kit::Hsla =
            gpui_kit::rgb(if dark { 0x393b40 } else { 0xebecf0 }).into();
        visual.simulate_mouse_move(row, None, Default::default());
        frame(visual, &view, &manager);
        assert_eq!(
            interaction::background(visual, row_selector),
            hover_color,
            "native gray hover"
        );
        visual.simulate_mouse_down(row, gpui_kit::MouseButton::Right, Default::default());
        visual.simulate_mouse_up(row, gpui_kit::MouseButton::Right, Default::default());
        frame(visual, &view, &manager);
        assert!(visual.debug_bounds("action-menu-column-0").is_some());
        let before = events.borrow().len();
        let disabled = visual.debug_bounds(pull_selector).unwrap().center();
        visual.simulate_mouse_move(disabled, None, Default::default());
        frame(visual, &view, &manager);
        assert_eq!(
            interaction::background(visual, row_selector),
            hover_color,
            "menu target retains hover after pointer leaves"
        );
        assert_eq!(
            interaction::background(visual, main_selector),
            main_color,
            "context menu preserves the selected branch color"
        );
        assert_eq!(
            visual.update(|_, cx| view.read(cx).native_tree_selected_id("branches", cx)),
            selected
        );
        visual.simulate_click(disabled, Default::default());
        frame(visual, &view, &manager);
        assert_eq!(
            events.borrow().len(),
            before,
            "disabled native menu dispatched an action"
        );
        assert_eq!(git(root, &["branch", "--show-current"]).trim(), "main");
        if !dark {
            visual.simulate_keystrokes("escape");
            frame(visual, &view, &manager);
            assert_eq!(
                interaction::background(visual, row_selector),
                plain_color,
                "closing menu clears retained hover"
            );
        }
    }
    let enabled = visual.debug_bounds(switch_selector).unwrap().center();
    let before_switch = events.borrow().len();
    visual.simulate_click(enabled, Default::default());
    visual.run_until_parked();
    settle(&mut manager.borrow_mut());
    frame(visual, &view, &manager);
    assert!(events.borrow()[before_switch..].iter().any(|event| {
        matches!(&event.action, ui::Action::Tree(tree::Event::Invoke { action: tree::NodeAction::MenuItem { id }, .. }) if id == &switch)
    }), "enabled native switch must dispatch its exact menu slot: {:?}", &events.borrow()[before_switch..]);
    assert_eq!(
        git(root, &["branch", "--show-current"]).trim(),
        "main",
        "switch must retain its impact review"
    );
    // Process quiescence alone does not finish an actor's queued typed command after a filter
    // cancellation. Wait for the real confirmation publication, as a user observes it.
    let deadline = Instant::now() + Duration::from_secs(15);
    let confirm = loop {
        if let Some(bounds) = visual.debug_bounds("plugin-ui-confirm-command") {
            break bounds.center();
        }
        assert!(
            Instant::now() < deadline,
            "visible switch confirmation; events={:?}; dialog={:?}",
            &events.borrow()[before_switch..],
            manager.borrow().live["git"].views["history"].dialog
        );
        settle(&mut manager.borrow_mut());
        frame(visual, &view, &manager);
    };
    visual.simulate_click(confirm, Default::default());
    visual.run_until_parked();
    settle(&mut manager.borrow_mut());
    frame(visual, &view, &manager);
    assert_eq!(git(root, &["branch", "--show-current"]).trim(), "topic");
    manager.borrow_mut().shutdown_checked().unwrap();
}
