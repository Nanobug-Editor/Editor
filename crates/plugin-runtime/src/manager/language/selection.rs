//! Selected registered contexts narrow only the original provider/source cache entry.
use super::*;
use crate::language_contexts::RegisteredContext;
use plugin_protocol::language::DocumentContext;

impl Manager {
    /// Resolve the current business owner's selection; cached paths never substitute for a lease.
    pub(super) fn selected_language_context(
        &self,
        provider: &str,
        document: &DocumentContext,
    ) -> Option<RegisteredContext> {
        let (plugin, local) = provider.split_once('/')?;
        let owner = self.live.get(plugin)?.instance_id()?;
        self.host_resources
            .contexts
            .selected(owner, &self.host_scope(), local, &document.path)
    }

    /// A prior native/borrowed plan cannot satisfy a new selection even when settings are unchanged.
    pub(super) fn selected_context_matches(
        &self,
        provider: &str,
        document: &DocumentContext,
        prepared: &Prepared,
    ) -> bool {
        let Some(selected) = self.selected_language_context(provider, document) else {
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
            && prepared
                .service
                .as_ref()
                .is_ok_and(|service| service.root == selected.root)
    }
}
