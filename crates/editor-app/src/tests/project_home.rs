//! Drive project selection through the same native window used by normal startup.

use crate::{app::projects::ProjectWindow, theme::apply_theme, theme::builtin_theme, typography};
use gpui_kit::VisualTestContext;
use gpui_kit::{AppContext as _, Entity, TestAppContext, component::Root, gpui, px, size};

#[path = "project_home/failures.rs"]
mod failures;
#[path = "project_home/ime.rs"]
mod ime;
#[path = "project_home/queue.rs"]
mod queue;

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

/// Click the painted control and drain native events before inspecting the resulting view.
fn click(visual: &mut VisualTestContext, selector: &'static str) {
    let bounds = visual
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("missing {selector}"));
    visual.simulate_click(bounds.center(), Default::default());
    visual.run_until_parked();
}

/// Argument-free startup must display an actionable home without creating a project editor.
#[gpui::test]
fn project_home_starts_without_a_workspace(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        typography::init(cx);
        apply_theme(builtin_theme(false), cx);
        cx.set_reduce_motion(true);
    });
    let profile = tempfile::tempdir().unwrap();
    let profile_path = profile.path().to_owned();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let view = cx.new(|cx| ProjectWindow::new(profile_path, window, cx));
        Root::new(view, window, cx)
    });
    visual.simulate_resize(size(px(1000.), px(750.)));
    visual.run_until_parked();
    assert!(visual.debug_bounds("project-open-folder").is_some());
    assert!(visual.debug_bounds("editor-shell").is_none());
}

/// Cancelling the operating system's folder picker must leave an unopened home intact.
#[gpui::test]
fn project_home_folder_picker_cancel_preserves_home(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        typography::init(cx);
        apply_theme(builtin_theme(false), cx);
        cx.set_reduce_motion(true);
    });
    let profile = tempfile::tempdir().unwrap();
    let profile_path = profile.path().to_owned();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let view = cx.new(|cx| ProjectWindow::new(profile_path, window, cx));
        Root::new(view, window, cx)
    });
    visual.simulate_resize(size(px(1000.), px(750.)));
    visual.run_until_parked();
    let button = visual.debug_bounds("project-open-folder").unwrap();
    visual.simulate_click(button.center(), Default::default());
    visual.run_until_parked();
    assert!(visual.did_prompt_for_paths());
    visual.simulate_path_prompt_response(|_| None);
    visual.run_until_parked();
    assert!(visual.debug_bounds("project-open-folder").is_some());
    assert!(visual.debug_bounds("editor-shell").is_none());
}

/// First opening requires consent; the same native flow can return home and reopen the project.
#[gpui::test]
fn project_home_restricted_project_round_trip(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        typography::init(cx);
        apply_theme(builtin_theme(false), cx);
        cx.set_reduce_motion(true);
    });
    let project = tempfile::tempdir().unwrap();
    std::fs::write(project.path().join("README.md"), "hello 中文\n").unwrap();
    let profile = tempfile::tempdir().unwrap();
    let profile_path = profile.path().to_owned();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let view = cx.new(|cx| ProjectWindow::new(profile_path, window, cx));
        Root::new(view, window, cx)
    });
    visual.simulate_resize(size(px(1000.), px(750.)));
    visual.run_until_parked();
    let button = visual.debug_bounds("project-open-folder").unwrap();
    visual.simulate_click(button.center(), Default::default());
    let path = project.path().to_owned();
    visual.simulate_path_prompt_response(move |options| {
        assert!(options.directories && !options.files && !options.multiple);
        Some(vec![path])
    });
    visual.run_until_parked();
    let restrict = visual
        .debug_bounds("project-trust-restricted")
        .expect("first open must ask for trust");
    assert!(visual.debug_bounds("editor-shell").is_none());
    visual.simulate_click(restrict.center(), Default::default());
    visual.run_until_parked();
    assert!(visual.debug_bounds("editor-shell").is_some());
    let close = visual.debug_bounds("project-close-project").unwrap();
    visual.simulate_click(close.center(), Default::default());
    visual.run_until_parked();
    assert!(visual.debug_bounds("project-open-folder").is_some());
    let recent = visual.debug_bounds("project-recent-0").unwrap();
    visual.simulate_click(recent.center(), Default::default());
    visual.run_until_parked();
    assert!(visual.debug_bounds("editor-shell").is_some());
    assert!(visual.debug_bounds("project-trust-restricted").is_none());
}

