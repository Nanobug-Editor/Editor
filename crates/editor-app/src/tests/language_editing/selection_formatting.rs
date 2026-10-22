//! A native selection reaches the chosen unknown-language formatter without changing whole-document Save.
use super::*;

/// UTF-8 native selection is sent as UTF-16, applied once, undone once and never saved implicitly.
#[gpui::test]
fn selected_native_range_uses_the_selected_formatter_and_single_undo(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("sample.linked");
    let source = "// before 中文😀\r\nconst value={x:1};\r\n// after\r\n";
    std::fs::write(&path, source).unwrap();
    let log = directory.path().join("range.jsonl");
    let package = editing_fixture::package(
        "unknown-range",
        "range-language",
        false,
        true,
        &log,
        80,
        "range",
    );
    let (app, visual, mut manager) = shell(cx, directory.path(), vec![package]);
    open(&app, &path, visual);
    let start = source.find("const").unwrap();
    let end = source[start..].find('\r').unwrap() + start;
    visual.update(|window, cx| {
        app.read(cx).editor.clone().update(cx, |editor, cx| {
            editor.set_selected_range(start..end, cx);
            editor.focus(window, cx);
            cx.notify();
        });
        let action = cx
            .build_action("me_editor::FormatSelection", None)
            .expect("native selection formatting must be an available user action");
        window.dispatch_action(action, cx);
    });
    let expected = source.replacen("const value={x:1};", "/* range */ const value={x:1};", 1);
    let deadline = Instant::now() + Duration::from_secs(15);
    while text(&app, visual) != expected {
        crate::extensions::lsp_tests::publish(&app, &mut manager, visual);
        pump(visual, 20);
        assert!(
            Instant::now() < deadline,
            "selected formatter did not edit the native source: {}",
            visual.update(|_, cx| app.read(cx).status.clone())
        );
    }
    assert_eq!(editing_fixture::count(&log, "textDocument/formatting"), 0);
    let request = std::fs::read_to_string(&log)
        .unwrap()
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(|message| message["method"] == "textDocument/rangeFormatting")
        .unwrap();
    assert_eq!(
        request["params"]["range"],
        serde_json::json!({
            "start":{"line":1,"character":0}, "end":{"line":1,"character":18}
        })
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    visual.simulate_keystrokes("ctrl-z");
    await_text(
        &app,
        source,
        visual,
        "one native Undo restores selection formatting",
    );
    manager.shutdown();
}
