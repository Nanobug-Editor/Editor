//! Explicit, bounded ownership of independent subscriptions and timers; Drop never calls the host.
use super::*;

/// A failed release of an actually owned resource, retained for an explicit retry.
#[derive(Debug)]
pub struct CleanupFailure {
    /// Owned identity; this metadata is not a new permission or an exportable snapshot.
    pub handle: ResourceHandle,
    /// Safe category; arbitrary host error messages are deliberately excluded.
    pub code: ErrorCode,
}

/// Separate successful releases, already revoked resources and failures from business outcomes.
#[derive(Debug, Default)]
#[must_use = "inspect cleanup failures before discarding resource ownership"]
pub struct CleanupReport {
    /// Resources explicitly released by this call.
    pub closed: usize,
    /// Previously owned resources the host had already revoked.
    pub retired: usize,
    /// Resources still owned by the helper and available for another explicit close.
    pub pending: Vec<CleanupFailure>,
}
impl CleanupReport {
    /// True only when this cleanup attempt has no unresolved resources.
    pub fn is_complete(&self) -> bool {
        self.pending.is_empty()
    }

    /// Combine `other` without losing partial failures from another owned helper.
    pub(crate) fn merge(&mut self, other: Self) {
        self.closed += other.closed;
        self.retired += other.retired;
        self.pending.extend(other.pending);
    }
}

/// Close `slot` only when it is locally owned; failed releases keep the original identity.
pub(crate) fn release(slot: &mut Option<ResourceHandle>) -> CleanupReport {
    let mut report = CleanupReport::default();
    if let Some(handle) = slot.as_ref() {
        match close_resource(handle.clone()) {
            Ok(()) => {
                report.closed = 1;
                *slot = None;
            }
            Err(error) if error.code == ErrorCode::InvalidHandle => {
                report.retired = 1;
                *slot = None;
            }
            Err(error) => report.pending.push(CleanupFailure {
                handle: handle.clone(),
                code: error.code,
            }),
        }
    }
    report
}

/// A timer is retired before a one-shot callback; subscriptions have their own terminal receipt.
enum OwnedKind {
    Timer { repeat: bool },
    Documents,
}

/// A resource exists only once within its group and retains its release semantics.
struct Owned {
    handle: ResourceHandle,
    kind: OwnedKind,
}

/// Up to 256 independent timers/subscriptions, closed in reverse acquisition order.
///
/// Call `observe` for notifications, and `close_all` when the owning feature ends.
/// A hidden panel may remain active by product policy. Dropping a group performs no host
/// calls; host instance retirement remains the mandatory fault/trap cleanup guard.
#[derive(Default)]
#[must_use = "retain the group and explicitly close it when the feature ends"]
pub struct ResourceGroup {
    owned: Vec<Owned>,
}
impl ResourceGroup {
    /// Allocate a timer with `delay_ms` in 10..=86,400,000; `repeat` keeps it until closed.
    /// Returns its identity for matching receipts/reset; the group retains cleanup ownership.
    pub fn timer(&mut self, delay_ms: u32, repeat: bool) -> Result<ResourceHandle, Failure> {
        self.capacity()?;
        let handle = crate::timers::start(delay_ms, repeat)?;
        self.owned.push(Owned {
            handle: handle.clone(),
            kind: OwnedKind::Timer { repeat },
        });
        Ok(handle)
    }

    /// Subscribe to detailed document events; errors are the existing capability/quota failures.
    /// Returns the subscription identity; its terminal failure is observed, never auto-resubscribed.
    pub fn documents(&mut self) -> Result<ResourceHandle, Failure> {
        self.capacity()?;
        let handle = subscribe_document_events()?;
        self.owned.push(Owned {
            handle: handle.clone(),
            kind: OwnedKind::Documents,
        });
        Ok(handle)
    }

    /// Forget only matching host-retired resources in `notification`; no unrelated receipt has effect.
    pub fn observe(&mut self, notification: &Notification) {
        self.owned
            .retain(|owned| !match (notification, &owned.kind) {
                (Notification::Timer { handle, .. }, OwnedKind::Timer { repeat: false }) => {
                    *handle == owned.handle
                }
                (Notification::SubscriptionFailed { subscription, .. }, OwnedKind::Documents) => {
                    *subscription == owned.handle
                }
                _ => false,
            });
    }

    /// Number of currently retained identities, including failed cleanup attempts.
    pub fn len(&self) -> usize {
        self.owned.len()
    }

    /// Whether all local ownership has been completed or released.
    pub fn is_empty(&self) -> bool {
        self.owned.is_empty()
    }

    /// Release all resources in reverse order, continuing after errors and retaining only failures.
    pub fn close_all(&mut self) -> CleanupReport {
        let mut report = CleanupReport::default();
        for index in (0..self.owned.len()).rev() {
            let mut slot = Some(self.owned[index].handle.clone());
            let result = release(&mut slot);
            if slot.is_none() {
                self.owned.remove(index);
            }
            report.merge(result);
        }
        report
    }

    /// Refuse new allocations before the host is called when this group's local budget is full.
    fn capacity(&self) -> Result<(), Failure> {
        if self.owned.len() >= 256 {
            Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Resource group capacity exceeded",
            ))
        } else {
            Ok(())
        }
    }
}
