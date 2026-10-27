//! All generic native drop ingress and selected reads share one instance/target lifetime check.
use super::*;
use api::{ErrorCode, Failure, Value};
use plugin_protocol::transfer::{
    self as model, Address, DropTarget, Event, Origin, Phase, Position,
};
use resource_roots::RootKind;
use std::sync::Arc;
mod drag;

/// Structural liveness excludes modal/inactive branches but grants no native display authority.
fn target(document: &ui::Document, address: &Address) -> Result<DropTarget, Failure> {
    if document.source != address.source || document.file != address.file {
        return Err(Failure::new(
            ErrorCode::StaleRevision,
            "Drop destination source was replaced",
        ));
    }
    let node = document
        .active_node(&address.node)
        .ok_or_else(|| Failure::new(ErrorCode::StaleRevision, "Drop destination is inactive"))?;
    let target = if let Some(anchor) = &address.canvas {
        let ui::Kind::Canvas(canvas) = &node.kind else {
            return Err(Failure::new(
                ErrorCode::InvalidRequest,
                "Drop scene requires a Canvas",
            ));
        };
        let scene = canvas
            .scene
            .as_ref()
            .ok_or_else(|| Failure::new(ErrorCode::StaleRevision, "Drop scene was removed"))?;
        if anchor
            .placement(scene)
            .is_none_or(|placement| !placement.visible)
        {
            return Err(Failure::new(
                ErrorCode::StaleRevision,
                "Drop target lifetime was replaced",
            ));
        }
        scene
            .targets
            .iter()
            .find(|target| {
                target.id == anchor.target.id
                    && target.epoch == anchor.target.epoch
                    && target.pointer_events
            })
            .and_then(|target| target.drop_target.as_ref())
    } else {
        node.drop_target.as_ref()
    };
    target
        .cloned()
        .ok_or_else(|| Failure::new(ErrorCode::InvalidState, "Node has no enabled drop target"))
}

impl State {
    /// Offer handles stay separate from native images, file roots and delegated editor authority.
    pub(super) fn prepare_transfer(
        &self,
        operation: &api::EditorOperation,
    ) -> Result<Option<Arc<crate::TransferOfferResource>>, Failure> {
        let api::EditorOperation::Transfer { operation } = operation else {
            return Ok(None);
        };
        if self
            .api
            .capabilities
            .get("ui.transfer")
            .is_none_or(|version| *version < semver::Version::new(1, 0, 0))
            || !self.permissions.contains("ui.transfer")
            || self.plugin_services.context.is_some()
        {
            return Err(Failure::new(
                ErrorCode::PermissionDenied,
                "Transfer reads require direct ui.transfer authority",
            ));
        }
        let offer = match operation {
            model::Operation::Read { offer, .. } => offer,
            model::Operation::ReadFile { file, .. } => &file.offer,
        };
        if !matches!(self.roots.resolve(offer)?, RootKind::TransferOffer) {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Not a transfer offer",
            ));
        }
        let resource = self.transfer_offers.get(&offer.resource).ok_or_else(|| {
            Failure::new(ErrorCode::InvalidHandle, "Transfer offer was withdrawn")
        })?;
        if resource.drag.is_some() {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Native drag sources are not guest-readable offers",
            ));
        }
        match operation {
            model::Operation::Read { preferred, .. } => {
                resource.select(preferred)?;
            }
            model::Operation::ReadFile {
                file,
                offset,
                max_bytes,
            } => {
                resource.check_file(file, *offset, *max_bytes)?;
            }
        }
        Ok(Some(resource.clone()))
    }
}

