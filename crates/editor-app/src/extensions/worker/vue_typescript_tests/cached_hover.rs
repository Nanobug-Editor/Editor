//! Real owner-bound hover caches must remain callable from ordinary native keyboard actions.
use super::*;

/// Reuse the exact positive template caches warmed by the mixed-project semantic requests.
/// Each original source receives Ctrl+I with its real provider; a synthetic ReadyHover would miss
/// the parent-context read that used to panic while the shell action updated EditorApp.
pub(super) fn assert_cached_keyboard_hover(
    app: &Entity<EditorApp>,
    paths: &[PathBuf],
    visual: &mut VisualTestContext,
) {
    for path in paths {
        visual.update(|window, cx| {
            window.activate_window();
            app.update(cx, |app, cx| app.open_file(path.clone(), window, cx));
            let editor = app.read(cx).editor.clone();
            let source = editor.read(cx).text().to_string();
            let offset = source.rfind("message").expect("previously queried symbol") + 2;
            editor.read(cx).focus_handle(cx).focus(window, cx);
            editor.update(cx, |editor, cx| {
                // Native selections use bytes; LSP UTF-16 columns differ after 中文😀.
                editor.set_selected_range(offset..offset, cx);
                assert_eq!(editor.cursor(), offset);
            });
        });
        // Dismiss presentation only: the real provider still owns the prior positive cache.
        visual.simulate_keystrokes("escape");
        frame(app, visual);
        assert!(visual.debug_bounds("editor-definition-card").is_none());
        visual.simulate_keystrokes("ctrl-i");
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            frame(app, visual);
            if visual.debug_bounds("editor-definition-card").is_some() {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "cached keyboard hover did not become visible for {}",
                path.display()
            );
        }
    }
}
