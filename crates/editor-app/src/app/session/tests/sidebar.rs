//! Legacy outer-left panels migrate once after contributions bind, preserving customized native peers.
use super::*;
use gpui_kit::{Bounds, Pixels};

const DEFAULT_PANEL: &str = "sidebar-session/default";
const CUSTOM_PANEL: &str = "sidebar-session/custom-right";
const SIDEBAR_VERSION: &str = "plugin_sidebar_layout_version";

/// Without a saved tree there is nothing to migrate; new user docking must never be marked legacy.
#[test]
fn legacy_session_without_saved_dock_starts_with_current_sidebar_layout() {
    let directory = tempfile::tempdir().unwrap();
    let workspace = directory.path().join("workspace");
    let profile = directory.path().join("profile");
    fs::create_dir_all(&workspace).unwrap();
    fs::create_dir_all(&profile).unwrap();
    let mut legacy = serde_json::to_value(SessionState::for_workspace(&workspace)).unwrap();
    legacy.as_object_mut().unwrap().remove(SIDEBAR_VERSION);
    legacy["dock_layout"] = serde_json::Value::Null;
    fs::write(
        SessionState::file_in_profile(&workspace, &profile),
        serde_json::to_vec_pretty(&legacy).unwrap(),
    )
    .unwrap();
    let loaded = SessionState::load_in_profile(&workspace, &profile);
    assert!(loaded.dock_layout.is_none());
    assert_eq!(
        serde_json::to_value(loaded).unwrap()[SIDEBAR_VERSION].as_u64(),
        Some(1),
        "a legacy session without a saved dock must use the current default before any new user move"
    );
    legacy[SIDEBAR_VERSION] = serde_json::json!(2);
    fs::write(
        SessionState::file_in_profile(&workspace, &profile),
        serde_json::to_vec_pretty(&legacy).unwrap(),
    )
    .unwrap();
    let future = SessionState::load_in_profile(&workspace, &profile);
    assert_eq!(
        serde_json::to_value(future).unwrap()[SIDEBAR_VERSION].as_u64(),
        Some(2),
        "loading the default layout must not lower a future marker"
    );
}

