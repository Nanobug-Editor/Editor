//! Native completion, formatting and rename share versioned document transactions and provider selection.
use crate::language::navigation;
use crate::*;
use gpui_base::input::InputState;
use lsp_types::{PrepareRenameResponse, TextEdit};
use plugin_runtime::plugin_protocol::api::DocumentVersion;
mod completion;
mod formatting;
mod rename;
mod workspace_edits;
use formatting::Reservation as FormatReservation;

/// Only metadata and UI state live here; native EditorState continues to own every mutable text byte.
#[derive(Default)]
pub(crate) struct State {
    /// A selected language completion retains its immutable origin until resolve or rejection.
    completion: Option<completion::Origin>,
    pub(crate) formatters: HashMap<String, Arc<navigation::LanguageServer>>,
    pub(crate) formatter_errors: HashMap<String, String>,
    pub(crate) bridge: Option<Entity<super::linked_input::Bridge>>,
    /// Replayed native history bypasses capture without allocating another Undo stack.
    pub(crate) history_forwarding: Rc<Cell<bool>>,
    rename: Option<RenameForm>,
    request: u64,
    /// The actual format intention owns its activation wait and Escape cancellation.
    format_pending: Option<plugin_runtime::RequestCancellation>,
    /// Escape, replacement requests and service switches revoke both prepare and rename replies.
    rename_request: u64,
    rename_pending: bool,
}

impl State {
    /// Cancel semantic work without touching already committed native text or history.
    fn cancel_rename(&mut self) -> bool {
        let had_work = self.rename.take().is_some() || self.rename_pending;
        self.rename_request = self.rename_request.wrapping_add(1);
        self.rename_pending = false;
        had_work
    }
}

/// Captured native name input never changes the document until the provider's complete proposal is valid.
struct RenameForm {
    document: DocumentVersion,
    /// Native URI is captured separately: the public document path is workspace-relative.
    lease: navigation::DocumentLease,
    position: lsp_types::Position,
    source: String,
    server: Arc<navigation::LanguageServer>,
    input: Entity<InputState>,
}

/// Document commands wait for earlier native input instead of reading a stale session revision.
#[derive(Clone, Copy)]
enum DocumentAction {
    Save,
    Rename,
}

impl EditorApp {
    /// Escape ends this format wait without touching native source or another provider's callers.
    pub(crate) fn cancel_pending_format(&mut self) {
        if let Some(pending) = self.language_edits.format_pending.take() {
            pending.cancel();
            self.language_edits.request = self.language_edits.request.wrapping_add(1);
        }
    }

    /// Rebind input semantics when the active document or selected service changes.
    pub(crate) fn sync_linked_input(&mut self, cx: &mut Context<Self>) {
        let language = self
            .active_path
            .as_deref()
            .map(|path| self.language_for_path(path))
            .unwrap_or_default();
        let server = self
            .active_path
            .as_deref()
            .and_then(|path| self.language_server_for_path(path))
            .filter(|server| server.provides_editing())
            .cloned();
        if self.active_text_tab_index().is_none()
            || !self.session_state.workspace_trusted
            || server.is_none()
        {
            self.language_edits.cancel_rename();
            if let Some(bridge) = self.language_edits.bridge.take() {
                bridge.update(cx, |bridge, _| bridge.retire());
                if bridge.read(cx).has_pending() {
                    self.language_edits.bridge = Some(bridge);
                }
            }
            return;
        }
        let server = server.unwrap();
        if let Some(bridge) = self
            .language_edits
            .bridge
            .as_ref()
            .filter(|bridge| bridge.read(cx).matches(&self.editor, &server))
        {
            bridge.update(cx, |bridge, cx| bridge.refresh(cx));
            return;
        }
        self.language_edits.cancel_rename();
        if let Some(bridge) = self.language_edits.bridge.take() {
            bridge.update(cx, |bridge, _| bridge.retire());
            if bridge.read(cx).has_pending() {
                self.language_edits.bridge = Some(bridge);
                return;
            }
        }
        let editor = self.editor.clone();
        let path = self.active_path.clone().unwrap();
        let history = self.language_edits.history_forwarding.clone();
        let owner = cx.entity().downgrade();
        self.language_edits.bridge = Some(cx.new(|cx| {
            super::linked_input::Bridge::new(
                editor,
                path,
                language,
                server,
                history,
                owner,
                self.providers.clone(),
                cx,
            )
        }));
        cx.notify();
    }

    /// Formatting is a native command; ordinary Save still uses the same committed document path.
    pub(crate) fn format_document_action(
        &mut self,
        _: &FormatDocument,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.reject_readonly_document_action(window, cx) {
            return;
        }
        self.request_format(false, formatting::Scope::Document, window, cx);
    }

    /// Format one native selection with the independently selected provider and existing guards.
    pub(crate) fn format_selection_action(
        &mut self,
        _: &FormatSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.reject_readonly_document_action(window, cx) {
            return;
        }
        self.request_format(false, formatting::Scope::Selection, window, cx);
    }

    /// Default Save never formats. Opt-in saves wait for one selected formatter and then the Change effects.
    pub(crate) fn save_document_with_formatting(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let language = self
            .active_path
            .as_deref()
            .map(|path| self.language_for_path(path))
            .unwrap_or_default();
        // Restricted workspaces retain ordinary native saving, while language tools remain forbidden.
        if self.session_state.workspace_trusted
            && self.providers.editing_preferences(&language).format_on_save
        {
            self.request_format(true, formatting::Scope::Document, window, cx);
        } else {
            if !self.wait_for_native_input(DocumentAction::Save, window, cx) {
                self.save_current(cx);
            }
        }
    }

