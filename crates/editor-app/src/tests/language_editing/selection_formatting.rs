//! A native selection reaches the chosen unknown-language formatter without changing whole-document Save.
use super::*;

/// Select through the native editor, then dispatch the same command registered in the source menu.
fn selected_action(
    app: &Entity<EditorApp>,
    visual: &mut VisualTestContext,
    range: std::ops::Range<usize>,
) {
    visual.update(|window, cx| {
        app.read(cx).editor.clone().update(cx, |editor, cx| {
            editor.set_selected_range(range, cx);
            editor.focus(window, cx);
            cx.notify();
        });
        window.dispatch_action(
            cx.build_action("me_editor::FormatSelection", None).unwrap(),
            cx,
        );
    });
}

/// UTF-8 native selection is sent as UTF-16, applied once, undone once and never saved implicitly.
#[gpui::test]
#[ignore = "build javascript fixture archive for its approved private Node runtime"]
fn selected_native_range_uses_the_selected_formatter_and_single_undo(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("sample.linked");
    let source = "// before\r\n/* 中文😀e\u{301} */ const value={x:1};\r\n// after\r\n";
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
            "start":{"line":1,"character":13}, "end":{"line":1,"character":31}
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
    manager.shutdown_checked().unwrap();
}

/// The real Windows child can receive IME input while a dedicated transport thread awaits its reply.
/// Its guardian covers startup only; the fixture keeps its original 350 ms response delay.
#[cfg(windows)]
#[test]
#[ignore = "requires Windows/D3D11 and the current javascript fixture archive for its approved Node runtime"]
fn in_flight_range_formatting_rejects_ime_preedit_even_when_bytes_return() {
    use std::{fs, process::Command};
    const CHILD: &str =
        "extensions::worker::native_selection_formatting_tests::range_formatting_ime_child";
    const RECEIPT: &str =
        "native range formatting: pending IME, fresh range, Undo and cleanup verified";
    let directory = tempfile::tempdir().unwrap();
    let workspace = directory.path().join("workspace");
    fs::create_dir_all(&workspace).unwrap();
    let workspace = workspace.canonicalize().unwrap();
    fs::write(
        workspace.join("sample.linked"),
        "// before\r\n/* 中文😀e\u{301} */ const value={x:1};\r\n// after\r\n",
    )
    .unwrap();
    let profile = directory.path().join("profile");
    let package = editing_fixture::package(
        "delayed-range",
        "range-language",
        false,
        true,
        &directory.path().join("ime-range.jsonl"),
        350,
        "range",
    );
    // The original private helper assembles the actual admitted Package. The child only consumes
    // the ordinary installed store, so no test API or copied fixture implementation is necessary.
    let mut manager = Manager::open(
        crate::extensions::profile_runtime_root(&profile),
        protocol::Environment {
            workspace: workspace.display().to_string(),
            os: "windows".into(),
            ..Default::default()
        },
    )
    .unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    manager.shutdown_checked().unwrap();
    drop(manager);
    let log_path = directory.path().join("native-range-formatting.log");
    let log = fs::File::create(&log_path).unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            CHILD,
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("NANOBUG_NATIVE_RANGE_FORMATTING_ROOT", directory.path())
        .env("ME_EDITOR_PROFILE_HOME", &profile)
        .env("RUST_BACKTRACE", "1")
        .stdin(std::process::Stdio::null())
        .stdout(log.try_clone().unwrap())
        .stderr(log)
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(180);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break Some(status);
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            break None;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let log = fs::read_to_string(log_path).unwrap();
    assert!(
        status.is_some_and(|status| status.success()),
        "native range child failed or stalled: {status:?}\n{log}"
    );
    assert!(
        log.contains(RECEIPT),
        "missing checked native range receipt:\n{log}"
    );
    eprintln!("{log}");
}

/// Missing range capability is an ordinary unavailable operation, without a whole-document fallback
/// or a restart of a healthy server that still provides whole-document formatting.
#[gpui::test]
#[ignore = "build javascript fixture archive for its approved private Node runtime"]
fn missing_range_support_preserves_whole_document_formatting_on_the_same_server(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("sample.linked");
    let source = "const value={x:1};\n";
    std::fs::write(&path, source).unwrap();
    let log = directory.path().join("whole-only.jsonl");
    let package = editing_fixture::package(
        "whole-only",
        "range-language",
        false,
        true,
        &log,
        20,
        "whole",
    );
    let (app, visual, mut manager) = shell(cx, directory.path(), vec![package]);
    open(&app, &path, visual);
    selected_action(&app, visual, 0..source.len() - 1);
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        crate::extensions::lsp_tests::publish(&app, &mut manager, visual);
        pump(visual, 10);
        if visual.update(|_, cx| app.read(cx).status.contains("range formatting")) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "range unavailability was not surfaced"
        );
    }
    assert_eq!(text(&app, visual), source);
    assert_eq!(
        editing_fixture::count(&log, "textDocument/rangeFormatting"),
        0
    );
    assert_eq!(editing_fixture::count(&log, "textDocument/formatting"), 0);
    visual.simulate_keystrokes("shift-alt-f");
    await_text(
        &app,
        &format!("/* whole */\n{source}"),
        visual,
        "whole formatting after unsupported range",
    );
    assert_eq!(editing_fixture::count(&log, "initialize"), 1);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    manager.shutdown_checked().unwrap();
}
