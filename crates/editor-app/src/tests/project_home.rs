//! Drive project selection through the same native window used by normal startup.

use crate::{app::projects::ProjectWindow, theme::apply_theme, theme::builtin_theme, typography};
use gpui_kit::VisualTestContext;
use gpui_kit::{AppContext as _, Entity, TestAppContext, component::Root, gpui, px, size};

#[path = "project_home/failures.rs"]
mod failures;
#[path = "project_home/ime.rs"]
mod ime;
#[path = "project_home/menu.rs"]
mod menu;
#[path = "project_home/queue.rs"]
mod queue;
#[path = "project_home/startup.rs"]
mod startup;
#[path = "project_home/switcher.rs"]
mod switcher;
#[path = "project_home/welcome.rs"]
mod welcome;

// Reuse the same production home/open path for cross-window language behavior.
#[path = "project_windows/catalog.rs"]
mod concurrent_catalog;
#[path = "project_windows/windows.rs"]
mod concurrent_windows;
// External requests exercise the same GPUI lifecycle and actual document targets.
#[path = "project_windows/external.rs"]
mod external_windows;

/// Use the production window and platform picker in an isolated profile for each behavioral scenario.
fn with_home(
    cx: &mut TestAppContext,
    profile: &std::path::Path,
    run: impl FnOnce(&mut VisualTestContext, &Entity<ProjectWindow>),
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        typography::init(cx);
        apply_theme(builtin_theme(false), cx);
        cx.set_reduce_motion(true);
        crate::app::shortcuts::bootstrap::configure(cx, profile.join("shortcuts.json")).unwrap();
    });
    let profile = profile.to_owned();
    let slot = std::rc::Rc::new(std::cell::RefCell::new(None));
    let capture = slot.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let host = cx.new(|cx| ProjectWindow::new(profile, window, cx));
        *capture.borrow_mut() = Some(host.clone());
        Root::new(host, window, cx)
    });
    visual.simulate_resize(size(px(1000.), px(750.)));
    visual.run_until_parked();
    run(visual, &slot.borrow_mut().take().unwrap());
}

/// Every selection enters through the native picker instead of a test-only editor constructor.
fn pick_restricted_project(visual: &mut VisualTestContext, path: &std::path::Path) {
    click(visual, "project-open-folder");
    let path = path.to_owned();
    visual.simulate_path_prompt_response(move |_| Some(vec![path]));
    visual.run_until_parked();
    click(visual, "project-trust-restricted");
}

/// The removed close-project entry keeps lifecycle coverage through its existing production owner.
/// These older home/session scenarios are separate from menu and native-window interaction checks.
fn request_close_project(visual: &mut VisualTestContext) {
    visual.update(|window, cx| {
        let root = window.root::<Root>().unwrap().unwrap();
        let host = root
            .read(cx)
            .view()
            .clone()
            .downcast::<ProjectWindow>()
            .unwrap();
        host.update(cx, |host, cx| {
            host.request_close(crate::app::projects::CloseIntent::Window, window, cx)
        });
    });
    visual.run_until_parked();
}