impl Instance {
    /// Native-only entry: verify capability, consent, source, declaration and actual Canvas presentation.
    pub(crate) fn begin_transfer(
        &mut self,
        panel: &str,
        address: Address,
        source: crate::HostTransferOffer,
        position: Position,
        expected: Option<DropTarget>,
    ) -> Result<model::Offer, Failure> {
        position.validate()?;
        let document = self
            .views
            .get(panel)
            .ok_or_else(|| Failure::new(ErrorCode::InvalidHandle, "Drop panel is unavailable"))?;
        if address.canvas.is_none()
            && address.ui_revision != document.revision
            && expected.is_none()
        {
            return Err(Failure::new(
                ErrorCode::StaleRevision,
                "Native drop UI was replaced",
            ));
        }
        let target = target(document, &address)?;
        if expected
            .as_ref()
            .is_some_and(|expected| expected != &target)
        {
            return Err(Failure::new(
                ErrorCode::StaleRevision,
                "Native receiver declaration was replaced",
            ));
        }
        if !source.is_live() {
            return Err(Failure::new(
                ErrorCode::Cancelled,
                "Native offer was already withdrawn",
            ));
        }
        if address.canvas.is_some() && !self.scene_frames.transfer_current(panel, &address) {
            return Err(Failure::new(
                ErrorCode::StaleRevision,
                "Drop target has no current native presentation",
            ));
        }
        if !source
            .formats
            .iter()
            .any(|format| target.preferred.contains(&format.mime))
        {
            return Err(Failure::new(
                ErrorCode::NotFound,
                "Drop offers no accepted format",
            ));
        }
        let state = self.store.data_mut();
        if !state.active
            || state.roots.retired
            || !state.api.capabilities.contains_key("ui.transfer")
            || !state.permissions.contains("ui.transfer")
            || (source.origin == Origin::Clipboard && !state.permissions.contains("clipboard"))
        {
            return Err(Failure::new(
                ErrorCode::PermissionDenied,
                "Drop requires live ui.transfer consent and original source grants",
            ));
        }
        if state.transfer_offers.len() + state.transfer_cancellations.len() >= 16 {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Transfer offer quota exhausted",
            ));
        }
        let Value::Resource(handle) = state.roots.open(RootKind::TransferOffer)? else {
            unreachable!()
        };
        let resource = Arc::new(crate::TransferOfferResource::new(
            handle.clone(),
            address.clone(),
            panel.into(),
            target,
            source,
            crate::EditorAuthority::new(state.plugin_services.alive.clone(), None),
            crate::ui_owners::lineage(document, &address.node).unwrap_or_default(),
        ));
        let offer = resource.metadata();
        state
            .transfer_offers
            .insert(handle.resource, resource.clone());
        if let Err(error) = self.notify(
            Some(panel.into()),
            api::Notification::Transfer(Event {
                address,
                phase: Phase::Enter,
                offer: offer.clone(),
                position,
            }),
        ) {
            resource.revoke();
            self.store
                .data_mut()
                .transfer_offers
                .remove(&handle.resource);
            self.store.data_mut().roots.remove(&handle);
            return Err(Failure::new(
                ErrorCode::OperationFailed,
                format!("Native drop delivery failed: {error:#}"),
            ));
        }
        if !resource.is_live() {
            return Err(Failure::new(
                ErrorCode::Cancelled,
                "Destination withdrew the entered offer",
            ));
        }
        Ok(offer)
    }

    /// Route an original offer without re-hit-testing a replacement target or borrowing another panel.
    pub(crate) fn update_transfer(
        &mut self,
        panel: &str,
        handle: &api::ResourceHandle,
        phase: Phase,
        position: Position,
        snapshot: Option<Address>,
    ) -> Result<(), Failure> {
        position.validate()?;
        if !matches!(
            self.store.data().roots.resolve(handle)?,
            RootKind::TransferOffer
        ) {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Not a transfer offer",
            ));
        }
        let resource = self
            .store
            .data()
            .transfer_offers
            .get(&handle.resource)
            .filter(|resource| resource.panel == panel)
            .cloned()
            .ok_or_else(|| {
                Failure::new(
                    ErrorCode::InvalidHandle,
                    "Transfer belongs to another panel",
                )
            })?;
        let live = self
            .views
            .get(panel)
            .and_then(|document| target(document, &resource.address).ok())
            .is_some_and(|target| target == resource.target);
        if !live && !matches!(phase, Phase::Cancel | Phase::Leave) {
            resource.revoke();
            return Err(Failure::new(
                ErrorCode::StaleRevision,
                "Drop destination was withdrawn",
            ));
        }
        let address = snapshot.unwrap_or_else(|| resource.address.clone());
        if !matches!(phase, Phase::Cancel | Phase::Leave) {
            let original = &resource.address;
            let same = address.node == original.node
                && address.source == original.source
                && address.file == original.file
                && address
                    .canvas
                    .as_ref()
                    .map(|anchor| (anchor.identity, &anchor.target))
                    == original
                        .canvas
                        .as_ref()
                        .map(|anchor| (anchor.identity, &anchor.target));
            let presented =
                address.canvas.is_none() || self.scene_frames.transfer_current(panel, &address);
            let declared = self
                .views
                .get(panel)
                .and_then(|document| target(document, &address).ok())
                .is_some_and(|target| target == resource.target);
            if !same || !presented || !declared {
                return Err(Failure::new(
                    ErrorCode::StaleRevision,
                    "Native drop frame or original receiver was replaced",
                ));
            }
        }
        resource.advance(phase)?;
        let event = Event {
            address,
            phase,
            offer: resource.metadata(),
            position,
        };
        if matches!(phase, Phase::Cancel | Phase::Leave) {
            self.store
                .data_mut()
                .transfer_offers
                .remove(&handle.resource);
            self.store.data_mut().roots.remove(handle);
        }
        self.notify(Some(panel.into()), api::Notification::Transfer(event))
            .map(|_| ())
            .map_err(|error| {
                Failure::new(
                    ErrorCode::OperationFailed,
                    format!("Native transfer delivery failed: {error:#}"),
                )
            })
    }

    /// Publications revoke old sources, targets and modal ownership before a queued reader can execute.
    pub(super) fn reconcile_transfer_offers(&mut self) {
        let stale = self
            .store
            .data()
            .transfer_offers
            .values()
            .filter(|resource| {
                !resource.is_live()
                    || self.views.get(&resource.panel).is_none_or(|document| {
                        if crate::ui_owners::lineage(document, &resource.address.node).as_ref()
                            != Some(&resource.lineage)
                        {
                            return true;
                        }
                        if let Some(source) = &resource.drag {
                            drag::declaration(document, &resource.address)
                                .map_or(true, |current| &current != source)
                        } else {
                            target(document, &resource.address)
                                .map_or(true, |target| target != resource.target)
                        }
                    })
            })
            .cloned()
            .collect::<Vec<_>>();
        let state = self.store.data_mut();
        for resource in stale {
            resource.revoke();
            if resource.drag.is_some()
                && resource
                    .drag_finished
                    .load(std::sync::atomic::Ordering::Acquire)
            {
                let handle = resource.handle();
                state.transfer_offers.remove(&handle.resource);
                state.roots.remove(handle);
                continue;
            }
            // Begin reserves this terminal slot together with the live offer; cancellation is never dropped.
            state.transfer_cancellations.push((
                resource.panel.clone(),
                if resource.drag.is_some() {
                    api::Notification::DragFinished(model::DragResult {
                        address: resource.address.clone(),
                        outcome: model::DragOutcome::Cancelled,
                    })
                } else {
                    api::Notification::Transfer(Event {
                        address: resource.address.clone(),
                        phase: Phase::Cancel,
                        offer: resource.metadata(),
                        position: Position { x: 0., y: 0. },
                    })
                },
            ));
            let handle = resource.handle();
            state.transfer_offers.remove(&handle.resource);
            state.roots.remove(handle);
        }
    }
}
