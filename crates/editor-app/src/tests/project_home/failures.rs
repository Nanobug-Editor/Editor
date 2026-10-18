//! Filesystem failure scenarios enter the same project owner's native close flow.

use super::*;

/// Cleanup persistence errors leave an editable project, with a successful close after the lock is released.
#[cfg(windows)]
#[gpui::test]
fn project_home_cleanup_persistence_failure_allows_retry(cx: &mut TestAppContext) {
    use std::os::windows::fs::OpenOptionsExt as _;
    let project = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    with_home(cx, profile.path(), |visual, host| {
        pick_restricted_project(visual, project.path());
        let session = visual.update(|_, cx| {
            host.read(cx)
                .editor
                .as_ref()
                .unwrap()
                .read(cx)
                .session_state
                .file_path()
                .unwrap()
        });
        let locked = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(session)
            .unwrap();
        request_close_project(visual);
        assert!(visual.debug_bounds("project-error").is_some());
        assert!(visual.debug_bounds("editor-shell").is_some());
        drop(locked);
        request_close_project(visual);
        assert_eq!(window_count(visual), 0);
    });
}

/// Saving another dirty tab cannot implicitly recreate a clean document deleted outside the editor.
#[gpui::test]
fn project_home_close_refuses_clean_deleted_tab_in_bulk_save(cx: &mut TestAppContext) {
    let project = tempfile::tempdir().unwrap();
    let deleted = project.path().join("README.md");
    let dirty = project.path().join("other.txt");
    std::fs::write(&deleted, "keep-deleted\n").unwrap();
    std::fs::write(&dirty, "original\n").unwrap();
    let profile = tempfile::tempdir().unwrap();
    with_home(cx, profile.path(), |visual, host| {
        pick_restricted_project(visual, project.path());
        let editor = visual.update(|_, cx| host.read(cx).editor.clone().unwrap());
        std::fs::remove_file(&deleted).unwrap();
        // Use the normal reconciliation receipt: tests have no untracked native watcher thread.
        visual.update(|window, cx| {
            editor.update(cx, |app, cx| {
                let canonical = app
                    .tabs
                    .iter()
                    .find(|tab| tab.path().file_name() == deleted.file_name())
                    .unwrap()
                    .path()
                    .to_owned();
                app.apply_reconciliation(
                    crate::Reconciliation {
                        file_events: vec![],
                        snapshot: None,
                        renames: vec![],
                        native: true,
                        documents: vec![(
                            canonical,
                            Err(std::io::Error::from(std::io::ErrorKind::NotFound)),
                            std::time::Instant::now(),
                        )],
                    },
                    window,
                    cx,
                );
                // An earlier explicit save confirmation must not authorize a later bulk close overwrite.
                app.save_current(cx);
                app.open_file(dirty.clone(), window, cx);
            })
        });
        visual.run_until_parked();
        visual.simulate_input("changed 中文🙂\n");
        visual.run_until_parked();
        request_close_project(visual);
        click(visual, "project-close-save");
        assert!(visual.debug_bounds("editor-shell").is_some());
        assert!(
            !deleted.exists(),
            "bulk close must not recreate the externally deleted document"
        );
        assert_eq!(std::fs::read_to_string(&dirty).unwrap(), "original\n");
        click(visual, "project-decision-cancel");
    });
}
