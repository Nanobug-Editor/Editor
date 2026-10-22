//! Actual save, capture and destructive-operation failure behavior.

use super::*;

/// Retained documents can save after a failed close without reviving an older in-flight preparation.
#[gpui::test]
fn history_failed_close_allows_new_saves_without_reviving_old_preparations(
    cx: &mut TestAppContext,
) {
    with_history_editor(cx, false, |visual, app, path| {
        visual.simulate_input("pending ");
        visual.run_until_parked();
        visual.update(|_, cx| {
            app.update(cx, |app, cx| {
                app.save_text_tab(0, cx);
                app.shutting_down = true;
                app.shutdown_history(cx).detach();
                app.resume_history_saves(cx);
            })
        });
        wait_until(visual, |visual| {
            visual.update(|_, cx| app.read(cx).plugin_saves.is_empty())
        });
        assert_eq!(std::fs::read_to_string(path).unwrap(), "original on disk");
        assert!(visual.update(|_, cx| app.read(cx).tabs[0].is_dirty()));
        visual.update(|_, cx| app.update(cx, |app, cx| app.save_text_tab(0, cx)));
        wait_until(visual, |visual| {
            visual.update(|_, cx| !app.read(cx).tabs[0].is_dirty())
        });
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "pending original on disk"
        );
    });
}

/// The same save command captures the old disk and only the bytes successfully committed by the session.
#[gpui::test]
fn history_native_save_retains_old_and_new_contents(cx: &mut TestAppContext) {
    with_history_editor(cx, false, |visual, app, path| {
        visual.simulate_input("new ");
        visual.run_until_parked();
        visual.simulate_keystrokes("ctrl-s");
        wait_until(visual, |visual| {
            visual.update(|_, cx| !app.read(cx).text_tab(0).unwrap().session.is_dirty())
        });
        let root = visual.update(|_, cx| app.read(cx).history.paths.as_ref().unwrap().root.clone());
        let history =
            editor_history::History::open(root, editor_history::Policy::default()).unwrap();
        let rows = history
            .list(&editor_history::Query::default())
            .unwrap()
            .iter()
            .map(|row| history.read(row.id).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            rows,
            vec![
                b"new original on disk".to_vec(),
                b"original on disk".to_vec()
            ]
        );
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "new original on disk"
        );
    });
}

/// History initialization failure is visible while the ordinary project save still succeeds.
#[gpui::test]
fn history_unavailable_does_not_prevent_native_save(cx: &mut TestAppContext) {
    with_history_editor(cx, true, |visual, app, path| {
        visual.simulate_input("changed ");
        visual.run_until_parked();
        visual.simulate_keystrokes("ctrl-s");
        wait_until(visual, |visual| {
            visual.update(|_, cx| !app.read(cx).text_tab(0).unwrap().session.is_dirty())
        });
        assert!(visual.update(|_, cx| app.read(cx).history.last_error.is_some()));
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "changed original on disk"
        );
    });
}

/// A destructive file operation has the stricter failure policy: no deletion before required protection.
#[gpui::test]
fn history_failed_delete_protection_preserves_the_project_file(cx: &mut TestAppContext) {
    with_history_editor(cx, true, |visual, app, path| {
        visual.run_until_parked();
        let path = path.canonicalize().unwrap();
        visual.update(|window, cx| {
            app.update(cx, |app, cx| {
                app.close_tab(path.clone(), window, cx);
                app.start_explorer_delete(path.clone(), window, cx);
                app.finish_explorer_delete(cx);
            })
        });
        visual.run_until_parked();
        assert!(
            path.exists(),
            "required history protection failed, so deletion must not begin"
        );
        assert!(visual.update(|_, cx| app.read(cx).explorer_delete.is_some()));
    });
}

