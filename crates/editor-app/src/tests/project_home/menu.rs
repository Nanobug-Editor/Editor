//! Enter project commands through the painted application menu and retain native editing context.

use super::*;

/// Project management replaces the old home label and close row; keyboard lookup retains focus.
#[gpui::test]
fn application_menu_preserves_shortcut_focus_and_project_close_consent(cx: &mut TestAppContext) {
    let project = tempfile::tempdir().unwrap();
    let path = project.path().join("README.md");
    std::fs::write(&path, "original\n").unwrap();
    let profile = tempfile::tempdir().unwrap();
    with_home(cx, profile.path(), |visual, _| {
        pick_restricted_project(visual, project.path());
        let trigger = visual
            .debug_bounds("application-menu-trigger")
            .expect("the former logo must be an actionable menu");
        assert!(trigger.left() < px(32.));
        for removed in [
            "shortcuts-trigger",
            "shortcuts-menu-trigger",
            "project-open-another",
            "project-show-home",
            "project-close-project",
            "project-quit-application",
        ] {
            assert!(
                visual.debug_bounds(removed).is_none(),
                "duplicate {removed}"
            );
        }
        let original_focus = visual.update(|window, cx| window.focused(cx));
        click(visual, "application-menu-trigger");
        let menu = visual.debug_bounds("native-popup-menu").unwrap();
        // The dropdown belongs directly below the title bar, rather than below its next header row.
        assert_eq!(
            menu.top(),
            visual
                .debug_bounds("title-bar-drag-region")
                .unwrap()
                .bottom()
        );
        for selector in [
            "native-menu-project-open-another",
            "native-menu-project-show-home",
            "native-menu-item-shortcuts",
            "native-menu-project-quit-application",
        ] {
            assert!(
                visual.debug_bounds(selector).is_some(),
                "missing {selector}"
            );
        }
        assert!(
            visual
                .debug_bounds("native-menu-project-close-project")
                .is_none()
        );
        // The third selectable row opens lookup after the two remaining project actions.
        visual.simulate_keystrokes("down down enter");
        visual.run_until_parked();
        assert!(visual.debug_bounds("shortcuts-panel").is_some());
        assert!(visual.debug_bounds("native-popup-menu").is_none());
        visual.simulate_keystrokes("escape");
        visual.run_until_parked();
        assert_eq!(
            visual.update(|window, cx| window.focused(cx)),
            original_focus
        );

        visual.simulate_keystrokes("ctrl-a");
        visual.simulate_input("menu edits 中文\n");
        visual.run_until_parked();
        // Closing the native window still goes through the production unsaved-data gate.
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
        assert!(visual.debug_bounds("project-close-save").is_some());
        assert!(visual.debug_bounds("native-popup-menu").is_none());
        click(visual, "project-decision-cancel");
        assert!(visual.debug_bounds("editor-shell").is_some());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "original\n");
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
        click(visual, "project-close-save");
        assert!(visual.cx.update(|cx| cx.windows().is_empty()));
        assert_eq!(std::fs::read_to_string(path).unwrap(), "menu edits 中文\n");
    });
}
