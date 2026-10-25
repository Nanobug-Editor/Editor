//! Native settings controls publish profile-wide tab placement without coupling document sessions.

use crate::*;
use app::editor_preferences::TabPlacement;
use gpui_kit::{AnyWindowHandle, TestAppContext, VisualTestContext, gpui};

/// Initialize shared native controls once before mounting several windows in the same test app.
fn init(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        editor_ui::init(cx);
        typography::init(cx);
        apply_theme(builtin_theme(false), cx);
        cx.set_reduce_motion(true);
    });
}

/// Open a distinct project in `profile` through the real session loader and return its retained owner.
fn mount_editor(
    cx: &mut TestAppContext,
    profile: &Path,
) -> (tempfile::TempDir, Entity<EditorApp>, AnyWindowHandle) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("settings.txt");
    std::fs::write(&path, "original document").unwrap();
    let workspace = Workspace::open(directory.path()).unwrap();
    let session = SessionState::load_in_profile(workspace.root(), profile);
    let mut application = None;
    let (_, ui) = cx.add_window_view(|window, cx| {
        let app = cx.new(|cx| {
            EditorApp::new_with_session(workspace, Some(path), Some(session), window, cx)
        });
        application = Some(app.clone());
        Root::new(app, window, cx)
    });
    ui.simulate_resize(size(px(1100.), px(800.)));
    draw(ui);
    let handle = ui.update(|window, _| window.window_handle());
    (directory, application.unwrap(), handle)
}

/// Draw `ui` only after settings observers and deferred owning-window updates have settled.
fn draw(ui: &mut VisualTestContext) {
    ui.run_until_parked();
    ui.update(|window, cx| window.draw(cx).clear(cx));
}

/// Activate the actual native control identified by `selector` in `ui`.
fn click(ui: &mut VisualTestContext, selector: &'static str) {
    let bounds = ui
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("missing {selector}"));
    ui.simulate_click(bounds.center(), Default::default());
    draw(ui);
}

/// Open `owner`'s modal settings through its visible trigger and navigate to the Editor page.
fn open_settings(owner: &mut VisualTestContext) -> VisualTestContext {
    let existing = owner.update(|_, cx| cx.windows());
    click(owner, "settings-trigger");
    let dialog = owner
        .update(|_, cx| cx.windows())
        .into_iter()
        .find(|handle| !existing.contains(handle))
        .expect("settings opens a distinct native window");
    let mut ui = VisualTestContext::from_window(dialog, &owner.cx);
    draw(&mut ui);
    click(&mut ui, "settings-nav-editor");
    ui
}

/// Read actual persisted bytes from `profile`, independently of the live application cache.
fn saved_placement(profile: &Path) -> TabPlacement {
    let stored: serde_json::Value =
        serde_json::from_slice(&std::fs::read(profile.join("editor.json")).unwrap()).unwrap();
    serde_json::from_value(stored["tab_placement"].clone()).unwrap()
}

/// Every real settings choice updates both projects in one profile while leaving another user alone.
#[gpui::test]
fn tab_settings_choices_persist_and_follow_across_project_windows(cx: &mut TestAppContext) {
    init(cx);
    let shared_profile = tempfile::tempdir().unwrap();
    let other_profile = tempfile::tempdir().unwrap();
    let (_first_files, first, first_window) = mount_editor(cx, shared_profile.path());
    let (_second_files, second, _) = mount_editor(cx, shared_profile.path());
    let (_other_files, other, _) = mount_editor(cx, other_profile.path());
    let mut first_ui = VisualTestContext::from_window(first_window, cx);
    let mut settings = open_settings(&mut first_ui);
    for (selector, expected) in [
        ("settings-tabs-top", TabPlacement::Top),
        ("settings-tabs-bottom", TabPlacement::Bottom),
        ("settings-tabs-left", TabPlacement::Left),
        ("settings-tabs-right", TabPlacement::Right),
        ("settings-tabs-tool-window", TabPlacement::ToolWindow),
    ] {
        click(&mut settings, selector);
        assert_eq!(saved_placement(shared_profile.path()), expected);
        settings.update(|_, cx| {
            for editor in [&first, &second] {
                assert_eq!(editor.read(cx).tab_placement, expected);
                assert_eq!(
                    editor.read(cx).editor.read(cx).text().to_string(),
                    "original document"
                );
            }
            assert_eq!(other.read(cx).tab_placement, TabPlacement::Top);
        });
        assert!(
            settings
                .debug_bounds("settings-tab-placement-error")
                .is_none()
        );
    }
    assert!(!other_profile.path().join("editor.json").exists());
    // A newly created project must read the saved non-default mode before its first visible frame.
    let (_new_files, newly_opened, _) = mount_editor(cx, shared_profile.path());
    cx.update(|cx| {
        assert_eq!(
            newly_opened.read(cx).tab_placement,
            TabPlacement::ToolWindow
        )
    });
}

/// Empty startup exposes the same settings and keeps malformed profile data visible until repaired.
#[gpui::test]
fn tab_settings_empty_window_reports_errors_and_saves_before_opening_a_project(
    cx: &mut TestAppContext,
) {
    init(cx);
    let profile = tempfile::tempdir().unwrap();
    let path = profile.path().join("editor.json");
    let original = b"{broken editor preferences";
    std::fs::write(&path, original).unwrap();
    let (_, empty_ui) = cx.add_window_view(|window, cx| {
        let empty =
            cx.new(|cx| app::projects::ProjectWindow::new(profile.path().into(), window, cx));
        Root::new(empty, window, cx)
    });
    empty_ui.simulate_resize(size(px(1100.), px(800.)));
    draw(empty_ui);
    let empty_window = empty_ui.update(|window, _| window.window_handle());
    assert!(empty_ui.debug_bounds("empty-editor-shell").is_some());
    let mut settings = open_settings(empty_ui);
    assert!(
        settings
            .debug_bounds("settings-tab-placement-error")
            .is_some()
    );
    click(&mut settings, "settings-tabs-left");
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert!(
        settings
            .debug_bounds("settings-tab-placement-error")
            .is_some()
    );

    // Repairing the isolated fixture permits a normal retry without reopening the settings dialog.
    std::fs::write(&path, b"{}").unwrap();
    click(&mut settings, "settings-tabs-left");
    assert_eq!(saved_placement(profile.path()), TabPlacement::Left);
    assert!(
        settings
            .debug_bounds("settings-tab-placement-error")
            .is_none()
    );
    settings.simulate_keystrokes("escape");
    settings.run_until_parked();
    let mut empty_ui = VisualTestContext::from_window(empty_window, cx);
    draw(&mut empty_ui);
    assert!(empty_ui.debug_bounds("empty-editor-shell").is_some());
    let (_files, editor, _) = mount_editor(cx, profile.path());
    cx.update(|cx| assert_eq!(editor.read(cx).tab_placement, TabPlacement::Left));
}
