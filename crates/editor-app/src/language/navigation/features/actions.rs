//! Standard current-source code actions become complete native workspace proposals, never effects.
use super::*;
use plugin_runtime::plugin_protocol::{
    diagnostics::Fix,
    workspace::{Change, Plan},
};

/// Preserve the original source identity while comparing optional LSP versions with the shared
/// connection's synchronized version. Unsupported commands and unresolved/disabled actions fail
/// explicitly; no apparently valid prefix can execute while part of the response was rejected.
pub(super) fn decode(
    connection: &LanguageServerConnection,
    response: Value,
    document: &DocumentLease,
    source: &SourceSnapshot,
) -> anyhow::Result<direct::Result> {
    if response.is_null() {
        return Ok(direct::Result::CodeActions {
            actions: Vec::new(),
        });
    }
    let proposals: Vec<Value> = serde_json::from_value(response).map_err(|error| {
        Failure::new(
            ErrorCode::InvalidRequest,
            format!("Decode code actions: {error}"),
        )
    })?;
    if proposals.len() > 32 {
        return Err(Failure::new(ErrorCode::LimitExceeded, "Too many code actions").into());
    }
    let version = connection
        .diagnostics
        .document_version(document.uri.as_str())
        .ok_or_else(|| {
            Failure::new(
                ErrorCode::StaleRevision,
                "Code action source is not synchronized",
            )
        })?;
    let actions = proposals
        .into_iter()
        .map(|proposal| {
            if proposal
                .get("command")
                .is_some_and(|command| !command.is_null())
            {
                return Err(Failure::new(
                    ErrorCode::UnsupportedOperation,
                    "Code-action commands and executeCommand are not supported",
                )
                .into());
            }
            let action: lsp_types::CodeAction =
                serde_json::from_value(proposal.clone()).map_err(|error| {
                    Failure::new(
                        ErrorCode::InvalidRequest,
                        format!("Decode code action: {error}"),
                    )
                })?;
            if let Some(disabled) = action.disabled {
                return Err(Failure::new(
                    ErrorCode::UnsupportedOperation,
                    format!(
                        "Code action is disabled: {}",
                        disabled.reason.chars().take(512).collect::<String>()
                    ),
                )
                .into());
            }
            let edit = proposal.get("edit").filter(|edit| !edit.is_null()).ok_or_else(|| {
            Failure::new(ErrorCode::UnsupportedOperation,
                "Code action requires an unresolved edit; codeAction/resolve is not supported")
        })?;
            // Keep the actual wire value: typed decoding can discard an illegal annotationId.
            let edits = super::super::editing::document_edits(
                edit.clone(),
                &document.uri,
                version,
                |target| connection.logical_uri(target),
            )?;
            let (edits, selection) = native_edits(&source.text, edits)?;
            let plan = Plan {
                title: action.title.clone(),
                origin: Some(source.document.clone()),
                conditions: Vec::new(),
                changes: vec![Change::EditDocument {
                    document: source.document.clone(),
                    edits,
                    selection,
                }],
            };
            plan.validate()?;
            Ok(Fix {
                title: action.title,
                plan,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(direct::Result::CodeActions { actions })
}

/// Each range addresses the original UTF-8 snapshot. Sorted, disjoint edits stay one complete
/// operation for DocumentSession's atomic proposal; the caret addresses the resulting text.
fn native_edits(
    source: &str,
    edits: Vec<lsp_types::TextEdit>,
) -> anyhow::Result<(Vec<api::TextEdit>, api::TextRange)> {
    if edits.is_empty() {
        return Err(Failure::new(
            ErrorCode::UnsupportedOperation,
            "Code action contains no text edits",
        )
        .into());
    }
    let mut edits = edits
        .into_iter()
        .map(|edit| {
            let range = api::DocumentRange::Utf16 {
                start: api::TextPosition {
                    line: edit.range.start.line,
                    character: edit.range.start.character,
                },
                end: api::TextPosition {
                    line: edit.range.end.line,
                    character: edit.range.end.character,
                },
            }
            .resolve(source)?;
            Ok(api::TextEdit {
                range,
                text: edit.new_text,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    edits.sort_by_key(|edit| (edit.range.start, edit.range.end));
    if edits.windows(2).any(|pair| {
        pair[0].range.end > pair[1].range.start || pair[0].range.start == pair[1].range.start
    }) {
        return Err(Failure::new(
            ErrorCode::InvalidRequest,
            "Code action has overlapping text edits",
        )
        .into());
    }
    let (mut resulting_end, mut original_end) = (0usize, 0usize);
    for edit in &edits {
        resulting_end = resulting_end
            .checked_add(edit.range.start - original_end)
            .and_then(|end| end.checked_add(edit.text.len()))
            .ok_or_else(|| {
                Failure::new(
                    ErrorCode::LimitExceeded,
                    "Code action caret exceeds document limits",
                )
            })?;
        original_end = edit.range.end;
    }
    let selection = api::TextRange {
        start: resulting_end,
        end: resulting_end,
    };
    api::validate_text_edits(&edits, selection)?;
    Ok((edits, selection))
}
