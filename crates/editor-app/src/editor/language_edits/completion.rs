//! Selected completion details and import edits share one native source seal and atomic transaction.
use super::*;
use lsp_types::{CompletionItem, CompletionResponse, CompletionTextEdit, InsertTextFormat};
use std::sync::atomic::{AtomicU64, Ordering};

/// One current menu retains immutable source and exact service lifetime, never another text model.
#[derive(Clone)]
pub(super) struct Origin {
    ticket: u64,
    document: DocumentVersion,
    source: String,
    cursor: usize,
    server: Arc<navigation::LanguageServer>,
    lease: navigation::DocumentLease,
    context: Option<navigation::ContextCapture>,
}

impl EditorApp {
    /// Stamp host metadata after a response passes native revision checks. Opaque server data is
    /// nested verbatim and restored before resolve; no server-supplied stamp grants authority.
    pub(crate) fn seal_completions(
        &mut self,
        mut response: CompletionResponse,
        document: DocumentVersion,
        source: String,
        cursor: usize,
        server: Arc<navigation::LanguageServer>,
        context: Option<navigation::ContextCapture>,
    ) -> CompletionResponse {
        static TICKET: AtomicU64 = AtomicU64::new(1);
        let ticket = TICKET.fetch_add(1, Ordering::Relaxed);
        let Some(uri) = navigation::file_uri(&self.workspace.root().join(&document.path)) else {
            return CompletionResponse::Array(Vec::new());
        };
        let Some(lease) = server.document(&uri) else {
            return CompletionResponse::Array(Vec::new());
        };
        self.language_edits.completion = Some(Origin {
            ticket,
            document,
            source,
            cursor,
            server,
            lease,
            context,
        });
        let items = match &mut response {
            CompletionResponse::Array(items) => items,
            CompletionResponse::List(list) => &mut list.items,
        };
        for item in items {
            item.data =
                Some(serde_json::json!({"_nanobugCompletion":ticket,"data":item.data.take()}));
        }
        response
    }

    /// Return false only for an unrelated Base provider; marked language items are always consumed,
    /// including stale or rejected items, so Base cannot apply a partial main edit as fallback.
    pub(crate) fn accept_language_completion(
        &mut self,
        mut item: CompletionItem,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(ticket) = item
            .data
            .as_ref()
            .and_then(|data| data["_nanobugCompletion"].as_u64())
        else {
            return false;
        };
        let Some(origin) = self
            .language_edits
            .completion
            .take()
            .filter(|origin| origin.ticket == ticket)
        else {
            return true;
        };
        let valid = self.completion_origin_current(&origin, cx);
        self.editor
            .update(cx, |editor, cx| editor.dismiss_completion_overlay(cx));
        if !valid {
            return true;
        }
        let context = match self.capture_language_context(
            &origin.server,
            &origin.document,
            &origin.source,
            cx,
        ) {
            Ok(context) => context,
            Err(_) => return true,
        };
        let response_context = context.clone();
        item.data = item
            .data
            .take()
            .and_then(|mut data| data.get_mut("data").map(serde_json::Value::take))
            .filter(|data| !data.is_null());
        let captured = origin.clone();
        cx.spawn_in(window, async move |this, cx| {
            let resolved = cx
                .background_executor()
                .scheduler_executor()
                .spawn_dedicated(move |_| async move {
                    let operation = || {
                        captured.server.resolve_completion_for(
                            captured.lease,
                            captured.source,
                            item,
                        )
                    };
                    match &context {
                        Some(context) => captured.server.with_context_snapshot(context, operation),
                        None => operation(),
                    }
                })
                .await;
            let _ = this.update_in(cx, |app, window, cx| {
                if !app.completion_origin_current(&origin, cx) {
                    return;
                }
                if response_context
                    .as_ref()
                    .is_some_and(|context| !app.language_context_is_current(context, cx))
                {
                    return;
                }
                let result = resolved.and_then(|item| {
                    let (edits, cursor) = completion_edits(&origin.source, origin.cursor, item)?;
                    app.apply_language_edits(
                        &origin.document,
                        &origin.source,
                        &origin.server,
                        &edits,
                        cursor,
                        window,
                        cx,
                    )
                });
                if let Err(error) = result {
                    app.status =
                        t!("editor.editing_failed", error = format!("{error:#}")).to_string();
                }
                cx.notify();
            });
        })
        .detach();
        true
    }

    /// Cursor changes, ABA document revisions, provider switches and closed leases expire a menu.
    fn completion_origin_current(&self, origin: &Origin, cx: &Context<Self>) -> bool {
        self.active_text_tab_index().is_some_and(|index| {
            self.plugin_document_version(index).ok().as_ref() == Some(&origin.document)
                && self
                    .language_server_for_path(self.tabs[index].path())
                    .is_some_and(|server| Arc::ptr_eq(server, &origin.server))
        }) && origin.lease.is_active()
            && origin.server.is_active()
            && self.session_state.workspace_trusted
            && origin
                .context
                .as_ref()
                .is_none_or(|context| self.language_context_is_current(context, cx))
            && self.editor.read(cx).cursor() == origin.cursor
            && self.editor.read(cx).text().to_string() == origin.source
    }
}

