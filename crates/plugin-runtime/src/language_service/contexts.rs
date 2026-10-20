//! Context provenance follows the actual approved native plan, independently of display language IDs.
use super::*;
use crate::{LanguageContexts, language_contexts::RegisteredContext};

impl LanguageService {
    /// Attach an explicitly selected configuration before publication. Its temporary seal is separate
    /// from enduring owner/caller authority so completed effects retain their recovery receipts.
    pub(crate) fn set_registered_context(
        &mut self,
        registry: LanguageContexts,
        context: RegisteredContext,
    ) {
        *self.registered_context.lock().unwrap() = Some((registry, context));
    }

    /// Registered configuration seals are analysis preconditions, never new persistent owner grants.
    pub(crate) fn analysis_conditions(&self) -> Vec<Arc<std::sync::atomic::AtomicBool>> {
        self.registered_context
            .lock()
            .unwrap()
            .as_ref()
            .map(|(_, context)| vec![context.guard()])
            .unwrap_or_default()
    }

    pub(super) fn registered_context_is_live(&self) -> bool {
        self.registered_context
            .lock()
            .unwrap()
            .as_ref()
            .is_none_or(|(_, context)| context.is_live())
    }

    /// Existing ordinary plans adopt provenance only when their exact approved root has one identity.
    pub(crate) fn adopt_registered_context(
        &self,
        registry: &LanguageContexts,
        owner: &str,
        scope: &str,
    ) {
        let mut current = self.registered_context.lock().unwrap();
        if current
            .as_ref()
            .is_some_and(|(_, context)| context.is_live())
        {
            return;
        }
        *current = registry
            .for_plan(owner, scope, &self.provider.id, &self.root)
            .map(|context| (registry.clone(), context));
    }

    /// Native navigation may lend a current context to one target without persisting a preference.
    /// The target's selected language provider remains unchanged; its owner interprets the hint.
    pub fn inherit_navigation_context(
        &self,
        path: &str,
    ) -> Result<bool, plugin_protocol::api::Failure> {
        if !self.is_active() {
            return Ok(false);
        }
        let current = self.registered_context.lock().unwrap();
        let Some((registry, context)) = current.as_ref().filter(|(_, context)| context.is_live())
        else {
            return Ok(false);
        };
        registry.navigation_hint(context, path)?;
        Ok(true)
    }
}
