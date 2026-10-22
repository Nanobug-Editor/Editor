//! Conventional rename input and cancellable proposals share the native document transaction gate.
use super::*;
use anyhow::Context as _;
use crate::language::navigation::editing::byte_at_position;
use gpui_kit::EntityInputHandler as _;

impl EditorApp {
    /// Prepare the ordinary rename field; an unsupported or unknown semantic target produces no edits.
    pub(crate) fn rename_symbol_action(
        &mut self,
        _: &RenameSymbol,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.reject_readonly_document_action(window, cx) {
            return;
        }
        if self.wait_for_native_input(DocumentAction::Rename, window, cx) {
            return;
        }
        let Some(index) = self.active_text_tab_index() else {
            return;
        };
        let Some(server) = self
            .language_server_for_path(self.tabs[index].path())
            .filter(|server| server.provides_editing())
            .cloned()
        else {
            self.status = t!("editor.rename_unavailable").to_string();
            cx.notify();
            return;
        };
        let Ok(document) = self.plugin_document_version(index) else {
            self.status = t!("editor.editing_stale").to_string();
            cx.notify();
            return;
        };
        let source = self.editor.read(cx).text().to_string();
        let cursor = self.editor.read(cx).cursor();
        let position = navigation::position_at_byte(&source, cursor);
        let Some(uri) = navigation::file_uri(self.tabs[index].path()) else {
            return;
        };
        let lease = server.open_document(uri);
        let captured_lease = lease.clone();
        let active = server.clone();
        let snapshot = source.clone();
        self.language_edits.cancel_rename();
        self.language_edits.rename_pending = true;
        let request = self.language_edits.rename_request;
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_executor()
                .scheduler_executor()
                .spawn_dedicated(move |_| async move {
                    server
                        .prepare_rename_for(lease, snapshot, position)
                        .map(|response| response)
                })
                .await;
            let _ = this.update_in(cx, |app, window, cx| {
                if app.language_edits.rename_request != request {
                    return;
                }
                app.language_edits.rename_pending = false;
                if app
                    .active_text_tab_index()
                    .and_then(|index| app.plugin_document_version(index).ok())
                    .as_ref()
                    != Some(&document)
                    || app.editor.read(cx).cursor() != cursor
                    || !active.is_active()
                    || !app
                        .language_server_for_path(&app.workspace.root().join(&document.path))
                        .is_some_and(|current| Arc::ptr_eq(current, &active))
                {
                    return;
                }
                let name = match result {
                    Ok(Some(PrepareRenameResponse::Range(range))) => {
                        byte_at_position(&source, range.start)
                            .and_then(|start| {
                                byte_at_position(&source, range.end).map(|end| (start, end))
                            })
                            .ok()
                            .and_then(|(start, end)| source.get(start..end))
                            .unwrap_or("")
                            .to_string()
                    }
                    Ok(Some(PrepareRenameResponse::RangeWithPlaceholder {
                        placeholder, ..
                    })) => placeholder,
                    Ok(Some(_)) => app.editor.read(cx).selected_text().to_string(),
                    _ => {
                        app.status = t!("editor.rename_unavailable").to_string();
                        cx.notify();
                        return;
                    }
                };
                let input = cx.new(|cx| InputState::new(window, cx).default_value(name));
                input.update(cx, |input, cx| {
                    input.focus(window, cx);
                    input.set_selected_range(0..input.text().len(), cx);
                });
                app.language_edits.rename = Some(RenameForm {
                    document,
                    lease: captured_lease,
                    position,
                    source,
                    server: active,
                    input,
                });
                cx.notify();
            });
        })
        .detach();
    }

    /// The name input is separate native UI; a complete rename always enters native preview.
    fn confirm_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(form) = self.language_edits.rename.as_ref() else {
            return;
        };
        let new_name = form.input.read(cx).value().to_string();
        if new_name.is_empty() || new_name.len() > 1024 {
            return;
        }
        let form = self.language_edits.rename.take().unwrap();
        self.dispatch_rename(form, new_name, false, window, cx);
    }

    /// At most one discovery round opens unknown targets, then recomputes against real snapshots.
    fn dispatch_rename(
        &mut self,
        form: RenameForm,
        new_name: String,
        discovered: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self.active_text_tab_index() else {
            return;
        };
        if self.plugin_document_version(index).ok().as_ref() != Some(&form.document)
            || !form.lease.is_active()
            || !self
                .language_server_for_path(self.tabs[index].path())
                .is_some_and(|current| Arc::ptr_eq(current, &form.server))
        {
            return;
        }
        let lease = form.lease.clone();
        let active = form.server.clone();
        let snapshot = form.source.clone();
        let context = match self.capture_language_context(&active, &form.document, &form.source, cx)
        {
            Ok(context) => context,
            Err(error) => {
                self.status = t!("editor.editing_failed", error = error.message).to_string();
                cx.notify();
                return;
            }
        };
        let response_context = context.clone();
        let snapshots = match if context.is_some() { Ok(Vec::new()) } else { self.rename_snapshots(&active, cx) } {
            Ok(snapshots) => snapshots,
            Err(error) => {
                self.status = t!("editor.editing_failed", error = format!("{error:#}")).to_string();
                cx.notify();
                return;
            }
        };
        let related = snapshots
            .iter()
            .filter(|_| context.is_none())
            .map(|source| (source.lease.clone(), source.text.clone()))
            .collect();
        let name = new_name.clone();
        let position = form.position;
        let server = form.server.clone();
        self.language_edits.rename_request = self.language_edits.rename_request.wrapping_add(1);
        self.language_edits.rename_pending = true;
        let request = self.language_edits.rename_request;
        self.editor
            .update(cx, |editor, cx| editor.focus(window, cx));
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_executor()
                .scheduler_executor()
                .spawn_dedicated(move |_| async move {
                    let operation = || server.rename_for(lease, snapshot, position, name, related);
                    match &context { Some(context) => server.with_context_snapshot(context, operation), None => operation() }
                })
                .await;
            let _ = this.update_in(cx, |app, window, cx| {
                if app.language_edits.rename_request != request {
                    return;
                }
                app.language_edits.rename_pending = false;
                if !app
                    .language_server_for_path(&app.workspace.root().join(&form.document.path))
                    .is_some_and(|current| Arc::ptr_eq(current, &active))
                {
                    return;
                }
                let result = result.and_then(|edits| {
                    anyhow::ensure!(response_context.as_ref().is_none_or(|context| app.language_context_is_current(context, cx)), "Rename project context changed");
                    let snapshots = if let Some(context) = &response_context {
                        context.selected_sources().context("Rename context is unfinished")?.into_iter().map(|source| {
                            Ok(super::workspace_edits::Snapshot { document: source.document.clone(), text: source.text.to_string(), lease: active.document(&source.uri).context("Rename member lease was closed")? })
                        }).collect::<anyhow::Result<Vec<_>>>()?
                    } else { snapshots };
                    app.check_rename_snapshots(&snapshots, cx)?;
                    if edits.is_empty() { return Ok(None); }
                    // Preserve the established single-document editing contract and local Undo.
                    // Multi-document changes use the stronger workspace preview capability below.
                    if edits.len() == 1 && edits[0].uri == form.lease.uri {
                        let cursor = app.editor.read(cx).cursor();
                        app.apply_language_edits(&form.document, &form.source, &active, &edits[0].edits, cursor, window, cx)?;
                        app.status = t!("editor.renamed").to_string();
                        return Ok(None);
                    }
                    // Detect a new target before opening anything on a second discovery reply.
                    if discovered {
                        anyhow::ensure!(edits.iter().all(|edit| snapshots.iter().any(|source| source.lease.uri == edit.uri)), "Rename discovered another uncaptured target; retry with its documents open");
                    }
                    if app.discover_rename_targets(&active, &snapshots, &edits, window, cx)? { return Ok(Some(None)); }
                    let conditions = response_context.as_ref().and_then(|context| context.documents()).map(|documents| documents.to_vec()).unwrap_or_else(|| snapshots.iter().map(|snapshot| snapshot.document.clone()).collect());
                    let fix = app.rename_plan(&form.document, t!("editor.rename_confirm").to_string(), conditions, &snapshots, edits)?;
                    Ok(Some(Some(fix)))
                });
                match result {
                    Ok(Some(None)) => {
                        app.status = t!("editor.rename_opened_targets").to_string();
                        app.dispatch_rename(form, new_name, true, window, cx);
                        return;
                    }
                    Ok(Some(Some(fix))) => {
                        let origin = response_context.as_ref().map_or_else(|| active.fix_origin(), |context| context.fix_origin(&active));
                        if let Err(error) = app.submit_native_language_fix(origin, form.document.clone(), fix, cx) {
                            app.status = t!("editor.editing_failed", error = error.message).to_string();
                        }
                    }
                    Ok(None) => {},
                    Err(error) => {
                        app.status =
                            t!("editor.editing_failed", error = format!("{error:#}")).to_string()
                    }
                }
                app.editor.update(cx, |editor, cx| editor.focus(window, cx));
                cx.notify();
            });
        })
        .detach();
    }

    /// Local input and buttons use Base behavior while preserving the project's theme and translations.
    pub(crate) fn render_rename_prompt(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<gpui_kit::AnyElement> {
        let form = self.language_edits.rename.as_ref()?;
        Some(
            h_flex()
                .debug_selector(|| "editor-rename-prompt".into())
                // Base emits Enter and propagates for its parent to submit. Consume it before
                // platform character fallback can replace a selected name with a filtered newline.
                .on_action(cx.listener(|app, _: &gpui_base::input::Enter, window, cx| {
                    let composing = app.language_edits.rename.as_ref().is_some_and(|form| {
                        form.input.update(cx, |input, cx| {
                            input.marked_text_range(window, cx).is_some()
                        })
                    });
                    if composing {
                        // The platform/Base input keeps ownership of confirming an active IME preedit.
                        cx.propagate();
                        return;
                    }
                    cx.stop_propagation();
                    app.confirm_rename(window, cx);
                }))
                .absolute()
                .top_2()
                .right_2()
                .gap_2()
                .p_2()
                .bg(cx.theme().background)
                .border_1()
                .border_color(cx.theme().border)
                .rounded(cx.theme().radius)
                .child(
                    div()
                        .w(px(220.))
                        .child(crate::ui::controls::Input::new(&form.input)),
                )
                .child(
                    Button::new("editor-rename-confirm")
                        .label(t!("editor.rename_confirm").to_string())
                        .on_click(cx.listener(|app, _, window, cx| app.confirm_rename(window, cx))),
                )
                .child(
                    Button::new("editor-rename-cancel")
                        .label(t!("editor.rename_cancel").to_string())
                        .on_click(cx.listener(|app, _, window, cx| {
                            app.language_edits.cancel_rename();
                            app.editor.update(cx, |editor, cx| editor.focus(window, cx));
                            cx.notify();
                        })),
                )
                .into_any_element(),
        )
    }

    /// Escape dismisses the ordinary rename field and returns focus to the native source.
    pub(crate) fn cancel_rename_action(
        &mut self,
        _: &gpui_base::input::Escape,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.language_edits.cancel_rename() {
            cx.stop_propagation();
            self.editor
                .update(cx, |editor, cx| editor.focus(window, cx));
            cx.notify();
        } else {
            cx.propagate();
        }
    }
}
