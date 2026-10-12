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
        click(visual, "project-close-project");
        assert!(visual.debug_bounds("project-error").is_some());
        assert!(visual.debug_bounds("editor-shell").is_some());
        drop(locked);
        click(visual, "project-close-project");
        assert!(visual.debug_bounds("project-open-folder").is_some());
        assert!(
            visual.debug_bounds("project-error").is_none(),
            "a successful retry must retire the old failure banner"
        );
    });
}
