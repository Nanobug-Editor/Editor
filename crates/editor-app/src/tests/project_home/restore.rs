//! Startup restoration is configured through the real settings dialog, even without a project.

use super::*;
use crate::app::startup::preferences::{self, Preferences};

/// The application preference must be reachable before acquiring a workspace.
#[gpui::test]
fn startup_restore_setting_is_available_without_a_project(cx: &mut TestAppContext) {
    let profile = tempfile::tempdir().unwrap();
    with_home(cx, profile.path(), |visual, host| {
        click(visual, "settings-trigger");
        let handle = visual.update(|_, cx| host.read(cx).dialog_window.unwrap());
        let mut settings = VisualTestContext::from_window(handle.into(), &visual.cx);
        settings.run_until_parked();
        assert!(
            settings
                .debug_bounds("settings-restore-last-project")
                .is_some(),
            "Appearance & Behavior must expose the startup restore setting"
        );
        assert!(
            Preferences::load(profile.path())
                .unwrap()
                .restore_last_project
        );
        click(&mut settings, "settings-restore-last-project");
        assert!(
            !Preferences::load(profile.path())
                .unwrap()
                .restore_last_project
        );
        click(&mut settings, "settings-theme-dark");
        click(&mut settings, "settings-restore-last-project");
        assert!(
            Preferences::load(profile.path())
                .unwrap()
                .restore_last_project
        );
        assert!(settings.debug_bounds("settings-startup-error").is_none());
        // Base activation requires the complete platform down/up pair, not only a command keystroke.
        settings.update(|window, cx| {
            let keystroke = gpui_kit::Keystroke::parse("space").unwrap();
            window.dispatch_event(
                gpui_kit::PlatformInput::KeyDown(gpui_kit::KeyDownEvent {
                    keystroke: keystroke.clone(),
                    is_held: false,
                    prefer_character_input: false,
                }),
                cx,
            );
            window.dispatch_event(
                gpui_kit::PlatformInput::KeyUp(gpui_kit::KeyUpEvent { keystroke }),
                cx,
            );
        });
        settings.run_until_parked();
        assert!(
            !Preferences::load(profile.path())
                .unwrap()
                .restore_last_project
        );
        assert!(visual.update(|_, cx| host.read(cx).editor.is_none()));
    });
}

/// Closing through the real lifecycle records a root and reopening preserves the saved restricted session.
#[gpui::test]
fn startup_restore_last_closed_project_and_session(cx: &mut TestAppContext) {
    let profile = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let root = project.path().canonicalize().unwrap();
    let path = project.path().join("README.md");
    std::fs::write(&path, "original 中文🙂\n").unwrap();
    with_home(cx, profile.path(), |visual, _| {
        pick_restricted_project(visual, project.path());
        visual.simulate_keystrokes("ctrl-a");
        visual.simulate_input("restored 中文🙂\n");
        visual.simulate_keystrokes("ctrl-s");
        visual.run_until_parked();
        request_close_project(visual);
        assert_eq!(window_count(visual), 0);
    });
    assert_eq!(
        preferences::target(profile.path(), None),
        Some(root.clone())
    );
    with_home(cx, profile.path(), |visual, host| {
        let target = preferences::target(profile.path(), None).unwrap();
        visual.update(|window, cx| {
            host.update(cx, |host, cx| host.open_path(target, window, cx));
        });
        visual.run_until_parked();
        assert!(visual.debug_bounds("editor-shell").is_some());
        assert!(visual.debug_bounds("project-trust-restricted").is_none());
        visual.update(|_, cx| {
            let editor = host.read(cx).editor.as_ref().unwrap().read(cx);
            assert_eq!(editor.workspace.root(), root);
            assert!(!editor.session_state.workspace_trusted);
            assert_eq!(
                editor.editor.read(cx).value().to_string(),
                "restored 中文🙂\n"
            );
        });
        click(visual, "settings-trigger");
        let handle = visual.update(|_, cx| {
            host.read(cx)
                .editor
                .as_ref()
                .unwrap()
                .read(cx)
                .dialog_window
                .unwrap()
        });
        let mut settings = VisualTestContext::from_window(handle.into(), &visual.cx);
        settings.run_until_parked();
        click(&mut settings, "settings-restore-last-project");
        assert!(
            !Preferences::load(profile.path())
                .unwrap()
                .restore_last_project
        );
        click(&mut settings, "app-dialog-close");
        request_close_project(visual);
    });
    assert!(preferences::target(profile.path(), None).is_none());
    assert_eq!(
        Preferences::load(profile.path()).unwrap().last_project,
        Some(root)
    );
}