/// A late disk change must refuse Save and Close, even before the native watcher has reported it.
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
        visual.run_until_parked();
        std::fs::write(&path, "external\n").unwrap();
        click(visual, "project-close-project");
        click(visual, "project-close-save");
        assert!(visual.debug_bounds("editor-shell").is_some());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "external\n");
        click(visual, "project-decision-cancel");
        assert!(visual.debug_bounds("editor-shell").is_some());
    });
}

/// Cancel keeps native edits; Save and Close writes them and returns this same window home.
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
        visual.run_until_parked();
        click(visual, "project-close-project");
        click(visual, "project-decision-cancel");
        assert!(visual.debug_bounds("editor-shell").is_some());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "original\n");
        click(visual, "project-close-project");
        click(visual, "project-close-save");
        assert!(visual.debug_bounds("project-open-folder").is_some());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "saved 中文🙂\n");
        click(visual, "project-recent-0");
        click(visual, "project-close-project");
        assert!(visual.debug_bounds("project-close-save").is_none());
        assert!(visual.debug_bounds("project-open-folder").is_some());
    });
}

/// Explicit discard returns home without writing the native editor's unsaved text to disk.
#[gpui::test]
fn project_home_discard_leaves_disk_content_unchanged(cx: &mut TestAppContext) {
    let project = tempfile::tempdir().unwrap();
    let path = project.path().join("README.md");
    std::fs::write(&path, "original\n").unwrap();
    let profile = tempfile::tempdir().unwrap();
    with_home(cx, profile.path(), |visual, _| {
        pick_restricted_project(visual, project.path());
        visual.simulate_input("discarded");
        visual.run_until_parked();
        click(visual, "project-close-project");
        click(visual, "project-close-discard");
        assert!(visual.debug_bounds("project-open-folder").is_some());
        assert_eq!(std::fs::read_to_string(path).unwrap(), "original\n");
    });
}

/// Trust cancellation and invalid folders cannot create an editor or redirect the home to cwd.
#[gpui::test]
fn project_home_cancel_trust_and_invalid_folder_keep_home(cx: &mut TestAppContext) {
    let project = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    with_home(cx, profile.path(), |visual, _| {
        click(visual, "project-open-folder");
        let path = project.path().to_owned();
        visual.simulate_path_prompt_response(move |_| Some(vec![path]));
        visual.run_until_parked();
        click(visual, "project-decision-cancel");
        assert!(visual.debug_bounds("editor-shell").is_none());
        click(visual, "project-open-folder");
        let missing = project.path().join("missing-folder");
        visual.simulate_path_prompt_response(move |_| Some(vec![missing]));
        visual.run_until_parked();
        assert!(visual.debug_bounds("project-error").is_some());
        assert!(visual.debug_bounds("project-open-folder").is_some());
        assert!(visual.debug_bounds("editor-shell").is_none());

        // Directory validity can change while the consent dialog is waiting for the user.
        let vanished = project.path().join("vanished");
        std::fs::create_dir(&vanished).unwrap();
        click(visual, "project-open-folder");
        let selected = vanished.clone();
        visual.simulate_path_prompt_response(move |_| Some(vec![selected]));
        visual.run_until_parked();
        std::fs::remove_dir(&vanished).unwrap();
        click(visual, "project-trust-restricted");
        assert!(visual.debug_bounds("project-error").is_some());
        assert!(visual.debug_bounds("editor-shell").is_none());
        assert!(!vanished.exists());
    });
}

/// A new home reloads recent records, and an absent directory is retained until explicit removal.
#[gpui::test]
fn project_home_recent_record_survives_missing_directory(cx: &mut TestAppContext) {
    let project = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    with_home(cx, profile.path(), |visual, _| {
        pick_restricted_project(visual, project.path());
        click(visual, "project-close-project");
        assert!(visual.debug_bounds("project-recent-0").is_some());
        visual.update(|window, _| window.remove_window());
    });
    let missing = project.path().to_owned();
    project.close().unwrap();
    with_home(cx, profile.path(), |visual, _| {
        click(visual, "project-recent-0");
        assert!(visual.debug_bounds("project-error").is_some());
        assert!(visual.debug_bounds("project-recent-0").is_some());
        assert!(!missing.exists());
        click(visual, "project-remove-0");
        assert!(visual.debug_bounds("project-recent-0").is_none());
    });
}

