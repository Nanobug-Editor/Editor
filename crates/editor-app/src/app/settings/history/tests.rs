//! Settings drafts use real native input states and an isolated profile without requiring a workspace.

use super::*;
use gpui_kit::{TestAppContext, VisualTestContext, gpui};

/// Invalid input preserves the prior policy; applying a complete valid draft persists every field.
#[gpui::test]
fn history_user_settings_validate_and_persist_without_a_workspace(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        editor_ui::init(cx);
        typography::init(cx);
        apply_theme(builtin_theme(false), cx);
    });
    let directory = tempfile::tempdir().unwrap();
    let profile = directory.path().to_owned();
    let mut form = None;
    let (_, visual) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| HistorySettings::new(profile.clone(), None, window, cx));
        form = Some(view.clone());
        Root::new(view, window, cx)
    });
    let form = form.unwrap();
    settled(visual, &form);
    visual.update(|window, cx| {
        form.update(cx, |form, cx| {
            form.days
                .update(cx, |input, cx| input.set_value("0", window, cx));
            form.apply(window, cx);
            assert!(form.error.is_some());
        })
    });
    assert!(!profile.join("history.json").exists());
    visual.update(|window, cx| {
        form.update(cx, |form, cx| {
            form.enabled = false;
            for (input, value) in [
                (&form.days, "45"),
                (&form.budget, "2000"),
                (&form.file_limit, "32"),
                (&form.exclude, "**/private/**; **/cache/**"),
            ] {
                input.update(cx, |input, cx| input.set_value(value, window, cx));
            }
            form.apply(window, cx);
        })
    });
    settled(visual, &form);
    let stored = crate::editor::history::load_policy(&profile.join("history.json")).unwrap();
    assert!(!stored.enabled);
    assert_eq!(stored.retention_days, 45);
    assert_eq!(stored.max_total_bytes, 2_000_000_000);
    assert_eq!(stored.max_file_bytes, 32 * 1024 * 1024);
    assert_eq!(stored.exclude, vec!["**/private/**", "**/cache/**"]);
    visual.update(|_, cx| assert!(form.read(cx).error.is_none()));
}

/// Completing the actual background acknowledgement avoids racing initialization with a draft edit.
fn settled(visual: &mut VisualTestContext, form: &Entity<HistorySettings>) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        visual.run_until_parked();
        if visual.update(|_, cx| !form.read(cx).busy) {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "history settings did not settle"
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}
