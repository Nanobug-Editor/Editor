//! Native source context changes withdraw one display binding without replacing document state.
use super::*;
use plugin_runtime::plugin_protocol::api::{DocumentVersion, ErrorCode, Failure};
use plugin_runtime::{EditorRequest, LanguageService};

impl EditorApp {
    /// Apply an admitted explicit selection only while the original native session/revision matches.
    pub(crate) fn refresh_registered_language_context(
        &mut self,
        provider: &str,
        document: &DocumentVersion,
        request: &EditorRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), Failure> {
        let index = (0..self.tabs.len())
            .find(|index| {
                self.plugin_document_version(*index)
                    .is_ok_and(|current| current == *document)
            })
            .ok_or_else(|| {
                Failure::new(ErrorCode::StaleRevision, "Context source changed or closed")
            })?;
        let path = self.tabs[index].path().to_owned();
        let language = self.language_for_path(&path);
        if self
            .providers
            .language_servers()
            .get(&language)
            .and_then(Option::as_deref)
            != Some(provider)
            || !self
                .extensions
                .read(cx)
                .language_provider_is_interactive(provider)
        {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Selected native provider changed",
            ));
        }
        if !request.enter_side_effect() {
            return Err(Failure::new(
                ErrorCode::Cancelled,
                "Context refresh cancelled",
            ));
        }
        request.commit_language_refresh()?;
        // The native reply may arrive after focus moved to another document. Its visible
        // requests belong to that document; the selected source is revoked by its own epoch.
        if self.active_path.as_deref() == Some(path.as_path()) {
            self.cancel_language_features(window, cx);
        }
        self.withdraw_source_language_binding(&path, cx);
        self.sync_dynamic_language_servers(cx);
        Ok(())
    }

    /// Only the next preparation of an interactive target receives the source plan's opaque hint.
    /// This never changes the user's primary provider or durable local project preference.
    pub(crate) fn inherit_source_language_context(
        &mut self,
        source: &Arc<LanguageService>,
        target: &Path,
        cx: &mut Context<Self>,
    ) -> Result<(), Failure> {
        if !self.session_state.workspace_trusted || !source.is_active() {
            return Ok(());
        }
        let Some(provider) = self
            .providers
            .language_servers()
            .get(&self.language_for_path(target))
            .and_then(Clone::clone)
        else {
            return Ok(());
        };
        if provider.split_once('/').map(|(owner, _)| owner) != Some(source.owner.as_str())
            || !self
                .extensions
                .read(cx)
                .language_provider_is_interactive(&provider)
        {
            return Ok(());
        }
        let Ok(relative) = target.strip_prefix(self.workspace.root()) else {
            return Ok(());
        };
        if source.inherit_navigation_context(&relative.to_string_lossy().replace('\\', "/"))? {
            self.withdraw_source_language_binding(target, cx);
        }
        Ok(())
    }

    /// The old source lease and wire document are withdrawn before the replacement is demanded.
    /// No process is stopped here: other native sources may still share the original context.
    fn withdraw_source_language_binding(&mut self, path: &Path, cx: &mut Context<Self>) {
        if let Some(record) = self.language_server_demands.contexts.remove(path) {
            self.extensions
                .read(cx)
                .release_language_document(&record.provider, &record.document);
            if let Some(server) = record.server {
                Self::close_server_document(server, path, None, cx);
            }
        }
        if let Some(editor) = self
            .tabs
            .iter()
            .find(|tab| tab.path() == path)
            .and_then(|tab| tab.text.as_ref())
            .map(|text| text.editor.clone())
        {
            editor::detach_language_server(&editor, cx);
        }
        self.invalidate_source_language_context(path);
    }
}
