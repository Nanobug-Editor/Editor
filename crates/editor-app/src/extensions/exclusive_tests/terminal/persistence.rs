//! Persist exclusive regional selection without changing the ordinary split's saved proportions.

use super::*;

/// Saving a migrated exclusive alternative must not change ordinary widths after fresh entity binding.
#[gpui::test]
fn terminal_exclusive_migration_preserves_unequal_widths_after_restart(cx: &mut TestAppContext) {
    for exclusive_selected in [false, true] {
        let (directory, app, visual) = setup(cx, true);
        let ids = panels(&app, visual);
        let before = [ids[0], ids[1], ids[3]].map(|id| native_panel_bounds(&app, id, visual));
        click(visual, "terminal-toggle");
        set_unequal_bottom(&app, ids, visual);
        let shared = [ids[2], ids[3]].map(|id| native_panel_bounds(&app, id, visual));
        click(visual, "plugin-window-region-fixture/exclusive");
        if !exclusive_selected {
            click(visual, "terminal-toggle");
            for (id, bounds) in [ids[2], ids[3]].into_iter().zip(shared) {
                unchanged(bounds, native_panel_bounds(&app, id, visual));
            }
        }
        let (workspace_root, manifest) = visual.update(|_, cx| {
            app.update(cx, |app, cx| {
                assert!(!app.pending_dock_restore);
                app.capture_dock_layout(cx);
                assert!(app.session_state.dock_layout.is_some());
                assert_eq!(
                    app.session_state.workspace,
                    app.workspace.root().to_string_lossy()
                );
                app.session_state.try_save().unwrap();
                (
                    app.workspace.root().to_owned(),
                    app.extensions.read(cx).entries[0].manifest.clone(),
                )
            })
        });
        let mut saved =
            SessionState::load_in_profile(&workspace_root, &directory.path().join("profile"));
        assert!(saved.native_terminal_visible);
        // This fixture publishes contributions without installing a package in the profile. Defer
        // its saved tree until publication, matching the real startup barrier before unavailable
        // plugin leaves are pruned; the original persisted tree and regional selection stay intact.
        let saved_layout = saved
            .dock_layout
            .take()
            .expect("the live dock must be saved");
        let saved_active = std::mem::take(&mut saved.dock_active_panels);
        let workspace = Workspace::open(&workspace_root).unwrap();
        let slot = Rc::new(RefCell::new(None));
        let capture = slot.clone();
        let (_, visual) = cx.add_window_view(move |window, cx| {
            let app =
                cx.new(|cx| EditorApp::new_with_session(workspace, None, Some(saved), window, cx));
            *capture.borrow_mut() = Some(app.clone());
            Root::new(app, window, cx)
        });
        let reopened = slot.borrow_mut().take().unwrap();
        ordinary_terminal(&reopened, visual);
        visual.simulate_resize(size(px(1280.), px(900.)));
        publish(&reopened, &manifest, visual);
        visual.update(|window, cx| {
            reopened.update(cx, |app, cx| {
                app.session_state.dock_layout = Some(saved_layout);
                app.session_state.dock_active_panels = saved_active;
                app.pending_dock_restore = true;
                app.restore_dock_layout(window, cx);
                assert!(
                    !app.pending_dock_restore,
                    "published fixture contributions must complete the saved layout restore"
                );
                cx.notify();
            });
        });
        paint(visual);
        let restored_ids = panels(&reopened, visual);
        assert_ne!(ids[2], restored_ids[2]);
        if exclusive_selected {
            assert!(visual.debug_bounds("plugin-ui-content-exclusive").is_some());
            assert!(visual.debug_bounds("native-terminal-output").is_none());
            click(visual, "terminal-toggle");
        }
        assert_shared_bottom(&reopened, restored_ids, before, visual);
        for (id, bounds) in [restored_ids[2], restored_ids[3]].into_iter().zip(shared) {
            unchanged(bounds, native_panel_bounds(&reopened, id, visual));
        }
    }
}