/// Fixture commands use the same painted dropdown actions as the user.
fn click(visual: &mut VisualTestContext, selector: &'static str) {
    let project_action = match selector {
        "project-open-folder" => Some("native-menu-project-open-folder"),
        "project-clone" => Some("native-menu-project-clone"),
        "project-close-project" => Some("native-menu-project-close-project"),
        _ => None,
    };
    if let Some(action) = project_action {
        if visual.debug_bounds(action).is_none() {
            click(visual, "project-switcher-trigger");
        }
        click(visual, action);
        return;
    }
    let app_action = match selector {
        "project-open-another" => Some("native-menu-project-open-another"),
        "project-show-home" => Some("native-menu-project-show-home"),
        "project-quit-application" => Some("native-menu-project-quit-application"),
        _ => None,
    };
    if let Some(action) = app_action {
        click(visual, "application-menu-trigger");
        click(visual, action);
        return;
    }
    if let Some(index) = selector
        .strip_prefix("project-recent-")
        .and_then(|index| index.parse::<usize>().ok())
    {
        if visual.debug_bounds("project-search").is_none() {
            click(visual, "project-switcher-trigger");
        }
        let bounds = recent_bounds(visual, index).expect("recent project must be visible");
        visual.simulate_click(bounds.center(), Default::default());
    } else if let Some(index) = selector
        .strip_prefix("project-remove-")
        .and_then(|index| index.parse::<usize>().ok())
    {
        if visual.debug_bounds("project-search").is_none() {
            click(visual, "project-switcher-trigger");
        }
        let path = recent_path(visual, index).expect("record must exist before removal");
        let selector =
            Box::leak(format!("native-menu-project-remove-{}", path_id(&path)).into_boxed_str());
        let bounds = visual
            .debug_bounds(selector)
            .expect("remove action must be visible");
        visual.simulate_click(bounds.center(), Default::default());
    } else {
        let bounds = visual
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("missing {selector}"));
        visual.simulate_click(bounds.center(), Default::default());
    }
    visual.run_until_parked();
}
/// Selector lookup uses persistent path identity while tests interact with painted controls.
fn path_id(path: &std::path::Path) -> String {
    use std::fmt::Write as _;
    let mut id = String::from("project-choice-");
    for byte in path.as_os_str().as_encoded_bytes() {
        let _ = write!(id, "{byte:02x}");
    }
    id
}
fn recent_path(visual: &mut VisualTestContext, index: usize) -> Option<std::path::PathBuf> {
    visual.update(|window, cx| {
        super::controller::recent_paths(window.window_handle(), cx)
            .get(index)
            .cloned()
    })
}
fn recent_bounds(
    visual: &mut VisualTestContext,
    index: usize,
) -> Option<gpui_kit::Bounds<gpui_kit::Pixels>> {
    let path = recent_path(visual, index)?;
    let selector = Box::leak(format!("native-menu-{}", path_id(&path)).into_boxed_str());
    visual.debug_bounds(selector)
}
/// Native window membership remains observable even after this fixture's root has closed.
fn window_count(visual: &VisualTestContext) -> usize {
    visual.cx.update(|cx| cx.windows().len())
}

/// Cancelling the picker preserves an empty editor with no workspace owner.
#[gpui::test]
fn project_home_folder_picker_cancel_preserves_home(cx: &mut TestAppContext) {
    let profile = tempfile::tempdir().unwrap();
    with_home(cx, profile.path(), |visual, host| {
        click(visual, "project-open-folder");
        assert!(visual.did_prompt_for_paths());
        visual.simulate_path_prompt_response(|_| None);
        visual.run_until_parked();
        assert!(visual.debug_bounds("empty-editor-shell").is_some());
        assert!(visual.update(|_, cx| host.read(cx).editor.is_none()));
        assert!(visual.debug_bounds("native-popup-menu").is_none());
    });
}

/// Closing the project removes its native window; a later explicit launch can reopen its recorded session.
#[gpui::test]
fn project_home_restricted_project_round_trip(cx: &mut TestAppContext) {
    let project = tempfile::tempdir().unwrap();
    std::fs::write(project.path().join("README.md"), "hello 中文\n").unwrap();
    let profile = tempfile::tempdir().unwrap();
    with_home(cx, profile.path(), |visual, _| {
        pick_restricted_project(visual, project.path());
        request_close_project(visual);
        assert_eq!(window_count(visual), 0);
    });
    with_home(cx, profile.path(), |visual, _| {
        click(visual, "project-recent-0");
        assert!(visual.debug_bounds("editor-shell").is_some());
        assert!(visual.debug_bounds("project-trust-accept").is_none());
    });
}

/// Bulk saving refuses an external disk change even before watcher reconciliation.
#[gpui::test]
fn project_home_close_refuses_external_disk_changes(cx: &mut TestAppContext) {
    let project = tempfile::tempdir().unwrap();
    let path = project.path().join("README.md");
    std::fs::write(&path, "original\n").unwrap();
    let profile = tempfile::tempdir().unwrap();
    with_home(cx, profile.path(), |visual, _| {
        pick_restricted_project(visual, project.path());
        visual.simulate_keystrokes("ctrl-a");
        visual.simulate_input("changed 中文\n");
        std::fs::write(&path, "external\n").unwrap();
        request_close_project(visual);
        click(visual, "project-close-save");
        assert!(visual.debug_bounds("editor-shell").is_some());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "external\n");
        click(visual, "project-decision-cancel");
    });
}

