//! Native offers expose finite metadata before consent and lazy bytes only after an actual drop.
use crate::EditorAuthority;
mod imports;
use plugin_protocol::{
    api::{ErrorCode, Failure, ResourceHandle},
    transfer::{self, Address, Format, Item, Offer, Origin, Phase},
};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

/// Native data stays outside guest JSON until the consumer explicitly selects a representation.
pub enum HostTransferData {
    /// Bounded inert encoded bytes.
    Bytes(Item),
    /// Native OS file-list data, never guest-provided paths or workspace root authority.
    Files(Vec<PathBuf>),
}

/// System boundary for delayed OLE/OS producers. Metadata enumeration never calls this reader.
pub trait TransferReader: Send + Sync {
    /// Read only this chosen MIME. The receiver independently checks the returned type and byte budget.
    fn read(&self, mime: &str) -> Result<HostTransferData, Failure>;
    /// Source-side retirement can revoke a provider before its destination's metadata expires.
    fn is_live(&self) -> bool {
        true
    }
}

/// A trusted native drag source supplies metadata and a delayed reader; WASM cannot construct it.
pub struct HostTransferOffer {
    pub(crate) origin: Origin,
    pub(crate) formats: Vec<Format>,
    reader: Arc<dyn TransferReader>,
    native_lifetime: Option<Arc<AtomicBool>>,
}
impl HostTransferOffer {
    /// The runtime alone turns the publishing package's admitted inert declaration into a native source.
    pub(crate) fn declared(
        package: &str,
        source: &transfer::DragSource,
        alive: Arc<AtomicBool>,
    ) -> Result<Self, Failure> {
        source.validate_for(package)?;
        struct Bytes(Vec<Item>);
        impl TransferReader for Bytes {
            fn read(&self, mime: &str) -> Result<HostTransferData, Failure> {
                self.0
                    .iter()
                    .find(|item| item.mime == mime)
                    .cloned()
                    .map(HostTransferData::Bytes)
                    .ok_or_else(|| {
                        Failure::new(ErrorCode::NotFound, "Drag representation unavailable")
                    })
            }
        }
        Self::new(
            Origin::Plugin {
                package: package.into(),
            },
            source
                .items
                .iter()
                .map(|item| Format {
                    mime: item.mime.clone(),
                    byte_len: Some(item.bytes.len() as u64),
                })
                .collect(),
            Arc::new(Bytes(source.items.clone())),
        )
        .map(|offer| offer.with_native_lifetime(alive))
    }
    /// Validate finite inert metadata without materializing any producer data.
    pub fn new(
        origin: Origin,
        formats: Vec<Format>,
        reader: Arc<dyn TransferReader>,
    ) -> Result<Self, Failure> {
        let unique = formats
            .iter()
            .map(|format| &format.mime)
            .collect::<std::collections::BTreeSet<_>>();
        if formats.is_empty()
            || formats.len() > transfer::MAX_FORMATS
            || unique.len() != formats.len()
            || formats.iter().any(|format| {
                !transfer::valid_mime(&format.mime)
                    || format
                        .byte_len
                        .is_some_and(|length| length > transfer::MAX_BYTES as u64)
            })
        {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Native offer format metadata quota exceeded",
            ));
        }
        Ok(Self {
            origin,
            formats,
            reader,
            native_lifetime: None,
        })
    }
    /// Bind a native destination's revocable lifetime, including hiding and modal takeover.
    pub fn with_native_lifetime(mut self, alive: Arc<AtomicBool>) -> Self {
        self.native_lifetime = Some(alive);
        self
    }
    pub(crate) fn is_live(&self) -> bool {
        self.reader.is_live()
            && self
                .native_lifetime
                .as_ref()
                .is_none_or(|alive| alive.load(Ordering::Acquire))
    }
}