/// A delayed old session moves only a still-left default contribution, retaining its existing native entity.
#[gpui::test]
fn legacy_left_plugin_session_migrates_into_shared_sidebar_after_startup(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        typography::init(cx);
        crate::theme::apply_theme(crate::theme::builtin_theme(false), cx);
        cx.set_reduce_motion(true);
    });
    let directory = tempfile::tempdir().unwrap();
    let workspace = Workspace::open(directory.path()).unwrap();
    let _session = SessionFile(
        SessionState::for_workspace(workspace.root())
            .file_path()
            .unwrap(),
    );
    let installed = sidebar_contributions();
    let registry = workspace.root().join(".runtime-plugin-test");
    fs::create_dir_all(&registry).unwrap();
    fs::write(
        registry.join("registry.json"),
        serde_json::to_vec(&std::collections::BTreeMap::from([(
            installed.manifest.id.clone(),
            installed.clone(),
        )]))
        .unwrap(),
    )
    .unwrap();
    let previous = save_legacy_sidebar(workspace.clone(), &installed, cx);

    let slot = Rc::new(RefCell::new(None));
    let capture = slot.clone();
    let reopening = workspace.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let app = cx.new(|cx| EditorApp::new(reopening, None, window, cx));
        *capture.borrow_mut() = Some(app.clone());
        Root::new(app, window, cx)
    });
    let restored = slot.borrow_mut().take().unwrap();
    visual.simulate_resize(size(px(1280.), px(820.)));
    let original_entities = visual.update(|window, cx| {
        restored.update(cx, |app, cx| {
            assert!(
                app.pending_dock_restore,
                "the legacy tree must wait for plugin startup"
            );
            // Bind the actual contributions while their ordinary startup remains pending. Loading
            // the saved tree and consuming its marker before this barrier would lose native views.
            app.extensions.update(cx, |owner, _| {
                assert!(
                    !owner.startup.is_empty(),
                    "fixture must exercise delayed startup"
                );
                owner.entries = vec![installed.clone()];
            });
            app.sync_plugin_panels(window, cx);
            assert!(app.pending_dock_restore);
            let version = serde_json::to_value(&app.session_state).unwrap();
            assert_eq!(version[SIDEBAR_VERSION].as_u64().unwrap_or(0), 0);
            [
                app.plugin_panels[DEFAULT_PANEL].entity_id(),
                app.plugin_panels[CUSTOM_PANEL].entity_id(),
            ]
        })
    });
    // The existing publication seam completes startup and lets the real EditorApp restore once.
    publish_panels(&restored, &installed, visual);
    visual.run_until_parked();
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
        let app = restored.read(cx);
        assert!(!app.pending_dock_restore);
        let area = app.dock_area.read(cx);
        let default = PanelId::from(app.plugin_panels[DEFAULT_PANEL].entity_id());
        let customized = PanelId::from(app.plugin_panels[CUSTOM_PANEL].entity_id());
        assert!(
            area.layout(DockPlacement::Center)
                .unwrap()
                .find_panel_node(default)
                .is_some(),
            "the still-default legacy left panel must migrate into the existing shared sidebar"
        );
        assert!(
            area.layout(DockPlacement::Right)
                .unwrap()
                .find_panel_node(customized)
                .is_some(),
            "a manifest left hint must not undo the user's saved right placement"
        );
        assert!(
            area.layout(DockPlacement::Left)
                .is_none_or(|tree| tree.find_panel_node(default).is_none())
        );
        assert_eq!(
            [
                app.plugin_panels[DEFAULT_PANEL].entity_id(),
                app.plugin_panels[CUSTOM_PANEL].entity_id(),
            ],
            original_entities,
            "migration must move registered views rather than recreate them"
        );
        for key in [DEFAULT_PANEL, CUSTOM_PANEL] {
            assert_eq!(
                app.session_state.plugin_panel_visibility.get(key),
                Some(&true)
            );
            assert!(
                area.panel(PanelId::from(app.plugin_panels[key].entity_id()))
                    .is_some()
            );
        }
        let actual = area.dump(cx);
        assert_center_split_preference(&actual, &previous);
        let old_right = previous.right_dock.as_ref().unwrap();
        let new_right = actual.right_dock.as_ref().unwrap();
        assert!((new_right.size() - old_right.size()).abs() < px(2.));
        assert_eq!(new_right.open(), old_right.open());
        let current = serde_json::to_value(&app.session_state).unwrap();
        assert_eq!(
            current[SIDEBAR_VERSION].as_u64(),
            Some(1),
            "only a completed native layout migration may consume the once-only marker"
        );
        let saved: serde_json::Value =
            serde_json::from_slice(&fs::read(app.session_state.file_path().unwrap()).unwrap())
                .unwrap();
        assert_eq!(
            saved[SIDEBAR_VERSION].as_u64(),
            Some(1),
            "completed migration must be persisted"
        );
    });
    assert_shared_sidebar_geometry(&restored, visual);
    let customized = save_current_left_customization(&restored, visual);
    assert_current_layout_survives_restart(workspace, &installed, customized, cx);
}

/// Both hints were left; the saved position, rather than the manifest hint alone, distinguishes customization.
fn sidebar_contributions() -> Installed {
    let mut manifest: Manifest = crate::extensions::test_manifest(include_str!(
        "../../../extensions/fixtures/panel-contract.json"
    ));
    manifest.id = "sidebar-session".into();
    manifest.panels.truncate(1);
    manifest.panels[0].id = "default".into();
    manifest.panels[0].position = "left".into();
    manifest.panels[0].default_visible = true;
    let mut customized = manifest.panels[0].clone();
    customized.id = "custom-right".into();
    manifest.panels.push(customized);
    Installed {
        replaced_by: None,
        grants: manifest.permissions.clone(),
        manifest,
        digest: "sidebar-session-fixture".into(),
        enabled: true,
        project_enabled: Default::default(),
        global_enabled: None,
        retired_ui_contract: false,
        error: None,
    }
}

