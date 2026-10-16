//! Native text construction shares editor/session subscriptions between local and readonly resources.
use super::attach_language_server;
use crate::*;

#[cfg(test)]
mod tests;

impl EditorApp {
    /// Every text tab uses one native EditorState and the same revision/IME/Undo event subscription.
    pub(crate) fn create_native_text_tab(
        &self,
        opened: editor_core::OpenedDocument,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> TextTab {
        let language = self.language_for_path(opened.session.path());
        // Only the selected, permission-checked runtime service can attach to a document.
        let server = if opened.session.is_readonly() {
            None
        } else {
            self.language_servers.get(&language).cloned()
        };
        let document_path = opened.session.path().to_path_buf();
        let readonly = opened.session.is_readonly();
        let contents = opened.contents;
        let disk_digest = Sha256::digest(contents.as_bytes()).into();
        let editor = cx.new(|cx| {
            EditorState::new(window, cx)
                // Keep the first render free of parser work while loading the file.
                .language("text")
                .line_number(true)
                .indent_guides(true)
                .folding(true)
                .tab_size(TabSize {
                    tab_size: 4,
                    hard_tabs: false,
                })
        });
        let app = cx.entity().downgrade();
        editor.update(cx, |editor, cx| {
            editor.set_value(contents, window, cx);
            editor.set_readonly(readonly, cx);
        });
        if let Some(server) = server {
            // All tabs for a plugin language share its workspace server.
            attach_language_server(&editor, &document_path, server, app, cx);
        }
        // Definition markers use their own layer beside plugin syntax highlighting.
        // One native adapter survives grammar/fold updates and ordinary document Undo.
        let highlighter =
            crate::outline::folding::Controller::attach(&editor, self.providers.clone(), cx);
        let definition_highlight = editor.update(cx, |editor, cx| {
            editor.create_decorations_collection(Vec::new(), cx)
        });
        let subscription = cx.subscribe(&editor, |this, changed_editor, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                // Handle typing, paste, undo and IME through the
                // document event, including edits without keydown.
                if this.editor.entity_id() == changed_editor.entity_id() {
                    this.invalidate_editor_previews(cx);
                    this.dismiss_pointer_hover(cx);
                }
                // Each tab keeps its own revision, including background edits.
                if let Some(index) = this
                    .tabs
                    .iter()
                    .position(|tab| tab.owns_editor(&changed_editor))
                {
                    let Some(tab) = this.tabs[index].text.as_mut() else {
                        return;
                    };
                    tab.capability_revision = tab.capability_revision.saturating_add(1);
                    tab.capability_content =
                        super::native_text::NativeTextIdentity::new(changed_editor.read(cx).text());
                    if tab.suppress_change {
                        return;
                    }
                    tab.session.note_edit();
                    if this.editor.entity_id() == changed_editor.entity_id() {
                        this.status =
                            t!("status.modified", revision = tab.session.revision()).to_string();
                    }
                }
                this.refresh_syntax_diagnostics(changed_editor.entity_id(), cx);
                this.sync_plugin_documents(cx);
                cx.notify();
            }
        });
        // Host-owned popovers follow upstream menu and hover notifications.
        let panel = self.editor_panel.downgrade();
        let observer = cx.observe(&editor, move |this, changed_editor, cx| {
            // Existing native observers report selection/viewport changes, including keyboard and IME.
            // Base preedit changes bytes but emits only Notify. Advance the plugin capability before
            // publishing metadata; normal Change still exclusively owns dirty/Undo bookkeeping.
            // The immutable shared-root check stays O(1) for every nontext notification and for
            // oversize documents, and detects equal-length candidate replacements as well.
            if let Some(tab) = this
                .tabs
                .iter_mut()
                .find(|tab| tab.owns_editor(&changed_editor))
                .and_then(|tab| tab.text.as_mut())
            {
                let text = changed_editor.read(cx).text();
                if !tab.capability_content.matches(text) {
                    tab.capability_content = super::native_text::NativeTextIdentity::new(text);
                    tab.capability_revision = tab.capability_revision.saturating_add(1);
                }
            }
            this.sync_plugin_documents(cx);
            if this.editor.entity_id() == changed_editor.entity_id() {
                let _ = panel.update(cx, |_, cx| cx.notify());
            }
        });
        TextTab {
            capability_revision: 0,
            capability_content: super::native_text::NativeTextIdentity::new(editor.read(cx).text()),
            session: opened.session,
            editor,
            disk_digest,
            last_saved_at: Instant::now(),
            disk_state: DiskState::Synced,
            suppress_change: false,
            overwrite_confirmed: false,
            definition_highlight,
            definition_highlight_generation: 0,
            highlighter,
            diagnostics: Default::default(),
            _subscription: subscription,
            _observer: observer,
        }
    }
}