/// Cancel preserves input; Save closes the window only after writing the real document.
#[gpui::test]
fn project_home_cancel_then_save_preserves_real_input(cx: &mut TestAppContext) {
    let project = tempfile::tempdir().unwrap();
    let path = project.path().join("README.md");
    std::fs::write(&path, "original\n").unwrap();
    let profile = tempfile::tempdir().unwrap();
    with_home(cx, profile.path(), |visual, _| {
        pick_restricted_project(visual, project.path());
        visual.simulate_keystrokes("ctrl-a");
        visual.simulate_input("saved 中文🙂\n");
        request_close_project(visual);
        click(visual, "project-decision-cancel");
        assert_eq!(window_count(visual), 1);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "original\n");
        request_close_project(visual);
        click(visual, "project-close-save");
        assert_eq!(window_count(visual), 0);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "saved 中文🙂\n");
    });
}

/// Explicit discard closes the native window without writing unsaved text.
#[gpui::test]
fn project_home_discard_leaves_disk_content_unchanged(cx: &mut TestAppContext) {
    let project = tempfile::tempdir().unwrap();
    let path = project.path().join("README.md");
    std::fs::write(&path, "original\n").unwrap();
    let profile = tempfile::tempdir().unwrap();
    with_home(cx, profile.path(), |visual, _| {
        pick_restricted_project(visual, project.path());
        visual.simulate_input("discarded");
        request_close_project(visual);
        click(visual, "project-close-discard");
        assert_eq!(window_count(visual), 0);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "original\n");
    });
}

/// Cancellation and disappearing directories cannot redirect empty startup to cwd.
#[gpui::test]
fn project_home_cancel_trust_and_invalid_folder_keep_home(cx: &mut TestAppContext) {
    let project = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    with_home(cx, profile.path(), |visual, host| {
        click(visual, "project-open-folder");
        let path = project.path().to_owned();
        visual.simulate_path_prompt_response(move |_| Some(vec![path]));
        visual.run_until_parked();
        click(visual, "project-decision-cancel");
        click(visual, "project-open-folder");
        let missing = project.path().join("missing-folder");
        visual.simulate_path_prompt_response(move |_| Some(vec![missing]));
        visual.run_until_parked();
        assert!(visual.debug_bounds("project-error").is_some());
        assert!(visual.update(|_, cx| host.read(cx).editor.is_none()));
        let vanished = project.path().join("vanished");
        std::fs::create_dir(&vanished).unwrap();
        click(visual, "project-open-folder");
        let selected = vanished.clone();
        visual.simulate_path_prompt_response(move |_| Some(vec![selected]));
        visual.run_until_parked();
        std::fs::remove_dir(&vanished).unwrap();
        click(visual, "project-trust-restricted");
        assert!(visual.debug_bounds("project-error").is_some());
        assert!(visual.update(|_, cx| host.read(cx).editor.is_none()));
    });
}

/// Missing history records remain until explicitly removed, without deleting any project files.
#[gpui::test]
fn project_home_recent_record_survives_missing_directory(cx: &mut TestAppContext) {
    let project = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    with_home(cx, profile.path(), |visual, _| {
        pick_restricted_project(visual, project.path());
        request_close_project(visual);
        assert_eq!(window_count(visual), 0);
    });
    project.close().unwrap();
    with_home(cx, profile.path(), |visual, _| {
        click(visual, "project-recent-0");
        assert!(visual.debug_bounds("project-error").is_some());
        click(visual, "project-switcher-trigger");
        assert!(recent_bounds(visual, 0).is_some());
        click(visual, "project-remove-0");
        assert!(recent_bounds(visual, 0).is_none());
    });
}

/// Settings cannot retain a closed project window or prevent last-window exit.
#[gpui::test]
fn project_home_close_releases_owned_settings_window(cx: &mut TestAppContext) {
    let project = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    with_home(cx, profile.path(), |visual, _| {
        pick_restricted_project(visual, project.path());
        click(visual, "settings-trigger");
        assert_eq!(window_count(visual), 2);
        request_close_project(visual);
        assert_eq!(window_count(visual), 0);
    });
}

