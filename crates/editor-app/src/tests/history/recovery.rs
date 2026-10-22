//! Native history preview, protected recovery and Undo keep the real document authoritative.

use super::*;

/// Diff boundaries are separate from file capture/recovery limits and reject before allocating an unbounded matrix.
#[test]
fn history_comparison_has_exact_byte_and_line_bounds() {
    let compare = crate::editor::comparison::changed_byte_ranges;
    assert!(compare(&"x".repeat(1024 * 1024), "x").is_ok());
    assert!(compare(&"x".repeat(1024 * 1024 + 1), "x").is_err());
    assert!(compare(&"x\n".repeat(2_000), &"y\n".repeat(2_000)).is_ok());
    assert!(compare(&"x\n".repeat(2_001), "x").is_err());
}

/// Restoring an old version first protects unsaved text, applies one native edit, and leaves disk alone.
#[gpui::test]
fn history_restore_protects_unsaved_content_and_preserves_native_undo(cx: &mut TestAppContext) {
    with_history_editor(cx, false, |visual, app, path| {
        visual.simulate_input("saved ");
        visual.run_until_parked();
        visual.simulate_keystrokes("ctrl-s");
        wait_until(visual, |visual| {
            visual.update(|_, cx| !app.read(cx).text_tab(0).unwrap().session.is_dirty())
        });
        visual.simulate_input("unsaved ");
        visual.run_until_parked();
        let protected_text =
            visual.update(|_, cx| app.read(cx).editor.read(cx).value().to_string());
        visual.update(|window, cx| {
            app.update(cx, |app, cx| {
                app.open_file_history(path.to_owned(), window, cx)
            })
        });
        wait_until(visual, |visual| {
            visual.update(|_, cx| {
                let panel = app.read(cx).history.panel.as_ref().unwrap();
                assert!(panel.error.is_none(), "history list: {:?}", panel.error);
                panel.rows.len() == 2
            })
        });
        visual
            .update(|window, cx| app.update(cx, |app, cx| app.select_history_item(1, window, cx)));
        wait_until(visual, |visual| {
            visual.update(|_, cx| {
                let panel = app.read(cx).history.panel.as_ref().unwrap();
                assert!(panel.error.is_none(), "history preview: {:?}", panel.error);
                panel.preview.is_some()
            })
        });
        visual.update(|window, cx| {
            app.update(cx, |app, cx| app.restore_history_selection(window, cx))
        });
        wait_until(visual, |visual| {
            visual.update(|_, cx| {
                let app = app.read(cx);
                if let Some(panel) = &app.history.panel {
                    assert!(panel.error.is_none(), "history recovery: {:?}", panel.error);
                }
                app.editor.read(cx).value() == "original on disk"
            })
        });
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "saved original on disk"
        );
        assert!(visual.update(|_, cx| app.read(cx).text_tab(0).unwrap().session.is_dirty()));
        visual.simulate_keystrokes("ctrl-z");
        visual.run_until_parked();
        assert_eq!(
            visual.update(|_, cx| app.read(cx).editor.read(cx).value().to_string()),
            protected_text
        );
        visual.simulate_keystrokes("ctrl-y");
        visual.run_until_parked();
        assert_eq!(
            visual.update(|_, cx| app.read(cx).editor.read(cx).value().to_string()),
            "original on disk"
        );
        let root = visual.update(|_, cx| app.read(cx).history.paths.as_ref().unwrap().root.clone());
        let history =
            editor_history::History::open(root, editor_history::Policy::default()).unwrap();
        let saved_protection = {
            let row = history
                .list(&editor_history::Query::default())
                .unwrap()
                .into_iter()
                .find(|row| row.reason == editor_history::Reason::BeforeRestore)
                .unwrap();
            history.read(row.id).unwrap()
        };
        assert_eq!(saved_protection, protected_text.as_bytes());
    });
}

/// Deletion's real explorer path keeps recoverable bytes; recovery creates disk content only on Save.
#[gpui::test]
fn history_deleted_file_recovers_as_a_dirty_create_only_document(cx: &mut TestAppContext) {
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
            visual.update(|_, cx| app.read(cx).history.panel.as_ref().is_some_and(|panel|
            !panel.busy && panel.rows.iter().any(|item| matches!(item, crate::editor::history::model::Item::Version(row) if row.size.is_some()))))
        });
        select_preview(visual, &app, 0);
        visual.update(|window, cx| {
            app.update(cx, |app, cx| app.restore_history_selection(window, cx))
        });
        wait_until(visual, |visual| {
            visual.update(|_, cx| app.read(cx).history.panel.is_none())
        });
        assert!(!path.exists());
        assert!(
            visual.update(|window, cx| app.read(cx).editor.focus_handle(cx).is_focused(window))
        );
        assert!(visual.update(|_, cx| {
            app.read(cx)
                .tabs
                .iter()
                .any(|file| file.path() == path && file.is_dirty())
        }));
        visual.simulate_keystrokes("ctrl-s");
        wait_until(visual, |_| path.exists());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "original on disk");
        wait_until(visual, |visual| {
            visual.update(|_, cx| !app.read(cx).tabs.iter().any(OpenTab::is_dirty))
        });
    });
}

