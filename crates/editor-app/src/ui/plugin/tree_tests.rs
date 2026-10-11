//! Real independently packaged providers exercise native keyboard, focus and retry behavior.
use super::*;
use gpui_kit::{TestAppContext, VisualTestContext, component::Root, gpui, px, size};
use plugin_runtime::{Manager, Package};
use std::cell::RefCell;

/// Publish only public manager resources, using the same keyed native projection as the worker.
fn frame(
    visual: &mut VisualTestContext,
    view: &Entity<PluginView>,
    manager: &Rc<RefCell<Manager>>,
) {
    // Typed node commands use the ordinary actor tick before a new native publication.
    manager.borrow_mut().poll();
    let publication = manager.borrow().live.get("database-tree").map(|instance| {
        (
            instance.views["presentation"].clone(),
            instance.tree_resources(),
        )
    });
    visual.update(|window, cx| {
        view.update(cx, |view, cx| {
            if let Some((scene, resources)) = publication {
                view.update_document((*scene).clone(), Environment::default(), window, cx);
                view.update_trees(&resources, cx);
            } else {
                // The worker withdraws resources and seals the scene after provider retirement.
                view.set_scene_current(false, cx);
                view.update_trees(&[], cx);
            }
        })
    });
    visual.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear(cx));
}

/// Inspect the real package's public status, so native selections prove the delivered typed arguments.
fn status(manager: &Rc<RefCell<Manager>>) -> String {
    let mut status = None;
    manager.borrow().live["database-tree"].views["presentation"]
        .root
        .visit(&mut |node| {
            if node.id == "catalog-status" {
                if let Kind::Text { text } = &node.kind {
                    status = Some(text.clone());
                }
            }
        });
    status.expect("provider publishes its command result")
}

