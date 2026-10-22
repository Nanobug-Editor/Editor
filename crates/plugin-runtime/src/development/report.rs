//! Bounded lifecycle evidence is separate from package admission and unexercised business behavior.
use crate::{observability::Snapshot, validation::ToolIdentity};
use serde::{Deserialize, Serialize};

/// Observable check states never collapse missing evidence into successful zero measurements.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    /// The requested lifecycle check was actually completed.
    Passed,
    /// An observed check positively failed.
    Failed,
    /// Required evidence is missing, interrupted or incomplete.
    Inconclusive,
    /// This feature was not used during the session.
    NotExercised,
}

/// Process termination explains evidence availability; it is not itself a cleanup receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Termination {
    /// A normal child exit, still requiring final owner observations.
    Normal,
    /// Explicit user cancellation before the requested evaluation was complete.
    Cancelled,
    /// Parent had to terminate its owned process tree after a grace deadline.
    Forced,
    /// Unexpected native exit or loss of the child transport.
    Crashed,
}

/// A bounded candidate result; failed activation can coexist with an old usable incarnation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReloadOutcome {
    /// Host-created ordinal, not a guest command or arbitrary log line.
    pub generation: u64,
    /// Candidate committed, or the existing install path rejected it.
    pub passed: bool,
}

/// Host-generated checkpoint contains only safe scalar/metadata observations.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Independent checkpoint schema.
    pub schema_version: u32,
    /// Exact controller session; stale files from a prior controller are rejected.
    pub session: String,
    /// The actual runtime owner reached its shutdown completion path.
    pub stopped: bool,
    /// Actual shared HTTP producer count; revoking a stream need not end OS DNS immediately.
    pub http_producers: usize,
    /// Bounded actual reload outcomes; no error strings or local candidate paths.
    pub reloads: Vec<ReloadOutcome>,
    /// Records were omitted, so the session cannot claim complete coverage.
    pub partial: bool,
    /// Per-incarnation runtime evidence; no plugin text/log payloads.
    pub observations: Snapshot,
}

/// This report certifies observed lifecycle checks only, never all plugin business or UI behavior.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DevelopmentReport {
    /// Report format, independent from guest protocol.
    pub schema_version: u32,
    /// Explicit report kind prevents a lifecycle pass being displayed as general compatibility.
    pub kind: String,
    /// Host-generated controller identity.
    pub session: String,
    /// Exact initial admitted manifest ID.
    pub plugin: String,
    /// Initial manifest version.
    pub version: String,
    /// Exact digest replaces oversized version text; an empty version is never a truncated identity.
    #[serde(default)]
    pub version_sha256: Option<String>,
    /// Missing printable identity data is explicit and prevents a complete lifecycle certification.
    #[serde(default)]
    pub metadata_partial: bool,
    /// Initial Package digest; reload incarnations retain their own package metadata.
    pub package_digest: String,
    /// Exact parent tool and SDK input identity.
    pub tool: ToolIdentity,
    /// Resource-only packages have no executable activation requirement.
    pub requires_activation: bool,
    /// Initial package preparation/admission result.
    pub admission: CheckStatus,
    /// An actual isolated runtime checkpoint, not merely successful process creation.
    pub startup: CheckStatus,
    /// Evidence of a committed executable activation when required.
    pub activation: CheckStatus,
    /// Final revoked owner maps and no outstanding shared HTTP producers.
    pub cleanup: CheckStatus,
    /// Aggregate of required lifecycle checks.
    pub status: CheckStatus,
    /// Native termination reason, absent while the controller still owns the child.
    pub termination: Option<Termination>,
    /// Dynamic business behavior outside the requested lifecycle checks is not certified.
    pub business: CheckStatus,
    /// Latest accepted checkpoint, bounded by the runtime observation limits.
    pub checkpoint: Option<Checkpoint>,
}
impl DevelopmentReport {
    /// Begin after `plugin` version/digest was admitted; tool IDs identify the exact input snapshot.
    pub fn new(
        session: &str,
        plugin: &str,
        version: &str,
        digest: &str,
        executable: bool,
        tool: ToolIdentity,
    ) -> Self {
        let version = crate::validation::bounded_metadata(version);
        Self {
            schema_version: 1,
            kind: "plugin_lifecycle".into(),
            session: session.into(),
            plugin: plugin.into(),
            metadata_partial: version.sha256.is_some(),
            version: version.value.unwrap_or_default(),
            version_sha256: version.sha256,
            package_digest: digest.into(),
            tool,
            requires_activation: executable,
            admission: CheckStatus::Passed,
            startup: CheckStatus::Inconclusive,
            activation: if executable {
                CheckStatus::NotExercised
            } else {
                CheckStatus::Passed
            },
            cleanup: CheckStatus::Inconclusive,
            status: CheckStatus::Inconclusive,
            termination: None,
            business: CheckStatus::NotExercised,
            checkpoint: None,
        }
    }