/// The preview is bound to the original native revision, including edits while history stays mounted.
#[gpui::test]
fn history_stale_preview_cannot_replace_new_input(cx: &mut TestAppContext) {
    with_history_editor(cx, false, |visual, app, path| {
        open_first_preview(visual, &app, path);
        visual.update(|window, cx| {
            let editor = app.read(cx).editor.clone();
            editor.update(cx, |editor, cx| editor.insert("later ", window, cx));
        });
        visual.run_until_parked();
        visual.update(|window, cx| {
            app.update(cx, |app, cx| app.restore_history_selection(window, cx))
        });
        visual.run_until_parked();
        assert_eq!(
            visual.update(|_, cx| app.read(cx).editor.read(cx).value().to_string()),
            "later original on disk"
        );
        assert!(
            visual.update(|_, cx| app.read(cx).history.panel.as_ref().unwrap().error.is_some())
        );
    });
}

/// A background disk change cannot be hidden by restoring an earlier preview.
#[gpui::test]
fn history_disk_change_invalidates_restore(cx: &mut TestAppContext) {
    with_history_editor(cx, false, |visual, app, path| {
        open_first_preview(visual, &app, path);
        std::fs::write(path, "external replacement").unwrap();
        visual.update(|window, cx| {
            app.update(cx, |app, cx| app.restore_history_selection(window, cx))
        });
        visual.run_until_parked();
        assert!(
            visual.update(|_, cx| app.read(cx).history.panel.as_ref().unwrap().error.is_some())
        );
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "external replacement"
        );
    });
}

/// Policy failures during required buffer protection preserve both unsaved text and disk.
#[gpui::test]
fn history_restore_policy_failure_preserves_unsaved_text(cx: &mut TestAppContext) {
    with_history_editor(cx, false, |visual, app, path| {
        visual.simulate_input("unsaved ");
        visual.run_until_parked();
        open_first_preview(visual, &app, path);
        let store = history_store(visual, &app);
        store
            .set_policy(editor_history::Policy {
                max_file_bytes: 1,
                ..Default::default()
            })
            .unwrap();
        visual.update(|window, cx| {
            app.update(cx, |app, cx| app.restore_history_selection(window, cx))
        });
        wait_until(visual, |visual| {
            visual.update(|_, cx| app.read(cx).history.panel.as_ref().unwrap().error.is_some())
        });
        assert_eq!(
            visual.update(|_, cx| app.read(cx).editor.read(cx).value().to_string()),
            "unsaved original on disk"
        );
        assert_eq!(std::fs::read_to_string(path).unwrap(), "original on disk");
    });
}

/// Disabled history retains browsing but cannot skip the required restore preimage.
#[gpui::test]
fn history_disabled_recording_keeps_browsing_and_refuses_restore(cx: &mut TestAppContext) {
    with_history_editor(cx, false, |visual, app, path| {
        visual.run_until_parked();
        let store = history_store(visual, &app);
        store
            .set_policy(editor_history::Policy {
                enabled: false,
                ..Default::default()
            })
            .unwrap();
        open_first_preview(visual, &app, path);
        visual.update(|window, cx| {
            app.update(cx, |app, cx| app.restore_history_selection(window, cx))
        });
        wait_until(visual, |visual| {
            visual.update(|_, cx| app.read(cx).history.panel.as_ref().unwrap().error.is_some())
        });
        assert_eq!(std::fs::read_to_string(path).unwrap(), "original on disk");
    });
}

/// Cleanup confirmation changes only stored observations; the live native buffer remains untouched.
#[gpui::test]
fn history_clear_confirmation_preserves_current_editor_and_project_file(cx: &mut TestAppContext) {
    with_history_editor(cx, false, |visual, app, path| {
        open_first_preview(visual, &app, path);
        visual.update(|_, cx| {
            app.update(cx, |app, cx| {
                app.preview_history_clear(editor_history::ClearScope::All, "all".into(), cx)
            })
        });
        wait_until(visual, |visual| {
            visual.update(|_, cx| app.read(cx).history.panel.as_ref().unwrap().clear.is_some())
        });
        assert_eq!(
            visual.update(|_, cx| app
                .read(cx)
                .history
                .panel
                .as_ref()
                .unwrap()
                .clear
                .as_ref()
                .unwrap()
                .preview
                .versions),
            1
        );
        visual.update(|_, cx| app.update(cx, |app, cx| app.confirm_history_clear(cx)));
        wait_until(visual, |visual| {
            visual.update(|_, cx| {
                app.read(cx)
                    .history
                    .panel
                    .as_ref()
                    .is_some_and(|panel| !panel.busy && panel.rows.is_empty())
            })
        });
        assert_eq!(
            visual.update(|_, cx| app.read(cx).editor.read(cx).value().to_string()),
            "original on disk"
        );
        assert_eq!(std::fs::read_to_string(path).unwrap(), "original on disk");
    });
}

