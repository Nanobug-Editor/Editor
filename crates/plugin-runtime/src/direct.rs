//! Pure language leases and explicit cancellation share the original instance's authority.
use crate::{EditorAuthority, Instance};
use plugin_protocol::{
    api::{ErrorCode, Failure},
    direct::{self, Feature, Request},
    language::SourceSnapshot,
};
use std::{
    collections::BTreeSet,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

/// Cancelling stops a native wait/delivery, never an unrelated server or completed edit.
#[derive(Clone, Default)]
pub struct RequestCancellation(Arc<AtomicBool>);
impl RequestCancellation {
    /// Seal this particular request; clones share its single terminal cancellation condition.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    /// Poll from bounded worker waits and before accepting a result on the native UI thread.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
    /// Typed errors let transports distinguish ordinary user cancellation from connection failure.
    pub fn check(&self) -> anyhow::Result<()> {
        if self.is_cancelled() {
            Err(Failure::new(ErrorCode::Cancelled, "Language request cancelled").into())
        } else {
            Ok(())
        }
    }
}

/// Native fixes retain the actual result owner, not just a package name or a stronger host grant.
#[derive(Clone)]
pub enum LanguageFixOrigin {
    Diagnostics(Arc<crate::DiagnosticCollection>),
    Direct(Arc<DirectProvider>),
    Lsp(Arc<crate::LanguageService>),
    /// Native analysis adds preparation seals without replacing its owner. Configuration, source
    /// selection and connection seals prevent stale pending effects but do not own committed Undo.
    GuardedLsp {
        service: Arc<crate::LanguageService>,
        guards: Vec<Arc<AtomicBool>>,
        /// Keep original root leases across the asynchronous host queue until request admission.
        /// These proofs grant nothing beyond the service's independently rechecked permissions.
        files: Vec<crate::AuthorizedFile>,
    },
}
impl LanguageFixOrigin {
    /// A retired collection/provider cannot execute a result already rendered in native UI.
    pub fn is_live(&self) -> bool {
        match self {
            Self::Diagnostics(owner) => owner.is_live(),
            Self::Direct(owner) => owner.is_active(),
            Self::Lsp(owner) => owner.is_active() && owner.authority().is_live(),
            Self::GuardedLsp {
                service,
                guards,
                files,
            } => {
                guards.iter().all(|guard| guard.load(Ordering::Acquire))
                    && files.iter().all(crate::AuthorizedFile::is_live)
                    && service.is_active()
                    && service.authority().is_live()
            }
        }
    }
    /// Immutable package owner routes effects back to that instance's normal authority checks.
    pub fn owner(&self) -> &str {
        match self {
            Self::Diagnostics(owner) => owner.owner(),
            Self::Direct(owner) => &owner.owner,
            Self::Lsp(owner) => &owner.owner,
            Self::GuardedLsp { service, .. } => &service.owner,
        }
    }
    /// This sealed chain distinguishes disable/re-enable or replacement by the same package ID.
    pub fn authority(&self) -> &EditorAuthority {
        match self {
            Self::Diagnostics(owner) => owner.authority(),
            Self::Direct(owner) => owner.authority(),
            Self::Lsp(owner) => owner.authority(),
            Self::GuardedLsp { service, .. } => service.authority(),
        }
    }
    /// Revealing related files needs the originating installation's workspace read grant.
    pub fn can_read_workspace(&self) -> bool {
        match self {
            Self::Diagnostics(owner) => owner.can_read_workspace(),
            Self::Direct(owner) => owner.can_read_workspace(),
            Self::Lsp(owner) => owner.can_read_workspace(),
            Self::GuardedLsp { service, .. } => service.can_read_workspace(),
        }
    }
}

/// Package admission separates the experimental role from stable LSP and lifecycle permissions.
pub(crate) fn validate_manifest(manifest: &plugin_protocol::Manifest) -> anyhow::Result<()> {
    anyhow::ensure!(
        manifest.direct_language_providers.len() <= 64,
        "Too many direct language providers"
    );
    let mut ids = BTreeSet::new();
    for provider in &manifest.direct_language_providers {
        anyhow::ensure!(
            provider.valid() && ids.insert(&provider.id),
            "Invalid direct language provider declaration"
        );
        anyhow::ensure!(
            manifest.scope == plugin_protocol::api::InstanceScope::Workspace
                && manifest.component.is_some()
                && manifest.permissions.contains("editor.read")
                && manifest
                    .api
                    .as_ref()
                    .is_some_and(|api| api.required.contains_key("language.direct")
                        || api.optional.contains_key("language.direct")),
            "Direct language requires workspace WASM, language.direct and editor.read"
        );
    }
    Ok(())
}

/// An immutable pure executor produces data; native documents and original instance grants govern effects.
pub struct DirectProvider {
    /// Immutable package source label; effect authority remains sealed in the original instance.
    pub owner: String,
    /// Admitted experimental declaration, reused by the common role-selection registry.
    pub declaration: direct::Provider,
    pub(crate) instance: Mutex<Option<Instance>>,
    pub(crate) settings: plugin_protocol::settings::Effective,
    pub(crate) authority: EditorAuthority,
    pub(crate) active: AtomicBool,
    pub(crate) sequence: AtomicU64,
    pub(crate) workspace_read: bool,
}
impl DirectProvider {
    /// Request one feature on a readonly source. Cancelled/retired replies never enter native UI.
    /// Queue waits poll cancellation; guest execution retains its existing fuel/time/memory budget.
    pub fn request(
        &self,
        feature: Feature,
        source: SourceSnapshot,
        cursor: usize,
        cancellation: &RequestCancellation,
    ) -> anyhow::Result<direct::Result> {
        cancellation.check()?;
        anyhow::ensure!(
            self.is_active() && self.declaration.features.contains(&feature),
            "Direct provider or feature is unavailable"
        );
        let request = Request {
            request: self
                .sequence
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                    value.checked_add(1)
                })
                .map_err(|_| anyhow::anyhow!("Language request identity exhausted"))?,
            provider: self.declaration.id.clone(),
            feature,
            source,
            cursor,
            settings: self.settings.clone(),
        };
        anyhow::ensure!(request.valid(), "Invalid direct language source or cursor");
        let output = loop {
            cancellation.check()?;
            anyhow::ensure!(self.is_active(), "Direct language owner was retired");
            match self.instance.try_lock() {
                Ok(mut executor) => {
                    cancellation.check()?;
                    anyhow::ensure!(self.is_active(), "Direct language owner was retired");
                    let result = executor
                        .as_mut()
                        .ok_or_else(|| anyhow::anyhow!("Direct executor was retired"))?
                        .direct_snapshot(request.clone());
                    if result.is_err() {
                        self.active.store(false, Ordering::Release);
                        if let Some(mut instance) = executor.take() {
                            instance.stop();
                        }
                    }
                    break result?;
                }
                Err(std::sync::TryLockError::WouldBlock) => {
                    std::thread::sleep(std::time::Duration::from_millis(10))
                }
                Err(error) => return Err(anyhow::anyhow!("Direct executor lock failed: {error}")),
            }
        };
        cancellation.check()?;
        anyhow::ensure!(self.is_active(), "Direct language owner was retired");
        let proposal = output
            .language_direct
            .ok_or_else(|| anyhow::anyhow!("Direct hook returned no proposal"))?;
        anyhow::ensure!(
            proposal.valid_for(&request),
            "Invalid or stale direct language result"
        );
        Ok(proposal.result)
    }
    /// Native actions retain sealed authority, so a replacement with the same ID cannot inherit them.
    pub fn authority(&self) -> &EditorAuthority {
        &self.authority
    }
    /// Readonly results cannot implicitly open workspace targets without the original installation grant.
    pub fn can_read_workspace(&self) -> bool {
        self.workspace_read
    }
    pub fn is_active(&self) -> bool {
        self.authority.is_live() && self.active.load(Ordering::Acquire)
    }
    /// Revocation precedes waiting for bounded guest cleanup; retained UI leases cannot revive work.
    pub(crate) fn retire(&self) {
        self.active.store(false, Ordering::Release);
        if let Some(mut instance) = self.instance.lock().unwrap().take() {
            instance.stop();
        }
    }
}
