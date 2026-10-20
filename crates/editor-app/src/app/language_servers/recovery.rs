//! A lost native pipe re-enters its declared domain preparation without replaying failed requests.
use super::*;
use std::collections::BTreeSet;

impl EditorApp {
    /// Delegated providers may install process-local adapters during their public preparation.
    /// Refresh their source aliases after an observed transport failure, retaining the original
    /// native plan/adapter and its finite retry budget. Ordinary LSP providers need only reconnect.
    pub(super) fn reprepare_failed_language_aliases(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.session_state.workspace_trusted {
            return false;
        }
        let delegated = self
            .extensions
            .read(cx)
            .entries
            .iter()
            .flat_map(|entry| {
                entry
                    .manifest
                    .language_servers
                    .iter()
                    .filter(|provider| provider.delegate.is_some())
                    .map(|provider| format!("{}/{}", entry.manifest.id, provider.id))
            })
            .collect::<BTreeSet<_>>();
        let failed = self
            .language_server_demands
            .contexts
            .iter()
            .filter_map(|(path, record)| {
                let server = record.server.as_ref()?;
                (delegated.contains(&record.provider)
                    && server.is_active()
                    && server.recovery_generation() != record.recovery_generation
                    && server.recovery_state() != Some(language_navigation::RecoveryState::Paused))
                .then(|| path.clone())
            })
            .collect::<Vec<_>>();
        for path in &failed {
            let record = self.language_server_demands.contexts.remove(path).unwrap();
            self.extensions
                .read(cx)
                .release_language_document(&record.provider, &record.document);
            // Do not close the wire document or retire the backing plan: a subsequent control call
            // may already have rebuilt that same pipe, and other sources can still share its owner.
            // Normal demand below invokes the package's declared prepare/resolve/attach sequence.
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
        !failed.is_empty()
    }
}
