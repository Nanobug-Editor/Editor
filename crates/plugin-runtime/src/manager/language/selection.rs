//! Selected registered contexts narrow only the original provider/source cache entry.
use super::*;
use crate::language_contexts::RegisteredContext;
use plugin_protocol::language::DocumentContext;

impl Manager {
    /// A pending navigation outranks a cached alias. Otherwise resolve the approved native plan's
    /// provider: a delegated facade can have a different name from its registered execution context.
    pub(super) fn selected_language_context(
        &self,
        provider: &str,
        document: &DocumentContext,
        prepared: &Prepared,
    ) -> Option<RegisteredContext> {
        let (plugin, local) = provider.split_once('/')?;
        let owner = self.live.get(plugin)?.instance_id()?;
        let contexts = &self.host_resources.contexts;
        let scope = self.host_scope();
        // Only an interactive delegate consumes this handoff. A simultaneous explicit control
        // of an internal/native provider must continue to use that provider's own selected context.
        self.installed[plugin]
            .manifest
            .language_servers
            .iter()
            .any(|provider| {
                provider.id == local
                    && provider
                        .delegate
                        .as_ref()
                        .is_some_and(|delegate| delegate.interactive)
            })
            .then(|| contexts.navigation_context(owner, &scope, &document.path))
            .flatten()
            .or_else(|| {
                let native_provider = prepared
                    .service
                    .as_ref()
                    .ok()
                    .map_or(local, |service| service.provider.id.as_str());
                contexts.selected(owner, &scope, native_provider, &document.path)
            })
    }

    /// A prior native/borrowed plan cannot satisfy a new selection even when settings are unchanged.
    pub(super) fn selected_context_matches(
        &self,
        provider: &str,
        document: &DocumentContext,
        prepared: &Prepared,
    ) -> bool {
        let Some(selected) = self.selected_language_context(provider, document, prepared) else {
            return true;
        };
        prepared
            .context
            .as_ref()
            .and_then(|binding| binding.project.as_ref())
            .is_some_and(|project| {
                project.key == selected.reference.descriptor.key
                    && project.generation == selected.reference.descriptor.generation
            })
            && prepared.service.as_ref().map_or(true, |service| {
                service.provider.id == selected.reference.descriptor.provider
                    && service.root == selected.root
            })
    }
}
