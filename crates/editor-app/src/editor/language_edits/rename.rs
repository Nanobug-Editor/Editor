//! Conventional rename input and cancellable proposals share the native document transaction gate.
use super::*;
use crate::language::navigation::editing::byte_at_position;
use crate::language::navigation::workspace_edits::{Rename, Reply, Sources};
use gpui_kit::EntityInputHandler as _;
mod missing;

/// One analysis round owns its original form, revision vector and immutable context seals.
struct Round {
    form: RenameForm,
    request: u64,
    context: Option<navigation::ContextCapture>,
    revisions: Vec<DocumentVersion>,
    source_bytes: usize,
}

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
        // The original source and every provider-selected dirty dependency must remain current
        // before the rename prompt can offer a semantic target from this analysis round.
        let context = match self.capture_language_context(&server, &document, &source, cx) {
            Ok(context) => context,
            Err(error) => {
                self.status = t!("editor.editing_failed", error = error.message).to_string();
                cx.notify();
                return;
            }
        };
        let response_context = context.clone();
        self.language_edits.cancel_rename();
        self.language_edits.rename_pending = true;
        let request = self.language_edits.rename_request;
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_executor()
                .scheduler_executor()
                .spawn_dedicated(move |_| async move {
                    let operation = || server.prepare_rename_for(lease, snapshot, position);
                    match &context {
                        Some(context) => server.with_context_snapshot(context, operation),
                        None => operation(),
                    }
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
                    || response_context
                        .as_ref()
                        .is_some_and(|context| !app.language_context_is_current(context, cx))
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

    /// The name input is separate native UI; the full paired rename is a single document transaction.
    fn confirm_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(form) = self.language_edits.rename.as_ref() else {
            return;
        };
        let new_name = form.input.read(cx).value().to_string();
        if new_name.is_empty() || new_name.len() > 1024 {
            return;
        }
        let form = self.language_edits.rename.take().unwrap();
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
        // Confirmation takes a fresh immutable round; waiting in the name input does not retain
        // permission to apply results computed from an earlier project graph.
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
        let server = form.server.clone();
        let sources = if server.supports_workspace_edits() {
            match self.capture_rename_sources(&server, &form.document, cx) {
                Ok(sources) => Some(sources),
                Err(error) => {
                    self.status = t!("editor.editing_failed", error = error.message).to_string();
                    cx.notify();
                    return;
                }
            }
        } else {
            None
        };
        let revisions = sources
            .as_ref()
            .map(|sources| {
                sources
                    .documents
                    .iter()
                    .map(|source| source.document.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let title = t!("editor.rename_confirm").to_string();
        let source_bytes = sources
            .as_ref()
            .map(|sources| {
                sources
                    .documents
                    .iter()
                    .map(|source| source.text.len())
                    .sum()
            })
            .unwrap_or(0);
        self.language_edits.rename_request = self.language_edits.rename_request.wrapping_add(1);
        self.language_edits.rename_pending = true;
        let request = self.language_edits.rename_request;
        let position = form.position;
        let round = Round {
            form,
            request,
            context: response_context,
            revisions,
            source_bytes,
        };
        self.editor
            .update(cx, |editor, cx| editor.focus(window, cx));
        cx.spawn_in(window, async move |this, cx| {
            let result =
                cx.background_executor()
                    .scheduler_executor()
                    .spawn_dedicated(move |_| async move {
                        let operation = || match sources {
                            Some(sources) => server
                                .rename_workspace_for(lease, position, new_name, sources, title),
                            None => server.rename_for(lease, snapshot, position, new_name).map(
                                |edits| Reply {
                                    proposal: Rename::Document(edits),
                                    origin: server.fix_origin(),
                                },
                            ),
                        };
                        match &context {
                            Some(context) => server.with_context_snapshot(context, operation),
                            None => operation(),
                        }
                    })
                    .await;
            let _ = this.update_in(cx, |app, window, cx| {
                app.finish_rename(round, result, window, cx)
            });
        })
        .detach();
    }

    /// Delivery rechecks original owner, all captured revisions and transport seals before any effect.
    fn finish_rename(
        &mut self,
        round: Round,
        result: anyhow::Result<Reply>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.language_edits.rename_request != round.request {
            return;
        }
        self.language_edits.rename_pending = false;
        let form = &round.form;
        if !self
            .language_server_for_path(&self.workspace.root().join(&form.document.path))
            .is_some_and(|current| Arc::ptr_eq(current, &form.server))
            || round
                .context
                .as_ref()
                .is_some_and(|context| !self.language_context_is_current(context, cx))
            || round.revisions.iter().any(|expected| {
                !(0..self.tabs.len()).any(|index| {
                    self.plugin_document_version(index).ok().as_ref() == Some(expected)
                })
            })
        {
            return;
        }
        let reply = match result {
            Ok(reply) if reply.origin.is_live() => reply,
            Ok(_) => return,
            Err(error) => {
                self.status = t!("editor.editing_failed", error = format!("{error:#}")).to_string();
                cx.notify();
                return;
            }
        };
        if let Rename::Sources(targets) = reply.proposal {
            if let Err(error) = self.rename_missing_sources(round, targets, window, cx) {
                self.status = t!("editor.editing_failed", error = error.message).to_string();
                cx.notify();
            }
            return;
        }
        let cursor = self.editor.read(cx).cursor();
        let outcome = match reply.proposal {
            Rename::Document(edits) => self
                .apply_language_edits(
                    &form.document,
                    &form.source,
                    &form.server,
                    &edits,
                    cursor,
                    window,
                    cx,
                )
                .map(|_| false),
            Rename::Workspace(plan) => {
                let mut origin = reply.origin;
                if let (Some(context), plugin_runtime::LanguageFixOrigin::GuardedLsp { guards, .. }) =
                    (&round.context, &mut origin)
                    && let plugin_runtime::LanguageFixOrigin::GuardedLsp { guards: extra, .. } =
                        context.fix_origin(&form.server)
                {
                    guards.extend(extra);
                }
                self.submit_native_language_fix(
                    origin,
                    form.document.clone(),
                    plugin_runtime::plugin_protocol::diagnostics::Fix {
                        title: plan.title.clone(),
                        plan,
                    },
                    cx,
                )
                .map(|_| true)
                .map_err(Into::into)
            }
            Rename::Sources(_) => unreachable!(),
        };
        match outcome {
            Ok(false) => self.status = t!("editor.renamed").to_string(),
            Ok(true) => self.status = t!("editor.rename_preview").to_string(),
            Err(error) => {
                self.status = t!("editor.editing_failed", error = format!("{error:#}")).to_string()
            }
        }
        self.editor
            .update(cx, |editor, cx| editor.focus(window, cx));
        cx.notify();
    }

    /// Capture immutable text only from exact native sources sharing this selected transport.
    /// Bounds are checked while collecting; the host owns every editable byte and later revision check.
    fn capture_rename_sources(
        &self,
        server: &Arc<navigation::LanguageServer>,
        document: &DocumentVersion,
        cx: &App,
    ) -> Result<Sources, plugin_runtime::plugin_protocol::api::Failure> {
        use plugin_runtime::plugin_protocol::{
            api::{ErrorCode, Failure},
            language::SourceSnapshot,
        };
        let mut documents = Vec::new();
        let mut bytes = 0usize;
        for (index, tab) in self.tabs.iter().enumerate() {
            if tab.virtual_document.is_some()
                || !tab.path().starts_with(self.workspace.root())
                || !self
                    .language_server_for_path(tab.path())
                    .is_some_and(|current| Arc::ptr_eq(current, server))
            {
                continue;
            }
            let Some(tab) = tab.text.as_ref() else {
                continue;
            };
            let text = tab.editor.read(cx).text().to_string();
            bytes += text.len();
            if documents.len() >= 64 || bytes > 8 * 1024 * 1024 || text.len() > 1024 * 1024 {
                return Err(Failure::new(
                    ErrorCode::LimitExceeded,
                    "Workspace refactor source budget exceeded",
                ));
            }
            documents.push(SourceSnapshot {
                document: self.plugin_document_version(index)?,
                text,
            });
        }
        Ok(Sources {
            workspace: self.workspace.root().to_owned(),
            documents,
            conditions: self.language_result_conditions(&document.path)?,
        })
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
                .child(div().w(px(220.)).child(editor_ui::Input::new(&form.input)))
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
