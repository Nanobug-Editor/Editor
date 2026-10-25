//! Native Open Files visibility and docking preserve the existing document and session owners.

use super::*;
use gpui_base::dock::{DockLayout, DockPlacement, InsertTarget, PanelId};
use gpui_kit::{TestAppContext, VisualTestContext, gpui, size};

/// Fresh and legacy sessions opt into the tool window; an explicitly hidden choice survives serde.
#[test]
fn open_files_visibility_defaults_visible_and_preserves_saved_choice() {
    let directory = tempfile::tempdir().unwrap();
    let state = SessionState::for_workspace(directory.path());
    assert!(state.open_files_visible);
    let mut legacy = serde_json::to_value(&state).unwrap();
    legacy.as_object_mut().unwrap().remove("open_files_visible");
    let mut migrated: SessionState = serde_json::from_value(legacy).unwrap();
    assert!(migrated.open_files_visible);
    migrated.open_files_visible = false;
    let restored: SessionState =
        serde_json::from_value(serde_json::to_value(migrated).unwrap()).unwrap();
    assert!(!restored.open_files_visible);
}

/// Keep the project and profile alive while a real editor owns their file and dock sessions.
struct Fixture {
    /// The watched source file must outlive its native editor session.
    _directory: tempfile::TempDir,
    /// Every preference and session write stays inside this test's profile.
    profile: tempfile::TempDir,
    /// Canonical root resolves the same persisted workspace record used at startup.
    workspace: Workspace,
    /// Sole owner of the documents and live Base dock layout.
    app: Entity<EditorApp>,
}

/// Mount one real source in ToolWindow mode in `cx`, with isolated global and workspace preferences.
fn mount(cx: &mut TestAppContext) -> (Fixture, &mut VisualTestContext) {
    let directory = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("source.rs"), "fn main() {}\n").unwrap();
    let workspace = Workspace::open(directory.path()).unwrap();
    cx.update(|cx| {
        gpui_kit::init(cx);
        typography::init(cx);
        apply_theme(builtin_theme(false), cx);
        crate::app::editor_preferences::set_placement(profile.path(), TabPlacement::ToolWindow, cx)
            .unwrap();
    });
    let session = SessionState::load_in_profile(workspace.root(), profile.path());
    let opened = workspace.clone();
    let initial = workspace.root().join("source.rs");
    let mut application = None;
    let (_, ui) = cx.add_window_view(|window, cx| {
        let app = cx.new(|cx| {
            EditorApp::new_with_session(opened, Some(initial), Some(session), window, cx)
        });
        application = Some(app.clone());
        Root::new(app, window, cx)
    });
    ui.simulate_resize(size(px(1100.), px(700.)));
    draw(ui);
    (
        Fixture {
            _directory: directory,
            profile,
            workspace,
            app: application.unwrap(),
        },
        ui,
    )
}

/// Drain subscriptions and paint `ui` so dock visibility and cached panel rendering are observable.
fn draw(ui: &mut VisualTestContext) {
    ui.run_until_parked();
    ui.update(|window, cx| window.draw(cx).clear(cx));
}

