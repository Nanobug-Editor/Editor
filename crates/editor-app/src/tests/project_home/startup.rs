//! Argument-free startup uses the normal editor chrome without acquiring a workspace.

use super::*;
use gpui_kit::VisualContext as _;

/// Send the platform's entered/submit sequence through the painted empty window.
fn drop_path(visual: &mut VisualTestContext, file: &std::path::Path) {
    use gpui_kit::{ExternalPaths, FileDropEvent, PlatformInput};
    let position = visual.debug_bounds("empty-editor-shell").unwrap().center();
    visual.update(|window, cx| {
        window.dispatch_event(
            PlatformInput::FileDrop(FileDropEvent::Entered {
                position,
                paths: ExternalPaths(vec![file.to_owned()].into()),
            }),
            cx,
        );
        window.draw(cx).clear(cx);
        // DragOver re-evaluates hitboxes after drawing a hover style or removing an earlier overlay.
        window.dispatch_event(
            PlatformInput::FileDrop(FileDropEvent::Pending { position }),
            cx,
        );
        window.dispatch_event(
            PlatformInput::FileDrop(FileDropEvent::Submit { position }),
            cx,
        );
        // The OS ends its drag even when a popup rejects the drop and leaves the typed payload unconsumed.
        window.dispatch_event(PlatformInput::FileDrop(FileDropEvent::Exited), cx);
    });
    visual.run_until_parked();
}

/// A platform file drop uses the containing real project and retains its explicit trust decision.
#[gpui::test]
fn empty_startup_opens_a_dropped_file_after_project_consent(cx: &mut TestAppContext) {
    let profile = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let file = project.path().join("拖入文件.txt");
    let contents = "Dropped file 中文🙂\n";
    std::fs::write(&file, contents).unwrap();
    with_home(cx, profile.path(), |visual, host| {
        drop_path(visual, &file);
        assert!(visual.debug_bounds("project-trust-restricted").is_some());
        assert!(visual.update(|_, cx| host.read(cx).editor.is_none()));
        click(visual, "project-trust-restricted");
        assert!(visual.debug_bounds("editor-shell").is_some());
        assert_eq!(window_count(visual), 1);
        visual.update(|_, cx| {
            let editor = host.read(cx).editor.as_ref().unwrap().read(cx);
            assert_eq!(
                editor.workspace.root(),
                project.path().canonicalize().unwrap()
            );
            assert_eq!(editor.editor.read(cx).value().to_string(), contents);
            assert!(!editor.session_state.workspace_trusted);
        });
        assert_eq!(std::fs::read_to_string(&file).unwrap(), contents);
    });
}

/// A menu or pending consent cannot silently replace the user's selected drop target.
#[gpui::test]
fn empty_startup_file_drop_respects_overlays_and_consent(cx: &mut TestAppContext) {
    let profile = tempfile::tempdir().unwrap();
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let a = first.path().join("first.txt");
    let b = second.path().join("second.txt");
    std::fs::write(&a, "first target").unwrap();
    std::fs::write(&b, "second target").unwrap();
    with_home(cx, profile.path(), |visual, host| {
        click(visual, "project-switcher-trigger");
        drop_path(visual, &a);
        assert!(visual.debug_bounds("project-search").is_some());
        assert!(visual.debug_bounds("project-trust-restricted").is_none());
        visual.simulate_keystrokes("escape");
        visual.run_until_parked();
        assert!(
            visual.debug_bounds("project-search").is_none(),
            "Escape should dismiss the retained popup after the OS drag ends"
        );
        drop_path(visual, &a);
        assert!(
            visual.debug_bounds("project-trust-restricted").is_some(),
            "a fresh drop must enter consent"
        );
        drop_path(visual, &b);
        assert!(
            visual.debug_bounds("project-trust-restricted").is_some(),
            "a second drop must retain pending consent"
        );
        click(visual, "project-trust-restricted");
        assert_eq!(
            visual.update(|_, cx| host
                .read(cx)
                .editor
                .as_ref()
                .unwrap()
                .read(cx)
                .editor
                .read(cx)
                .value()
                .to_string()),
            "first target"
        );
        assert_eq!(window_count(visual), 1);
    });
}

/// Cancelling project consent leaves an empty surface that can accept a different file later.
#[gpui::test]
fn empty_startup_cancelled_file_drop_allows_a_new_target(cx: &mut TestAppContext) {
    let profile = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let a = project.path().join("first.txt");
    let b = project.path().join("second.txt");
    std::fs::write(&a, "first target").unwrap();
    std::fs::write(&b, "second target").unwrap();
    with_home(cx, profile.path(), |visual, host| {
        drop_path(visual, &a);
        click(visual, "project-decision-cancel");
        assert!(visual.update(|_, cx| host.read(cx).editor.is_none()));
        drop_path(visual, &b);
        click(visual, "project-trust-restricted");
        assert_eq!(
            visual.update(|_, cx| host
                .read(cx)
                .editor
                .as_ref()
                .unwrap()
                .read(cx)
                .editor
                .read(cx)
                .value()
                .to_string()),
            "second target"
        );
        assert_eq!(window_count(visual), 1);
    });
}

