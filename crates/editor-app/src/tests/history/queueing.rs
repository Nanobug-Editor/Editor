//! Real startup and watcher bursts must retain every accepted immutable history observation.

use super::*;

/// Restore more tabs than the native history queue holds before the deterministic worker can run.
/// Every opened disk state must remain readable; ordinary startup contention is not a history fault.
#[gpui::test]
fn history_startup_restores_many_tabs_without_losing_observations(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        editor_ui::init(cx);
        typography::init(cx);
        apply_theme(builtin_theme(false), cx);
        cx.set_reduce_motion(true);
        app::bind_editor_shell_keys(cx);
    });
    let directory = tempfile::tempdir().unwrap();
    let project = directory.path().join("project");
    let profile = directory.path().join("profile");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::create_dir_all(&profile).unwrap();
    let paths = (0..24)
        .map(|index| {
            let path = project.join(format!("restored-{index:02}.txt"));
            std::fs::write(&path, format!("restored contents {index:02}")).unwrap();
            path.canonicalize().unwrap()
        })
        .collect::<Vec<_>>();
    let workspace = Workspace::open(&project).unwrap();
    let mut session = SessionState::load_in_profile(workspace.root(), &profile);
    session.document_session_loaded = true;
    session.open_tabs = paths
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect();
    session.active_file = Some(paths[7].to_string_lossy().into_owned());
    let mut application = None;
    let (_, visual) = cx.add_window_view(|window, cx| {
        let app =
            cx.new(|cx| EditorApp::new_with_session(workspace, None, Some(session), window, cx));
        application = Some(app.clone());
        Root::new(app, window, cx)
    });
    let app = application.unwrap();
    // Startup submits all observations in one native constructor. Release its ordinary scheduler
    // only afterwards, preserving the exact queue pressure of a restored document session.
    visual.run_until_parked();
    visual.update(|_, cx| {
        let app = app.read(cx);
        assert_eq!(
            app.tabs.iter().map(OpenTab::path).collect::<Vec<_>>(),
            paths.iter().map(PathBuf::as_path).collect::<Vec<_>>()
        );
        assert_eq!(app.active_path.as_ref(), Some(&paths[7]));
        assert!(
            app.history.last_error.is_none(),
            "ordinary restored tabs must not report a history failure: {:?}",
            app.history.last_error
        );
    });
    let store = history_store(visual, &app);
    wait_until(visual, |_| {
        store.list(&editor_history::Query::default()).unwrap().len() == paths.len()
    });
    let rows = store.list(&editor_history::Query::default()).unwrap();
    for (index, path) in paths.iter().enumerate() {
        let row = rows
            .iter()
            .find(|row| row.path == *path)
            .expect("each restored file retains its actual opened disk state");
        assert_eq!(
            store.read(row.id).unwrap(),
            format!("restored contents {index:02}").into_bytes()
        );
    }
}

/// A single native watcher reconciliation may exceed the worker's immediate submission slots.
/// Its unopened removals must not discard the later immutable bytes of an opened document.
#[gpui::test]
fn history_reconciliation_keeps_deletion_bursts_and_open_document_contents(
    cx: &mut TestAppContext,
) {
    use crate::editor::file_watch::DiskContent;
    use plugin_runtime::plugin_protocol::workspace::FileEventKind;

    with_history_editor(cx, false, |visual, app, path| {
        visual.run_until_parked();
        let path = path.canonicalize().unwrap();
        let root = path.parent().unwrap().to_owned();
        let store = history_store(visual, &app);
        let deleted = (0..24)
            .map(|index| {
                let path = root.join(format!("deleted-{index:02}.txt"));
                let bytes = format!("before removal {index:02}").into_bytes();
                std::fs::write(&path, &bytes).unwrap();
                // Seed actual prior durable history for closed files, as a previous session would.
                store
                    .capture(editor_history::Capture {
                        file: None,
                        path: path.clone(),
                        workspace: Some(root.clone()),
                        contents: bytes,
                        reason: editor_history::Reason::External,
                        at_ms: crate::editor::history::now_ms(),
                    })
                    .unwrap();
                std::fs::remove_file(&path).unwrap();
                path
            })
            .collect::<Vec<_>>();
        let changed = "actual disk contents observed after the removal batch";
        std::fs::write(&path, changed).unwrap();
        visual.update(|window, cx| {
            app.update(cx, |app, cx| {
                app.apply_reconciliation(
                    Reconciliation {
                        file_events: deleted
                            .iter()
                            .map(|path| FileEventKind::Deleted {
                                path: path.file_name().unwrap().to_string_lossy().into_owned(),
                            })
                            .collect(),
                        snapshot: None,
                        renames: Vec::new(),
                        native: true,
                        documents: vec![(
                            path.clone(),
                            Ok(DiskContent::Text(changed.into())),
                            Instant::now(),
                        )],
                    },
                    window,
                    cx,
                )
            })
        });
        visual.run_until_parked();
        visual.update(|_, cx| {
            let app = app.read(cx);
            assert_eq!(app.editor.read(cx).value().as_ref(), changed);
            assert!(
                app.history.last_error.is_none(),
                "one watcher batch must not report a history failure: {:?}",
                app.history.last_error
            );
        });
        let query = editor_history::Query {
            limit: 200,
            ..Default::default()
        };
        wait_until(visual, |_| {
            let rows = store.list(&query).unwrap();
            deleted.iter().all(|path| {
                rows.iter()
                    .any(|row| row.path == *path && row.reason == editor_history::Reason::Deleted)
            }) && rows.iter().any(|row| {
                row.path == path
                    && row.reason == editor_history::Reason::External
                    && store.read(row.id).unwrap() == changed.as_bytes()
            })
        });
        assert!(deleted.iter().all(|path| !path.exists()));
    });
}

