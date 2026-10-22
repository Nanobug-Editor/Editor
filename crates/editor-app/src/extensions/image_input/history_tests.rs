//! The existing image-save-first action participates in the same profile history as normal Save.

use super::*;
use gpui_kit::{TestAppContext, gpui};

/// Source creation is acknowledged by DocumentSession before history records the exact saved bytes.
#[gpui::test]
fn history_image_save_first_records_the_successfully_created_source(cx: &mut TestAppContext) {
    crate::editor::history::tests::with_history_editor(cx, false, |visual, app, path| {
        visual.simulate_input("image source ");
        visual.run_until_parked();
        std::fs::remove_file(path).unwrap();
        visual.update(|_, cx| {
            app.update(cx, |app, cx| {
                let index = app.active_text_tab_index().unwrap();
                let capture = Capture {
                    plugin: "fixture".into(),
                    panel: "fixture".into(),
                    epoch: 1,
                    document: app.plugin_document_version(index).unwrap(),
                    selection: TextRange { start: 0, end: 0 },
                };
                app.save_image_source(&capture, cx).unwrap();
            })
        });
        visual.run_until_parked();
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "image source original on disk"
        );
        let root = visual.update(|_, cx| app.read(cx).history.paths.as_ref().unwrap().root.clone());
        let history =
            editor_history::History::open(root, editor_history::Policy::default()).unwrap();
        let rows = history.list(&editor_history::Query::default()).unwrap();
        assert!(
            rows.iter()
                .any(|row| row.reason == editor_history::Reason::Saved
                    && history.read(row.id).unwrap() == b"image source original on disk")
        );
    });
}
