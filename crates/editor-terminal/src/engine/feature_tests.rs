//! Observable regressions for wrapped search, Vi selection, hyperlink metadata and native styling.

use super::*;

/// Integrating the screen into a differently versioned crate must preserve the existing DA2 reply.
#[test]
fn integrated_screen_keeps_device_attributes_compatibility() {
    let mut engine = terminal();
    let replies = engine.process(b"\x1b[>c");
    assert!(
        replies
            .iter()
            .any(|event| matches!(event, Event::PtyWrite(reply) if reply == "\x1b[>0;2600;1c"))
    );
}

/// A child that omits ESU still publishes its final frame on timeout or actual EOF.
#[test]
fn synchronized_output_has_timeout_and_eof_boundaries() {
    let mut engine = terminal();
    engine.process(b"\x1b[?2026hTIMEOUT_FRAME");
    let deadline = engine.parser.sync_timeout().sync_timeout().unwrap();
    assert!(engine.expire_sync(deadline + std::time::Duration::from_millis(1)));
    engine.process(&[]);
    assert!(
        engine
            .snapshot()
            .lines
            .iter()
            .flatten()
            .map(|cell| cell.c)
            .collect::<String>()
            .contains("TIMEOUT_FRAME")
    );
    engine.clear();
    engine.process(b"\x1b[?2026hEOF_FRAME");
    engine.end_process();
    assert!(
        engine
            .snapshot()
            .lines
            .iter()
            .flatten()
            .map(|cell| cell.c)
            .collect::<String>()
            .contains("EOF_FRAME")
    );
}

/// Use a deliberately narrow viewport to exercise wide-cell and soft-wrap behavior.
fn terminal() -> Engine {
    Engine::new(
        GridSize {
            columns: 16,
            rows: 4,
        },
        100,
    )
}

/// Navigation searches retained history, wraps, respects explicit case and rejects malformed regex.
#[test]
fn search_history_unicode_wrapping_and_case() {
    let mut engine = terminal();
    engine.process(
        "FIRST 中文needle末尾\r\nsecond\r\nthird\r\nfourth\r\nfifth\r\nLAST NEEDLE".as_bytes(),
    );
    engine.set_search("needle".into(), false, false);
    assert!(engine.search_next(false));
    let first = engine.search.current.clone().unwrap();
    assert_eq!(
        engine.term.bounds_to_string(*first.start(), *first.end()),
        "NEEDLE"
    );
    assert!(engine.search_next(false));
    let second = engine.search.current.clone().unwrap();
    assert_ne!(first, second);
    assert_eq!(
        engine.term.bounds_to_string(*second.start(), *second.end()),
        "needle"
    );
    assert!(engine.offset() > 0);
    assert!(engine.search_next(true));
    assert_eq!(engine.search.current, Some(first));
    engine.set_search("NEEDLE".into(), false, true);
    assert!(engine.search_next(false));
    let found = engine.search.current.clone();
    assert!(engine.search_next(false));
    assert_eq!(engine.search.current, found);
    engine.set_search("[".into(), true, false);
    assert!(engine.search.invalid);
    assert!(!engine.search_next(false));
    engine.set_search("".into(), false, false);
    assert!(!engine.search_next(false));
}

/// Navigation moves only the read cursor; selection uses Alacritty's Unicode-aware expansion.
#[test]
fn vi_selection_and_exit_preserve_process_cursor() {
    let mut engine = terminal();
    engine.process("hello 中文 world".as_bytes());
    let cursor = engine.term.grid().cursor.point;
    engine.toggle_vi();
    engine.vi_key("0", false, false);
    engine.vi_key("v", false, false);
    for _ in 0..4 {
        engine.vi_key("l", false, false);
    }
    assert_eq!(engine.selected_text().as_deref(), Some("hello"));
    assert_eq!(engine.term.grid().cursor.point, cursor);
    engine.vi_key("i", false, false);
    assert!(!engine.mode().contains(TerminalMode::VI));
    assert!(!engine.vi_key("l", false, false));
    assert_eq!(engine.term.grid().cursor.point, cursor);
}

/// Explicit OSC 8 links override displayed labels; ordinary wrapped paths retain their full token.
#[test]
fn link_metadata_and_wrapped_paths() {
    let mut engine = terminal();
    engine.process(b"\x1b]8;;https://example.com/docs\x1b\\label\x1b]8;;\x1b\\");
    assert_eq!(
        engine.link_at(0, 1).unwrap().target,
        "https://example.com/docs"
    );
    engine.clear();
    engine.process(b"src/some_long_file.rs:12:3");
    assert_eq!(
        engine.link_at(1, 2).unwrap().target,
        "src/some_long_file.rs:12:3"
    );
    assert!(engine.link_at(100, 2).is_none());
}

/// Rendering honors DECSCUSR, italic and colored double underline, keeping the legacy beam default.
#[test]
fn cursor_styles_and_rendition_reach_native_drawing() {
    let mut engine = terminal();
    let colors = Colors {
        background: 0,
        foreground: 0xffffff,
        cursor: 0x00ff00,
        selection: 0x333333,
        ansi: [0xffffff; 16],
        overrides: Default::default(),
    };
    let draw =
        |engine: &mut Engine| engine.drawing(400., 200., 10., 20., "monospace", 14., &colors);
    let default = draw(&mut engine);
    assert!(
        default
            .paint
            .iter()
            .any(|op| matches!(op, Paint::Fill { rect, color: 0x00ff00, .. } if rect.w == 1.5))
    );
    engine.process(b"\x1b[3;4:2;58:2::255:0:0mA\x1b[0m\x1b[4 q");
    let drawing = draw(&mut engine);
    assert!(
        drawing
            .paint
            .iter()
            .any(|op| matches!(op, Paint::Text { italic: true, .. }))
    );
    assert_eq!(
        drawing
            .paint
            .iter()
            .filter(|op| matches!(
                op,
                Paint::Fill {
                    color: 0xff0000,
                    ..
                }
            ))
            .count(),
        2
    );
    assert!(drawing.paint.iter().any(|op| matches!(op, Paint::Fill { rect, color: 0x00ff00, .. } if rect.w == 10. && rect.h == 2.)));
    engine.process(b"\x1b[?25l");
    assert!(draw(&mut engine).caret.is_none());
}