/// Build and serialize the old two-column arrangement through the real DockArea, omitting the new JSON marker.
fn save_legacy_sidebar(
    workspace: Workspace,
    installed: &Installed,
    cx: &mut TestAppContext,
) -> DockAreaState {
    let slot = Rc::new(RefCell::new(None));
    let capture = slot.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let app = cx.new(|cx| EditorApp::new(workspace, None, window, cx));
        *capture.borrow_mut() = Some(app.clone());
        Root::new(app, window, cx)
    });
    let app = slot.borrow_mut().take().unwrap();
    publish_panels(&app, installed, visual);
    visual.simulate_resize(size(px(1280.), px(820.)));
    visual.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.session_state.explorer_visible = true;
            app.session_state.outline_visible = true;
            let explorer = crate::dock::panel_handle(app.explorer_panel.clone());
            let outline = crate::dock::panel_handle(app.outline_panel.clone());
            let editor = crate::dock::panel_handle(app.editor_panel.clone());
            let default = crate::dock::panel_handle(app.plugin_panels[DEFAULT_PANEL].clone());
            let customized = crate::dock::panel_handle(app.plugin_panels[CUSTOM_PANEL].clone());
            app.dock_area.update(cx, |area, cx| {
                area.set_center(
                    DockLayout::h_split()
                        .child(
                            DockLayout::v_split()
                                .child(DockLayout::tabs().panel_view(explorer, cx), None)
                                .child(DockLayout::tabs().panel_view(outline, cx), None),
                            Some(px(260.)),
                        )
                        .child(DockLayout::tabs().panel_view(editor, cx), None),
                    window,
                    cx,
                );
                area.set_dock(
                    DockPlacement::Left,
                    DockLayout::tabs().panel_view(default, cx),
                    window,
                    cx,
                );
                area.set_dock_size(DockPlacement::Left, px(245.), window, cx);
                area.set_dock(
                    DockPlacement::Right,
                    DockLayout::tabs().panel_view(customized, cx),
                    window,
                    cx,
                );
                area.set_dock_size(DockPlacement::Right, px(317.), window, cx);
            });
        });
    });
    visual.run_until_parked();
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
        app.update(cx, |app, cx| {
            app.capture_dock_layout(cx);
            let layout = app.dock_area.read(cx).dump(cx);
            assert!(layout.left_dock.is_some() && layout.right_dock.is_some());
            let mut legacy = serde_json::to_value(&app.session_state).unwrap();
            legacy.as_object_mut().unwrap().remove(SIDEBAR_VERSION);
            assert!(legacy.get(SIDEBAR_VERSION).is_none());
            let path = app.session_state.file_path().unwrap();
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, serde_json::to_vec_pretty(&legacy).unwrap()).unwrap();
            layout
        })
    })
}

/// Measure native rendered slots after normalization; no second mutable layout model stands in for Base.
fn assert_shared_sidebar_geometry(app: &Entity<EditorApp>, visual: &mut VisualTestContext) {
    let ids = visual.update(|_, cx| {
        let app = app.read(cx);
        [
            PanelId::from(app.explorer_panel.entity_id()),
            PanelId::from(app.outline_panel.entity_id()),
            PanelId::from(app.plugin_panels[DEFAULT_PANEL].entity_id()),
        ]
    });
    let [explorer, outline, plugin] = ids.map(|id| sidebar_frame(app, id, visual));
    for bounds in [explorer, outline, plugin] {
        assert!(bounds.size.width > px(0.) && bounds.size.height > px(0.));
        assert!((bounds.left() - explorer.left()).abs() < px(2.));
        assert!((bounds.size.width - explorer.size.width).abs() < px(2.));
    }
    let editor = visual.update(|_, cx| PanelId::from(app.read(cx).editor_panel.entity_id()));
    let editor = sidebar_frame(app, editor, visual);
    assert!(
        plugin.right() <= editor.left() + px(2.),
        "the editor must remain beside the shared sidebar"
    );
}

/// Removing the outer dock changes available pixels; Base retains its native center split preferences.
fn assert_center_split_preference(actual: &DockAreaState, previous: &DockAreaState) {
    assert_eq!(previous.center.info.axis(), Some(Axis::Horizontal));
    assert_eq!(actual.center.info.axis(), previous.center.info.axis());
    let previous = previous.center.info.sizes().unwrap();
    let actual = actual.center.info.sizes().unwrap();
    assert_eq!(previous.len(), 2);
    assert_eq!(actual.len(), previous.len());
    let fraction = |sizes: &[Pixels]| {
        let total = sizes.iter().copied().map(f32::from).sum::<f32>();
        assert!(total > 0.);
        f32::from(sizes[0]) / total
    };
    assert!(
        (fraction(actual) - fraction(previous)).abs() < 0.01,
        "migration must preserve Base's center split preference, not two incompatible pixel widths: {previous:?} -> {actual:?}"
    );
}

