//! Shared test transport for explicitly demanded public roles; installation remains cold.
use plugin_runtime::{ActivationReceipt, DemandReason, Manager, plugin_protocol::api};
use std::time::{Duration, Instant};

/// Admit a receipt for `plugin` and `reason` through `manager`, or return its public failure.
/// The bounded test wait panics after sixty seconds and never invents a host-only activation path.
pub(crate) fn demand(
    manager: &mut Manager,
    plugin: &str,
    reason: DemandReason,
) -> Result<ActivationReceipt, api::Failure> {
    let availability = manager.availability(plugin)?;
    let request = manager.begin_demand(&availability, reason, 60_000)?;
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        manager.poll();
        match request.status() {
            api::RequestUpdate::Completed { result } => {
                let receipt = result?;
                manager.admit_receipt(&availability, &receipt)?;
                return Ok(receipt);
            }
            api::RequestUpdate::Cancelled { .. } => {
                return Err(api::Failure::new(
                    api::ErrorCode::Cancelled,
                    "Fixture demand cancelled",
                ));
            }
            _ => {}
        }
        assert!(
            Instant::now() < deadline,
            "public fixture demand did not complete"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}