/// Hidden Open Files retains a bottom placement, documents and identity through reopening and save.
#[gpui::test]
fn open_files_hide_keeps_documents_and_moved_dock_slot(cx: &mut TestAppContext) {
    let (fixture, ui) = mount(cx);
    let original = ui.update(|window, cx| {
        fixture.app.update(cx, |app, cx| {
            let id = PanelId::from(app.open_files_panel.entity_id());
            assert!(local_dock::panel_visible(app.dock_area.read(cx), id, cx));
            app.ensure_problems_dock(window, cx);
            app.dock_area.update(cx, |area, cx| {
                // The new default keeps Problems above Bottom, so establish a native target
                // before exercising Open Files in a user-created bottom region.
                if !area.has_dock(DockPlacement::Bottom) {
                    area.set_dock(DockPlacement::Bottom, DockLayout::tabs(), window, cx);
                }
                let node = area.layout(DockPlacement::Bottom).unwrap().root().id();
                area.move_panel(
                    id,
                    InsertTarget::Split {
                        node,
                        placement: gpui_base::Placement::Right,
                        size: Some(px(260.)),
                    },
                    window,
                    cx,
                );
            });
            (
                app.tabs[0].file_id,
                app.editor.clone(),
                app.active_path.clone(),
            )
        })
    });
    draw(ui);
    ui.update(|window, cx| {
        fixture.app.update(cx, |app, cx| {
            app.toggle_open_files(window, cx);
            app.sync_plugin_panels(window, cx);
        });
    });
    draw(ui);
    ui.update(|_, cx| {
        let app = fixture.app.read(cx);
        let id = PanelId::from(app.open_files_panel.entity_id());
        assert!(!app.session_state.open_files_visible);
        assert!(!local_dock::panel_visible(app.dock_area.read(cx), id, cx));
        assert!(
            app.dock_area
                .read(cx)
                .layout(DockPlacement::Bottom)
                .unwrap()
                .find_panel_node(id)
                .is_some()
        );
        assert_eq!(app.tabs.len(), 1);
        assert_eq!(app.tabs[0].file_id, original.0);
        assert_eq!(app.editor, original.1);
        assert_eq!(app.active_path, original.2);
    });
    let saved = SessionState::load_in_profile(fixture.workspace.root(), fixture.profile.path());
    assert!(!saved.open_files_visible);
    assert_eq!(saved.open_tabs.len(), 1);
    ui.update(|window, cx| {
        fixture
            .app
            .update(cx, |app, cx| app.toggle_open_files(window, cx));
    });
    draw(ui);
    ui.update(|_, cx| {
        let app = fixture.app.read(cx);
        let id = PanelId::from(app.open_files_panel.entity_id());
        assert!(local_dock::panel_visible(app.dock_area.read(cx), id, cx));
        assert!(
            app.dock_area
                .read(cx)
                .layout(DockPlacement::Bottom)
                .unwrap()
                .find_panel_node(id)
                .is_some()
        );
        assert_eq!(app.tabs[0].file_id, original.0);
        assert_eq!(app.editor, original.1);
    });
}

/// Move host peers into an explicit outer right dock to exercise saved custom placements.
fn move_open_files_to_native_right(
    app: &mut EditorApp,
    window: &mut Window,
    cx: &mut Context<EditorApp>,
) {
    let messages = PanelId::from(app.messages.entity_id());
    let open_files = PanelId::from(app.open_files_panel.entity_id());
    app.dock_area.update(cx, |area, cx| {
        area.set_dock(DockPlacement::Right, DockLayout::tabs(), window, cx);
        let node = area.layout(DockPlacement::Right).unwrap().root().id();
        area.move_panel(
            messages,
            InsertTarget::Split {
                node,
                placement: gpui_base::Placement::Right,
                size: None,
            },
            window,
            cx,
        );
        let node = area.layout(DockPlacement::Right).unwrap().root().id();
        area.move_panel(
            open_files,
            InsertTarget::Split {
                node,
                placement: gpui_base::Placement::Bottom,
                size: Some(px(240.)),
            },
            window,
            cx,
        );
    });
}

/// Save a moved panel while inline tabs close its otherwise empty right dock.
fn saved_closed_right(fixture: &Fixture, ui: &mut VisualTestContext) -> SessionState {
    ui.update(|window, cx| {
        fixture.app.update(cx, |app, cx| {
            move_open_files_to_native_right(app, window, cx);
        });
    });
    draw(ui);
    ui.update(|window, cx| {
        fixture.app.update(cx, |app, cx| {
            app.apply_tab_placement(TabPlacement::Top, window, cx);
            app.sync_plugin_panels(window, cx);
        });
    });
    draw(ui);
    ui.update(|_, cx| {
        fixture.app.update(cx, |app, cx| {
            assert!(app.session_state.open_files_visible);
            assert!(!app.dock_area.read(cx).is_dock_open(DockPlacement::Right));
            app.capture_dock_layout(cx);
            app.persist_session();
        });
    });
    SessionState::load_in_profile(fixture.workspace.root(), fixture.profile.path())
}

