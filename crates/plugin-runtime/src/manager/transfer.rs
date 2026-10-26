//! Trusted native drag ingress remains distinct from guest-injectable UI notifications.
use super::*;
use plugin_protocol::transfer::{Address, Offer, Phase, Position};

impl Manager {
    /// A native gesture may export only the declaring live instance's inert package-owned bytes.
    pub fn begin_drag(
        &mut self,
        id: &str,
        panel: &str,
        address: Address,
        alive: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<std::sync::Arc<crate::TransferOfferResource>, api::Failure> {
        if !self.trusted || !self.workspace_open {
            return Err(api::Failure::new(
                api::ErrorCode::PermissionDenied,
                "Drag requires trusted active workspace",
            ));
        }
        self.live
            .get_mut(id)
            .ok_or_else(|| {
                api::Failure::new(api::ErrorCode::InvalidState, "Drag owner is not live")
            })?
            .begin_drag(panel, address, alive, None)
    }
    /// Native layout echoes immutable source representations; newer accepted artwork cannot lend unseen data.
    /// Canvas geometry must match the latest actual frame while node/source/target ownership stays current.
    pub fn begin_drag_snapshot(
        &mut self,
        id: &str,
        panel: &str,
        address: Address,
        expected: plugin_protocol::transfer::DragSource,
        alive: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<std::sync::Arc<crate::TransferOfferResource>, api::Failure> {
        if !self.trusted || !self.workspace_open {
            return Err(api::Failure::new(
                api::ErrorCode::PermissionDenied,
                "Drag requires trusted active workspace",
            ));
        }
        self.live
            .get_mut(id)
            .ok_or_else(|| {
                api::Failure::new(api::ErrorCode::InvalidState, "Drag owner is not live")
            })?
            .begin_drag(panel, address, alive, Some(expected))
    }
    /// Report one original source's OS completion; a replacement instance never receives the old result.
    pub fn finish_drag(
        &mut self,
        id: &str,
        panel: &str,
        handle: &api::ResourceHandle,
        outcome: plugin_protocol::transfer::DragOutcome,
    ) -> Result<(), api::Failure> {
        self.live
            .get_mut(id)
            .ok_or_else(|| {
                api::Failure::new(api::ErrorCode::InvalidState, "Drag owner is not live")
            })?
            .finish_drag(panel, handle, outcome)
    }
    /// Enter one actually hit native destination, validating its exact incarnation and source first.
    /// This native API never demands or activates a cold plugin and never grants workspace access.
    pub fn begin_transfer(
        &mut self,
        id: &str,
        panel: &str,
        address: Address,
        source: crate::HostTransferOffer,
        position: Position,
    ) -> Result<Offer, api::Failure> {
        if !self.trusted || !self.workspace_open {
            return Err(api::Failure::new(
                api::ErrorCode::PermissionDenied,
                "Drop requires a trusted active workspace",
            ));
        }
        self.live
            .get_mut(id)
            .ok_or_else(|| {
                api::Failure::new(api::ErrorCode::InvalidState, "Drop owner is not live")
            })?
            .begin_transfer(panel, address, source, position, None)
    }
    /// Native receivers echo their actually painted declaration before borrowing an older UI revision.
    /// Replacement data/targets are rejected even when ordinary geometry or unrelated text has changed.
    pub fn begin_transfer_snapshot(
        &mut self,
        id: &str,
        panel: &str,
        address: Address,
        expected: plugin_protocol::transfer::DropTarget,
        source: crate::HostTransferOffer,
        position: Position,
    ) -> Result<Offer, api::Failure> {
        if !self.trusted || !self.workspace_open {
            return Err(api::Failure::new(
                api::ErrorCode::PermissionDenied,
                "Drop requires a trusted active workspace",
            ));
        }
        self.live
            .get_mut(id)
            .ok_or_else(|| {
                api::Failure::new(api::ErrorCode::InvalidState, "Drop owner is not live")
            })?
            .begin_transfer(panel, address, source, position, Some(expected))
    }
    /// Move/drop/leave/cancel the original receiver's offer, preserving opaque instance/panel ownership.
    pub fn update_transfer(
        &mut self,
        id: &str,
        panel: &str,
        offer: &api::ResourceHandle,
        phase: Phase,
        position: Position,
    ) -> Result<(), api::Failure> {
        self.live
            .get_mut(id)
            .ok_or_else(|| {
                api::Failure::new(api::ErrorCode::InvalidState, "Transfer owner is not live")
            })?
            .update_transfer(panel, offer, phase, position, None)
    }
    /// Native motion echoes its current painted basis while preserving the original receiver and offer.
    /// A newly painted scene must keep the same semantic target; replacement/hidden frames are rejected.
    pub fn update_transfer_snapshot(
        &mut self,
        id: &str,
        panel: &str,
        offer: &api::ResourceHandle,
        address: Address,
        phase: Phase,
        position: Position,
    ) -> Result<(), api::Failure> {
        self.live
            .get_mut(id)
            .ok_or_else(|| {
                api::Failure::new(api::ErrorCode::InvalidState, "Transfer owner is not live")
            })?
            .update_transfer(panel, offer, phase, position, Some(address))
    }
}
