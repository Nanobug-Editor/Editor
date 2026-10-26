//! Clipboard enumeration issues a finite metadata lease; selected reads retain only that same lease.
use super::*;
use api::{ErrorCode, Failure, Value};
use plugin_protocol::transfer::ClipboardOperation;
use resource_roots::RootKind;
use std::sync::Arc;

impl State {
    /// Enumeration owns a fresh hidden slot until its exact offer is delivered; failures cannot orphan it.
    /// A selected Read borrows a previously delivered offer and must never withdraw it on local failure.
    /// `None` closes a request before guest delivery, even if native completion already succeeded.
    pub(super) fn finish_clipboard_enumeration(
        &mut self,
        call: &crate::EditorRequest,
        update: Option<&api::RequestUpdate<api::EditorValue>>,
    ) {
        if !matches!(
            call.operation(),
            api::EditorOperation::Clipboard {
                operation: ClipboardOperation::Enumerate
            }
        ) {
            return;
        }
        let Some(resource) = call.clipboard_offer() else {
            return;
        };
        let delivered = matches!(update, Some(api::RequestUpdate::Completed { result: Ok(api::EditorValue::ClipboardFormats(plugin_protocol::transfer::ClipboardValue::Offer(offer))) }) if offer.handle == *resource.handle() && resource.is_live());
        if !delivered {
            resource.revoke();
            let handle = resource.handle();
            self.clipboard_offers.remove(&handle.resource);
            self.roots.remove(handle);
        }
    }
    /// Release expired native metadata and its slot even when no subsequent clipboard call arrives.
    pub(super) fn prune_clipboard_offers(&mut self) {
        let expired = self
            .clipboard_offers
            .values()
            .filter(|offer| !offer.is_live())
            .map(|offer| offer.handle().clone())
            .collect::<Vec<_>>();
        for handle in expired {
            self.clipboard_offers.remove(&handle.resource);
            self.roots.remove(&handle);
        }
    }
    /// Admission checks precede OS calls; no service can borrow an instance's clipboard offer handle.
    pub(super) fn prepare_clipboard(
        &mut self,
        operation: &api::EditorOperation,
    ) -> Result<Option<Arc<crate::ClipboardOfferResource>>, Failure> {
        let api::EditorOperation::Clipboard { operation } = operation else {
            return Ok(None);
        };
        if self
            .api
            .capabilities
            .get("ui.clipboard")
            .is_none_or(|version| *version < semver::Version::new(1, 1, 0))
        {
            return Err(Failure::new(
                ErrorCode::CapabilityUnavailable,
                "Formatted clipboard requires ui.clipboard 1.1",
            ));
        }
        if !self.permissions.contains("clipboard") || self.plugin_services.context.is_some() {
            return Err(Failure::new(
                ErrorCode::PermissionDenied,
                "Formatted clipboard requires direct clipboard authority",
            ));
        }
        operation.validate(&self.plugin_services.principal.plugin)?;
        self.prune_clipboard_offers();
        match operation {
            ClipboardOperation::Write { .. } => Ok(None),
            ClipboardOperation::Enumerate => {
                if self.clipboard_offers.len() >= 16 {
                    return Err(Failure::new(
                        ErrorCode::LimitExceeded,
                        "Clipboard offer quota exhausted",
                    ));
                }
                let Value::Resource(handle) = self.roots.open(RootKind::ClipboardOffer)? else {
                    unreachable!()
                };
                let resource = Arc::new(crate::ClipboardOfferResource::new(
                    handle.clone(),
                    crate::EditorAuthority::new(self.plugin_services.alive.clone(), None),
                ));
                self.clipboard_offers
                    .insert(handle.resource, resource.clone());
                Ok(Some(resource))
            }
            ClipboardOperation::Read { offer, preferred } => {
                if !matches!(self.roots.resolve(offer)?, RootKind::ClipboardOffer) {
                    return Err(Failure::new(
                        ErrorCode::InvalidHandle,
                        "Not a clipboard offer",
                    ));
                }
                let resource = self
                    .clipboard_offers
                    .get(&offer.resource)
                    .filter(|offer| offer.is_live())
                    .ok_or_else(|| {
                        Failure::new(ErrorCode::InvalidHandle, "Clipboard offer expired")
                    })?;
                resource.select(preferred)?;
                Ok(Some(resource.clone()))
            }
        }
    }
}
