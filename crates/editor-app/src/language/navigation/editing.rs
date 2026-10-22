//! Standard editing requests retain native leases and validate complete source edit proposals.
use super::*;
use lsp_types::{
    DocumentChanges, LinkedEditingRanges, OneOf, PrepareRenameResponse, TextEdit, WorkspaceEdit,
};
use plugin_runtime::plugin_protocol::api::{ErrorCode, Failure};
use plugin_runtime::plugin_protocol::language::SEMANTIC_LINKED_EDITING_METHOD;
#[cfg(test)]
mod tests;

/// Pairing authority comes from the selected negotiated method, never an untrusted response flag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LinkedEditingAuthority {
    Standard,
    Semantic,
}

/// A method-bound proposal keeps range validation independent of any language or package identity.
#[derive(Debug)]
pub(crate) struct LinkedEditingProposal {
    pub response: LinkedEditingRanges,
    pub authority: LinkedEditingAuthority,
}

impl LanguageServer {
    /// Resolve only the selected completion, on its original source lease and shared transport.
    /// Unsupported resolve returns the original item; no completion command is executed here.
    pub(crate) fn resolve_completion_for(
        &self,
        document: DocumentLease,
        source: String,
        item: lsp_types::CompletionItem,
    ) -> anyhow::Result<lsp_types::CompletionItem> {
        self.edit_request(document, "completionResolve", move |connection, uri| {
            connection.sync_document(uri, source)?;
            if !connection.completion_resolve_provider {
                return Ok(item);
            }
            let value =
                connection.request("completionItem/resolve", serde_json::to_value(&item)?)?;
            serde_json::from_value(value).context("Decode resolved completion")
        })
    }

    /// Declaration authority is separate from the server's runtime advertised support.
    pub(crate) fn provides_editing(&self) -> bool {
        self.service.provider.editing && self.is_active()
    }

    /// Whole-document, range and Save requests share one immutable source and service lease.
    /// `selection` uses standard UTF-16 coordinates. Unsupported ranges fail without a document
    /// fallback; every proposal still enters the complete native edit validator.
    pub(crate) fn format_for(
        &self,
        document: DocumentLease,
        source: String,
        selection: Option<lsp_types::Range>,
        tab_size: usize,
        insert_spaces: bool,
    ) -> anyhow::Result<Vec<TextEdit>> {
        ensure!(
            self.service.provider.formatting,
            "Service did not declare formatting"
        );
        self.edit_request(document, "formatting", move |connection, uri| {
            let method = if selection.is_some() {
                if !connection.range_formatting_provider {
                    return Err(plugin_runtime::plugin_protocol::api::Failure::new(
                        plugin_runtime::plugin_protocol::api::ErrorCode::UnsupportedOperation,
                        "Language service does not support range formatting",
                    ).into());
                }
                "textDocument/rangeFormatting"
            } else {
                if !connection.formatting_provider {
                    return Err(plugin_runtime::plugin_protocol::api::Failure::new(
                        plugin_runtime::plugin_protocol::api::ErrorCode::UnsupportedOperation,
                        "Language service does not support formatting",
                    ).into());
                }
                "textDocument/formatting"
            };
            let wire = connection.sync_document(uri, source)?;
            let mut parameters = json!({
                "textDocument":{"uri":wire}, "options":{"tabSize":tab_size,"insertSpaces":insert_spaces}
            });
            if let Some(selection) = selection { parameters["range"] = serde_json::to_value(selection)?; }
            let response = connection.request(method, parameters)?;
            if response.is_null() { Ok(Vec::new()) } else { serde_json::from_value(response).context("Decode formatting edits") }
        })
    }

    /// A prepare response controls whether the ordinary native rename field can be opened.
    pub(crate) fn prepare_rename_for(
        &self,
        document: DocumentLease,
        source: String,
        position: Position,
    ) -> anyhow::Result<Option<PrepareRenameResponse>> {
        ensure!(self.provides_editing(), "Service did not declare editing");
        self.edit_request(document, "prepareRename", move |connection, uri| {
            ensure!(
                connection.rename_provider,
                "Language service does not support rename"
            );
            if !connection.prepare_rename_provider {
                return Ok(Some(PrepareRenameResponse::DefaultBehavior {
                    default_behavior: true,
                }));
            }
            let wire = connection.sync_document(uri, source)?;
            let response = connection.request(
                "textDocument/prepareRename",
                json!({"textDocument":{"uri":wire},"position":position}),
            )?;
            if response.is_null() {
                Ok(None)
            } else {
                serde_json::from_value(response)
                    .map(Some)
                    .context("Decode rename target")
            }
        })
    }

