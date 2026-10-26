//! Initial terminal geometry must not expose empty scrollback as a visible scroll rail.

use super::*;

/// A wrapped first row is real transcript data even when the snapshot has no history prefix.
#[test]
fn wrapped_first_row_survives_restore_and_resize_without_history() {
    let size = GridSize {
        columns: 74,
        rows: 10,
    };
    let mut original = Engine::new(size, 1000);
    let prompt = "PS C:\\Users\\Tester\\AppData\\Local\\Temp\\.tmprestore> ";
    original.process(
        format!("{prompt}Write-Output ('preserved-' + 'history')\r\npreserved-history\r\n{prompt}")
            .as_bytes(),
    );
    let transcript = |engine: &Engine| {
        engine
            .snapshot()
            .lines
            .iter()
            .flatten()
            .map(|cell| cell.c)
            .filter(|ch| !ch.is_whitespace())
            .collect::<String>()
    };
    let expected = transcript(&original);
    let saved = serde_json::to_vec(&original.snapshot()).unwrap();
    for columns in [2, 40, 73, 74, 80, 120] {
        for rows in [1, 3, 10, 12] {
            let mut restored = Engine::new(size, 1000);
            restored
                .restore(serde_json::from_slice(&saved).unwrap())
                .unwrap();
            assert_eq!(transcript(&restored), expected, "restore alone");
            restored.resize(GridSize { columns, rows });
            restored.resize(size);
            assert_eq!(
                transcript(&restored),
                expected,
                "resize via {columns}x{rows}"
            );
            let colors = Colors {
                background: 0xffffff,
                foreground: 0,
                cursor: 0x3574f0,
                selection: 0xcccccc,
                ansi: [0; 16],
                overrides: BTreeMap::new(),
            };
            let drawing = restored.drawing(608., 226., 8., 21., "monospace", 14., &colors);
            let visible: String = drawing
                .paint
                .iter()
                .filter_map(|paint| match paint {
                    Paint::Text { text, .. } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<String>()
                .chars()
                .filter(|ch| !ch.is_whitespace())
                .collect();
            assert_eq!(
                visible,
                expected,
                "visible transcript after {columns}x{rows}: history={}, offset={}",
                restored.history(),
                restored.offset()
            );
        }
    }
}

/// Real transcript rows, including styled spaces, survive clear/redraw and still offer scrolling.
#[test]
fn native_clear_preserves_real_history_and_styled_spaces() {
    let mut engine = Engine::new(
        GridSize {
            columns: 80,
            rows: 12,
        },
        100,
    );
    engine.begin_process(true, None);
    engine.process(b"ORIGINAL\r\n\x1b[41m   \x1b[0m\x1b[2J\x1b[H");
    assert_eq!(engine.history(), 2);
    engine.process(b"\x1b[2J\x1b[HPS C:\\workspace> ");
    let saved = engine.snapshot();
    assert_eq!(
        saved.lines[0].iter().map(|cell| cell.c).collect::<String>(),
        "ORIGINAL"
    );
    assert_eq!(saved.lines[1].len(), 3);
    assert!(
        saved.lines[1]
            .iter()
            .all(|cell| cell.bg == Color::Named(NamedColor::Red))
    );
    assert_eq!(engine.history(), 2);
}

/// Removing synthetic empty history must never strip a newline actually emitted by a process.
#[test]
fn native_output_preserves_explicit_leading_newline() {
    let mut engine = Engine::new(
        GridSize {
            columns: 80,
            rows: 12,
        },
        100,
    );
    engine.begin_process(true, None);
    engine.process(b"\r\nPS C:\\workspace> ");
    let saved = engine.snapshot();
    assert!(saved.lines[0].is_empty());
    assert_eq!(saved.lines[1][0].c, 'P');
    assert_eq!(engine.history(), 0);
}

/// ConPTY's first clear/redraw must not archive an empty row above the first PowerShell prompt.
#[test]
fn first_native_redraw_does_not_create_blank_history() {
    let mut engine = Engine::new(
        GridSize {
            columns: 100,
            rows: 24,
        },
        100,
    );
    engine.begin_process(true, Some("PS C:\\workspace>"));
    let history_before_output = engine.history();
    engine.process(b"\x1b[?9001h\x1b[?1004h\x1b[?25l\x1b[2J\x1b[m\x1b[H\x1b]0;powershell.exe\x07\x1b[?25hPS C:\\workspace> ");
    let first = engine
        .snapshot()
        .lines
        .iter()
        .position(|row| row.iter().any(|cell| !cell.c.is_whitespace()));
    assert!(
        first == Some(0) && engine.history() == 0,
        "first native prompt acquired empty history: before_output={history_before_output}, after_output={}, first_row={first:?}",
        engine.history()
    );
}

/// A single prompt fits without scrolling, including a viewport ending in a partial cell row.
#[test]
fn prompt_without_history_has_no_scrollbar() {
    let mut engine = Engine::new(
        GridSize {
            columns: 80,
            rows: 12,
        },
        100,
    );
    engine.process(b"PS C:\\workspace> ");
    let colors = Colors {
        background: 0xffffff,
        foreground: 0,
        cursor: 0x3574f0,
        selection: 0xcccccc,
        ansi: [0; 16],
        overrides: BTreeMap::new(),
    };
    for height in [268., 279.] {
        let drawing = engine.drawing(800., height, 8., 21., "monospace", 14., &colors);
        assert_eq!(engine.history(), 0);
        assert!(
            drawing.scroll.is_none(),
            "a single prompt cannot need a scroll rail at height {height}: {:?}",
            drawing.scroll
        );
    }
}