/// A separate production constructor reads global placement while retaining this workspace's layout.
fn reopen(
    fixture: &Fixture,
    ui: &mut VisualTestContext,
    saved: SessionState,
    placement: TabPlacement,
) -> (Entity<EditorApp>, VisualTestContext) {
    let workspace = fixture.workspace.clone();
    let mut application = None;
    let handle = ui.update(|_, cx| {
        crate::app::editor_preferences::set_placement(fixture.profile.path(), placement, cx)
            .unwrap();
        cx.open_window(Default::default(), |window, cx| {
            let app =
                cx.new(|cx| EditorApp::new_with_session(workspace, None, Some(saved), window, cx));
            application = Some(app.clone());
            cx.new(|cx| Root::new(app, window, cx))
        })
        .unwrap()
    });
    let mut reopened = VisualTestContext::from_window(handle.into(), &ui.cx);
    reopened.simulate_resize(size(px(1100.), px(700.)));
    draw(&mut reopened);
    (application.unwrap(), reopened)
}

/// Global ToolWindow mode reopens an inline-saved closed region at its existing position and width.
#[gpui::test]
fn open_files_restore_reopens_saved_region_for_global_tool_window(cx: &mut TestAppContext) {
    let (fixture, ui) = mount(cx);
    let saved = saved_closed_right(&fixture, ui);
    let width = saved
        .dock_layout
        .as_ref()
        .unwrap()
        .right_dock
        .as_ref()
        .unwrap()
        .size();
    let (app, mut reopened) = reopen(&fixture, ui, saved, TabPlacement::ToolWindow);
    assert!(reopened.debug_bounds("open-files-panel").is_some());
    reopened.update(|_, cx| {
        let app = app.read(cx);
        let id = PanelId::from(app.open_files_panel.entity_id());
        let area = app.dock_area.read(cx);
        assert_eq!(app.tab_placement, TabPlacement::ToolWindow);
        assert!(app.session_state.open_files_visible);
        assert!(area.is_dock_open(DockPlacement::Right));
        assert!(
            area.layout(DockPlacement::Right)
                .unwrap()
                .find_panel_node(id)
                .is_some()
        );
        assert!(local_dock::panel_visible(area, id, cx));
        assert_eq!(app.dock_ui.dump(area, cx).right_dock.unwrap().size(), width);
        assert_eq!(app.tabs.len(), 1);
    });
}

/// Inline startup and an explicit workspace hide both retain the saved closed region.
#[gpui::test]
fn open_files_restore_respects_inline_mode_and_saved_hide(cx: &mut TestAppContext) {
    let (fixture, ui) = mount(cx);
    let saved = saved_closed_right(&fixture, ui);
    for (placement, visible) in [(TabPlacement::Top, true), (TabPlacement::ToolWindow, false)] {
        let mut state = saved.clone();
        state.open_files_visible = visible;
        let (app, mut reopened) = reopen(&fixture, ui, state, placement);
        assert!(reopened.debug_bounds("open-files-panel").is_none());
        reopened.update(|_, cx| {
            let app = app.read(cx);
            let id = PanelId::from(app.open_files_panel.entity_id());
            let area = app.dock_area.read(cx);
            assert_eq!(app.session_state.open_files_visible, visible);
            assert!(!area.is_dock_open(DockPlacement::Right));
            assert!(
                area.layout(DockPlacement::Right)
                    .unwrap()
                    .find_panel_node(id)
                    .is_some()
            );
            assert!(!local_dock::panel_visible(area, id, cx));
        });
    }
}

/// Reopening a native region does not activate Open Files over a restored exclusive peer.
#[gpui::test]
fn open_files_restore_preserves_existing_exclusive_selection(cx: &mut TestAppContext) {
    let (fixture, ui) = mount(cx);
    let mut saved = saved_closed_right(&fixture, ui);
    saved.messages_visible = true;
    saved.dock_active_panels = vec!["HostMessages".into()];
    ui.update(|window, cx| {
        fixture.app.update(cx, |app, cx| {
            let messages = PanelId::from(app.messages.entity_id());
            app.messages
                .update(cx, |panel, cx| panel.set_visible(true, cx));
            app.dock_presentation.set_exclusive(messages, true);
            app.session_state = saved;
            app.tab_placement = TabPlacement::ToolWindow;
            app.sync_open_files_visibility(cx);
            app.pending_dock_restore = true;
            app.restore_dock_layout(window, cx);
        });
    });
    draw(ui);
    ui.update(|_, cx| {
        let app = fixture.app.read(cx);
        let open_files = PanelId::from(app.open_files_panel.entity_id());
        let messages = PanelId::from(app.messages.entity_id());
        assert!(app.dock_area.read(cx).is_dock_open(DockPlacement::Right));
        assert!(app.dock_presentation.is_suppressed(open_files));
        assert!(!app.dock_presentation.is_suppressed(messages));
        assert!(app.messages_visible(cx));
    });
}