/// One destination's immutable semantic binding and transient data authority.
pub struct TransferOfferResource {
    /// Nested foreground ownership is revoked independently of an unchanged child scene/target ID.
    pub(crate) lineage: crate::ui_owners::Lineage,
    handle: ResourceHandle,
    pub(crate) address: Address,
    pub(crate) panel: String,
    pub(crate) target: transfer::DropTarget,
    /// Native sources share the finite registry/lease but are never guest-readable drop offers.
    pub(crate) drag: Option<transfer::DragSource>,
    /// OS completion is terminal once; a successful copy may retain its finite delayed-read lease.
    pub(crate) drag_finished: AtomicBool,
    source: HostTransferOffer,
    authority: EditorAuthority,
    expires: Instant,
    live: AtomicBool,
    phase: Mutex<Phase>,
    reading: AtomicBool,
    /// Imported native files are opened once and released together with this exact drop lease.
    imports: Mutex<Option<Arc<Vec<imports::NativeImport>>>>,
}
impl TransferOfferResource {
    pub(crate) fn new(
        handle: ResourceHandle,
        address: Address,
        panel: String,
        target: transfer::DropTarget,
        mut source: HostTransferOffer,
        authority: EditorAuthority,
        lineage: crate::ui_owners::Lineage,
    ) -> Self {
        source
            .formats
            .retain(|format| target.preferred.contains(&format.mime));
        Self {
            lineage,
            handle,
            address,
            panel,
            target,
            drag: None,
            drag_finished: AtomicBool::new(false),
            source,
            authority,
            expires: Instant::now() + Duration::from_millis(u64::from(transfer::OFFER_LIFETIME_MS)),
            live: AtomicBool::new(true),
            phase: Mutex::new(Phase::Enter),
            reading: AtomicBool::new(false),
            imports: Mutex::new(None),
        }
    }
    /// Expose only descriptive metadata and the receiving instance's opaque capability.
    pub fn metadata(&self) -> Offer {
        Offer {
            handle: self.handle.clone(),
            formats: self.source.formats.clone(),
            source: self.source.origin.clone(),
            expires_ms: self
                .expires
                .saturating_duration_since(Instant::now())
                .as_millis()
                .min(u128::from(transfer::OFFER_LIFETIME_MS)) as u32,
        }
    }
    /// An explicit close, original source/target retirement or expiry cannot be reversed by late IO.
    pub fn is_live(&self) -> bool {
        self.live.load(Ordering::Acquire)
            && self.authority.is_live()
            && self.source.is_live()
            && Instant::now() < self.expires
    }
    pub(crate) fn handle(&self) -> &ResourceHandle {
        &self.handle
    }
    /// Mark a privately issued native source while retaining its exact admitted byte declaration.
    pub(crate) fn with_drag(mut self, source: transfer::DragSource) -> Self {
        self.drag = Some(source);
        self
    }
    pub(crate) fn revoke(&self) {
        self.live.store(false, Ordering::Release);
        self.imports.lock().unwrap().take();
    }
    /// Advance the original offer only; duplicate drops and out-of-order terminal transitions fail.
    pub(crate) fn advance(&self, next: Phase) -> Result<(), Failure> {
        // OLE retires its data object before the actor drains Leave; terminal cleanup needs no data grant.
        if !self.is_live() && !matches!(next, Phase::Leave | Phase::Cancel) {
            return Err(invalid());
        }
        let mut phase = self.phase.lock().unwrap();
        if !matches!(
            (*phase, next),
            (
                Phase::Enter | Phase::Move,
                Phase::Move | Phase::Drop | Phase::Leave | Phase::Cancel
            ) | (Phase::Drop, Phase::Cancel)
        ) {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "Invalid native transfer transition",
            ));
        }
        *phase = next;
        if matches!(next, Phase::Leave | Phase::Cancel) {
            self.revoke();
        }
        Ok(())
    }
    /// Only explicit post-drop selection can call an external producer, and at most one read runs.
    pub fn read(&self, preferred: &[String]) -> Result<HostTransferData, Failure> {
        let mime = self.select(preferred)?;
        if self.reading.swap(true, Ordering::AcqRel) {
            return Err(Failure::new(
                ErrorCode::Conflict,
                "Transfer read already in progress",
            ));
        }
        struct Reading<'a>(&'a AtomicBool);
        impl Drop for Reading<'_> {
            fn drop(&mut self) {
                self.0.store(false, Ordering::Release);
            }
        }
        let _reading = Reading(&self.reading);
        let result = self.source.reader.read(&mime)?;
        if !self.is_live() {
            return Err(invalid());
        }
        match &result {
            HostTransferData::Bytes(item) => {
                item.validate()?;
                if item.mime != mime {
                    return Err(Failure::new(
                        ErrorCode::InvalidRequest,
                        "Native provider returned an unselected format",
                    ));
                }
            }
            HostTransferData::Files(_) if mime != transfer::FILES_MIME => {
                return Err(Failure::new(
                    ErrorCode::InvalidRequest,
                    "Native provider returned an unselected format",
                ));
            }
            HostTransferData::Files(_) => {}
        }
        Ok(result)
    }
    /// Check data admission without calling the native reader or exposing producer paths.
    pub(crate) fn select(&self, preferred: &[String]) -> Result<String, Failure> {
        transfer::validate_preferences(preferred)?;
        if !self.is_live() {
            return Err(invalid());
        }
        if *self.phase.lock().unwrap() != Phase::Drop {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "Transfer data requires an actual drop",
            ));
        }
        preferred
            .iter()
            .find(|mime| {
                self.source
                    .formats
                    .iter()
                    .any(|format| format.mime == **mime)
            })
            .cloned()
            .ok_or_else(|| {
                Failure::new(
                    ErrorCode::NotFound,
                    "No selected transfer format is offered",
                )
            })
    }
}
fn invalid() -> Failure {
    Failure::new(
        ErrorCode::InvalidHandle,
        "Transfer offer expired or was withdrawn",
    )
}