    /// One asynchronous turn after the queue drains lets native Change update DocumentSession first.
    /// Capturing the entity prevents a save or edit command from following focus into a different tab.
    fn wait_for_native_input(
        &mut self,
        action: DocumentAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(bridge) = self
            .language_edits
            .bridge
            .clone()
            .filter(|bridge| bridge.read(cx).has_pending())
        else {
            return false;
        };
        let editor = self.editor.clone();
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(10))
                    .await;
                if !this
                    .update_in(cx, |app, _, _| app.editor == editor)
                    .unwrap_or(false)
                {
                    return;
                }
                if !bridge.read_with(cx, |bridge, _| bridge.document_is_open()) {
                    return;
                }
                let done = bridge.read_with(cx, |bridge, _| !bridge.has_pending());
                if done {
                    break;
                }
            }
            let _ = this.update_in(cx, |app, window, cx| {
                if app.editor != editor {
                    return;
                }
                match action {
                    DocumentAction::Save => app.save_document_with_formatting(window, cx),
                    DocumentAction::Rename => app.rename_symbol_action(&RenameSymbol, window, cx),
                }
            });
        })
        .detach();
        true
    }

    /// Dispatch only the already frozen post-drain source; neither demand nor completion resamples it.
    fn dispatch_format(
        &mut self,
        reservation: FormatReservation,
        server: Arc<navigation::LanguageServer>,
        source: plugin_runtime::plugin_protocol::language::SourceSnapshot,
        cursor: usize,
        uri: lsp_types::Uri,
        range: Option<lsp_types::Range>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Continue cancellation through the readonly transport, including an Escape after birth.
        // The demand wait owns a different token and may be dropped now without cancelling format.
        let cancellation = plugin_runtime::RequestCancellation::default();
        self.language_edits.format_pending = Some(cancellation.clone());
        let lease = server.open_document_for_language(uri, &reservation.language);
        let request = reservation.request;
        let save = reservation.save;
        let document = source.document;
        let source = source.text;
        let active = server.clone();
        let snapshot = source.clone();
        let context = match self.capture_language_context(&server, &document, &source, cx) {
            Ok(context) => context.map(|context| context.with_cancellation(cancellation.clone())),
            Err(error) => {
                self.language_edits.format_pending.take();
                self.status = error.to_string();
                cx.notify();
                return;
            }
        };
        let response_context = context.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_executor()
                .scheduler_executor()
                .spawn_dedicated(move |_| async move {
                    let operation = || server.format_for(lease, snapshot, range, 4, true);
                    match &context {
                        Some(context) => server.with_context_snapshot(context, operation),
                        None => operation(),
                    }
                })
                .await;
            let _ = this.update_in(cx, |app, window, cx| {
                if cancellation.is_cancelled()
                    || app.language_edits.request != request
                    || !app.format_reservation_is_current(&reservation, cx)
                    || response_context
                        .as_ref()
                        .is_some_and(|context| !app.language_context_is_current(context, cx))
                {
                    if app.language_edits.request == request {
                        app.language_edits.format_pending.take();
                    }
                    return;
                }
                app.language_edits.format_pending.take();
                if !app.language_formatter_is_current(
                    &app.workspace.root().join(&document.path),
                    &active,
                    cx,
                ) {
                    return;
                }
                match result.and_then(|edits| {
                    let has_edits = !edits.is_empty();
                    app.apply_language_edits(
                        &document, &source, &active, &edits, cursor, window, cx,
                    )?;
                    Ok(has_edits)
                }) {
                    Ok(has_edits) => {
                        app.status = if has_edits {
                            t!("editor.formatted").to_string()
                        } else {
                            t!("editor.format_no_changes").to_string()
                        };
                        if save {
                            let target = app.editor.clone();
                            let owner = cx.entity().downgrade();
                            // Change events run before Save; the entity also guards a same-path reopen.
                            cx.defer(move |cx| {
                                let _ = owner.update(cx, |app, cx| {
                                    if app.editor == target {
                                        app.save_current(cx);
                                    }
                                });
                            });
                        }
                    }
                    Err(error) => {
                        app.status =
                            t!("editor.editing_failed", error = format!("{error:#}")).to_string()
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Validate a full proposal before changing native history; no valid prefix of an invalid list is applied.
    fn apply_language_edits(
        &mut self,
        document: &DocumentVersion,
        source: &str,
        server: &Arc<navigation::LanguageServer>,
        edits: &[TextEdit],
        cursor: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        let index = self
            .active_text_tab_index()
            .ok_or_else(|| anyhow::anyhow!(t!("editor.editing_stale")))?;
        anyhow::ensure!(
            self.session_state.workspace_trusted
                && self.plugin_document_version(index).ok().as_ref() == Some(document)
                && server.is_active(),
            "{}",
            t!("editor.editing_stale")
        );
        anyhow::ensure!(
            self.language_server_for_path(self.tabs[index].path())
                .is_some_and(|current| Arc::ptr_eq(current, server))
                || self.language_formatter_is_current(self.tabs[index].path(), server, cx),
            "{}",
            t!("editor.editing_stale")
        );
        self.editor.update(cx, |editor, cx| {
            super::text_transactions::apply_language_proposal(
                editor, source, edits, cursor, window, cx,
            )
        })
    }
}
