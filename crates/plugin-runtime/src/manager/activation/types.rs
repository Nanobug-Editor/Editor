//! Host-only demand identities retain metadata intent independently of a guest's first birth.
use crate::Completion;
use plugin_protocol::{api, direct};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// Opaque static availability captured before queueing an original command, source or view.
/// It grants no guest permission and cannot be created or deserialized by a plugin.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Availability {
    pub(super) manager: u64,
    pub(super) plugin: String,
    pub(super) scope: String,
    pub(super) generation: u64,
    pub(super) configuration: u64,
}
impl Availability {
    /// Installed plugin identity; this descriptive name is not an incarnation or permission.
    pub fn plugin(&self) -> &str {
        &self.plugin
    }
    /// Canonical logical scope captured at publication, never the manager's later selected scope.
    pub fn scope(&self) -> &str {
        &self.scope
    }
    /// Current host configuration intent, for document discovery and native result seals.
    /// Guests cannot manufacture an Availability; zero in a control request asks the host to adopt it.
    pub fn configuration_generation(&self) -> u64 {
        self.configuration
    }
}

/// A selected language role materializes only its explicitly named provider.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LanguageRole {
    /// Standard language-server analysis and its shared feature transport.
    Lsp,
    /// Explicit formatting demand, independent of the selected analyzer.
    Formatter,
    /// One negotiated direct feature; unsupported optional roles retain their LSP fallback.
    Direct(direct::Feature),
    /// A pure structure callback used by an actual document.
    Structure,
}

/// Original declared entry being consumed. Discovery alone is never a demand.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DemandReason {
    /// Package-local command identifier, preserving separately validated original arguments.
    Command(String),
    /// Package-local view identifier shown by the native host.
    View(String),
    /// Contract identifier selected by the broker before queueing the original call.
    Service(String),
    /// Canonical `plugin/provider` key and the role selected for an original source.
    Language {
        provider: String,
        role: LanguageRole,
    },
    /// language.lsp 1.5: discover and prepare only the selected native role for one source context.
    /// The host generation seals configuration intent; this never supplies text/revision authority.
    LanguageDocument {
        provider: String,
        role: LanguageRole,
        document: plugin_protocol::language::DocumentContext,
    },
    /// An explicitly declared continuous purpose, still subject to trust and resource budgets.
    Background,
}
impl DemandReason {
    /// Both legacy workspace roles and document-bound native roles share the same admission lifecycle.
    pub(super) fn is_language(&self) -> bool {
        matches!(self, Self::Language { .. } | Self::LanguageDocument { .. })
    }
}

/// Final admission binds first birth once; retaining this receipt cannot resurrect a retired owner.
#[derive(Clone, Debug)]
pub struct ActivationReceipt {
    pub(super) availability: Availability,
    pub(super) owner: Option<String>,
}
impl ActivationReceipt {
    /// Exact component incarnation, or `None` for a declaration-only package without empty WASM.
    pub fn owner(&self) -> Option<&str> {
        self.owner.as_deref()
    }
    /// The original static identity, unchanged by a successful first birth.
    pub fn availability(&self) -> &Availability {
        &self.availability
    }
}

/// One waiter shares preparation with others while retaining its own deadline and cancellation.
/// Dropping it withdraws only that wait; the last withdrawn waiter abandons unpublished work.
pub struct DemandRequest {
    pub(super) completion: Completion<ActivationReceipt>,
    /// Historical evidence belongs to the exact original document reservation, not its cache key.
    /// Non-document purposes and admission/cancellation errors have no such publication authority.
    pub(super) failure_publication: Option<Arc<AtomicBool>>,
}
impl DemandRequest {
    /// Whether the original shared document reservation published a terminal preparation Err.
    /// Another waiter can set this historical bit after this wait ends or the cache is withdrawn.
    /// Callers must also require their own Completed Err before retrying; a cancelled/timed-out
    /// wait or an admission failure never authorizes re-entry into a native interaction.
    pub fn has_published_language_error(&self) -> bool {
        self.failure_publication
            .as_ref()
            .is_some_and(|publication| publication.load(Ordering::Acquire))
    }

    /// Return current typed progress/result; this also expires this waiter's original deadline.
    pub fn status(&self) -> api::RequestUpdate<ActivationReceipt> {
        self.completion.status()
    }
    /// Monotonic publication number lets native hosts deliver a completion only once.
    pub fn update(&self) -> (u64, api::RequestUpdate<ActivationReceipt>) {
        self.completion.update()
    }
    /// Withdraw this wait before effects, without cancelling another caller's shared preparation.
    pub fn cancel(&self) -> Result<api::CancellationEffect, api::Failure> {
        self.completion
            .cancel(api::CancelMode::TryTerminate, api::ErrorCode::Cancelled)
    }
}
impl Drop for DemandRequest {
    fn drop(&mut self) {
        let _ = self.cancel();
    }
}
