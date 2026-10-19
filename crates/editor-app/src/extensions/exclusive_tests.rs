//! Regional replacement is exercised through manifest publication, native painting and real footer clicks.
use super::dock_tests::native_panel_bounds;
use super::*;
use gpui_base::dock::{DockPlacement, PanelId};
use gpui_kit::{TestAppContext, VisualTestContext, gpui};

/// Publish arbitrary declarative panels at the same boundary used by installed contributions.
fn publish(app: &Entity<EditorApp>, manifest: &protocol::Manifest, visual: &mut VisualTestContext) {
    visual.update(|window, cx| {
        let owner = app.read(cx).extensions.clone();
        owner.update(cx, |owner, cx| {
            let mut state = owner.worker.state.lock().unwrap();
            state.entries = vec![Installed {
                replaced_by: None,
                manifest: manifest.clone(),
                digest: "exclusive-fixture".into(),
                grants: manifest.permissions.clone(),
                enabled: true,
                project_enabled: Default::default(),
                global_enabled: None,
                retired_ui_contract: false,
                error: None,
            }];
            for panel in &manifest.panels {
                state.views.insert(
                    format!("region-fixture/{}", panel.id),
                    Arc::new(
                        protocol::ui::Document::new(
                            protocol::ui::Node::new(
                                format!("content-{}", panel.id),
                                protocol::ui::Kind::Text {
                                    text: panel.title.clone(),
                                },
                            )
                            .grow(),
                        )
                        .revision(1),
                    ),
                );
            }
            drop(state);
            owner.poll(cx);
        });
        app.update(cx, |app, cx| app.sync_plugin_panels(window, cx));
    });
    paint(visual);
}

/// Settle native layout, then clear the painted frame so hidden selectors cannot survive a prior draw.
fn paint(visual: &mut VisualTestContext) {
    visual.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear(cx));
}

/// Explicitly reveal `id` through the production panel path, then reconcile the current dock.
fn show(app: &Entity<EditorApp>, id: &str, visual: &mut VisualTestContext) {
    visual.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.plugin_panels[&format!("region-fixture/{id}")].update(cx, |panel, cx| {
                panel.show(window, cx);
                cx.notify();
            });
            app.sync_plugin_panels(window, cx);
            cx.notify();
        })
    });
    paint(visual);
}

