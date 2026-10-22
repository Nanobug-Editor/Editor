//! Real disk observations preserve document conflicts, stable lineage and standalone ownership.

use super::*;
use crate::editor::file_watch::DiskContent;

/// Repeated watcher publications retain the disk state once, while dirty native text remains intact.
#[gpui::test]
fn history_external_observations_preserve_clean_and_dirty_document_behavior(
    cx: &mut TestAppContext,
) {
    with_history_editor(cx, false, |visual, app, path| {
        visual.run_until_parked();
        let path = path.canonicalize().unwrap();
        let store = history_store(visual, &app);
        for (contents, dirty) in [
            ("clean external state", false),
            ("new external conflict", true),
        ] {
            if dirty {
                visual.update(|window, cx| {
                    let editor = app.read(cx).editor.clone();
                    editor.update(cx, |editor, cx| {
                        editor.set_selected_range(0..0, cx);
                        editor.insert("unsaved ", window, cx);
                    });
                });
                visual.run_until_parked();
            }
            let before = visual.update(|_, cx| app.read(cx).editor.read(cx).value().to_string());
            std::fs::write(&path, contents).unwrap();
            for _ in 0..2 {
                visual.update(|window, cx| {
                    app.update(cx, |app, cx| {
                        app.apply_reconciliation(
                            Reconciliation {
                                file_events: Vec::new(),
                                snapshot: None,
                                renames: Vec::new(),
                                native: true,
                                documents: vec![(
                                    path.clone(),
                                    Ok(DiskContent::Text(std::fs::read_to_string(&path).unwrap())),
                                    Instant::now(),
                                )],
                            },
                            window,
                            cx,
                        );
                    })
                });
            }
            wait_until(visual, |_| {
                store
                    .list(&editor_history::Query::default())
                    .unwrap()
                    .iter()
                    .filter(|row| {
                        row.reason == editor_history::Reason::External
                            && store.read(row.id).unwrap() == contents.as_bytes()
                    })
                    .count()
                    == 1
            });
            visual.update(|_, cx| {
                let app = app.read(cx);
                assert_eq!(
                    app.editor.read(cx).value().as_ref(),
                    if dirty { before.as_str() } else { contents }
                );
                assert_eq!(app.text_tab(0).unwrap().session.is_dirty(), dirty);
                assert_eq!(
                    app.text_tab(0).unwrap().disk_state,
                    if dirty {
                        DiskState::Conflict
                    } else {
                        DiskState::Synced
                    }
                );
            });
        }
    });
}

/// A reliable paired event updates historical paths and the existing native session together.
#[gpui::test]
fn history_paired_rename_and_move_keep_the_native_document_lineage(cx: &mut TestAppContext) {
    with_history_editor(cx, false, |visual, app, path| {
        visual.run_until_parked();
        let store = history_store(visual, &app);
        let mut previous = path.canonicalize().unwrap();
        let identity = store.file_at(&previous).unwrap().unwrap();
        let document = visual.update(|_, cx| app.read(cx).tabs[0].file_id);
        let renamed = previous.parent().unwrap().join("renamed.txt");
        let folder = previous.parent().unwrap().join("moved");
        std::fs::create_dir(&folder).unwrap();
        for target in [renamed, folder.join("renamed.txt")] {
            std::fs::rename(&previous, &target).unwrap();
            let target = target.canonicalize().unwrap();
            visual.update(|window, cx| {
                app.update(cx, |app, cx| {
                    app.apply_reconciliation(
                        Reconciliation {
                            file_events: Vec::new(),
                            snapshot: None,
                            documents: Vec::new(),
                            renames: vec![(previous.clone(), target.clone())],
                            native: true,
                        },
                        window,
                        cx,
                    )
                })
            });
            wait_until(visual, |_| {
                store.file_at(&target).unwrap() == Some(identity)
            });
            assert!(store.file_at(&previous).unwrap().is_none());
            visual.update(|_, cx| {
                let app = app.read(cx);
                assert_eq!(app.tabs[0].file_id, document);
                assert_eq!(app.tabs[0].path(), target);
            });
            previous = target;
        }
        let rows = store
            .list(&editor_history::Query {
                file: Some(identity),
                ..Default::default()
            })
            .unwrap();
        for reason in [
            editor_history::Reason::Renamed,
            editor_history::Reason::Moved,
        ] {
            assert!(
                rows.iter()
                    .any(|row| row.reason == reason && row.previous_path.is_some())
            );
        }
    });
}

/// Reopening catches the current disk version without inventing intermediates; external files stay standalone.
#[gpui::test]
fn history_reopen_observes_offline_content_and_standalone_scope(cx: &mut TestAppContext) {
    with_history_editor(cx, false, |visual, app, path| {
        visual.run_until_parked();
        let path = path.canonicalize().unwrap();
        let store = history_store(visual, &app);
        let identity = store.file_at(&path).unwrap().unwrap();
        visual
            .update(|window, cx| app.update(cx, |app, cx| app.close_tab(path.clone(), window, cx)));
        std::fs::write(&path, "offline replacement").unwrap();
        visual
            .update(|window, cx| app.update(cx, |app, cx| app.open_file(path.clone(), window, cx)));
        wait_until(visual, |_| {
            store
                .list(&editor_history::Query::default())
                .unwrap()
                .iter()
                .any(|row| {
                    row.file == identity && store.read(row.id).unwrap() == b"offline replacement"
                })
        });
        let standalone = path
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("standalone.txt");
        std::fs::write(&standalone, "outside workspace").unwrap();
        visual.update(|window, cx| {
            app.update(cx, |app, cx| app.open_file(standalone.clone(), window, cx))
        });
        wait_until(visual, |_| {
            store
                .list(&editor_history::Query {
                    scope: editor_history::Scope::Standalone,
                    ..Default::default()
                })
                .unwrap()
                .iter()
                .any(|row| {
                    row.path == standalone.canonicalize().unwrap()
                        && store.read(row.id).unwrap() == b"outside workspace"
                })
        });
    });
}
