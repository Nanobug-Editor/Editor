//! Complete text proposals enter the unique native state after strict source and UTF-16 checks.

use crate::language::navigation::editing::{byte_at_position, merge_edits};
use crate::*;
use gpui_kit::EntityInputHandler as _;
use lsp_types::TextEdit;
use plugin_runtime::plugin_protocol::api::{self, TextRange};

/// Immutable proposal bytes are preparation and recovery data, never a second mutable text session.
/// Construction validates all original-source ranges and the complete resulting selection.
pub(crate) struct PreparedTextEdits {
    source: String,
    result: String,
    replacement: Option<(std::ops::Range<usize>, String)>,
    selection: std::ops::Range<usize>,
}

/// Causal postimage of one synchronous native application, without another writable text or Undo.
pub(crate) struct AppliedTextEdits {
    /// Shared COW identity rejects notify-only IME changes even when no session Change was emitted.
    postimage: super::native_text::NativeTextIdentity,
    /// Replacing bytes emits one native Change; selection-only applications emit none.
    emitted_change: bool,
}

impl AppliedTextEdits {
    /// Expected DocumentSession revision after this application's own Change listener has drained.
    /// Capability revisions may advance for both Notify and Change and are deliberately not inferred.
    pub(crate) fn session_revision_after(&self, before: u64) -> Option<u64> {
        before.checked_add(u64::from(self.emitted_change))
    }

    /// Match the exact synchronously captured Rope instance, not merely equal source bytes.
    pub(crate) fn matches_postimage(&self, text: &gpui_base::input::Rope) -> bool {
        self.postimage.matches(text)
    }
}

impl PreparedTextEdits {
    /// Validate bounded UTF-8 ranges, reject overlap, and merge once without changing native history.
    pub(crate) fn new(
        source: String,
        edits: &[api::TextEdit],
        selection: TextRange,
    ) -> anyhow::Result<Self> {
        api::validate_text_edits(edits, selection).map_err(anyhow::Error::msg)?;
        let edits = edits
            .iter()
            .map(|edit| {
                anyhow::ensure!(
                    source.get(edit.range.start..edit.range.end).is_some(),
                    "Edit is outside UTF-8 boundaries"
                );
                Ok(TextEdit::new(
                    lsp_types::Range::new(
                        crate::language::navigation::position_at_byte(&source, edit.range.start),
                        crate::language::navigation::position_at_byte(&source, edit.range.end),
                    ),
                    edit.text.clone(),
                ))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        let replacement = merge_edits(&source, &edits)?
            .map(|edit| -> anyhow::Result<_> {
                Ok((
                    byte_at_position(&source, edit.range.start)?
                        ..byte_at_position(&source, edit.range.end)?,
                    edit.new_text,
                ))
            })
            .transpose()?;
        let mut result = source.clone();
        if let Some((range, text)) = &replacement {
            result.replace_range(range.clone(), text);
        }
        anyhow::ensure!(result.len() <= 32 * 1024 * 1024, "Result exceeds 32 MiB");
        anyhow::ensure!(
            result.get(selection.start..selection.end).is_some(),
            "Selection is outside resulting UTF-8 boundaries"
        );
        Ok(Self {
            source,
            result,
            replacement,
            selection: selection.start..selection.end,
        })
    }

    /// Readonly generated content is useful to a preview or disk preparation, never a live editor model.
    pub(crate) fn result(&self) -> &str {
        &self.result
    }

    /// Host recovery admits its own bounded preimage rather than applying a guest transport byte limit.
    /// This still produces one Atomic replacement on the existing native history and keeps saved state.
    pub(crate) fn restore(
        source: String,
        result: String,
        selection: TextRange,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            source.len() <= 32 * 1024 * 1024 && result.len() <= 32 * 1024 * 1024,
            "Recovery exceeds 32 MiB"
        );
        anyhow::ensure!(
            result.get(selection.start..selection.end).is_some(),
            "Recovery selection is outside UTF-8 boundaries"
        );
        let replacement = (source != result).then(|| (0..source.len(), result.clone()));
        Ok(Self {
            source,
            result,
            replacement,
            selection: selection.start..selection.end,
        })
    }

    /// Apply after identity/revision validation and return the exact native mutation's postimage.
    /// The caller may ignore the receipt; IME, readonly and source checks remain native.
    pub(crate) fn apply(
        &self,
        editor: &mut EditorState,
        window: &mut Window,
        cx: &mut Context<EditorState>,
    ) -> anyhow::Result<AppliedTextEdits> {
        self.validate_current(editor, window, cx)?;
        if let Some((range, text)) = &self.replacement {
            // One public Atomic replacement preserves Base's sole Undo stack and separates nearby typing.
            editor.set_selected_range(range.clone(), cx);
            editor.replace(text.clone(), window, cx);
        }
        editor.set_selected_range(self.selection.clone(), cx);
        Ok(AppliedTextEdits {
            postimage: super::native_text::NativeTextIdentity::new(editor.text()),
            emitted_change: self.replacement.is_some(),
        })
    }

    /// Preflight the exact native editor preimage, access and IME state without mutating history.
    /// Multi-document commits validate every session in one UI turn before their first replacement.
    pub(crate) fn validate_current(
        &self,
        editor: &mut EditorState,
        window: &mut Window,
        cx: &mut Context<EditorState>,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            editor.is_editable()
                && editor.marked_text_range(window, cx).is_none()
                && editor.text().to_string() == self.source,
            "{}",
            t!("editor.editing_stale")
        );
        Ok(())
    }
}

/// Apply a complete language proposal as one atomic native edit, separate from surrounding typing.
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