/// Immutable A/B/A observations retain their operation order, including the final return to A.
/// Sample durable rows at the shutdown acknowledgement itself, before any later UI turn can help.
#[gpui::test]
fn history_batch_keeps_repeated_states_in_order_before_shutdown_acknowledges(
    cx: &mut TestAppContext,
) {
    use crate::editor::file_watch::DiskContent;
    use std::{cell::RefCell, rc::Rc};

    with_history_editor(cx, false, |visual, app, path| {
        visual.run_until_parked();
        let path = path.canonicalize().unwrap();
        let store = history_store(visual, &app);
        let file = store.file_at(&path).unwrap().unwrap();
        let observations = ["observed A", "observed B", "observed A"]
            .into_iter()
            .map(|contents| {
                std::fs::write(&path, contents).unwrap();
                let observed = std::fs::read_to_string(&path).unwrap();
                (
                    path.clone(),
                    Ok(DiskContent::Text(observed)),
                    Instant::now(),
                )
            })
            .collect();
        let checkpoint = Rc::new(RefCell::new(None));
        let completed = checkpoint.clone();
        let at_shutdown = store.clone();
        visual.update(|window, cx| {
            app.update(cx, |app, cx| {
                app.apply_reconciliation(
                    Reconciliation {
                        file_events: Vec::new(),
                        snapshot: None,
                        renames: Vec::new(),
                        native: true,
                        documents: observations,
                    },
                    window,
                    cx,
                );
                let shutdown = app.shutdown_history(cx);
                cx.spawn(async move |_, _| {
                    shutdown.await;
                    let mut rows = at_shutdown
                        .list(&editor_history::Query {
                            file: Some(file),
                            ..Default::default()
                        })
                        .unwrap();
                    rows.sort_by_key(|row| row.id.0);
                    *completed.borrow_mut() = Some(
                        rows.into_iter()
                            .map(|row| at_shutdown.read(row.id).unwrap())
                            .collect::<Vec<_>>(),
                    );
                })
                .detach();
            })
        });
        wait_until(visual, |_| checkpoint.borrow().is_some());
        assert_eq!(
            checkpoint.borrow_mut().take().unwrap(),
            ["original on disk", "observed A", "observed B", "observed A"]
                .map(|contents| contents.as_bytes().to_vec())
        );
        assert!(visual.update(|_, cx| app.read(cx).history.last_error.is_none()));
        assert_eq!(std::fs::read_to_string(path).unwrap(), "observed A");
    });
}

/// A rename destination can disappear before its native reconciliation reaches history.
/// That real I/O failure remains visible while a later independent capture in the batch persists.
#[gpui::test]
fn history_batch_reports_a_failed_move_and_still_records_later_document_changes(
    cx: &mut TestAppContext,
) {
    use crate::editor::file_watch::DiskContent;

    with_history_editor(cx, false, |visual, app, path| {
        visual.run_until_parked();
        let path = path.canonicalize().unwrap();
        let store = history_store(visual, &app);
        let old = path.parent().unwrap().join("before-move.txt");
        let disappeared = path.parent().unwrap().join("already-removed.txt");
        std::fs::write(&old, "moved then removed").unwrap();
        std::fs::rename(&old, &disappeared).unwrap();
        std::fs::remove_file(&disappeared).unwrap();
        let changed = "a later actual observation survives the failed move";
        std::fs::write(&path, changed).unwrap();
        let observed = std::fs::read_to_string(&path).unwrap();
        visual.update(|window, cx| {
            app.update(cx, |app, cx| {
                app.apply_reconciliation(
                    Reconciliation {
                        file_events: Vec::new(),
                        snapshot: None,
                        renames: vec![(old, disappeared)],
                        native: true,
                        documents: vec![(
                            path.clone(),
                            Ok(DiskContent::Text(observed)),
                            Instant::now(),
                        )],
                    },
                    window,
                    cx,
                )
            })
        });
        wait_until(visual, |visual| {
            visual.update(|_, cx| app.read(cx).history.last_error.is_some())
                && store
                    .list(&editor_history::Query::default())
                    .unwrap()
                    .iter()
                    .any(|row| {
                        row.path == path
                            && row.reason == editor_history::Reason::External
                            && store.read(row.id).unwrap() == changed.as_bytes()
                    })
        });
        visual.update(|_, cx| {
            let app = app.read(cx);
            assert_eq!(app.editor.read(cx).value().as_ref(), changed);
            let error = app.history.last_error.as_ref().unwrap();
            assert!(
                !error.contains("worker is busy"),
                "the real rename I/O failure must remain visible: {error}"
            );
        });
    });
}
