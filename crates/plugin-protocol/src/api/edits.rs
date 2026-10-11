//! Bounded UTF-8 text proposals; native sessions retain exclusive ownership of live text and Undo.

use super::{ErrorCode, Failure, TextRange};
use serde::{Deserialize, Serialize};

/// One half-open UTF-8 range in the proposal's original document, never in an intermediate result.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextEdit {
    pub range: TextRange,
    pub text: String,
}

/// Reject transport abuse before allocating a native request. Source boundaries are checked later.
pub fn validate_text_edits(edits: &[TextEdit], selection: TextRange) -> Result<(), Failure> {
    if edits.is_empty() || edits.len() > 10_000 {
        return Err(Failure::new(
            ErrorCode::LimitExceeded,
            "Text proposals require 1..10000 ranges",
        ));
    }
    if selection.start > selection.end || edits.iter().any(|edit| edit.range.start > edit.range.end)
    {
        return Err(Failure::new(
            ErrorCode::InvalidRequest,
            "Invalid half-open text range",
        ));
    }
    let bytes = edits
        .iter()
        .try_fold(0usize, |total, edit| total.checked_add(edit.text.len()));
    if bytes.is_none_or(|bytes| bytes > 1024 * 1024) {
        return Err(Failure::new(
            ErrorCode::LimitExceeded,
            "Text replacements exceed 1 MiB",
        ));
    }
    Ok(())
}

#[cfg(feature = "guest")]
/// Apply one exact opened document as one local Undo unit. Requires editor.edit 1.1 and editor.write.
/// Every range addresses the original revision; selection addresses the resulting complete text.
/// The task completes with EditorValue::Edited after the native Change event is observed.
/// Closing, renaming, composing, readonly state, stale revisions or malformed UTF-8 boundaries fail.
pub fn apply_edits(
    document: super::DocumentVersion,
    edits: Vec<TextEdit>,
    selection: TextRange,
    timeout_ms: u32,
) -> Result<super::ResourceHandle, Failure> {
    validate_text_edits(&edits, selection)?;
    super::guest::editor(
        super::EditorOperation::ApplyEdits {
            document,
            edits,
            selection,
        },
        timeout_ms,
    )
}