/// Only the explicit visible failure action permits deleting without that failed history backup.
#[gpui::test]
fn history_delete_without_backup_requires_the_explicit_failure_action(cx: &mut TestAppContext) {
    with_history_editor(cx, true, |visual, app, path| {
        let path = path.canonicalize().unwrap();
        visual.update(|window, cx| {
            app.update(cx, |app, cx| {
                app.close_tab(path.clone(), window, cx);
                app.start_explorer_delete(path.clone(), window, cx);
                app.finish_explorer_delete(cx);
            })
        });
        wait_until(visual, |visual| {
            visual
                .debug_bounds("explorer-delete-without-history")
                .is_some()
        });
        assert!(path.exists());
        let position = visual
            .debug_bounds("explorer-delete-without-history")
            .unwrap()
            .center();
        visual.simulate_click(position, Modifiers::default());
        wait_until(visual, |visual| {
            !path.exists() && visual.update(|_, cx| app.read(cx).explorer_delete.is_none())
        });
    });
}

/// Capture policy skips are visible without changing a successful ordinary save into an error.
#[gpui::test]
fn history_save_reports_the_file_limit_skip(cx: &mut TestAppContext) {
    with_history_editor(cx, false, |visual, app, path| {
        visual.run_until_parked();
        let store = history_store(visual, &app);
        store
            .set_policy(editor_history::Policy {
                max_file_bytes: 1,
                ..Default::default()
            })
            .unwrap();
        visual.simulate_input("changed ");
        visual.run_until_parked();
        visual.simulate_keystrokes("ctrl-s");
        wait_until(visual, |visual| {
            visual.update(|_, cx| {
                app.read(cx)
                    .history
                    .skipped
                    .as_ref()
                    .is_some_and(|(_, reason)| *reason == editor_history::SkipReason::TooLarge)
                    && !app.read(cx).tabs[0].is_dirty()
            })
        });
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "changed original on disk"
        );
    });
}

/// A failed history transaction after a successful file write is visible without undoing that save.
#[gpui::test]
fn history_quota_failure_does_not_prevent_ordinary_save(cx: &mut TestAppContext) {
    with_history_editor(cx, false, |visual, app, path| {
        visual.run_until_parked();
        let store = history_store(visual, &app);
        store
            .set_policy(editor_history::Policy {
                max_total_bytes: 1,
                ..Default::default()
            })
            .unwrap();
        visual.simulate_input("new ");
        visual.run_until_parked();
        visual.simulate_keystrokes("ctrl-s");
        wait_until(visual, |visual| {
            visual.update(|_, cx| {
                app.read(cx).history.last_error.is_some() && !app.read(cx).tabs[0].is_dirty()
            })
        });
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "new original on disk"
        );
        assert_eq!(store.stats().unwrap().versions, 1);
        let previous = store.list(&editor_history::Query::default()).unwrap();
        assert_eq!(store.read(previous[0].id).unwrap(), b"original on disk");
    });
}

/// A native publication error leaves the editor dirty and never labels proposed bytes as saved history.
#[cfg(windows)]
#[gpui::test]
fn history_failed_native_write_does_not_record_new_saved_bytes(cx: &mut TestAppContext) {
    use std::os::windows::fs::OpenOptionsExt;
    with_history_editor(cx, false, |visual, app, path| {
        let held = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(path)
            .unwrap();
        visual.simulate_input("cannot publish ");
        visual.run_until_parked();
        visual.simulate_keystrokes("ctrl-s");
        wait_until(visual, |visual| {
            visual.update(|_, cx| app.read(cx).plugin_saves.is_empty())
        });
        assert!(visual.update(|_, cx| app.read(cx).tabs[0].is_dirty()));
        assert_eq!(std::fs::read_to_string(path).unwrap(), "original on disk");
        let store = history_store(visual, &app);
        assert!(
            store
                .list(&editor_history::Query::default())
                .unwrap()
                .iter()
                .all(|row| store.read(row.id).unwrap() == b"original on disk")
        );
        drop(held);
    });
}
