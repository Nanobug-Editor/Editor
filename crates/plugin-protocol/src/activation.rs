//! Stable `lifecycle.activation` 1.0: visible static contributions and demand-driven guests.
//!
//! Protocol 7 and base API 1 retain declarations while no business instance runs. The first
//! command, selected language feature, actually shown view or service call demands activation;
//! metadata discovery and opening a menu do not. Old SDK packages that omit this policy are
//! demand-driven too. Declarative assets and fixed LSP declarations need no empty WASM guest.
//!
//! Continuous work opts into `Policy.background`, requests `lifecycle.activation = ^1`, and
//! supplies a purpose. This grants no permissions, bypasses no workspace trust and expands no
//! CPU/fuel, memory, process, subscription, timer or network budget. Disable, scope closure and
//! replacement revoke pending demands and all instance resources. Snapshots preserve logical
//! data only; activation cannot restore process memory or undo external side effects.
//!
//! Host consumers preserve the original scope, document/revision, grants and deadline while
//! waiting. Concurrent first requests share one activation, with at most 64 waiting requests
//! per manager. Cancellation releases that waiter; the final cancellation abandons preparation.
//! Callers receive typed cancellation, timeout, stale-owner, permission or activation failures
//! rather than automatic unbounded retries. First birth does not invalidate its waiting intent.
use serde::{Deserialize, Serialize};

/// The existing declaration identifies the demand; there is no plugin-specific activation list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    /// Invoke an available static command after validating its original target and arguments.
    Command,
    /// Consume the selected provider and role for an original document or formatting request.
    Language,
    /// Show an explicitly selected view or a view with an actual nonempty painted viewport.
    View,
    /// Call a resolved service; discovering its metadata never starts the provider.
    Service,
}

/// Omitted or empty policies start only on demand, including packages built with older SDKs.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    /// Real continuous work must describe its purpose and negotiate lifecycle.activation 1.0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<Background>,
}

impl Policy {
    /// Empty policies need not be serialized, so an older manifest parser sees no new field.
    pub fn is_default(&self) -> bool {
        self.background.is_none()
    }

    /// Bound consent text; controls and whitespace-only purposes are rejected at package admission.
    pub fn valid(&self) -> bool {
        self.background.as_ref().is_none_or(|background| {
            !background.purpose.trim().is_empty()
                && background.purpose.len() <= 256
                && !background.purpose.chars().any(char::is_control)
        })
    }
}

/// A user-readable reason for running without a command, language, view or service demand.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Background {
    /// 1–256 UTF-8 bytes, nonblank and without control characters; never contains credentials.
    pub purpose: String,
}
