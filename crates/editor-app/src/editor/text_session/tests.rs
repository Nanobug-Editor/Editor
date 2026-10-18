//! Native IME bytes revoke public document capabilities without creating a second edit session.
use crate::EditorApp;
use gpui_kit::{App, Entity, EntityInputHandler as _, TestAppContext, gpui};
use plugin_runtime::plugin_protocol::api::DocumentInfo;

/// Read the same current metadata exposed to plugins, rather than inspecting revision bookkeeping.
fn info(app: &Entity<EditorApp>, cx: &App) -> DocumentInfo {
    let app = app.read(cx);
    app.plugin_document_info(app.active_text_tab_index().unwrap(), cx)
        .unwrap()
}

/// Preedit, equal-length candidate replacement and cancellation each invalidate old proposal bytes.
/// Selection notifications stay cheap; committing and Undo still use the original native session.
#[gpui::test]
fn native_document_capability_tracks_ime_without_dirtying_preedit(cx: &mut TestAppContext) {
    crate::tests::with_shortcut_editor(cx, false, Vec::new(), |visual, app, path| {
        let source = visual.update(|_, cx| app.read(cx).editor.clone());
        let before = visual.update(|_, cx| info(&app, cx));
        visual.update(|_, cx| {
            source.update(cx, |source, cx| {
                source.set_selected_range(0..0, cx);
                cx.notify();
            });
        });
        assert_eq!(
            visual.update(|_, cx| info(&app, cx).document),
            before.document,
            "selection and nontext notifications retain the same capability"
        );
        visual.update(|window, cx| {
            source.update(cx, |source, cx| {
                source.replace_and_mark_text_in_range(None, "拼😀", Some(0..3), window, cx);
                assert!(source.marked_text_range(window, cx).is_some());
            });
        });
        let marked = visual.update(|_, cx| info(&app, cx));
        assert!(
            marked.document.revision > before.document.revision,
            "notify-only IME bytes must revoke the old plugin document capability"
        );
        assert!(!marked.dirty, "preedit must not dirty DocumentSession");
        assert_eq!(marked.byte_len, before.byte_len + "拼😀".len());
        visual.update(|window, cx| {
            source.update(cx, |source, cx| {
                source.replace_and_mark_text_in_range(None, "漢😃", Some(0..3), window, cx);
            });
        });
        let replaced = visual.update(|_, cx| info(&app, cx));
        assert!(replaced.document.revision > marked.document.revision);
        assert_eq!(replaced.byte_len, marked.byte_len);
        assert!(!replaced.dirty);
        visual.update(|window, cx| {
            source.update(cx, |source, cx| {
                source.replace_and_mark_text_in_range(None, "", None, window, cx);
                assert!(source.marked_text_range(window, cx).is_none());
            });
        });
        let cancelled = visual.update(|_, cx| info(&app, cx));
        assert!(cancelled.document.revision > replaced.document.revision);
        assert_eq!(cancelled.byte_len, before.byte_len);
        assert!(!cancelled.dirty);
        assert_eq!(std::fs::read_to_string(path).unwrap(), "original on disk");
        visual.update(|window, cx| {
            source.update(cx, |source, cx| {
                source.replace_and_mark_text_in_range(None, "确认", Some(0..2), window, cx);
                source.replace_text_in_range(None, "确认", window, cx);
                assert!(source.marked_text_range(window, cx).is_none());
            });
        });
        let committed = visual.update(|_, cx| info(&app, cx));
        assert!(committed.document.revision > cancelled.document.revision);
        assert!(committed.dirty);
        assert!(visual.update(|_, cx| source.read(cx).value().starts_with("确认")));
        visual.simulate_keystrokes("ctrl-z");
        visual.update(|_, cx| {
            assert_eq!(source.read(cx).value().as_ref(), "original on disk");
            assert!(info(&app, cx).document.revision > committed.document.revision);
        });
    });
}