/// A declared region replaces all its peers, retains their intent and resources, and restores on close.
fn exercise_region(cx: &mut TestAppContext, position: &str, dark: bool) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        typography::init(cx);
        apply_theme(builtin_theme(dark), cx);
        init(cx);
        cx.set_reduce_motion(true);
    });
    let directory = tempfile::tempdir().unwrap();
    let workspace_path = directory.path().join("workspace");
    std::fs::create_dir_all(&workspace_path).unwrap();
    let mut session =
        SessionState::load_in_profile(&workspace_path, &directory.path().join("profile"));
    session.explorer_visible = true;
    session.outline_visible = true;
    session.messages_visible = true;
    let workspace = Workspace::open(&workspace_path).unwrap();
    let slot = Rc::new(RefCell::new(None));
    let capture = slot.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let app =
            cx.new(|cx| EditorApp::new_with_session(workspace, None, Some(session), window, cx));
        *capture.borrow_mut() = Some(app.clone());
        Root::new(app, window, cx)
    });
    let app = slot.borrow_mut().take().unwrap();
    visual.simulate_resize(size(px(1280.), px(900.)));
    let manifest: protocol::Manifest = serde_json::from_value(serde_json::json!({
        "id":"region-fixture", "name":"Regional panels", "version":"1.0.0", "protocol":7,
        "permissions":["ui.panels"], "storage_limit":1024,
        "api":{"base":"^1","required":{"ui.panels":">=1.2,<2"}},
        "panels":[
            {"id":"shared","title":"Shared", "position":position},
            {"id":"first","title":"First", "position":position,"exclusive":true,"default_visible":false},
            {"id":"second","title":"Second", "position":position,"exclusive":true,"default_visible":false},
            {"id":"other","title":"Other region", "position":if position == "left" { "bottom" } else { "left" }}
        ]
    })).unwrap();
    let mut ordinary = manifest.clone();
    ordinary.panels.retain(|panel| !panel.exclusive);
    publish(&app, &ordinary, visual);
    let older_session = visual.update(|_, cx| {
        app.update(cx, |app, cx| {
            app.capture_dock_layout(cx);
            app.session_state.clone()
        })
    });
    publish(&app, &manifest, visual);
    let (explorer, outline, editor, shared) = visual.update(|_, cx| {
        let app = app.read(cx);
        (
            PanelId::from(app.explorer_panel.entity_id()),
            PanelId::from(app.outline_panel.entity_id()),
            PanelId::from(app.editor_panel.entity_id()),
            PanelId::from(app.plugin_panels["region-fixture/shared"].entity_id()),
        )
    });
    let editor_before = native_panel_bounds(&app, editor, visual);
    let shared_before = native_panel_bounds(&app, shared, visual);
    let explorer_before = native_panel_bounds(&app, explorer, visual);
    let outline_before = native_panel_bounds(&app, outline, visual);
    show(&app, "first", visual);
    assert!(visual.debug_bounds("plugin-ui-content-first").is_some());
    assert!(visual.debug_bounds("plugin-ui-content-shared").is_none());
    let first = visual.update(|_, cx| app.read(cx).plugin_panels["region-fixture/first"].clone());
    let first_id = PanelId::from(first.entity_id());
    let first_bounds = native_panel_bounds(&app, first_id, visual);
    let editor_after = native_panel_bounds(&app, editor, visual);
    assert!((editor_before.left() - editor_after.left()).abs() < px(2.));
    assert!((editor_before.top() - editor_after.top()).abs() < px(2.));
    assert!((editor_before.size.width - editor_after.size.width).abs() < px(2.));
    assert!((editor_before.size.height - editor_after.size.height).abs() < px(2.));
    if position == "left" {
        assert!(
            first_bounds.size.height
                > explorer_before.size.height + outline_before.size.height - px(40.)
        );
        // Right-side display changes consume editor width even while a plugin owns all of Left.
        for _ in 0..2 {
            let button = visual.debug_bounds("host-messages-toggle").unwrap();
            visual.simulate_click(button.center(), Default::default());
            let bounds = native_panel_bounds(&app, first_id, visual);
            assert!((bounds.size.width - first_bounds.size.width).abs() < px(2.));
            assert!((bounds.left() - first_bounds.left()).abs() < px(2.));
        }
    } else {
        assert!(first_bounds.size.width >= shared_before.size.width);
    }
    visual.update(|_, cx| {
        let app = app.read(cx);
        assert!(app.explorer_visible && app.session_state.outline_visible);
        assert!(
            app.plugin_panels["region-fixture/shared"]
                .read(cx)
                .visible
                .get()
        );
        assert_eq!(
            app.dock_presentation.is_suppressed(explorer),
            position == "left"
        );
        assert_eq!(
            app.dock_presentation.is_suppressed(outline),
            position == "left"
        );
        assert!(
            !app.plugin_panels["region-fixture/shared"]
                .read(cx)
                .panel_visibility
                .displayed
        );
    });

    // A second exclusive reveal wins without recreating either entity or an ordinary peer.
    show(&app, "second", visual);
    assert!(visual.debug_bounds("plugin-ui-content-first").is_none());
    assert!(visual.debug_bounds("plugin-ui-content-second").is_some());
    // A real footer click on an obscured shared surface selects its ordinary combination.
    let trigger = visual
        .debug_bounds("plugin-window-region-fixture/shared")
        .unwrap()
        .center();
    visual.simulate_click(trigger, Default::default());
    paint(visual);
    assert!(visual.debug_bounds("plugin-ui-content-shared").is_some());
    assert!(visual.debug_bounds("plugin-ui-content-second").is_none());
    if position == "left" {
        let explorer_after = native_panel_bounds(&app, explorer, visual);
        let outline_after = native_panel_bounds(&app, outline, visual);
        assert!(
            (explorer_after.size.height - explorer_before.size.height).abs() < px(2.),
            "Explorer changed: {explorer_before:?} -> {explorer_after:?}"
        );
        assert!(
            (outline_after.size.height - outline_before.size.height).abs() < px(2.),
            "Outline changed: {outline_before:?} -> {outline_after:?}"
        );
    }
    show(&app, "first", visual);
    visual.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.hide_plugin_panel("region-fixture", "first", cx);
            app.hide_plugin_panel("region-fixture", "second", cx);
            app.sync_plugin_panels(window, cx);
        })
    });
    paint(visual);
    assert!(visual.debug_bounds("plugin-ui-content-shared").is_some());
    assert_eq!(
        first,
        visual.update(|_, cx| app.read(cx).plugin_panels["region-fixture/first"].clone())
    );

    // Saved selection survives a real Base layout reload and a fresh presentation policy.
    show(&app, "first", visual);
    let saved_session = visual.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.capture_dock_layout(cx);
            app.pending_dock_restore = true;
            app.restore_dock_layout(window, cx);
            app.sync_plugin_panels(window, cx);
            app.session_state.clone()
        })
    });
    paint(visual);
    assert!(visual.debug_bounds("plugin-ui-content-first").is_some());
    assert!(visual.debug_bounds("plugin-ui-content-shared").is_none());
    // Disabling the owner removes the exclusive leaf and immediately restores host peers.
    let mut removed = manifest.clone();
    removed.panels.clear();
    publish(&app, &removed, visual);
    native_panel_bounds(&app, explorer, visual);
    native_panel_bounds(&app, outline, visual);

    // A new editor has new entity IDs and a fresh display policy; only persisted names may restore selection.
    let capture = slot.clone();
    let workspace = Workspace::open(&workspace_path).unwrap();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let app = cx.new(|cx| {
            EditorApp::new_with_session(workspace, None, Some(saved_session), window, cx)
        });
        *capture.borrow_mut() = Some(app.clone());
        Root::new(app, window, cx)
    });
    let reopened = slot.borrow_mut().take().unwrap();
    visual.simulate_resize(size(px(1280.), px(900.)));
    publish(&reopened, &manifest, visual);
    assert!(visual.debug_bounds("plugin-ui-content-first").is_some());
    assert!(visual.debug_bounds("plugin-ui-content-shared").is_none());

    // Old saved trees have no exclusive leaves. Their first two reveals must still preserve peer sizes.
    let capture = slot.clone();
    let workspace = Workspace::open(&workspace_path).unwrap();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let app = cx.new(|cx| {
            EditorApp::new_with_session(workspace, None, Some(older_session), window, cx)
        });
        *capture.borrow_mut() = Some(app.clone());
        Root::new(app, window, cx)
    });
    let restored = slot.borrow_mut().take().unwrap();
    visual.simulate_resize(size(px(1280.), px(900.)));
    publish(&restored, &manifest, visual);
    let (shared, explorer, first, other) = visual.update(|_, cx| {
        let app = restored.read(cx);
        (
            PanelId::from(app.plugin_panels["region-fixture/shared"].entity_id()),
            PanelId::from(app.explorer_panel.entity_id()),
            PanelId::from(app.plugin_panels["region-fixture/first"].entity_id()),
            PanelId::from(app.plugin_panels["region-fixture/other"].entity_id()),
        )
    });
    let before = native_panel_bounds(&restored, shared, visual);
    show(&restored, "first", visual);
    show(&restored, "second", visual);
    show(&restored, "shared", visual);
    let after = native_panel_bounds(&restored, shared, visual);
    assert!(
        (before.size.width - after.size.width).abs() < px(2.),
        "legacy peer width: {before:?} -> {after:?}"
    );
    assert!(
        (before.size.height - after.size.height).abs() < px(2.),
        "legacy peer height: {before:?} -> {after:?}"
    );
    if position == "left" {
        show(&restored, "first", visual);
        visual.update(|window, cx| {
            restored.update(cx, |app, cx| {
                app.dock_area.update(cx, |area, cx| {
                    let node = area
                        .layout(DockPlacement::Bottom)
                        .unwrap()
                        .find_panel_node(other)
                        .unwrap();
                    area.move_panel(
                        first,
                        gpui_base::dock::InsertTarget::Split {
                            node,
                            placement: gpui_base::Placement::Right,
                            size: None,
                        },
                        window,
                        cx,
                    );
                });
                app.sync_plugin_panels(window, cx);
                assert!(!app.dock_presentation.is_suppressed(explorer));
                assert!(
                    app.dock_presentation.is_suppressed(other),
                    "exclusivity must follow the actual bottom placement"
                );
            })
        });
        native_panel_bounds(&restored, first, visual);
        native_panel_bounds(&restored, explorer, visual);
        assert!(visual.debug_bounds("plugin-ui-content-other").is_none());
        // A retained but entirely hidden sidebar must also reopen through a plugin's
        // actual footer action, without reattaching or recreating its exclusive leaf.
        visual.update(|_, cx| {
            restored.update(cx, |app, cx| {
                app.hide_plugin_panel("region-fixture", "shared", cx);
                app.hide_plugin_panel("region-fixture", "second", cx);
            });
        });
        for selector in ["explorer-panel-toggle", "outline-toggle"] {
            let button = visual.debug_bounds(selector).unwrap();
            visual.simulate_click(button.center(), Default::default());
            paint(visual);
        }
        visual.update(|_, cx| {
            assert!(
                !restored
                    .read(cx)
                    .dock_area
                    .read(cx)
                    .is_dock_open(DockPlacement::Left)
            );
        });
        let button = visual
            .debug_bounds("plugin-window-region-fixture/second")
            .unwrap();
        visual.simulate_click(button.center(), Default::default());
        paint(visual);
        let second = visual.update(|_, cx| {
            PanelId::from(restored.read(cx).plugin_panels["region-fixture/second"].entity_id())
        });
        let reopened = native_panel_bounds(&restored, second, visual);
        assert!((reopened.size.width - before.size.width).abs() < px(2.));
        assert!(visual.debug_bounds("plugin-ui-content-second").is_some());
    }
}

#[gpui::test]
fn exclusive_left_replaces_explorer_outline_and_restores_combination(cx: &mut TestAppContext) {
    exercise_region(cx, "left", true);
}

#[gpui::test]
fn exclusive_right_replaces_host_and_plugin_peers(cx: &mut TestAppContext) {
    exercise_region(cx, "right", false);
}

#[gpui::test]
fn exclusive_bottom_replaces_only_bottom_peers(cx: &mut TestAppContext) {
    exercise_region(cx, "bottom", true);
}
