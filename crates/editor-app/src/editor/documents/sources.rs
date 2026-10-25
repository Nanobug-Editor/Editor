//! Refactor targets join the sole native document model without replacing focus or existing dirty text.
use super::*;
use plugin_runtime::plugin_protocol::api::{ErrorCode, Failure};

impl EditorApp {
    /// A checked read transfers its initial text while native path pins remain held. Existing tabs
    /// keep their own text/revision; a new background tab gains normal language and watcher lifecycle.
    pub(crate) fn open_refactor_source(
        &mut self,
        source: &plugin_runtime::LanguageSource,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), Failure> {
        if !source.is_live() || !source.path().starts_with(self.workspace.root()) {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Refactor source authority changed",
            ));
        }
        if self.tabs.iter().any(|tab| tab.path() == source.path()) {
            return Ok(());
        }
        let path = source.path().to_owned();
        let language = self.language_for_path(&path);
        let native = self.create_native_text_tab(
            DocumentSession::from_file_contents(&path, source.text().to_owned()),
            window,
            cx,
        );
        let editor = native.editor.clone();
        self.insert_file_tab(
            OpenTab {
                preview: false,
                source_only: None,
                virtual_document: None,
                path,
                file_id: NEXT_FILE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                file_revision: 0,
                opened_at: Instant::now(),
                file_digest: None,
                text: Some(native),
                file_error: None,
            },
            cx,
        );
        self.sync_watched_documents();
        self.sync_plugin_documents(cx);
        self.sync_dynamic_language_servers(cx);
        editor.update(cx, |editor, cx| editor.set_highlighter(language, cx));
        cx.notify();
        Ok(())
    }
}
