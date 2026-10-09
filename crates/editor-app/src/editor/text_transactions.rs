//! Complete text proposals enter the unique native state after strict source and UTF-16 checks.

use crate::language::navigation::editing::{byte_at_position, merge_edits};
use crate::*;
use gpui_kit::EntityInputHandler as _;
use lsp_types::TextEdit;

/// Keep the existing language edit application while sharing proposal validation with new consumers.
/// The caller owns document revision, trust and provider checks; this boundary rejects an IME
/// composition, a readonly session or any source change before altering native text or history.
pub(super) fn apply_language_proposal(
    editor: &mut EditorState,
    source: &str,
    edits: &[TextEdit],
    cursor: usize,
    window: &mut Window,
    cx: &mut Context<EditorState>,
) -> anyhow::Result<()> {
    let merged = merge_edits(source, edits)?;
    anyhow::ensure!(
        editor.is_editable()
            && editor.marked_text_range(window, cx).is_none()
            && editor.text().to_string() == source,
        "{}",
        t!("editor.editing_stale")
    );
    if let Some(edit) = merged {
        let start = byte_at_position(source, edit.range.start)?;
        let end = byte_at_position(source, edit.range.end)?;
        // Base's public replace explicitly marks Atomic. A pure LSP insertion otherwise inherits
        // Typing and can undo the user's earlier input along with the generated proposal.
        editor.set_selected_range(start..end, cx);
        editor.replace(edit.new_text, window, cx);
        let mut cursor = cursor.min(editor.text().len());
        let text = editor.text().to_string();
        while !text.is_char_boundary(cursor) {
            cursor -= 1;
        }
        editor.set_selected_range(cursor..cursor, cx);
    }
    Ok(())
}

/// Adapt a validated protocol edit to Base 0.7's scalar columns only at native application.
/// Language messages remain UTF-16. Strict byte conversion rejects surrogate midpoints and
/// prevents supplementary characters before an edit from shifting either native endpoint.
pub(super) fn native_edit(source: &str, mut edit: TextEdit) -> anyhow::Result<TextEdit> {
    for position in [&mut edit.range.start, &mut edit.range.end] {
        let byte = byte_at_position(source, *position)?;
        let prefix = &source[..byte];
        position.character = prefix
            .rsplit_once('\n')
            .map_or(prefix, |(_, tail)| tail)
            .chars()
            .count() as u32;
    }
    Ok(edit)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Supplementary characters change native scalar columns, while surrogate midpoints are errors.
    #[test]
    fn native_application_preserves_utf16_boundaries_after_emoji() {
        let source = "header🙂\n汉🙂<tag>";
        let edit = TextEdit::new(
            lsp_types::Range::new(
                lsp_types::Position::new(1, 3),
                lsp_types::Position::new(1, 8),
            ),
            "<changed>".into(),
        );
        let adapted = native_edit(source, edit).unwrap();
        assert_eq!(adapted.range.start, lsp_types::Position::new(1, 2));
        assert_eq!(adapted.range.end, lsp_types::Position::new(1, 7));
        assert!(
            native_edit(
                source,
                TextEdit::new(
                    lsp_types::Range::new(
                        lsp_types::Position::new(1, 2),
                        lsp_types::Position::new(1, 8)
                    ),
                    "bad surrogate midpoint".into(),
                )
            )
            .is_err()
        );
    }
}

#[cfg(test)]
mod history_tests;