/// A still-open deleted document keeps its logical history even though the live path binding is gone.
#[gpui::test]
fn history_deleted_open_tab_keeps_its_file_timeline(cx: &mut TestAppContext) {
    with_history_editor(cx, false, |visual, app, path| {
        visual.run_until_parked();
        let path = path.canonicalize().unwrap();
        std::fs::remove_file(&path).unwrap();
        visual.update(|_, cx| {
            app.update(cx, |app, cx| {
                app.observe_history_removal(path.clone(), true, cx)
            })
        });
        visual.run_until_parked();
        visual.update(|window, cx| {
            app.update(cx, |app, cx| {
                app.open_file_history(path.clone(), window, cx)
            })
        });
        wait_until(visual, |visual| {
            visual.update(|_, cx| {
                app.read(cx)
                    .history
                    .panel
                    .as_ref()
                    .is_some_and(|panel| !panel.busy)
            })
        });
        assert!(
            visual.update(|_, cx| !app.read(cx).history.panel.as_ref().unwrap().rows.is_empty())
        );
    });
}

/// A late protection acknowledgement cannot reopen a cancelled history view or replace its buffer.
#[gpui::test]
fn history_cancelled_restore_never_applies_its_late_result(cx: &mut TestAppContext) {
    with_history_editor(cx, false, |visual, app, path| {
        visual.simulate_input("dirty ");
        visual.run_until_parked();
        open_first_preview(visual, &app, path);
        visual.update(|window, cx| {
            app.update(cx, |app, cx| {
                app.restore_history_selection(window, cx);
                app.close_history(window, cx);
            })
        });
        visual.run_until_parked();
        assert_eq!(
            visual.update(|_, cx| app.read(cx).editor.read(cx).value().to_string()),
            "dirty original on disk"
        );
        assert!(visual.update(|_, cx| app.read(cx).history.panel.is_none()));
    });
}

/// Selecting another tab after preview does not silently reactivate and restore an old target.
#[gpui::test]
fn history_tab_switch_invalidates_the_visible_restore_proposal(cx: &mut TestAppContext) {
    with_history_editor(cx, false, |visual, app, path| {
        open_first_preview(visual, &app, path);
        let other = path.parent().unwrap().join("other.txt");
        std::fs::write(&other, "other document").unwrap();
        visual.update(|window, cx| {
            app.update(cx, |app, cx| app.open_file(other.clone(), window, cx))
        });
        visual.run_until_parked();
        visual.update(|window, cx| {
            app.update(cx, |app, cx| app.restore_history_selection(window, cx))
        });
        visual.run_until_parked();
        assert_eq!(
            visual.update(|_, cx| app.read(cx).editor.read(cx).value().to_string()),
            "other document"
        );
        assert!(
            visual.update(|_, cx| app.read(cx).history.panel.as_ref().unwrap().error.is_some())
        );
    });
}

/// Native read-only state and an active preedit both retain their original text and composition.
#[gpui::test]
fn history_restore_refuses_readonly_and_active_native_ime(cx: &mut TestAppContext) {
    with_history_editor(cx, false, |visual, app, path| {
        open_first_preview(visual, &app, path);
        visual.update(|window, cx| {
            app.update(cx, |app, cx| {
                app.editor
                    .update(cx, |editor, cx| editor.set_readonly(true, cx));
                app.restore_history_selection(window, cx);
                assert!(app.history.panel.as_ref().unwrap().error.is_some());
            })
        });
        visual.update(|window, cx| {
            let editor = app.read(cx).editor.clone();
            editor.update(cx, |editor, cx| {
                editor.set_readonly(false, cx);
                editor.replace_and_mark_text_in_range(None, "拼", Some(0..1), window, cx);
            });
        });
        select_preview(visual, &app, 0);
        visual.update(|window, cx| {
            app.update(cx, |app, cx| {
                let before = app.editor.read(cx).value().to_string();
                app.restore_history_selection(window, cx);
                assert!(app.history.panel.as_ref().unwrap().error.is_some());
                assert_eq!(app.editor.read(cx).value().as_ref(), before);
                app.editor.update(cx, |editor, cx| {
                    assert!(editor.marked_text_range(window, cx).is_some())
                });
            })
        });
    });
}