    /// Unnegotiated workspace refactors remain restricted to the original open document.
    /// Cross-document and resource effects require `rename_workspace_for` and its guarded preview.
    pub(crate) fn rename_for(
        &self,
        document: DocumentLease,
        source: String,
        position: Position,
        new_name: String,
    ) -> anyhow::Result<Vec<TextEdit>> {
        ensure!(self.provides_editing(), "Service did not declare editing");
        self.edit_request(document, "rename", move |connection, uri| {
            ensure!(
                connection.rename_provider,
                "Language service does not support rename"
            );
            let wire = connection.sync_document(uri.clone(), source)?;
            let version = connection
                .diagnostics
                .document_version(uri.as_str())
                .context("Rename document is not synchronized")?;
            let response = connection.request(
                "textDocument/rename",
                json!({"textDocument":{"uri":wire},"position":position,"newName":new_name}),
            )?;
            if response.is_null() {
                return Ok(Vec::new());
            }
            document_edits(response, &uri, version, |target| {
                connection.logical_uri(target)
            })
        })
    }

    /// Plugins provide semantic pairing; the host neither parses tags nor guesses matching names.
    pub(crate) fn linked_ranges_for(
        &self,
        document: DocumentLease,
        source: String,
        position: Position,
    ) -> anyhow::Result<Option<LinkedEditingProposal>> {
        ensure!(self.provides_editing(), "Service did not declare editing");
        self.edit_request(document, "linkedEditingRange", move |connection, uri| {
            let (method, authority) = if connection.semantic_linked_editing_provider {
                (
                    SEMANTIC_LINKED_EDITING_METHOD,
                    LinkedEditingAuthority::Semantic,
                )
            } else if connection.linked_editing_provider {
                (
                    "textDocument/linkedEditingRange",
                    LinkedEditingAuthority::Standard,
                )
            } else {
                return Ok(None);
            };
            let wire = connection.sync_document(uri, source)?;
            let response = connection.request(
                method,
                json!({"textDocument":{"uri":wire},"position":position}),
            )?;
            if response.is_null() {
                Ok(None)
            } else {
                serde_json::from_value(response)
                    .map(|response| {
                        Some(LinkedEditingProposal {
                            response,
                            authority,
                        })
                    })
                    .context("Decode linked ranges")
            }
        })
    }

    /// Every editing method binds before sync and rechecks authority after a native response arrives.
    pub(super) fn edit_request<T>(
        &self,
        document: DocumentLease,
        operation: &str,
        action: impl FnOnce(&mut LanguageServerConnection, Uri) -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        self.prepare()?;
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| anyhow!("LSP connection lock poisoned"))?;
        let current = connection
            .as_mut()
            .context("LSP provider has been retired")?;
        current.bind_document(&document)?;
        let result = action(current, document.uri.clone());
        self.connection_failed(operation, &result, &mut connection);
        ensure!(
            self.is_active() && document.is_active(),
            "Editing request owner has been retired"
        );
        result
    }
}

/// Decode either standard WorkspaceEdit representation before any native text transaction.
/// Optional wire versions must match the synchronized snapshot; null versions still rely on
/// the caller's document lease and final native revision/source guards. Resource operations,
/// cross-document edits, mixed representations and unnegotiated annotations are rejected whole.
pub(super) fn document_edits(
    response: Value,
    uri: &Uri,
    version: i32,
    logical_uri: impl Fn(&Uri) -> Uri,
) -> anyhow::Result<Vec<TextEdit>> {
    let response = without_empty_representations(response);
    // Single-document fallback must not discard an unsupported effect while accepting its text.
    ensure!(
        response.get("command").is_none(),
        Failure::new(
            ErrorCode::UnsupportedOperation,
            "Workspace edit commands are not supported"
        )
    );
    // OneOf<TextEdit, AnnotatedTextEdit> may deserialize the plain variant first and discard
    // annotationId as an unknown field. Check the actual wire before that lossy typed decoding.
    for document in response
        .get("documentChanges")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        for edit in document
            .get("edits")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            ensure!(
                edit.get("annotationId").is_none(),
                Failure::new(
                    ErrorCode::UnsupportedOperation,
                    "Workspace edit annotations are not supported"
                )
            );
        }
    }
    // `changes` only permits plain TextEdit too; reject an illegal annotation before serde drops it.
    for edits in response
        .get("changes")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|changes| changes.values())
    {
        for edit in edits.as_array().into_iter().flatten() {
            ensure!(
                edit.get("annotationId").is_none(),
                Failure::new(
                    ErrorCode::UnsupportedOperation,
                    "Workspace edit annotations are not supported"
                )
            );
        }
    }
    let edit: WorkspaceEdit = serde_json::from_value(response).map_err(|error| {
        Failure::new(
            ErrorCode::InvalidRequest,
            format!("Decode workspace edits: {error}"),
        )
    })?;
    ensure!(
        edit.change_annotations
            .as_ref()
            .is_none_or(|annotations| annotations.is_empty()),
        Failure::new(
            ErrorCode::UnsupportedOperation,
            "Workspace edit annotations are not supported"
        )
    );
    ensure!(
        edit.changes.is_none() || edit.document_changes.is_none(),
        Failure::new(
            ErrorCode::InvalidRequest,
            "Workspace edit mixes changes and documentChanges"
        )
    );
    let mut edits = Vec::new();
    for (target, changes) in edit.changes.unwrap_or_default() {
        ensure!(
            logical_uri(&target) == *uri,
            Failure::new(
                ErrorCode::UnsupportedOperation,
                "Workspace edit targets another document"
            )
        );
        edits.extend(changes);
    }
    let documents = match edit.document_changes {
        Some(DocumentChanges::Edits(documents)) => documents,
        Some(DocumentChanges::Operations(_)) => {
            return Err(Failure::new(
                ErrorCode::UnsupportedOperation,
                "Workspace resource operations are not supported",
            )
            .into());
        }
        None => return Ok(edits),
    };
    for document in documents {
        ensure!(
            logical_uri(&document.text_document.uri) == *uri,
            Failure::new(
                ErrorCode::UnsupportedOperation,
                "Workspace edit targets another document"
            )
        );
        ensure!(
            document
                .text_document
                .version
                .is_none_or(|declared| declared == version),
            Failure::new(
                ErrorCode::StaleRevision,
                "Workspace edit has a stale document version"
            )
        );
        for edit in document.edits {
            match edit {
                OneOf::Left(edit) => edits.push(edit),
                OneOf::Right(_) => {
                    return Err(Failure::new(
                        ErrorCode::UnsupportedOperation,
                        "Workspace edit annotations are not supported",
                    )
                    .into());
                }
            }
        }
    }
    Ok(edits)
}

