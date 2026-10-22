//! Native adapter code groups share process ownership while exact document intents seal control.
use super::*;

impl LanguageService {
    /// Whether the original service and each loaded code group still retain live authority.
    pub fn is_active(&self) -> bool {
        !self.state.lock().unwrap().retired
            && self.authority.is_live()
            && Self::modules_live(&self.modules.lock().unwrap())
    }
    /// Each distinct loaded code closure needs one live consumer; callers sharing the same closure
    /// do not gain the right to stop each other's context when just one caller releases its lease.
    pub(super) fn modules_live(modules: &[Arc<crate::NativeModuleLease>]) -> bool {
        modules.iter().all(|module| {
            module.is_live()
                || modules
                    .iter()
                    .any(|other| other.is_live() && other.same_code(module))
        })
    }
    /// Only the Manager updates the set of admitted intents; request arguments cannot sign a path.
    pub(crate) fn reconcile_document_contexts(
        &self,
        documents: &BTreeSet<plugin_protocol::language::DocumentContext>,
    ) {
        *self.document_contexts.lock().unwrap() = documents.clone();
    }
    /// Native control must target the issuing guest's exact incarnation, workspace and admitted
    /// document. Generation zero adopts the Manager's current intent without creating a new one.
    pub(crate) fn admits_control(
        &self,
        authority: &crate::EditorAuthority,
        workspace: &std::path::Path,
        document: &plugin_protocol::language::DocumentContext,
    ) -> bool {
        self.is_active()
            && self
                .policy
                .inherited
                .as_ref()
                .is_some_and(|owner| owner.same_instance(authority))
            && crate::native_path::normalized_target(&self.policy.workspace)
                .ok()
                .zip(crate::native_path::normalized_target(workspace).ok())
                .is_some_and(|(owner, requested)| owner == requested)
            && self.document_contexts.lock().unwrap().iter().any(|bound| {
                bound.path == document.path
                    && (document.generation == 0 || bound.generation == document.generation)
            })
    }
    /// Revoke the process only when no independent live lease still authorizes these same bytes.
    /// Retirement is sealed under the launch lock before another attachment could revive the plan.
    pub(crate) fn module_revoked(&self, module: &crate::NativeModuleLease) {
        let mut state = self.state.lock().unwrap();
        if self
            .modules
            .lock()
            .unwrap()
            .iter()
            .any(|other| other.is_live() && other.same_code(module))
        {
            return;
        }
        state.retired = true;
        self.native_lifetime
            .store(false, std::sync::atomic::Ordering::Release);
        drop(state);
        self.retire();
    }
    /// Only runtime-issued module leases can extend a native plan. No native caller supplies paths.
    pub(crate) fn retain_module(
        &self,
        module: Arc<crate::NativeModuleLease>,
    ) -> Result<(), plugin_protocol::api::Failure> {
        let state = self.state.lock().unwrap();
        if state.retired || !self.authority.is_live() {
            return Err(plugin_protocol::api::Failure::new(
                plugin_protocol::api::ErrorCode::InvalidHandle,
                "Native service retired",
            ));
        }
        let mut modules = self.modules.lock().unwrap();
        if !Self::modules_live(&modules) || !module.is_live() {
            return Err(plugin_protocol::api::Failure::new(
                plugin_protocol::api::ErrorCode::InvalidHandle,
                "Loaded module authority was revoked",
            ));
        }
        if modules.iter().any(|old| Arc::ptr_eq(old, &module)) {
            return Ok(());
        }
        // Every old code group still has a live member, so expired caller members no longer need a
        // slot. Their peers retain the same pinned bytes through the existing process lifetime.
        modules.retain(|old| old.is_live());
        if modules.len() >= 64 {
            return Err(plugin_protocol::api::Failure::new(
                plugin_protocol::api::ErrorCode::LimitExceeded,
                "Native module quota exceeded",
            ));
        }
        modules.push(module);
        Ok(())
    }
}
