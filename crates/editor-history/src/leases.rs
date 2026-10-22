//! Temporary roots keep a selected version and a restore preimage alive through asynchronous work.

use crate::{Capture, CaptureOutcome, Error, History, Reason, Result, VersionId};

/// Short-lived ownership of a retained version; dropping it makes normal retention possible again.
pub struct Lease {
    /// Profile and pin registry outlive the operation that originally acquired the version.
    history: History,
    /// This exact version, not merely its shared content object, is protected from cleanup.
    version: VersionId,
}

impl Lease {
    /// Identify the durable protected state without exposing the pin registry.
    pub fn version(&self) -> VersionId {
        self.version
    }
}

impl Drop for Lease {
    /// Release a single owner; no I/O or document mutation occurs during native teardown.
    fn drop(&mut self) {
        if let Ok(mut pins) = self.history.pins.lock() {
            if let Some(owners) = pins.get_mut(&self.version.0) {
                *owners -= 1;
                if *owners == 0 {
                    pins.remove(&self.version.0);
                }
            }
        }
    }
}

impl History {
    /// Persist capture as an independent restore preimage and pin it before any cleanup can run.
    pub fn protect(&self, mut capture: Capture) -> Result<Lease> {
        capture.reason = Reason::BeforeRestore;
        self.capture_pinned(capture)
    }

    /// Commit an eligible observation and acquire its temporary lease without a cleanup race.
    /// Destructive native operations keep these leases until their actual filesystem result is known.
    pub fn capture_pinned(&self, capture: Capture) -> Result<Lease> {
        let _activity = self.activity.lock().map_err(|_| Error::Unavailable)?;
        self.drain_legacy_gc()?;
        let version = match self.capture_inner(capture, None, None)? {
            CaptureOutcome::Recorded(version) | CaptureOutcome::Unchanged(version) => version,
            CaptureOutcome::Skipped(reason) => return Err(Error::ProtectionSkipped(reason)),
        };
        let lease = self.lease_inner(version)?;
        self.drain_legacy_gc()?;
        Ok(lease)
    }

    /// Pin an existing selection before starting preview or restoration; expired selections fail.
    pub fn pin(&self, version: VersionId) -> Result<Lease> {
        let _activity = self.activity.lock().map_err(|_| Error::Unavailable)?;
        self.lock()?
            .query_row("SELECT id FROM versions WHERE id=?1", [version.0], |row| {
                row.get::<_, i64>(0)
            })?;
        self.lease_inner(version)
    }

    /// Add an owner while activity excludes concurrent capture/cleanup decisions.
    fn lease_inner(&self, version: VersionId) -> Result<Lease> {
        *self
            .pins
            .lock()
            .map_err(|_| Error::Unavailable)?
            .entry(version.0)
            .or_default() += 1;
        Ok(Lease {
            history: self.clone(),
            version,
        })
    }
}