/// A Windows sharing denial preserves unsaved input, and retry closes after the lock is released.
#[cfg(windows)]
#[gpui::test]
fn project_home_save_failure_keeps_editor_and_allows_retry(cx: &mut TestAppContext) {
    use std::os::windows::fs::OpenOptionsExt as _;
    let project = tempfile::tempdir().unwrap();
    let path = project.path().join("README.md");
    std::fs::write(&path, "original\n").unwrap();
    let profile = tempfile::tempdir().unwrap();
    with_home(cx, profile.path(), |visual, _| {
        pick_restricted_project(visual, project.path());
        visual.simulate_keystrokes("ctrl-a");
        visual.simulate_input("retry 中文\n");
        let locked = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&path)
            .unwrap();
        request_close_project(visual);
        click(visual, "project-close-save");
        assert_eq!(window_count(visual), 1);
        assert!(visual.debug_bounds("project-close-save").is_some());
        click(visual, "project-decision-cancel");
        drop(locked);
        request_close_project(visual);
        click(visual, "project-close-save");
        assert_eq!(window_count(visual), 0);
        assert_eq!(std::fs::read_to_string(path).unwrap(), "retry 中文\n");
    });
}

/// The project menu remains usable in both locales; plain Enter never grants trust.
#[gpui::test]
fn project_home_keyboard_consent_is_safe_in_both_locales(cx: &mut TestAppContext) {
    struct RestoreLocale(String);
    impl Drop for RestoreLocale {
        fn drop(&mut self) {
            rust_i18n::set_locale(&self.0);
        }
    }
    let _restore = RestoreLocale(rust_i18n::locale().to_string());
    let project = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    for (locale, dark) in [("zh-CN", false), ("en", true)] {
        rust_i18n::set_locale(locale);
        with_home(cx, profile.path(), |visual, _| {
            visual.update(|_, cx| apply_theme(builtin_theme(dark), cx));
            visual.run_until_parked();
            click(visual, "project-switcher-trigger");
            visual.simulate_keystrokes("enter");
            visual.run_until_parked();
            assert!(visual.did_prompt_for_paths());
            let path = project.path().to_owned();
            visual.simulate_path_prompt_response(move |_| Some(vec![path]));
            visual.run_until_parked();
            assert!(visual.debug_bounds("project-trust-accept").is_some());
            visual.simulate_keystrokes("enter");
            visual.run_until_parked();
            assert!(visual.debug_bounds("editor-shell").is_none());
            visual.simulate_keystrokes("escape");
            visual.run_until_parked();
            assert!(visual.debug_bounds("project-trust-accept").is_none());
            visual.update(|window, _| window.remove_window());
        });
    }
}
/// The real terminal runs until consent, and restoring its painted history must not restart it.
#[gpui::test]
fn project_home_terminal_reopen_keeps_programs_stopped(cx: &mut TestAppContext) {
    let project = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    with_home(cx, profile.path(), |visual, host| {
        click(visual, "project-open-folder");
        let path = project.path().to_owned();
        visual.simulate_path_prompt_response(move |_| Some(vec![path]));
        visual.run_until_parked();
        click(visual, "project-trust-accept");
        let editor = visual.update(|_, cx| host.read(cx).editor.clone().unwrap());
        click(visual, "terminal-toggle");
        // Wait for the actual Shell prompt, rather than treating a submitted launch as success.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            visual
                .executor()
                .advance_clock(std::time::Duration::from_millis(30));
            visual.run_until_parked();
            visual.update(|window, cx| window.draw(cx).clear(cx));
            let output = crate::terminal::tests::painted(&editor, visual);
            if output.contains(if cfg!(windows) { "PS" } else { "$" }) {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "Shell did not produce a prompt"
            );
            std::thread::sleep(std::time::Duration::from_millis(15));
        }
        request_close_project(visual);
        click(visual, "project-decision-cancel");
        assert!(visual.update(|_, cx| editor.read(cx).terminal.read(cx).has_running_programs()));
        request_close_project(visual);
        click(visual, "project-close-discard");
        assert_eq!(window_count(visual), 0);
        // A selected profile must own terminal snapshots and upgrades, not the ambient MeEditor store.
        let isolated_terminals = profile.path().join("run/terminal");
        assert!(
            std::fs::read_dir(&isolated_terminals)
                .unwrap()
                .any(|entry| { entry.unwrap().path().join("state.json").is_file() })
        );
        drop(editor);
    });
    with_home(cx, profile.path(), |visual, _| {
        click(visual, "project-recent-0");
        visual
            .executor()
            .advance_clock(std::time::Duration::from_millis(90));
        visual.run_until_parked();
        request_close_project(visual);
        assert_eq!(
            window_count(visual),
            0,
            "restored terminal history cannot restart programs"
        );
    });
}