/// Strict UTF-16 edits retain their original positions; main replacement and all imports validate
/// together before one DocumentSession mutation. Commands/snippets are not negotiated or executed.
fn completion_edits(
    source: &str,
    cursor: usize,
    item: CompletionItem,
) -> anyhow::Result<(Vec<TextEdit>, usize)> {
    use navigation::editing::byte_at_position;
    anyhow::ensure!(
        item.command.is_none(),
        "Completion commands are unsupported"
    );
    anyhow::ensure!(
        item.insert_text_format != Some(InsertTextFormat::SNIPPET),
        "Completion snippets are unsupported"
    );
    let mut main = match item.text_edit {
        Some(CompletionTextEdit::Edit(edit)) => edit,
        Some(CompletionTextEdit::InsertAndReplace(edit)) => {
            anyhow::ensure!(
                edit.insert.start == edit.replace.start && edit.insert.end <= edit.replace.end,
                "Invalid completion insert/replace ranges"
            );
            TextEdit {
                range: edit.replace,
                new_text: edit.new_text,
            }
        }
        None => {
            let prefix = crate::language::completion::identifier_prefix(source, cursor);
            TextEdit {
                range: lsp_types::Range {
                    start: navigation::position_at_byte(source, cursor - prefix.len()),
                    end: navigation::position_at_byte(source, cursor),
                },
                new_text: item.insert_text.unwrap_or(item.label),
            }
        }
    };
    let start = byte_at_position(source, main.range.start)?;
    let main_end = byte_at_position(source, main.range.end)?;
    anyhow::ensure!(
        main.range.start.line == main.range.end.line && start <= cursor && cursor <= main_end,
        "Completion replacement must contain its original caret on one line"
    );
    let mut caret = start + main.new_text.len();
    let additional = item.additional_text_edits.unwrap_or_default();
    // Validate additional edits together first. Combining one boundary insertion must not conceal
    // duplicated import positions or an invalid later edit from the whole-proposal validator.
    navigation::editing::merge_edits(source, &additional)?;
    let mut edits = Vec::with_capacity(additional.len() + 1);
    for edit in additional {
        let begin = byte_at_position(source, edit.range.start)?;
        let end = byte_at_position(source, edit.range.end)?;
        if end <= start {
            caret = caret
                .checked_sub(end - begin)
                .and_then(|caret| caret.checked_add(edit.new_text.len()))
                .ok_or_else(|| anyhow::anyhow!("Completion caret exceeds limits"))?;
        }
        if begin == start && end == start && main_end > start {
            // A first-line import inserts immediately before a nonempty token replacement.
            // Express their unambiguous order as one edit; the generic validator stays strict
            // about equal-start ranges. Two insertions or a true interior overlap still fail.
            main.new_text.insert_str(0, &edit.new_text);
        } else {
            edits.push(edit);
        }
    }
    edits.push(main);
    navigation::editing::merge_edits(source, &edits)?;
    Ok((edits, caret))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::{Position, Range};

    /// A real TS auto-import may insert at the start of a nonempty first-token replacement.
    /// Both operations must reach one native transaction, with the caret after the completed name.
    #[test]
    fn completion_import_at_first_token_boundary_forms_one_atomic_proposal() {
        let import = "import { double } from './math';\r\n\r\n";
        let item = CompletionItem {
            label: "double".into(),
            text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                range: Range::new(Position::new(0, 0), Position::new(0, 4)),
                new_text: "double".into(),
            })),
            additional_text_edits: Some(vec![TextEdit {
                range: Range::new(Position::new(0, 0), Position::new(0, 0)),
                new_text: import.into(),
            }]),
            ..Default::default()
        };
        let (edits, caret) = completion_edits("doub", 4, item).unwrap();
        let expected = format!("{import}double");
        assert_eq!(
            navigation::editing::merge_edits("doub", &edits)
                .unwrap()
                .unwrap()
                .new_text,
            expected
        );
        assert_eq!(caret, expected.len());
    }

    /// Boundary coalescing must not allow an overlapping import, duplicate insert or two insertions.
    #[test]
    fn completion_boundary_does_not_admit_conflicting_additional_edits() {
        let at = |start, end| Range::new(Position::new(0, start), Position::new(0, end));
        for (main, extra) in [
            (at(0, 4), vec![at(0, 1)]),
            (at(0, 4), vec![at(0, 0), at(0, 0)]),
            (at(0, 0), vec![at(0, 0)]),
            (at(0, 4), vec![at(2, 2)]),
        ] {
            let item = CompletionItem {
                label: "double".into(),
                text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                    range: main,
                    new_text: "double".into(),
                })),
                additional_text_edits: Some(
                    extra
                        .into_iter()
                        .map(|range| TextEdit {
                            range,
                            new_text: "import".into(),
                        })
                        .collect(),
                ),
                ..Default::default()
            };
            assert!(completion_edits("doub", main.end.character as usize, item).is_err());
        }
    }
}
