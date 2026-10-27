//! A clipboard offer retains only native sequence metadata, never eagerly fetched OS bytes.
use crate::EditorAuthority;
use plugin_protocol::{
    api::{ErrorCode, Failure, ResourceHandle},
    transfer::{self, Format, Offer},
};
use std::{
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

/// Runtime-issued, per-instance transient authority. Native adapters may fill metadata once;
/// guests cannot construct this object or borrow it through a different resource kind.
pub struct ClipboardOfferResource {
    handle: ResourceHandle,
    authority: EditorAuthority,
    expires: Instant,
    live: AtomicBool,
    snapshot: Mutex<Option<(u64, Vec<Format>)>>,
}

impl ClipboardOfferResource {
    /// Issue only after capability/permission admission and allocation of an owned root slot.
    pub(crate) fn new(handle: ResourceHandle, authority: EditorAuthority) -> Self {
        Self {
            handle,
            authority,
            expires: Instant::now() + Duration::from_millis(u64::from(transfer::OFFER_LIFETIME_MS)),
            live: AtomicBool::new(true),
            snapshot: Mutex::new(None),
        }
    }
    /// Metadata can be retained only by its original instance and within the original deadline.
    pub fn is_live(&self) -> bool {
        self.live.load(Ordering::Acquire)
            && self.authority.is_live()
            && Instant::now() < self.expires
    }
    /// Explicit close and owner retirement cannot be reversed by a late native enumeration.
    pub(crate) fn revoke(&self) {
        self.live.store(false, Ordering::Release);
    }
    /// Return the original opaque slot for manager cleanup; it is never a file-system capability.
    pub(crate) fn handle(&self) -> &ResourceHandle {
        &self.handle
    }
    /// Publish bounded format names once. Enumeration must not obtain bytes or force delayed rendering.
    pub fn publish(&self, sequence: u64, formats: Vec<Format>) -> Result<Offer, Failure> {
        if !self.is_live() {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Clipboard offer expired",
            ));
        }
        if formats.len() > transfer::MAX_FORMATS
            || formats.iter().any(|format| {
                !transfer::valid_mime(&format.mime)
                    || format
                        .byte_len
                        .is_some_and(|bytes| bytes > transfer::MAX_BYTES as u64)
            })
            || formats
                .iter()
                .map(|format| &format.mime)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != formats.len()
        {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Clipboard format metadata quota exceeded",
            ));
        }
        let mut snapshot = self.snapshot.lock().unwrap();
        if snapshot.is_some() {
            return Err(Failure::new(
                ErrorCode::Conflict,
                "Clipboard offer was already enumerated",
            ));
        }
        *snapshot = Some((sequence, formats.clone()));
        Ok(Offer {
            source: transfer::Origin::Clipboard,
            handle: self.handle.clone(),
            formats,
            expires_ms: self
                .expires
                .saturating_duration_since(Instant::now())
                .as_millis()
                .min(u128::from(transfer::OFFER_LIFETIME_MS)) as u32,
        })
    }
    /// Select only advertised metadata, preserving caller priority without materializing any format.
    pub fn select(&self, preferred: &[String]) -> Result<(u64, String), Failure> {
        transfer::validate_preferences(preferred)?;
        if !self.is_live() {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Clipboard offer expired",
            ));
        }
        let snapshot = self.snapshot.lock().unwrap();
        let (sequence, formats) = snapshot.as_ref().ok_or_else(|| {
            Failure::new(
                ErrorCode::InvalidState,
                "Clipboard enumeration is not complete",
            )
        })?;
        let mime = preferred
            .iter()
            .find(|mime| formats.iter().any(|format| format.mime == **mime))
            .ok_or_else(|| {
                Failure::new(
                    ErrorCode::NotFound,
                    "No preferred clipboard format is offered",
                )
            })?;
        Ok((*sequence, mime.clone()))
    }
}
