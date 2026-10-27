//! Native source admission reuses the existing transient offer registry without lending it to guests.
use super::*;
use std::sync::atomic::AtomicBool;

/// Source authority follows semantic identity, enabled scope and exact editable/binary ownership.
pub(super) fn declaration(
    document: &ui::Document,
    address: &Address,
) -> Result<model::DragSource, Failure> {
    if document.source != address.source || document.file != address.file {
        return Err(Failure::new(
            ErrorCode::StaleRevision,
            "Drag source authority was replaced",
        ));
    }
    let node = document
        .active_node(&address.node)
        .ok_or_else(|| Failure::new(ErrorCode::StaleRevision, "Drag source is inactive"))?;
    let source = if let Some(anchor) = &address.canvas {
        let ui::Kind::Canvas(canvas) = &node.kind else {
            return Err(Failure::new(
                ErrorCode::InvalidRequest,
                "Drag source is not a Canvas",
            ));
        };
        let scene = canvas
            .scene
            .as_ref()
            .ok_or_else(|| Failure::new(ErrorCode::StaleRevision, "Drag scene was removed"))?;
        if anchor.placement(scene).is_none_or(|group| !group.visible) {
            return Err(Failure::new(
                ErrorCode::StaleRevision,
                "Drag target retired",
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
            .and_then(|target| target.drag_source.as_ref())
    } else {
        node.drag_source.as_ref()
    };
    source
        .cloned()
        .ok_or_else(|| Failure::new(ErrorCode::InvalidState, "Node has no declared drag source"))
}
impl Instance {
    /// Only the trusted native host calls this after a real gesture; no guest operation starts OS drag.
    pub(crate) fn begin_drag(
        &mut self,
        panel: &str,
        address: Address,
        alive: Arc<AtomicBool>,
        expected: Option<model::DragSource>,
    ) -> Result<Arc<crate::TransferOfferResource>, Failure> {
        let document = self
            .views
            .get(panel)
            .ok_or_else(|| Failure::new(ErrorCode::InvalidHandle, "Drag panel unavailable"))?;
        let source = declaration(document, &address)?;
        let snapshot = expected
            .as_ref()
            .is_some_and(|expected| expected == &source);
        if expected.is_some() && !snapshot {
            return Err(Failure::new(
                ErrorCode::StaleRevision,
                "Native source representations were replaced",
            ));
        }
        let presented = if snapshot {
            address.canvas.is_none() || self.scene_frames.transfer_current(panel, &address)
        } else {
            document.revision == address.ui_revision
                && (address.canvas.is_none()
                    || document
                        .active_node(&address.node)
                        .is_some_and(|node| self.scene_frames.current(panel, node, document)))
        };
        if !presented {
            return Err(Failure::new(
                ErrorCode::StaleRevision,
                "Drag source has no current native presentation",
            ));
        }
        let state = self.store.data_mut();
        if !state.active
            || state.roots.retired
            || !state.api.capabilities.contains_key("ui.transfer")
            || !state.permissions.contains("ui.transfer")
            || state.plugin_services.context.is_some()
        {
            return Err(Failure::new(
                ErrorCode::PermissionDenied,
                "Drag requires direct ui.transfer consent",
            ));
        }
        if state.transfer_offers.len() + state.transfer_cancellations.len() >= 16 {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Transfer offer quota exhausted",
            ));
        }
        let native = crate::HostTransferOffer::declared(
            &state.plugin_services.principal.plugin,
            &source,
            alive,
        )?;
        let Value::Resource(handle) = state.roots.open(RootKind::TransferOffer)? else {
            unreachable!()
        };
        let target = DropTarget {
            preferred: source.items.iter().map(|item| item.mime.clone()).collect(),
        };
        let lineage = crate::ui_owners::lineage(document, &address.node).unwrap_or_default();
        let resource = Arc::new(
            crate::TransferOfferResource::new(
                handle.clone(),
                address,
                panel.into(),
                target,
                native,
                crate::EditorAuthority::new(state.plugin_services.alive.clone(), None),
                lineage,
            )
            .with_drag(source),
        );
        if let Err(error) = resource.advance(Phase::Drop) {
            state.roots.remove(&handle);
            return Err(error);
        }
        state
            .transfer_offers
            .insert(handle.resource, resource.clone());
        Ok(resource)
    }
    /// Seal completion before notification; successful copies retain only their bounded delayed-read lease.
    pub(crate) fn finish_drag(
        &mut self,
        panel: &str,
        handle: &api::ResourceHandle,
        outcome: model::DragOutcome,
    ) -> Result<(), Failure> {
        self.store.data().roots.resolve(handle)?;
        let state = self.store.data_mut();
        let resource = state
            .transfer_offers
            .get(&handle.resource)
            .filter(|resource| resource.panel == panel && resource.drag.is_some())
            .cloned()
            .ok_or_else(|| Failure::new(ErrorCode::InvalidHandle, "Drag source was withdrawn"))?;
        if resource
            .drag_finished
            .swap(true, std::sync::atomic::Ordering::AcqRel)
        {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Drag completion was already reported",
            ));
        }
        if outcome == model::DragOutcome::Cancelled {
            state.transfer_offers.remove(&handle.resource);
            state.roots.remove(handle);
            resource.revoke();
        }
        self.notify(
            Some(panel.into()),
            api::Notification::DragFinished(model::DragResult {
                address: resource.address.clone(),
                outcome,
            }),
        )
        .map(|_| ())
        .map_err(|error| {
            Failure::new(
                ErrorCode::OperationFailed,
                format!("Native drag completion failed: {error:#}"),
            )
        })
    }
}
