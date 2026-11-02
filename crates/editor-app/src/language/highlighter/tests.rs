//! A pending parser must still supply complete native text runs for the live viewport.
use super::SyntaxHighlighter;
use gpui_base::input::{HighlightStyleResolver, Rope};
use gpui_kit::HighlightStyle;

#[cfg(target_os = "windows")]
mod native;

/// Unrecognized captures use the editor's ordinary foreground in these coverage checks.
struct PlainStyles;

impl HighlightStyleResolver for PlainStyles {
    fn style(&self, _: &str) -> Option<HighlightStyle> {
        None
    }
}

/// Cold grammar preparation has no parsed source, but must not erase the loaded document.
#[test]
fn deferred_highlighter_covers_loaded_text_before_parsing() {
    let source = "const message = '中文😀';\r\nconsole.log(message);\r\n";
    let highlighter = SyntaxHighlighter::deferred("pending-load-fixture");
    for range in [0..source.len(), 6..source.len()] {
        assert_eq!(
            highlighter.styles(&range, &PlainStyles),
            vec![(range, HighlightStyle::default())],
            "a pending parser must leave every requested byte visible"
        );
    }
}

/// While a longer edit is parsed asynchronously, the old snapshot cannot clip its new tail.
#[test]
fn deferred_highlighter_covers_viewport_beyond_previous_snapshot() {
    let mut highlighter = SyntaxHighlighter::deferred("pending-edit-fixture");
    assert!(highlighter.update(None, &Rope::from("old\n"), None));
    for range in [0..32, 4..32, 12..32] {
        assert_eq!(
            highlighter.styles(&range, &PlainStyles),
            vec![(range, HighlightStyle::default())],
            "the live viewport must remain paintable beyond the parsed snapshot"
        );
    }
}

/// Empty requests have no glyphs and must not create zero-length shaping runs.
#[test]
fn deferred_highlighter_keeps_empty_ranges_empty() {
    let highlighter = SyntaxHighlighter::deferred("empty-load-fixture");
    assert!(highlighter.styles(&(0..0), &PlainStyles).is_empty());
    assert!(highlighter.styles(&(7..7), &PlainStyles).is_empty());
}
