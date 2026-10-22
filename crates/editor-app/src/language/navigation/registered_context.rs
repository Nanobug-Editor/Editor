//! Navigation carries the actual approved plan as provenance, never a user supplied project path.
use super::*;
impl LanguageServer {
    /// Native source bindings may borrow immutable plan metadata for a single target navigation.
    pub(crate) fn navigation_context_service(
        &self,
    ) -> Option<Arc<plugin_runtime::LanguageService>> {
        self.is_active().then(|| self.service.clone())
    }
}
