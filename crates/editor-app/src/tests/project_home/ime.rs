//! Native platform composition must be committed before project-level save consent is assessed.

use super::*;
use gpui_kit::EntityInputHandler as _;

/// Closing during Chinese preedit saves the displayed composition through the existing document session.
#[gpui::test]
fn project_home_close_commits_chinese_ime_preedit(cx: &mut TestAppContext) {
    let project = tempfile::tempdir().unwrap();
    let path = project.path().join("README.md");
    std::fs::write(&path, "original\n").unwrap();
    let profile = tempfile::tempdir().unwrap();
    with_home(cx, profile.path(), |visual, host| {
        pick_restricted_project(visual, project.path());
        let editor = visual.update(|_, cx| host.read(cx).editor.clone().unwrap());
        visual.simulate_keystrokes("ctrl-a");
        visual.update(|window, cx| {
            editor.read(cx).editor.clone().update(cx, |state, cx| {
                // Use the production platform handler's UTF-16 preedit and marked selection.
                state.replace_and_mark_text_in_range(None, "中文🙂", Some(0..2), window, cx);
                assert!(state.marked_text_range(window, cx).is_some());
            });
        });
        visual.run_until_parked();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "original\n");
        request_close_project(visual);
        assert!(visual.debug_bounds("project-close-save").is_some());
        click(visual, "project-decision-cancel");
        // Closing consent must not create an extra history entry or another undo implementation.
        visual.simulate_keystrokes("ctrl-z");
        visual.run_until_parked();
        assert_eq!(
            visual.update(|_, cx| editor.read(cx).editor.read(cx).value().to_string()),
            "original\n"
        );
        visual.simulate_keystrokes("ctrl-y");
        visual.run_until_parked();
        assert_eq!(
            visual.update(|_, cx| editor.read(cx).editor.read(cx).value().to_string()),
            "中文🙂"
        );
        request_close_project(visual);
        click(visual, "project-close-save");
        assert_eq!(window_count(visual), 0);
        assert_eq!(std::fs::read_to_string(path).unwrap(), "中文🙂");
    });
}
