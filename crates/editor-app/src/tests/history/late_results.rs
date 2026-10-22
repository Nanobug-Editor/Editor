//! Async history replies cannot publish writable state after the user changes the active document.

use super::*;

/// Switching in the initiating UI turn reliably precedes publication of the background diff reply.
#[gpui::test]
fn history_inflight_comparison_preserves_the_new_active_tab(cx: &mut TestAppContext) {
    with_history_editor(cx, false, |visual, app, path| {
        open_first_preview(visual, &app, path);
        let other = path.parent().unwrap().join("other.txt");
        std::fs::write(&other, "other document").unwrap();
        visual.update(|window, cx| {
            app.update(cx, |app, cx| {
                app.compare_history_selection(window, cx);
                app.open_file(other.clone(), window, cx);
            })
        });
        wait_until(visual, |visual| {
            visual.update(|_, cx| app.read(cx).history.panel.as_ref().unwrap().error.is_some())
        });
        visual.update(|_, cx| {
            let app = app.read(cx);
            assert_eq!(
                app.active_path.as_deref(),
                Some(other.canonicalize().unwrap().as_path())
            );
            assert_eq!(app.editor.read(cx).value().as_ref(), "other document");
            assert!(app.history.panel.as_ref().unwrap().comparison.is_none());
        });
    });
}

/// A deleted-file policy reply cannot reopen the source and steal a newer tab's focus.
#[gpui::test]
fn history_inflight_deleted_recovery_preserves_the_new_active_tab(cx: &mut TestAppContext) {
    with_history_editor(cx, false, |visual, app, path| {
        visual.run_until_parked();
        let path = path.canonicalize().unwrap();
        visual.update(|window, cx| {
            app.update(cx, |app, cx| {
                app.close_tab(path.clone(), window, cx);
                app.start_explorer_delete(path.clone(), window, cx);
                app.finish_explorer_delete(cx);
            })
        });
        wait_until(visual, |visual| {
            !path.exists() && visual.update(|_, cx| app.read(cx).explorer_delete.is_none())
        });
        visual.update(|window, cx| {
            app.update(cx, |app, cx| {
                app.open_history_category(
                    crate::editor::history::model::Category::Deleted,
                    window,
                    cx,
                )
            })
        });
        wait_until(visual, |visual| {
            visual.update(|_, cx| {
                app.read(cx).history.panel.as_ref().is_some_and(|panel| {
                    !panel.busy && panel.rows.iter().any(|item| item.has_content())
                })
            })
        });
        select_preview(visual, &app, 0);
        let other = path.parent().unwrap().join("other.txt");
        std::fs::write(&other, "other document").unwrap();
        visual.update(|window, cx| {
            app.update(cx, |app, cx| {
                app.restore_history_selection(window, cx);
                app.open_file(other.clone(), window, cx);
            })
        });
        wait_until(visual, |visual| {
            visual.update(|_, cx| app.read(cx).history.panel.as_ref().unwrap().error.is_some())
        });
        visual.update(|_, cx| {
            let app = app.read(cx);
            assert_eq!(app.editor.read(cx).value().as_ref(), "other document");
            assert!(app.tabs.iter().all(|tab| tab.path() != path));
            assert!(app.history.pending_create.is_empty());
        });
        assert!(!path.exists());
    });
}
