//! Native dock display state is distinct from persisted show/hide intent and cold activation.
use super::*;
use gpui_base::dock::{DockPlacement, InsertTarget, PanelId};
use gpui_kit::{TestAppContext, VisualTestContext, gpui};

/// Static declarations can exercise native placement without executing any guest code.
fn publish_panels(app: &Entity<EditorApp>, panels: serde_json::Value, cx: &mut App) {
    let manifest: protocol::Manifest = serde_json::from_value(serde_json::json!({
        "id":"visibility.fixture","name":"Visibility","version":"1.0.0","protocol":7,
        "permissions":["ui.panels"],"storage_limit":1024,
        "api":{"base":"^1","required":{"ui.native":">=1.1,<2","ui.windows":"^1","ui.visibility":"^1"}},
        "panels":panels
    })).unwrap();
    let owner = app.read(cx).extensions.clone();
    owner.update(cx, |owner, cx| {
        let mut state = owner.worker.state.lock().unwrap();
        state.entries = vec![Installed {
            grants: manifest.permissions.clone(),
            manifest,
            digest: "visibility".into(),
            enabled: true,
            project_enabled: Default::default(),
            replaced_by: None,
            global_enabled: None,
            retired_ui_contract: false,
            error: None,
        }];
        drop(state);
        owner.poll(cx);
    });
}

/// Collapsing the containing dock and selecting a peer must stop visibility, even when the user's
/// panel intent stays true. A hidden active slot must instead report Base's displayed fallback.
#[gpui::test]
fn panel_visibility_tracks_native_dock_collapse_and_hidden_active_fallback(
    cx: &mut TestAppContext,
) {
    crate::tests::with_shortcut_editor(cx, false, Vec::new(), |visual, app, _| {
        visual.update(|window, cx| {
            publish_panels(
                &app,
                serde_json::json!([
                    {"id":"first","title":"First","position":"bottom","default_visible":true},
                    {"id":"second","title":"Second","position":"bottom","default_visible":true}
                ]),
                cx,
            );
            app.update(cx, |app, cx| {
                app.sync_plugin_panels(window, cx);
                let first = app.plugin_panels["visibility.fixture/first"].clone();
                let second = app.plugin_panels["visibility.fixture/second"].clone();
                app.dock_area.update(cx, |area, cx| {
                    // Nanobug initially places panels in separate split leaves. Explicitly merge
                    // this fixture's peers before checking the selected-tab and fallback cases.
                    let node = area
                        .layout(DockPlacement::Bottom)
                        .unwrap()
                        .find_panel_node(PanelId::from(first.entity_id()))
                        .unwrap();
                    area.move_panel(
                        PanelId::from(second.entity_id()),
                        InsertTarget::Tabs {
                            node,
                            ix: None,
                            activate: false,
                        },
                        window,
                        cx,
                    );
                    area.select_panel(PanelId::from(first.entity_id()), window, cx);
                });
                app.sync_plugin_panel_visibility(window, cx);
                assert!(first.read(cx).panel_visibility.displayed);
                assert!(
                    !app.plugin_panels["visibility.fixture/second"]
                        .read(cx)
                        .panel_visibility
                        .displayed
                );
                app.dock_area.update(cx, |area, cx| {
                    area.set_dock_collapsible(DockPlacement::Bottom, true, window, cx);
                    area.toggle_dock(DockPlacement::Bottom, window, cx);
                });
                app.sync_plugin_panel_visibility(window, cx);
                assert!(
                    first.read(cx).visible.get(),
                    "collapsing does not rewrite the user's panel choice"
                );
                assert!(!first.read(cx).panel_visibility.displayed);
                app.dock_area.update(cx, |area, cx| {
                    area.toggle_dock(DockPlacement::Bottom, window, cx);
                });
                first.read(cx).visible.set(false);
                app.sync_plugin_panel_visibility(window, cx);
                assert!(!first.read(cx).panel_visibility.displayed);
                assert!(
                    app.plugin_panels["visibility.fixture/second"]
                        .read(cx)
                        .panel_visibility
                        .displayed
                );
                let worker = app.extensions.read(cx).worker.clone();
                assert!(
                    !worker
                        .recorded
                        .lock()
                        .unwrap()
                        .try_iter()
                        .any(|work| matches!(
                            work,
                            Work::Event(_, _, _, PluginEvent::PanelVisibility { .. })
                        )),
                    "sampling must not emit callbacks to a cold owner"
                );
            });
        });
    });
}

/// Native work windows participate independently from the main dock and stop after close/hide.
#[gpui::test]
fn panel_visibility_tracks_work_window_hide_without_demanding_a_guest(cx: &mut TestAppContext) {
    crate::tests::with_shortcut_editor(cx, false, Vec::new(), |visual, app, _| {
        let (handle, panel) = visual.update(|window, cx| {
            publish_panels(
                &app,
                serde_json::json!([
                    {"id":"separate","title":"Separate","position":"window","default_visible":true}
                ]),
                cx,
            );
            app.update(cx, |app, cx| {
                app.sync_plugin_panels(window, cx);
                let panel = app.plugin_panels["visibility.fixture/separate"].clone();
                assert!(
                    app.plugin_windows
                        .contains_key("visibility.fixture/separate")
                );
                assert!(panel.read(cx).panel_visibility.displayed);
                (app.plugin_windows["visibility.fixture/separate"], panel)
            })
        });
        let mut separate = VisualTestContext::from_window(handle.into(), &visual.cx);
        assert!(
            separate.simulate_close(),
            "the actual native close gate accepts hiding"
        );
        visual.update(|window, cx| {
            app.update(cx, |app, cx| {
                app.sync_plugin_panel_visibility(window, cx);
                assert!(!panel.read(cx).visible.get());
                assert!(!panel.read(cx).panel_visibility.displayed);
                assert!(
                    !app.plugin_windows
                        .contains_key("visibility.fixture/separate")
                );
                // Explicitly show a fresh native surface, then cover ordinary programmatic hide.
                panel.read(cx).visible.set(true);
                app.sync_plugin_windows(cx);
                app.sync_plugin_panel_visibility(window, cx);
                assert!(panel.read(cx).panel_visibility.displayed);
                panel.read(cx).visible.set(false);
                app.sync_plugin_windows(cx);
                app.sync_plugin_panel_visibility(window, cx);
                assert!(!panel.read(cx).panel_visibility.displayed);
                assert!(
                    !app.plugin_windows
                        .contains_key("visibility.fixture/separate")
                );
            });
        });
    });
}
