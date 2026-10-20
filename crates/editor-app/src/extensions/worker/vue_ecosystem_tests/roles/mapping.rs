//! Transformed spans reach two original native files through ordinary navigation and workspace edits.
use super::*;

/// Open the SFC through its existing session before each source action; F12 can leave state.ts active.
fn open_source(fixture: &Fixture, path: &Path, visual: &mut VisualTestContext) {
    visual.update(|window, cx| {
        fixture.app.update(cx, |app, cx| {
            app.open_file(path.to_owned(), window, cx);
            app.editor.update(cx, |editor, cx| editor.focus(window, cx));
        });
        window.draw(cx).clear(cx);
    });
}

/// F12 must reach the exported original TS declaration, never its import alias or generated template.
pub(super) fn original_definition(
    fixture: &Fixture,
    path: &Path,
    symbol: &str,
    visual: &mut VisualTestContext,
) {
    open_source(fixture, path, visual);
    caret(fixture, SOURCE.rfind(symbol).unwrap() + 1, visual);
    visual.simulate_keystrokes("f12");
    let state = path.parent().unwrap().join("state.ts");
    let declaration = STATE_SOURCE.find(symbol).unwrap();
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "mapped external original definition",
        30,
        |app, cx| {
            app.active_path.as_deref() == Some(state.as_path())
                && (declaration..=declaration + symbol.len())
                    .contains(&app.editor.read(cx).cursor())
                && app.editor.read(cx).text().to_string() == STATE_SOURCE
        },
    );
    open_source(fixture, path, visual);
}

/// References must include the export, import and transformed use exactly once at native UTF-16 positions.
pub(super) fn references(
    fixture: &Fixture,
    path: &Path,
    symbol: &str,
    visual: &mut VisualTestContext,
) {
    open_source(fixture, path, visual);
    caret(fixture, SOURCE.rfind(symbol).unwrap() + 1, visual);
    visual.update(|window, cx| {
        fixture.app.update(cx, |app, cx| {
            app.request_language_feature(
                plugin_runtime::plugin_protocol::direct::Feature::References,
                window,
                cx,
            )
        })
    });
    let state = path.parent().unwrap().join("state.ts");
    let mut expected = [(path, SOURCE), (state.as_path(), STATE_SOURCE)]
        .into_iter()
        .flat_map(|(path, text)| {
            let relative = path
                .strip_prefix(&fixture.workspace)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            text.match_indices(symbol).map(move |(offset, _)| {
                let prefix = &text[..offset];
                let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
                let column = prefix.rsplit('\n').next().unwrap().encode_utf16().count() + 1;
                format!("{relative}:{line}:{column}")
            })
        })
        .collect::<Vec<_>>();
    expected.sort();
    assert_eq!(
        expected.len(),
        3,
        "one export, one import and one mapped use"
    );
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut observed = loop {
        wait(
            &fixture.app,
            &fixture.actor,
            visual,
            "mapped References frame",
            1,
            |_, _| true,
        );
        let rows = visual.update(|window, _| {
            gpui_base::test_support::snapshots(window)
                .into_iter()
                .filter(|node| node.visible() && node.role() == Some(gpui_kit::Role::Button))
                .filter_map(|node| node.label().map(str::to_owned))
                .filter(|label| {
                    label.contains(':')
                        && [".vue", ".ts", ".js"]
                            .iter()
                            .any(|extension| label.contains(extension))
                })
                .collect::<Vec<_>>()
        });
        if !rows.is_empty() {
            break rows;
        }
        assert!(
            Instant::now() < deadline,
            "mapped References produced no original source rows"
        );
    };
    observed.sort();
    assert_eq!(
        observed, expected,
        "mapped References must not omit imports or leak virtual/foreign locations"
    );
    visual.simulate_keystrokes("escape");
}

/// Compare both live DocumentSessions; no response-time disk snapshot can stand in for an edited target.
pub(super) fn sources_equal(
    app: &EditorApp,
    cx: &gpui::App,
    path: &Path,
    component: &str,
    state: &str,
) -> bool {
    let state_path = path.parent().unwrap().join("state.ts");
    [(path, component), (state_path.as_path(), state)]
        .into_iter()
        .all(|(path, expected)| {
            app.tabs
                .iter()
                .find(|tab| tab.path() == path)
                .and_then(|tab| tab.text.as_ref())
                .is_some_and(|text| text.editor.read(cx).text().to_string() == expected)
        })
}

/// Both original files must preview, cancel, apply and return through one actual whole-batch Undo.
pub(super) fn mapped_rename(
    fixture: &Fixture,
    path: &Path,
    symbol: &str,
    visual: &mut VisualTestContext,
) {
    let renamed = format!("mapped_{symbol}");
    for apply in [false, true] {
        open_source(fixture, path, visual);
        eprintln!("S4 mapped rename: symbol={symbol}, apply={apply}, open prompt");
        caret(fixture, SOURCE.rfind(symbol).unwrap() + 1, visual);
        visual.simulate_keystrokes("f2");
        control(fixture, visual, "editor-rename-prompt", true);
        eprintln!("S4 mapped rename: symbol={symbol}, apply={apply}, prompt ready");
        visual.simulate_input(&renamed);
        visual.simulate_keystrokes("enter");
        eprintln!("S4 mapped rename: symbol={symbol}, apply={apply}, confirmation dispatched");
        control(fixture, visual, "workspace-plan-confirm", true);
        eprintln!("S4 mapped rename: symbol={symbol}, apply={apply}, full preview ready");
        assert!(visual.update(|_, cx| sources_equal(
            fixture.app.read(cx),
            cx,
            path,
            SOURCE,
            STATE_SOURCE
        )));
        click(
            visual,
            if apply {
                "workspace-plan-confirm"
            } else {
                "workspace-plan-cancel"
            },
        );
        control(fixture, visual, "workspace-plan-confirm", false);
        if !apply {
            assert!(visual.update(|_, cx| sources_equal(
                fixture.app.read(cx),
                cx,
                path,
                SOURCE,
                STATE_SOURCE
            )));
        }
    }
    let component_expected = SOURCE.replace(symbol, &renamed);
    let state_expected = STATE_SOURCE.replace(symbol, &renamed);
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "complete mapped cross-file rename",
        15,
        |app, cx| sources_equal(app, cx, path, &component_expected, &state_expected),
    );
    assert_eq!(
        std::fs::read_to_string(path).unwrap(),
        "<template><p>saved</p></template>\r\n"
    );
    assert_eq!(
        std::fs::read_to_string(path.parent().unwrap().join("state.ts")).unwrap(),
        STATE_SOURCE
    );
    // Preserve every older receipt. The shared driver uses actual wheel and pointer events.
    super::super::super::vue_test_support::receipts::undo(
        fixture,
        visual,
        "S4 mapped receipt native frame",
    );
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "mapped cross-file group Undo",
        15,
        |app, cx| sources_equal(app, cx, path, SOURCE, STATE_SOURCE),
    );
}
