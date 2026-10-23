//! Hidden native peer preferences survive disk restoration beside a visible right-hand tool column.

use super::*;
use gpui_base::dock::DockLayout;

/// Use the production command entry without requiring an open document's shortcut context.
fn open_problems(app: &Entity<EditorApp>, visual: &mut VisualTestContext) {
    visual.update(|window, cx| {
        app.update(cx, |app, cx| app.show_problems(window, cx));
    });
    paint(visual);
    assert!(
        visual.debug_bounds("native-problems-panel").is_some(),
        "the production command must paint Problems before measuring or saving the fixture"
    );
}

/// Reject an untriggered input at the fixture boundary instead of reporting it as a missing layout.
fn assert_peers_visible(
    app: &Entity<EditorApp>,
    ids: [PanelId; 5],
    visual: &mut VisualTestContext,
) {
    visual.update(|_, cx| {
        let app = app.read(cx);
        for id in ids {
            assert!(
                local_dock::panel_visible(app.dock_area.read(cx), id, cx),
                "fixture panel {id:?} must be visibly registered before measuring its allocation"
            );
        }
    });
}

/// Keep Editor between real columns and two differently sized native leaves inside the right band.
fn install_right_native_peers(app: &Entity<EditorApp>, visual: &mut VisualTestContext) {
    visual.update(|window, cx| {
        app.update(cx, |app, cx| {
            let leaf = |panel| DockLayout::tabs().panel_view(panel, cx);
            let presentation = &app.dock_presentation;
            let sidebar = leaf(presentation.panel_handle(app.explorer_panel.clone()));
            let editor = leaf(presentation.panel_handle(app.editor_panel.clone()));
            let messages = leaf(presentation.panel_handle(app.messages.clone()));
            let problems = leaf(presentation.panel_handle(app.problems.panel.clone()));
            let shared =
                leaf(presentation.panel_handle(app.plugin_panels["region-fixture/shared"].clone()));
            let layout = DockLayout::h_split()
                .child(sidebar, Some(px(437.)))
                .child(editor, Some(px(900.)))
                .child(
                    DockLayout::v_split().child(shared, None).child(
                        DockLayout::h_split()
                            .child(problems, Some(px(230.)))
                            .child(messages, Some(px(310.))),
                        Some(px(260.)),
                    ),
                    Some(px(540.)),
                );
            app.dock_area.update(cx, |area, cx| {
                for placement in [
                    DockPlacement::Left,
                    DockPlacement::Right,
                    DockPlacement::Bottom,
                ] {
                    area.remove_dock(placement, window, cx);
                }
                area.set_center(layout, window, cx);
            });
            cx.notify();
        });
    });
    paint(visual);
}

/// Read the live identities in Editor, Explorer, visible right peer, Problems and Messages order.
fn right_peer_ids(app: &Entity<EditorApp>, visual: &mut VisualTestContext) -> [PanelId; 5] {
    visual.update(|_, cx| {
        let app = app.read(cx);
        [
            PanelId::from(app.editor_panel.entity_id()),
            PanelId::from(app.explorer_panel.entity_id()),
            PanelId::from(app.plugin_panels["region-fixture/shared"].entity_id()),
            PanelId::from(app.problems.panel.entity_id()),
            PanelId::from(app.messages.entity_id()),
        ]
    })
}

/// Wait for fixture contributions before invoking the same persisted-tree restore used at startup.
fn reopen_hidden_peers<'a>(
    cx: &'a mut TestAppContext,
    workspace_root: &std::path::Path,
    profile: &std::path::Path,
    manifest: &protocol::Manifest,
) -> (Entity<EditorApp>, &'a mut VisualTestContext) {
    let mut saved = SessionState::load_in_profile(workspace_root, profile);
    assert!(!saved.problems_visible && !saved.messages_visible);
    let layout = saved
        .dock_layout
        .take()
        .expect("hidden layout must be saved");
    let active = std::mem::take(&mut saved.dock_active_panels);
    let workspace = Workspace::open(workspace_root).unwrap();
    let slot = Rc::new(RefCell::new(None));
    let capture = slot.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let app =
            cx.new(|cx| EditorApp::new_with_session(workspace, None, Some(saved), window, cx));
        *capture.borrow_mut() = Some(app.clone());
        Root::new(app, window, cx)
    });
    let app = slot.borrow_mut().take().unwrap();
    visual.simulate_resize(size(px(1877.), px(900.)));
    publish(&app, manifest, visual);
    visual.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.session_state.dock_layout = Some(layout);
            app.session_state.dock_active_panels = active;
            app.pending_dock_restore = true;
            app.restore_dock_layout(window, cx);
            assert!(!app.pending_dock_restore);
            cx.notify();
        });
    });
    paint(visual);
    (app, visual)
}

/// Hidden non-default widths survive new native entities while a visible tool remains after Editor.
#[gpui::test]
fn hidden_native_peer_sizes_restore_beside_visible_right_column(cx: &mut TestAppContext) {
    let (directory, app, visual) = setup(cx, false);
    visual.simulate_resize(size(px(1877.), px(900.)));
    paint(visual);
    click(visual, "host-messages-toggle");
    open_problems(&app, visual);
    install_right_native_peers(&app, visual);
    let ids = right_peer_ids(&app, visual);
    assert_peers_visible(&app, ids, visual);
    let before = ids.map(|id| native_panel_bounds(&app, id, visual));
    assert!((before[3].size.width - px(230.)).abs() < px(2.));
    assert!((before[4].size.width - px(310.)).abs() < px(2.));
    assert!(before[2].left() >= before[0].right() - px(2.));
    click(visual, "problem-close");
    click(visual, "host-messages-toggle");
    let (workspace_root, manifest) = visual.update(|_, cx| {
        app.update(cx, |app, cx| {
            let area = app.dock_area.read(cx);
            for id in [ids[3], ids[4]] {
                assert!(!area.panel(id).unwrap().visible(cx));
            }
            app.capture_dock_layout(cx);
            app.session_state.try_save().unwrap();
            (
                app.workspace.root().to_owned(),
                app.extensions.read(cx).entries[0].manifest.clone(),
            )
        })
    });
    let (reopened, visual) = reopen_hidden_peers(
        cx,
        &workspace_root,
        &directory.path().join("profile"),
        &manifest,
    );
    let restored = right_peer_ids(&reopened, visual);
    assert_ne!(ids[3], restored[3]);
    assert_ne!(ids[4], restored[4]);
    visual.update(|_, cx| {
        let area = reopened.read(cx).dock_area.read(cx);
        for id in [restored[3], restored[4]] {
            assert!(!area.panel(id).unwrap().visible(cx));
        }
        assert!(area.panel(restored[2]).unwrap().visible(cx));
    });
    open_problems(&reopened, visual);
    click(visual, "host-messages-toggle");
    assert_peers_visible(&reopened, restored, visual);
    for (id, expected) in restored.into_iter().zip(before) {
        unchanged(expected, native_panel_bounds(&reopened, id, visual));
    }
}
