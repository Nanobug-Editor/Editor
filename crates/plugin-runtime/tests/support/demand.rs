//! Existing package fixtures consume their declared entry through the public first-birth boundary.
use plugin_runtime::{DemandReason, Manager, plugin_protocol::api};
use std::time::{Duration, Instant};

/// Keep installation cold, then wait for the original declared command/view/role intent.
/// The shared host timeout bounds compilation as well as guest hooks; it does not alter HTTP deadlines.
pub fn ready(manager: &mut Manager, id: &str, reason: DemandReason) {
    try_ready(manager, id, reason).unwrap();
}

/// Observe an original demand's typed failure as well as successful exact-owner admission.
/// Negative role/search tests must retain the published error rather than assuming metadata starts work.
pub fn try_ready(
    manager: &mut Manager,
    id: &str,
    reason: DemandReason,
) -> Result<(), api::Failure> {
    let original = manager.availability(id)?;
    let request = manager.begin_demand(&original, reason, 120_000)?;
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        manager.poll();
        match request.status() {
            api::RequestUpdate::Completed { result } => {
                return result.and_then(|receipt| manager.admit_receipt(&original, &receipt));
            }
            api::RequestUpdate::Cancelled { reason, .. } => {
                return Err(api::Failure::new(reason, "Fixture demand ended"));
            }
            _ => {}
        }
        assert!(
            Instant::now() < deadline,
            "Fixture's original demand must terminate"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}