/// Opening project management is explicit; empty startup keeps workspace tools unavailable.
#[gpui::test]
fn empty_startup_keeps_project_dropdown_closed(cx: &mut TestAppContext) {
    let profile = tempfile::tempdir().unwrap();
    with_home(cx, profile.path(), |visual, host| {
        assert!(visual.debug_bounds("custom-title-bar").is_some());
        assert!(visual.debug_bounds("project-switcher-trigger").is_some());
        assert!(visual.debug_bounds("native-popup-menu").is_none());
        assert!(visual.debug_bounds("project-home").is_none());
        assert!(visual.debug_bounds("settings-trigger").is_some());
        assert!(visual.debug_bounds("plugin-manager-trigger").is_some());
        assert!(visual.update(|_, cx| host.read(cx).editor.is_none()));
        click(visual, "project-switcher-trigger");
        assert!(visual.debug_bounds("native-popup-menu").is_some());
        assert!(visual.debug_bounds("project-search").is_some());
        visual.simulate_keystrokes("escape");
        visual.run_until_parked();
        assert!(visual.debug_bounds("native-popup-menu").is_none());
    });
}

/// Global dialogs use their real native views and leave the root without a workspace.
#[gpui::test]
fn empty_startup_can_configure_theme_and_manage_plugins(cx: &mut TestAppContext) {
    let profile = tempfile::tempdir().unwrap();
    with_home(cx, profile.path(), |visual, host| {
        click(visual, "settings-trigger");
        let handle = visual.update(|_, cx| host.read(cx).dialog_window.unwrap());
        let mut settings = VisualTestContext::from_window(handle.into(), &visual.cx);
        settings.run_until_parked();
        assert!(settings.debug_bounds("settings-workspace-trust").is_none());
        assert!(settings.debug_bounds("settings-nav-languages").is_none());
        click(&mut settings, "settings-theme-dark");
        assert!(settings.update(|_, cx| crate::ui::theme::ActiveTheme::theme(cx).is_dark()));
        click(&mut settings, "settings-nav-editor");
        let before = settings.update(|_, cx| typography::font_size(cx));
        click(&mut settings, "font-size-increase");
        assert!(settings.update(|_, cx| typography::font_size(cx)) > before);
        click(&mut settings, "app-dialog-close");
        click(visual, "plugin-manager-trigger");
        let handle = visual.update(|_, cx| host.read(cx).extensions_window.unwrap());
        let mut manager = VisualTestContext::from_window(handle.into(), &visual.cx);
        manager.run_until_parked();
        assert!(manager.debug_bounds("runtime-plugin-manager").is_some());
        assert!(visual.update(|_, cx| host.read(cx).editor.is_none()));
        assert!(!profile.path().join("run/terminal").exists());
        click(&mut manager, "app-dialog-close");
        assert_eq!(window_count(visual), 1);
        request_close_project(visual);
        assert_eq!(window_count(visual), 0);
    });
}

/// External no-target requests reuse the most recently activated registered root, preserving input.
#[gpui::test]
fn repeated_empty_launch_activates_last_used_project_window(cx: &mut TestAppContext) {
    let profile = tempfile::tempdir().unwrap();
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    std::fs::write(a.path().join("a.txt"), "A").unwrap();
    std::fs::write(b.path().join("b.txt"), "B").unwrap();
    with_home(cx, profile.path(), |visual, host| {
        pick_restricted_project(visual, a.path());
        click(visual, "project-open-folder");
        let path = b.path().to_owned();
        visual.simulate_path_prompt_response(move |_| Some(vec![path]));
        visual.run_until_parked();
        let second = visual.update(|window, cx| {
            cx.windows()
                .into_iter()
                .find(|handle| handle.window_id() != window.window_handle().window_id())
                .unwrap()
        });
        let mut peer = VisualTestContext::from_window(second, &visual.cx);
        peer.run_until_parked();
        click(&mut peer, "project-trust-restricted");
        peer.update(|window, _| window.activate_window());
        peer.run_until_parked();
        peer.update(|_, cx| {
            crate::app::projects::open_request(profile.path().to_owned(), None, cx).unwrap()
        });
        peer.run_until_parked();
        assert_eq!(
            peer.update(|_, cx| cx.active_window().unwrap().window_id()),
            second.window_id()
        );
        visual.update(|window, _| window.activate_window());
        visual.run_until_parked();
        visual.simulate_input("kept 中文");
        let original = visual.update(|_, cx| host.read(cx).editor.clone().unwrap());
        let text = visual.update(|_, cx| original.read(cx).editor.read(cx).value().to_string());
        visual.update(|_, cx| {
            crate::app::projects::open_request(profile.path().to_owned(), None, cx).unwrap()
        });
        visual.run_until_parked();
        assert_eq!(
            visual.update(|_, cx| cx.active_window().unwrap().window_id()),
            visual.window_handle().window_id()
        );
        assert_eq!(window_count(visual), 2);
        assert_eq!(
            visual.update(|_, cx| original.read(cx).editor.read(cx).value().to_string()),
            text
        );
        click(visual, "project-close-project");
        click(visual, "project-close-discard");
        assert_eq!(window_count(&peer), 1);
        assert!(peer.debug_bounds("editor-shell").is_some());
        assert!(peer.debug_bounds("native-popup-menu").is_none());
    });
}
