//! Geometry changes must update layout immediately without flushing the session on every input.

use crate::{EditorApp, SessionState, theme, typography};
use editor_core::Workspace;
use gpui_base::Root;
use gpui_kit::{AppContext as _, Entity, TestAppContext, VisualTestContext, gpui, px, size};
use std::time::Duration;

/// Exercise the actual shell bounds observer with isolated session files and no plugin installation.
fn with_geometry_editor(
    cx: &mut TestAppContext,
    scenario: impl FnOnce(&mut VisualTestContext, Entity<EditorApp>),
) {
    let profile = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    cx.update(|cx| {
        gpui_kit::init(cx);
        typography::init(cx);
        theme::apply_theme(theme::builtin_theme(false), cx);
        editor_ui::init(cx);
        cx.set_reduce_motion(true);
    });
    let workspace = Workspace::open(project.path()).unwrap();
    let session = SessionState::load_in_profile(workspace.root(), profile.path());
    let mut editor = None;
    let (_, visual) = cx.add_window_view(|window, cx| {
        let app =
            cx.new(|cx| EditorApp::new_with_session(workspace, None, Some(session), window, cx));
        editor = Some(app.clone());
        Root::new(app, window, cx)
    });
    let editor = editor.unwrap();
    visual.simulate_resize(size(px(1000.), px(750.)));
    visual.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear(cx));
    visual.executor().advance_clock(Duration::from_secs(1));
    visual.run_until_parked();
    scenario(visual, editor);
}

/// A simulated 240 Hz burst lasts longer than the quiet period without flushing during input.
#[gpui::test]
fn window_geometry_resize_burst_defers_disk_flush(cx: &mut TestAppContext) {
    with_geometry_editor(cx, |visual, editor| {
        let path = visual.update(|_, cx| {
            let session = &editor.read(cx).session_state;
            session.try_save().unwrap();
            session.file_path().unwrap()
        });
        let before = std::fs::read(&path).unwrap();
        for width in 1001..=1120 {
            visual.simulate_resize(size(px(width as f32), px(750.)));
            visual.update(|window, cx| {
                assert_eq!(editor.read(cx).session_state.window_width, width as f32);
                window.draw(cx).clear(cx);
            });
            visual
                .executor()
                .advance_clock(Duration::from_secs(1) / 240);
            visual.run_until_parked();
            assert!(
                std::fs::read(&path).unwrap() == before,
                "pointer updates must not flush and replace the session file"
            );
        }
        visual.executor().advance_clock(Duration::from_secs(1));
        visual.run_until_parked();
        let saved: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(saved["window_width"], 1120.);
        assert_eq!(saved["window_height"], 750.);
    });
}

/// A move or redundant bounds notification with the same size needs no persistence work.
#[gpui::test]
fn window_geometry_unchanged_size_preserves_session_file(cx: &mut TestAppContext) {
    with_geometry_editor(cx, |visual, editor| {
        let (path, extent) = visual.update(|window, cx| {
            let session = &editor.read(cx).session_state;
            session.try_save().unwrap();
            (session.file_path().unwrap(), window.bounds().size)
        });
        // Valid JSON whitespace distinguishes an untouched file from a redundant serialization.
        let mut before = std::fs::read(&path).unwrap();
        before.extend_from_slice(b"\n\n");
        std::fs::write(&path, &before).unwrap();
        visual.simulate_resize(extent);
        visual.run_until_parked();
        visual.executor().advance_clock(Duration::from_secs(1));
        visual.run_until_parked();
        assert!(
            std::fs::read(path).unwrap() == before,
            "unchanged bounds must not rewrite the session"
        );
    });
}

/// Closing before the quiet period still commits the final dimensions through real shutdown.
#[gpui::test]
fn window_geometry_close_flushes_latest_size(cx: &mut TestAppContext) {
    with_geometry_editor(cx, |visual, editor| {
        visual.simulate_resize(size(px(1170.), px(790.)));
        let path = visual.update(|_, cx| {
            editor.update(cx, |editor, cx| {
                let shutdown = editor.prepare_project_shutdown(cx).unwrap();
                shutdown.detach();
                editor.session_state.file_path().unwrap()
            })
        });
        let saved: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(saved["window_width"], 1170.);
        assert_eq!(saved["window_height"], 790.);
    });
}
