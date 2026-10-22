//! A delegated language provider borrows an existing native connection through a one-use sealed token.
//! Tokens carry no paths or resource authority to guests and cannot choose arbitrary native programs.
use crate::{EditorAuthority, LanguageService};
use plugin_protocol::{
    api::{ErrorCode, Failure},
    language::DocumentContext,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

struct Binding {
    owner: String,
    borrower: String,
    scope: String,
    document: DocumentContext,
    service: Arc<LanguageService>,
    authority: EditorAuthority,
    /// Source replacement revokes only the unconsumed handoff, not the shared native service.
    selection_guard: Option<Arc<std::sync::atomic::AtomicBool>>,
    deadline: Instant,
}

/// Host-issued native transport leases shared by the runtime and its editor request dispatcher.
#[derive(Clone, Default)]
pub struct LanguageBindings(Arc<Mutex<BTreeMap<String, Binding>>>);
impl std::fmt::Debug for LanguageBindings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LanguageBindings").finish_non_exhaustive()
    }
}
impl LanguageBindings {
    /// Issue only after a declared control operation succeeds on this exact native plan.
    /// Bounded lifetime and one-use consumption prevent abandoned replies from pinning resources.
    pub(crate) fn issue(
        &self,
        metadata: &crate::LanguageControlMetadata,
        service: Arc<LanguageService>,
        authority: EditorAuthority,
    ) -> Result<String, Failure> {
        let document = metadata.binding_document.as_ref().ok_or_else(|| {
            Failure::new(
                ErrorCode::InvalidRequest,
                "Language binding was not requested",
            )
        })?;
        if !authority.is_live()
            || !service.is_active()
            || metadata.imports.iter().any(|module| !module.is_live())
        {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Language binding authority was revoked",
            ));
        }
        let mut bindings = self.0.lock().unwrap();
        bindings.retain(|_, binding| binding.live());
        if bindings.len() >= 256 {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Language binding quota exceeded",
            ));
        }
        let token = uuid::Uuid::new_v4().to_string();
        bindings.insert(
            token.clone(),
            Binding {
                owner: metadata.owner.clone(),
                borrower: metadata.borrower.clone(),
                scope: metadata.scope.clone(),
                document: document.clone(),
                service,
                authority,
                selection_guard: metadata.selection_guard.clone(),
                deadline: Instant::now() + Duration::from_secs(30),
            },
        );
        Ok(token)
    }
    /// Redeem only for the selected provider incarnation, original logical workspace and exact intent.
    /// A forged, reused, expired or mismatched token fails without exposing the borrowed plan.
    pub(crate) fn take(
        &self,
        token: &str,
        borrower: &str,
        scope: &str,
        document: &DocumentContext,
    ) -> Result<Arc<LanguageService>, Failure> {
        let mut bindings = self.0.lock().unwrap();
        bindings.retain(|_, binding| binding.live());
        let valid = bindings.get(token).is_some_and(|binding| {
            binding.borrower == borrower && binding.scope == scope && binding.document == *document
        });
        if !valid {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Language binding does not belong to this document intent",
            ));
        }
        Ok(bindings.remove(token).unwrap().service)
    }
    /// Lifecycle retirement releases pending tokens before immutable package resources are collected.
    pub(crate) fn revoke_owner(&self, owner: &str) {
        self.0.lock().unwrap().retain(|_, binding| {
            binding.owner != owner && binding.borrower != owner && binding.live()
        });
    }
    pub(crate) fn sweep_dead(&self) {
        self.0.lock().unwrap().retain(|_, binding| binding.live());
    }
    /// Pending delegation bridges the gap between native control completion and alias publication.
    /// Normal document release must retain the original cache owner during that bounded handoff.
    pub(crate) fn retains(&self, service: &Arc<LanguageService>) -> bool {
        self.0
            .lock()
            .unwrap()
            .values()
            .any(|binding| binding.live() && Arc::ptr_eq(&binding.service, service))
    }
    /// Pending native handoffs retain only their exact original document intents until redeemed or
    /// expired. The Manager combines these with published aliases before reconciling service authority.
    pub(crate) fn documents(&self, service: &Arc<LanguageService>) -> BTreeSet<DocumentContext> {
        let mut bindings = self.0.lock().unwrap();
        bindings.retain(|_, binding| binding.live());
        bindings
            .values()
            .filter(|binding| Arc::ptr_eq(&binding.service, service))
            .map(|binding| binding.document.clone())
            .collect()
    }
}
impl Binding {
    fn live(&self) -> bool {
        Instant::now() < self.deadline
            && self.authority.is_live()
            && self.service.is_active()
            && self
                .selection_guard
                .as_ref()
                .is_none_or(|guard| guard.load(std::sync::atomic::Ordering::Acquire))
    }
}