    /// Accept only the current session's versioned checkpoint; never adopt a previous run's success.
    pub fn observe(&mut self, checkpoint: Checkpoint) -> bool {
        if checkpoint.schema_version != 1 || checkpoint.session != self.session {
            return false;
        }
        self.startup = CheckStatus::Passed;
        let owners = checkpoint
            .observations
            .owners
            .iter()
            .filter(|owner| owner.owner.resource_owner && owner.owner.plugin == self.plugin)
            .collect::<Vec<_>>();
        if owners.iter().any(|owner| owner.ever_committed) {
            self.activation = CheckStatus::Passed;
        }
        if owners.iter().any(|owner| {
            owner.failed_calls.get("activation").copied().unwrap_or(0) > 0
                || owner.failed_calls.get("guest").copied().unwrap_or(0) > 0
        }) {
            self.activation = CheckStatus::Failed;
        }
        if checkpoint.stopped {
            let retired = owners.iter().all(|owner| {
                owner.resources.as_ref().is_some_and(|resources| {
                    resources.retired
                        && resources.handles == 0
                        && resources.requests == 0
                        && resources.http == 0
                        && resources.timers == 0
                        && resources.subscriptions == 0
                        && resources.projections == 0
                        && resources.processes == 0
                        && resources.native_pending == 0
                        && !resources.native_cleanup_unconfirmed
                })
            });
            self.cleanup = if retired && checkpoint.http_producers == 0 {
                CheckStatus::Passed
            } else {
                CheckStatus::Inconclusive
            };
        }
        self.checkpoint = Some(checkpoint);
        true
    }

    /// Record actual parent-side build/reload failure while preserving evidence that the old child runs.
    pub fn reload_failed(&mut self) {
        self.admission = CheckStatus::Failed;
    }

    /// Finish with the real native `termination`; normal exit alone is never a lifecycle pass.
    pub fn finish(&mut self, termination: Termination) {
        self.termination = Some(termination);
        let checkpoint_failed = self
            .checkpoint
            .as_ref()
            .is_some_and(|checkpoint| checkpoint.reloads.iter().any(|reload| !reload.passed));
        self.status = if self.admission == CheckStatus::Failed
            || self.activation == CheckStatus::Failed
            || checkpoint_failed
        {
            CheckStatus::Failed
        } else if self.metadata_partial
            || termination != Termination::Normal
            || self.startup != CheckStatus::Passed
            || self.activation != CheckStatus::Passed
            || self.cleanup != CheckStatus::Passed
            || self
                .checkpoint
                .as_ref()
                .is_none_or(|checkpoint| checkpoint.partial || checkpoint.observations.partial)
        {
            CheckStatus::Inconclusive
        } else {
            CheckStatus::Passed
        };
    }

    /// Stable exits apply only to report-enabled development, leaving existing CLI behavior intact.
    pub fn exit_code(&self) -> i32 {
        if self.termination == Some(Termination::Cancelled) {
            return 3;
        }
        match self.status {
            CheckStatus::Passed => 0,
            CheckStatus::Failed => 1,
            _ => 2,
        }
    }
}