/// A completed migration makes a later explicit outer-left placement ordinary current user state.
fn save_current_left_customization(
    app: &Entity<EditorApp>,
    visual: &mut VisualTestContext,
) -> DockAreaState {
    visual.update(|window, cx| {
        app.update(cx, |app, cx| {
            assert_eq!(
                serde_json::to_value(&app.session_state).unwrap()[SIDEBAR_VERSION].as_u64(),
                Some(1)
            );
            let panel = PanelId::from(app.plugin_panels[DEFAULT_PANEL].entity_id());
            app.dock_area.update(cx, |area, cx| {
                // Seed only an empty outer region; the real public move transfers the existing
                // view out of Center, with Base owning detachment, normalization and identity.
                area.set_dock(DockPlacement::Left, DockLayout::tabs(), window, cx);
                let node = area.layout(DockPlacement::Left).unwrap().root().id();
                area.move_panel(
                    panel,
                    InsertTarget::Split {
                        node,
                        placement: Placement::Bottom,
                        size: None,
                    },
                    window,
                    cx,
                );
                area.set_dock_size(DockPlacement::Left, px(231.), window, cx);
                assert!(
                    area.layout(DockPlacement::Left)
                        .unwrap()
                        .find_panel_node(panel)
                        .is_some()
                );
            });
        });
    });
    visual.run_until_parked();
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
        app.update(cx, |app, cx| {
            app.capture_dock_layout(cx);
            app.persist_session();
            app.dock_area.read(cx).dump(cx)
        })
    })
}

/// A normal second startup must load the completed layout verbatim instead of repeating migration.
fn assert_current_layout_survives_restart(
    workspace: Workspace,
    installed: &Installed,
    expected: DockAreaState,
    cx: &mut TestAppContext,
) {
    let slot = Rc::new(RefCell::new(None));
    let capture = slot.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let app = cx.new(|cx| EditorApp::new(workspace, None, window, cx));
        *capture.borrow_mut() = Some(app.clone());
        Root::new(app, window, cx)
    });
    let app = slot.borrow_mut().take().unwrap();
    visual.simulate_resize(size(px(1280.), px(820.)));
    publish_panels(&app, installed, visual);
    visual.run_until_parked();
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
        let app = app.read(cx);
        assert!(!app.pending_dock_restore);
        assert_eq!(
            serde_json::to_value(&app.session_state).unwrap()[SIDEBAR_VERSION].as_u64(),
            Some(1)
        );
        let area = app.dock_area.read(cx);
        let panel = PanelId::from(app.plugin_panels[DEFAULT_PANEL].entity_id());
        assert!(
            area.layout(DockPlacement::Left)
                .unwrap()
                .find_panel_node(panel)
                .is_some(),
            "a new user move to Left must not be migrated a second time"
        );
        let right = PanelId::from(app.plugin_panels[CUSTOM_PANEL].entity_id());
        assert!(
            area.layout(DockPlacement::Right)
                .unwrap()
                .find_panel_node(right)
                .is_some()
        );
        for key in [DEFAULT_PANEL, CUSTOM_PANEL] {
            assert_eq!(
                app.session_state.plugin_panel_visibility.get(key),
                Some(&true)
            );
        }
        assert_layout_eq(area.dump(cx), expected);
    });
}

/// Resolve each live leaf anew because a completed Base move can normalize earlier node identities.
fn sidebar_frame(
    app: &Entity<EditorApp>,
    panel: PanelId,
    visual: &mut VisualTestContext,
) -> Bounds<Pixels> {
    visual.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear(cx));
    let node = visual.update(|_, cx| {
        let area = app.read(cx).dock_area.read(cx);
        [
            DockPlacement::Center,
            DockPlacement::Left,
            DockPlacement::Right,
            DockPlacement::Bottom,
        ]
        .into_iter()
        .find_map(|placement| area.layout(placement)?.find_panel_node(panel))
        .unwrap()
    });
    let selector = format!("local-dock-content-{}", node.as_u64());
    visual
        .debug_bounds(Box::leak(selector.into_boxed_str()))
        .unwrap()
}
