//! Native adapter code groups share process ownership while exact document intents seal control.
use super::*;

/// Accepted code and in-flight potential loading share one lifetime, but a rejected request owns
/// only its pending reservation. Releasing it cannot remove an earlier accepted or parallel use.
pub(super) struct AttachedModule {
    module: Arc<crate::NativeModuleLease>,
    committed: bool,
    pending: usize,
}

impl LanguageService {
    /// Whether the original service and each loaded code group still retain live authority.
    pub fn is_active(&self) -> bool {
        !self.state.lock().unwrap().retired
            && self.authority.is_live()
            && self.registered_context_is_live()
            && Self::modules_live(&self.modules.lock().unwrap())
    }
    /// Each distinct loaded code closure needs one live consumer; callers sharing the same closure
    /// do not gain the right to stop each other's context when just one caller releases its lease.
    pub(super) fn modules_live(modules: &[AttachedModule]) -> bool {
        modules.iter().all(|module| {
            module.module.is_live()
                || modules
                    .iter()
                    .any(|other| other.module.is_live() && other.module.same_code(&module.module))
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
        let modules = self.modules.lock().unwrap();
        if !modules
            .iter()
            .any(|entry| std::ptr::eq(entry.module.as_ref(), module))
            || modules
                .iter()
                .any(|other| other.module.is_live() && other.module.same_code(module))
        {
            return;
        }
        drop(modules);
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
        pending: bool,
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
        if let Some(old) = modules
            .iter_mut()
            .find(|old| Arc::ptr_eq(&old.module, &module))
        {
            if pending {
                old.pending = old.pending.checked_add(1).ok_or_else(|| {
                    plugin_protocol::api::Failure::new(
                        plugin_protocol::api::ErrorCode::LimitExceeded,
                        "Native module reservation count exceeded",
                    )
                })?;
            } else {
                old.committed = true;
            }
            return Ok(());
        }
        // A retired caller can still represent previously loaded code while a live peer only has
        // a provisional use. Keep that committed/pending evidence until this process retires;
        // pruning it here would let a later release forget the last group's cleanup obligation.
        if modules.len() >= 64 {
            return Err(plugin_protocol::api::Failure::new(
                plugin_protocol::api::ErrorCode::LimitExceeded,
                "Native module quota exceeded",
            ));
        }
        modules.push(AttachedModule {
            module,
            committed: !pending,
            pending: usize::from(pending),
        });
        Ok(())
    }

    /// Complete one dispatched reservation under the same lock as revocation. A receipt may remove
    /// only that use; another pending load or an earlier accepted use still pins the exact code.
    pub(crate) fn finish_module_reservation(
        &self,
        module: &crate::NativeModuleLease,
        commit: bool,
    ) -> bool {
        let mut state = self.state.lock().unwrap();
        let mut modules = self.modules.lock().unwrap();
        let Some(index) = modules
            .iter()
            .position(|entry| std::ptr::eq(entry.module.as_ref(), module))
        else {
            return false;
        };
        let entry = &mut modules[index];
        if entry.pending > 0 {
            entry.pending -= 1;
            entry.committed |= commit;
        }
        let retained = entry.committed || entry.pending != 0;
        if !retained {
            modules.remove(index);
        }
        // A live provisional peer may have kept a retired committed member's code authorized.
        // Once that last provisional use is released, seal under the launch lock before reaping
        // the actual process; a healthy baseline with no imported code is still reusable.
        let must_retire = !Self::modules_live(&modules);
        if must_retire {
            state.retired = true;
            self.native_lifetime
                .store(false, std::sync::atomic::Ordering::Release);
        }
        drop(modules);
        drop(state);
        if must_retire {
            self.retire();
        }
        retained
    }
}