/// Layout persistence during startup must not erase fixed documents before their restoration loop.
#[gpui::test]
fn open_files_restart_restores_fixed_order_active_file_and_moved_dock(cx: &mut TestAppContext) {
    let (fixture, ui) = mount(cx);
    let first = fixture.workspace.root().join("source.rs");
    let second = fixture.workspace.root().join("second.rs");
    let preview = fixture.workspace.root().join("preview.rs");
    std::fs::write(&second, "fn second() {}\n").unwrap();
    std::fs::write(&preview, "fn preview() {}\n").unwrap();
    ui.update(|window, cx| {
        fixture.app.update(cx, |app, cx| {
            app.open_file(second.clone(), window, cx);
            app.open_preview_file(preview.clone(), window, cx);
            app.open_file(second.clone(), window, cx);
            let first_id = app.tabs[0].file_id;
            let second_id = app.tabs[1].file_id;
            // Keep the active file first, so selecting the last restored file cannot pass unnoticed.
            app.reorder_file_tab(second_id, Some(first_id), cx);
            assert_eq!(app.tabs.len(), 3);
            assert!(app.tabs[2].preview);
            let id = PanelId::from(app.open_files_panel.entity_id());
            move_open_files_to_native_right(app, window, cx);
            app.dock_area.update(cx, |area, cx| {
                local_dock::reveal_panel(area, id, window, cx);
            });
        });
    });
    draw(ui);
    ui.update(|_, cx| {
        fixture.app.update(cx, |app, cx| {
            app.capture_dock_layout(cx);
            app.persist_session();
        });
    });
    // Read before removing the original window; its later teardown cannot rewrite this snapshot.
    let saved = SessionState::load_in_profile(fixture.workspace.root(), fixture.profile.path());
    let expected = vec![
        second.to_string_lossy().into_owned(),
        first.to_string_lossy().into_owned(),
    ];
    assert_eq!(saved.open_tabs, expected);
    assert_eq!(saved.active_file.as_deref(), Some(expected[0].as_str()));
    assert!(saved.dock_layout.is_some());
    let workspace = fixture.workspace.clone();
    let mut application = None;
    let handle = ui.update(|window, cx| {
        window.remove_window();
        cx.open_window(Default::default(), |window, cx| {
            let app =
                cx.new(|cx| EditorApp::new_with_session(workspace, None, Some(saved), window, cx));
            application = Some(app.clone());
            cx.new(|cx| Root::new(app, window, cx))
        })
        .unwrap()
    });
    let mut reopened = VisualTestContext::from_window(handle.into(), &ui.cx);
    reopened.simulate_resize(size(px(1100.), px(700.)));
    draw(&mut reopened);
    reopened.update(|_, cx| {
        let app = application.as_ref().unwrap().read(cx);
        assert_eq!(app.tab_placement, TabPlacement::ToolWindow);
        assert_eq!(
            app.tabs
                .iter()
                .map(|tab| tab.path().to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            expected
        );
        assert!(app.tabs.iter().all(|tab| !tab.preview));
        assert_eq!(app.active_path.as_deref(), Some(second.as_path()));
        assert_eq!(app.session_state.open_tabs, expected);
        let id = PanelId::from(app.open_files_panel.entity_id());
        let area = app.dock_area.read(cx);
        assert!(
            area.layout(DockPlacement::Right)
                .unwrap()
                .find_panel_node(id)
                .is_some()
        );
        assert!(local_dock::panel_visible(area, id, cx));
    });
    assert!(reopened.debug_bounds("open-files-panel").is_some());
}