/// Project-owned dialogs must not retain an old editor or survive its return to the home.
#[gpui::test]
fn project_home_close_releases_owned_settings_window(cx: &mut TestAppContext) {
    let project = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    with_home(cx, profile.path(), |visual, _| {
        pick_restricted_project(visual, project.path());
        click(visual, "settings-trigger");
        assert_eq!(visual.update(|_, cx| cx.windows().len()), 2);
        click(visual, "project-close-project");
        assert!(visual.debug_bounds("project-open-folder").is_some());
        assert_eq!(visual.update(|_, cx| cx.windows().len()), 1);
        click(visual, "project-recent-0");
        click(visual, "settings-trigger");
        assert_eq!(visual.update(|_, cx| cx.windows().len()), 2);
    });
}

/// A real Windows sharing denial must retain unsaved text, and retry succeeds after the lock ends.
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
        visual.run_until_parked();
        // FILE_SHARE_READ permits the preflight read, while replacement/writes remain OS-denied.
        let locked = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&path)
            .unwrap();
        click(visual, "project-close-project");
        click(visual, "project-close-save");
        assert!(visual.debug_bounds("editor-shell").is_some());
        assert!(visual.debug_bounds("project-close-save").is_some());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "original\n");
        click(visual, "project-decision-cancel");
        drop(locked);
        click(visual, "project-close-project");
        click(visual, "project-close-save");
        assert!(visual.debug_bounds("project-open-folder").is_some());
        assert_eq!(std::fs::read_to_string(path).unwrap(), "retry 中文\n");
    });
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
        click(visual, "project-close-project");
        click(visual, "project-decision-cancel");
        assert!(visual.update(|_, cx| editor.read(cx).terminal.read(cx).has_running_programs()));
        click(visual, "project-close-project");
        click(visual, "project-close-discard");
        assert!(visual.debug_bounds("project-open-folder").is_some());
        // A selected profile must own terminal snapshots and upgrades, not the ambient MeEditor store.
        let isolated_terminals = profile.path().join("run/terminal");
        assert!(
            std::fs::read_dir(&isolated_terminals)
                .unwrap()
                .any(|entry| { entry.unwrap().path().join("state.json").is_file() })
        );
        drop(editor);
        click(visual, "project-recent-0");
        visual
            .executor()
            .advance_clock(std::time::Duration::from_millis(90));
        visual.run_until_parked();
        visual.update(|window, cx| window.draw(cx).clear(cx));
        click(visual, "project-close-project");
        assert!(
            visual.debug_bounds("project-open-folder").is_some(),
            "restored history must close without a running-program prompt"
        );
    });
}

/// Keyboard selection remains usable across locales/palettes; Enter cannot grant project authority.
#[gpui::test]
fn project_home_keyboard_consent_is_safe_in_both_locales(cx: &mut TestAppContext) {
    /// Keep process-wide translation state unchanged even if a behavioral assertion fails.
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
            visual.update(|window, cx| {
                apply_theme(builtin_theme(dark), cx);
                typography::step_by(cx, 2);
                crate::theme::sync_font_sizes(cx);
                window.refresh();
            });
            visual.simulate_resize(size(px(840.), px(600.)));
            visual.run_until_parked();
            visual.simulate_keystrokes("tab");
            visual.run_until_parked();
            // GPUI's keystroke helper emits key-down; native button activation also needs key-up.
            visual.simulate_keystrokes("enter");
            visual.simulate_event(gpui_kit::KeyUpEvent {
                keystroke: gpui_kit::Keystroke::parse("enter").unwrap(),
            });
            visual.run_until_parked();
            assert!(
                visual.did_prompt_for_paths(),
                "home must be keyboard accessible in {locale}"
            );
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
            assert!(visual.debug_bounds("project-open-folder").is_some());
            visual.update(|window, _| window.remove_window());
        });
    }
}