/// The last closed project can differ from the most recently opened history entry.
#[gpui::test]
fn startup_restore_uses_close_order_instead_of_recent_open_order(cx: &mut TestAppContext) {
    let profile = tempfile::tempdir().unwrap();
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let mut first = None;
    with_home(cx, profile.path(), |visual, _| {
        pick_restricted_project(visual, a.path());
        first = Some(visual.clone());
    });
    with_home(cx, profile.path(), |visual, _| {
        pick_restricted_project(visual, b.path());
        request_close_project(visual);
        assert_eq!(window_count(visual), 1);
    });
    request_close_project(&mut first.unwrap());
    assert_eq!(
        preferences::target(profile.path(), None),
        Some(a.path().canonicalize().unwrap())
    );
    let recent: serde_json::Value = serde_json::from_slice(
        &std::fs::read(profile.path().join("recent-projects.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        recent[0]["path"],
        b.path().canonicalize().unwrap().to_string_lossy().as_ref()
    );
}

/// A cancelled close leaves the prior exit record untouched even after a different project was opened.
#[gpui::test]
fn startup_restore_cancelled_close_keeps_previous_exit(cx: &mut TestAppContext) {
    let profile = tempfile::tempdir().unwrap();
    let previous = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    std::fs::write(project.path().join("README.md"), "clean\n").unwrap();
    preferences::remember_project(profile.path(), previous.path()).unwrap();
    with_home(cx, profile.path(), |visual, _| {
        pick_restricted_project(visual, project.path());
        visual.simulate_input("unsaved 中文");
        visual.run_until_parked();
        request_close_project(visual);
        click(visual, "project-decision-cancel");
        assert_eq!(
            preferences::target(profile.path(), None),
            Some(previous.path().to_owned())
        );
        assert!(visual.debug_bounds("editor-shell").is_some());
    });
}

/// Quit captures the last-used project before other windows receive cleanup and close prompts.
#[gpui::test]
fn startup_restore_application_quit_uses_last_active_project(cx: &mut TestAppContext) {
    let profile = tempfile::tempdir().unwrap();
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let mut first = None;
    with_home(cx, profile.path(), |visual, _| {
        pick_restricted_project(visual, a.path());
        first = Some(visual.clone());
    });
    with_home(cx, profile.path(), |visual, _| {
        pick_restricted_project(visual, b.path())
    });
    let mut first = first.unwrap();
    first.update(|window, _| window.activate_window());
    first.run_until_parked();
    click(&mut first, "project-quit-application");
    assert_eq!(window_count(&first), 0);
    assert_eq!(
        preferences::target(profile.path(), None),
        Some(a.path().canonicalize().unwrap())
    );
}

/// A failed session save cannot become the next startup project; a successful retry can.
#[cfg(windows)]
#[gpui::test]
fn startup_restore_failed_cleanup_keeps_previous_exit(cx: &mut TestAppContext) {
    use std::os::windows::fs::OpenOptionsExt as _;
    let profile = tempfile::tempdir().unwrap();
    let previous = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    preferences::remember_project(profile.path(), previous.path()).unwrap();
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
        assert_eq!(
            preferences::target(profile.path(), None),
            Some(previous.path().to_owned())
        );
        drop(locked);
        request_close_project(visual);
        assert_eq!(window_count(visual), 0);
    });
    assert_eq!(
        preferences::target(profile.path(), None),
        Some(project.path().canonicalize().unwrap())
    );
}

/// An atomic preference write failure leaves the checkbox unchanged and exposes a retryable error.
#[cfg(windows)]
#[gpui::test]
fn startup_restore_setting_write_failure_is_visible_and_retryable(cx: &mut TestAppContext) {
    use std::os::windows::fs::OpenOptionsExt as _;
    let profile = tempfile::tempdir().unwrap();
    Preferences::default().save(profile.path()).unwrap();
    with_home(cx, profile.path(), |visual, host| {
        click(visual, "settings-trigger");
        let handle = visual.update(|_, cx| host.read(cx).dialog_window.unwrap());
        let mut settings = VisualTestContext::from_window(handle.into(), &visual.cx);
        settings.run_until_parked();
        let locked = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(profile.path().join("startup.json"))
            .unwrap();
        click(&mut settings, "settings-restore-last-project");
        assert!(settings.debug_bounds("settings-startup-error").is_some());
        assert!(
            Preferences::load(profile.path())
                .unwrap()
                .restore_last_project
        );
        drop(locked);
        click(&mut settings, "settings-restore-last-project");
        assert!(settings.debug_bounds("settings-startup-error").is_none());
        assert!(
            !Preferences::load(profile.path())
                .unwrap()
                .restore_last_project
        );
    });
}