/// Native Retry, keyboard paging and menus retain focus through refresh and provider retirement.
#[gpui::test]
#[ignore = "package plugins/database-tree through the current host --plugin-package first"]
fn community_database_tree_native_keyboard_waits_for_manual_retry(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::ui::typography::init(cx);
        crate::ui::theme::apply_theme(crate::ui::theme::builtin_theme(false), cx);
    });
    let storage = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let package = Package::read(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/community-presentation/database-tree-0.1.1.zip"),
    )
    .unwrap();
    let mut manager = Manager::open(
        storage.path().into(),
        Environment {
            workspace: workspace.path().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    // This standalone native projection explicitly displays the provider's declared view.
    crate::extensions::demand_plugin_fixture(
        &mut manager,
        "database-tree",
        plugin_runtime::DemandReason::View("presentation".into()),
    )
    .unwrap();
    let resource = manager.live["database-tree"]
        .tree_resources()
        .pop()
        .unwrap();
    let manager = Rc::new(RefCell::new(manager));
    let source = manager.clone();
    let holder = Rc::new(RefCell::new(None));
    let captured = holder.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let scene = source.borrow().live["database-tree"].views["presentation"].clone();
        let trees = source.borrow().live["database-tree"].tree_resources();
        let sink = source.clone();
        let view = cx.new(|cx| {
            let mut view = PluginView::new(
                "database-tree".into(),
                (*scene).clone(),
                Environment::default(),
                move |event, _| {
                    sink.borrow_mut()
                        .event(
                            "database-tree",
                            Some("presentation".into()),
                            plugin_runtime::plugin_protocol::api::Notification::Ui(event),
                        )
                        .unwrap();
                },
                window,
                cx,
            );
            view.update_trees(&trees, cx);
            view
        });
        *captured.borrow_mut() = Some(view.clone());
        Root::new(view, window, cx)
    });
    visual.simulate_resize(size(px(480.), px(360.)));
    let view = holder.borrow().clone().unwrap();
    frame(visual, &view, &manager);
    let main = visual
        .debug_bounds("plugin-tree-row-main")
        .unwrap()
        .center();
    visual.simulate_click(main, Default::default());
    visual.simulate_keystrokes("right");
    frame(visual, &view, &manager);
    // Observe Base's native tree semantics after a real pointer selection and keyboard expansion.
    // The local row renderer must preserve these facts rather than replace Base interaction.
    visual.update(|window, _| {
        let items = gpui_base::test_support::snapshots(window);
        assert!(
            items
                .iter()
                .any(|item| item.role() == Some(gpui_kit::Role::Tree))
        );
        let main = items
            .iter()
            .find(|item| {
                item.role() == Some(gpui_kit::Role::TreeItem) && item.label() == Some("main")
            })
            .expect("expanded provider node has a native accessible name");
        assert_eq!(main.selected(), Some(true));
        assert_eq!(main.expanded(), Some(true));
    });
    let fault = visual
        .debug_bounds("plugin-tree-row-fault-once")
        .unwrap()
        .center();
    visual.simulate_click(fault, Default::default());
    visual.simulate_keystrokes("right");
    frame(visual, &view, &manager);
    assert!(
        resource.snapshot().branches[&Some("fault-once".into())]
            .error
            .is_some(),
        "failed page must wait for explicit user retry"
    );
    assert!(
        visual
            .debug_bounds("plugin-tree-row-@retry/fault-once")
            .is_some()
    );
    visual.simulate_keystrokes("down enter");
    frame(visual, &view, &manager);
    assert_eq!(resource.snapshot().nodes["recovered"].label, "已恢复");

    // Same-command actions are distinct: Enter selects the summary while the two real menu rows
    // request details and schema. The public Manager must preserve each row's complete arguments.
    let recovered = visual
        .debug_bounds("plugin-tree-row-recovered")
        .unwrap()
        .center();
    visual.simulate_click(recovered, Default::default());
    visual.simulate_keystrokes("enter");
    frame(visual, &view, &manager);
    assert!(status(&manager).ends_with("recovered:summary"));
    for (keys, expected) in [
        ("enter", "recovered:details"),
        ("down enter", "recovered:schema"),
    ] {
        visual.simulate_mouse_down(recovered, gpui_kit::MouseButton::Right, Default::default());
        visual.simulate_mouse_up(recovered, gpui_kit::MouseButton::Right, Default::default());
        frame(visual, &view, &manager);
        assert!(visual.debug_bounds("native-popup-menu").is_some());
        assert!(visual.debug_bounds("native-menu-tree-menu-0").is_some());
        assert!(visual.debug_bounds("native-menu-tree-menu-1").is_some());
        visual.simulate_keystrokes(keys);
        frame(visual, &view, &manager);
        assert!(
            status(&manager).ends_with(expected),
            "selected {expected}, got {}",
            status(&manager)
        );
    }

    // The first sixteen objects are loaded on expansion; keyboard navigation reaches the native More row.
    let users = visual
        .debug_bounds("plugin-tree-row-users")
        .unwrap()
        .center();
    visual.simulate_click(users, Default::default());
    visual.simulate_keystrokes("right");
    frame(visual, &view, &manager);
    visual.simulate_keystrokes(&"down ".repeat(17));
    frame(visual, &view, &manager);
    assert!(visual.debug_bounds("plugin-tree-row-@more/users").is_some());
    visual.simulate_keystrokes("enter");
    frame(visual, &view, &manager);
    assert_eq!(
        resource.snapshot().branches[&Some("users".into())]
            .children
            .len(),
        32
    );

    // Fill the real provider's remaining pages via its public callback transport, then inspect
    // the same native viewport. Off-screen metadata must not instantiate all 512 native rows.
    while resource.snapshot().branches[&Some("users".into())]
        .next
        .is_some()
    {
        let request = resource
            .request(Some("users".into()), true)
            .unwrap()
            .unwrap();
        manager
            .borrow_mut()
            .event(
                "database-tree",
                Some("presentation".into()),
                plugin_runtime::plugin_protocol::api::Notification::Ui(UiEvent {
                    revision: 1,
                    node: "catalog-tree".into(),
                    action: Action::Tree(plugin_runtime::plugin_protocol::tree::Event::Request(
                        request,
                    )),
                }),
            )
            .unwrap();
    }
    frame(visual, &view, &manager);
    assert_eq!(
        resource.snapshot().branches[&Some("users".into())]
            .children
            .len(),
        512
    );
    assert!(visual.debug_bounds("plugin-tree-row-user-511").is_none());
    visual.simulate_keystrokes(&"down ".repeat(16));
    frame(visual, &view, &manager);
    let object = visual
        .debug_bounds("plugin-tree-row-user-31")
        .unwrap()
        .center();
    let focus = visual.update(|window, cx| window.focused(cx));
    visual.simulate_mouse_down(object, gpui_kit::MouseButton::Right, Default::default());
    visual.simulate_mouse_up(object, gpui_kit::MouseButton::Right, Default::default());
    frame(visual, &view, &manager);
    assert!(visual.debug_bounds("native-menu-tree-menu-0").is_some());
    visual.simulate_keystrokes("enter");
    frame(visual, &view, &manager);
    assert_eq!(visual.update(|window, cx| window.focused(cx)), focus);
    let mut invoked = false;
    manager.borrow().live["database-tree"].views["presentation"]
        .root
        .visit(&mut |node| {
            if let Kind::Text { text } = &node.kind {
                invoked |= text.contains("user-31");
            }
        });
    assert!(
        invoked,
        "a native menu must deliver this node's declared arguments"
    );

    // Keep a stable first-page node selected while refreshing; dismiss the old menu normally.
    visual.simulate_keystrokes(&"up ".repeat(31));
    frame(visual, &view, &manager);
    let first = visual
        .debug_bounds("plugin-tree-row-user-0")
        .unwrap()
        .center();
    visual.simulate_mouse_down(first, gpui_kit::MouseButton::Right, Default::default());
    visual.simulate_mouse_up(first, gpui_kit::MouseButton::Right, Default::default());
    frame(visual, &view, &manager);
    manager
        .borrow_mut()
        .invoke_command("database-tree", "refresh-database", serde_json::Value::Null)
        .unwrap();
    for _ in 0..4 {
        frame(visual, &view, &manager);
    }
    assert!(visual.debug_bounds("native-popup-menu").is_none());
    assert_eq!(visual.update(|window, cx| window.focused(cx)), focus);
    visual.simulate_keystrokes("enter");
    frame(visual, &view, &manager);
    let mut restored = false;
    manager.borrow().live["database-tree"].views["presentation"]
        .root
        .visit(&mut |node| {
            if let Kind::Text { text } = &node.kind {
                restored |= text.contains("user-0");
            }
        });
    assert!(
        restored,
        "stable node identity must restore keyboard selection after refresh"
    );
    manager.borrow_mut().disable("database-tree").unwrap();
    frame(visual, &view, &manager);
    assert!(!resource.is_live());
    assert!(visual.debug_bounds("plugin-tree-row-main").is_none());
}