/// An empty companion representation carries no effects. Some standard servers serialize their
/// default empty map beside documentChanges; retain rejection of two nonempty representations.
pub(super) fn without_empty_representations(mut response: Value) -> Value {
    if let Some(object) = response.as_object_mut() {
        if object
            .get("changes")
            .is_some_and(|value| value.as_object().is_some_and(|map| map.is_empty()))
        {
            object.remove("changes");
        }
        if object
            .get("documentChanges")
            .is_some_and(|value| value.as_array().is_some_and(|items| items.is_empty()))
        {
            object.remove("documentChanges");
        }
    }
    response
}

/// Reject invalid UTF-16 positions instead of upstream clipping them to a different document location.
pub(crate) fn byte_at_position(source: &str, position: Position) -> anyhow::Result<usize> {
    let mut line_start = 0;
    for _ in 0..position.line {
        line_start += source[line_start..]
            .find('\n')
            .context("Edit line is outside document")?
            + 1;
    }
    let line = source[line_start..].split('\n').next().unwrap_or("");
    let mut units = 0;
    for (byte, character) in line.char_indices() {
        if units == position.character {
            return Ok(line_start + byte);
        }
        units += character.len_utf16() as u32;
    }
    ensure!(
        units == position.character,
        "Edit character is not a UTF-16 boundary"
    );
    Ok(line_start + line.len())
}

/// One contiguous edit preserves intervening bytes and yields one native atomic Undo transaction.
pub(crate) fn merge_edits(source: &str, edits: &[TextEdit]) -> anyhow::Result<Option<TextEdit>> {
    ensure!(edits.len() <= 10_000, "Too many editing ranges");
    let mut ordered = edits
        .iter()
        .map(|edit| {
            let start = byte_at_position(source, edit.range.start)?;
            let end = byte_at_position(source, edit.range.end)?;
            ensure!(
                start <= end && edit.new_text.len() <= 16 * 1024 * 1024,
                "Invalid edit range or quota"
            );
            Ok((start..end, edit.new_text.as_str()))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    ordered.sort_by_key(|(range, _)| (range.start, range.end));
    for pair in ordered.windows(2) {
        ensure!(
            pair[0].0.end <= pair[1].0.start && pair[0].0.start != pair[1].0.start,
            "Overlapping editing ranges"
        );
    }
    let Some((first, _)) = ordered.first() else {
        return Ok(None);
    };
    let start = first.start;
    let end = ordered.last().unwrap().0.end;
    let removed = ordered.iter().map(|(range, _)| range.len()).sum::<usize>();
    let inserted = ordered.iter().try_fold(0usize, |total, (_, text)| {
        total
            .checked_add(text.len())
            .context("Editing proposal exceeds quota")
    })?;
    ensure!(
        (end - start - removed)
            .checked_add(inserted)
            .is_some_and(|length| length <= 32 * 1024 * 1024),
        "Editing proposal exceeds quota"
    );
    let mut replacement = source[start..end].to_string();
    for (range, text) in ordered.into_iter().rev() {
        replacement.replace_range(range.start - start..range.end - start, text);
    }
    ensure!(
        replacement.len() <= 32 * 1024 * 1024,
        "Editing proposal exceeds quota"
    );
    if source[start..end] == replacement {
        return Ok(None);
    }
    Ok(Some(TextEdit {
        range: lsp_types::Range {
            start: position_at_byte(source, start),
            end: position_at_byte(source, end),
        },
        new_text: replacement,
    }))
}
