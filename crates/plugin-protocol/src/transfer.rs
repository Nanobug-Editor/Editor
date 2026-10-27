//! Inert multi-format interchange. MIME names never authorize execution, paths or another instance.
//! Clipboard 1.1 preserves legacy text calls; formatted items are separately bounded and lazy reads
//! choose the first available MIME in the consumer's preference order. HTML remains UTF-8 data:
//! hosts never evaluate scripts, follow URLs or load its external images.
use crate::api::ResourceHandle;
use serde::{Deserialize, Serialize};

/// Per-item and complete write payload budget, before JSON expansion.
pub const MAX_BYTES: usize = 256 * 1024;
/// One offer lists at most this many distinct supported formats.
pub const MAX_FORMATS: usize = 16;
/// Clipboard and drop authority is transient, even when the guest retains a handle.
pub const OFFER_LIFETIME_MS: u32 = 30_000;
/// Reserved native file-list representation. It cannot be constructed as an encoded byte item.
pub const FILES_MIME: &str = "application/vnd.nanobug.files";
/// One dropped batch admits at most 32 regular files and 32 MiB of total native file contents.
pub const MAX_FILES: usize = 32;
/// Total immutable file-size admission budget; reads remain individually bounded by MAX_BYTES.
pub const MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;

/// ui.transfer 1.0: an ordinary native node or Canvas target accepts only these inert formats.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DropTarget {
    /// Ordered preference, at most 16 supported MIME names. No payload is read by declaring it.
    pub preferred: Vec<String>,
}

/// ui.transfer 1.0: inert representations available only after a real native drag gesture.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DragSource {
    /// Complete supported representations within the shared 256 KiB total payload budget.
    pub items: Vec<Item>,
}
impl DragSource {
    /// Validate payload shape before native layout; package ownership is checked separately by runtime.
    pub fn validate(&self) -> Result<(), crate::api::Failure> {
        use crate::api::{ErrorCode, Failure};
        if self.items.is_empty()
            || self.items.len() > MAX_FORMATS
            || self
                .items
                .iter()
                .map(|item| item.bytes.len())
                .sum::<usize>()
                > MAX_BYTES
        {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Drag source payload quota exceeded",
            ));
        }
        let mut names = std::collections::BTreeSet::new();
        for item in &self.items {
            item.validate()?;
            if !names.insert(&item.mime) {
                return Err(Failure::new(
                    ErrorCode::InvalidRequest,
                    "Duplicate drag format",
                ));
            }
        }
        Ok(())
    }
    /// Native export cannot impersonate another package's custom format or publish file-list paths.
    pub fn validate_for(&self, package: &str) -> Result<(), crate::api::Failure> {
        ClipboardOperation::Write {
            items: self.items.clone(),
        }
        .validate(package)
    }
}

/// ui.transfer 1.0: an asynchronous selected read; the original offer must have been dropped.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    /// Read the first offered MIME in this explicit priority list, within the original transient lease.
    Read {
        offer: ResourceHandle,
        preferred: Vec<String>,
    },
    /// Read a bounded chunk of one explicitly imported regular file, with no filesystem write grant.
    ReadFile {
        /// Opaque entry issued only by a successful selected native file-list read.
        file: ImportHandle,
        /// Zero-based byte offset within the admitted file.
        offset: u64,
        /// Maximum returned bytes, in 1..=MAX_BYTES.
        max_bytes: u32,
    },
}

/// A read-only entry borrows its original drop offer's instance, target and expiry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportHandle {
    /// Parent native offer; closing or retiring it invalidates every imported entry.
    pub offer: ResourceHandle,
    /// Opaque batch slot, never a path or a general file resource.
    pub entry: u32,
}

/// Guest-visible file metadata deliberately excludes absolute paths and inherited source grants.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportedFile {
    /// Read-only capability for this exact admitted file object.
    pub handle: ImportHandle,
    /// Display-only final filename, unusable as filesystem authority.
    pub name: String,
    /// Size at admission; replacement or subsequent modification invalidates the entry.
    pub byte_len: u64,
}

/// Data provenance is descriptive; destination plugins never inherit source permissions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Origin {
    /// The separately authorized system clipboard.
    Clipboard,
    /// A user OS drag; file access remains limited to explicitly imported handles.
    OperatingSystem,
    /// Another ordinary plugin's inert representations, with no delegated grants.
    Plugin { package: String },
}

/// A destination captured by native layout; constructing this value grants no authority to guests.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Address {
    /// Destination native node in its explicitly routed panel.
    pub node: String,
    /// UI revision from the native presentation that accepted the drag.
    pub ui_revision: u64,
    /// Exact editable source authority, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<crate::api::DocumentVersion>,
    /// Exact binary source authority, independent of editable text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<crate::api::FileVersion>,
    /// Canvas destinations retain their actual displayed scene and target incarnation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canvas: Option<crate::ui::CanvasAnchor>,
}
impl Address {
    /// Address an unbound ordinary native node; source-bound hosts additionally echo source/file.
    pub fn node(node: impl Into<String>, ui_revision: u64) -> Self {
        Self {
            node: node.into(),
            ui_revision,
            source: None,
            file: None,
            canvas: None,
        }
    }
}

/// Canvas positions are content logical pixels; ordinary positions are node-local logical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}
impl Position {
    /// Reject nonfinite or unbounded native coordinates before delivery.
    pub fn validate(self) -> Result<(), crate::api::Failure> {
        if [self.x, self.y]
            .into_iter()
            .all(|value| value.is_finite() && value.abs() <= 1_000_000.)
        {
            Ok(())
        } else {
            Err(crate::api::Failure::new(
                crate::api::ErrorCode::InvalidRequest,
                "Invalid transfer position",
            ))
        }
    }
}

/// Ordered native lifecycle; Leave/Cancel revoke reads, while Drop grants the transient selected read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Enter,
    Move,
    Leave,
    Drop,
    Cancel,
}

/// OS copy completion is independent of later destination business processing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DragOutcome {
    Dropped,
    Cancelled,
}
/// One source receives exactly one terminal result for its original native drag lease.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DragResult {
    /// Original displayed source, including Canvas identity/target epoch when applicable.
    pub address: Address,
    /// Successful OS copy or explicit cancellation/refusal/retirement.
    pub outcome: DragOutcome,
}

/// Only the native Manager ingress issues these notifications; generic event injection is rejected.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    /// Original destination and source/target lifetime, not a newly hit-tested replacement.
    pub address: Address,
    /// Native transitions are strictly ordered per offer; movement alone may be coalesced.
    pub phase: Phase,
    /// Descriptive metadata and one destination-instance lease, with no source file/root authority.
    pub offer: Offer,
    /// Position mapped by the native destination frame.
    pub position: Position,
}

/// One selected inert representation, correlated through the normal editor request completion gate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Value {
    Data(Item),
    /// Read-only imports from the selected native file list.
    Files(Vec<ImportedFile>),
    /// A bounded chunk from an admitted file, with explicit end-of-file status.
    FileBytes {
        bytes: Vec<u8>,
        eof: bool,
    },
}
impl DropTarget {
    /// Validate a bounded priority list before allocating a native drop target.
    pub fn validate(&self) -> Result<(), crate::api::Failure> {
        validate_preferences(&self.preferred)
    }
}

/// Encoded inert bytes; PNG is checked by the existing image admission before native publication.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
    /// UTF-8 text/plain, inert text/html, image/png, or application/vnd.nanobug.<package>.<name>.
    /// The final dot separates the format name, which has no dots, from the exact package ID.
    pub mime: String,
    /// One bounded complete format; no host paths or executable object references.
    pub bytes: Vec<u8>,
}

/// Enumerating metadata must not force a provider or OS clipboard to materialize delayed bytes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Format {
    pub mime: String,
    /// None explicitly means the OS/provider has not supplied a length without reading data.
    pub byte_len: Option<u64>,
}

/// An instance-scoped capability, independent of source-plugin permissions or file-system roots.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Offer {
    pub handle: ResourceHandle,
    pub formats: Vec<Format>,
    /// Provenance never conveys the source plugin's grants.
    pub source: Origin,
    /// Remaining bounded lifetime at enumeration/delivery time.
    pub expires_ms: u32,
}

/// ui.clipboard 1.1 operations also require the existing installation clipboard grant.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClipboardOperation {
    /// Snapshot format names only; a clipboard change invalidates later reads of that snapshot.
    Enumerate,
    /// Load only the first available format in this ordered, bounded preference list.
    Read {
        offer: ResourceHandle,
        preferred: Vec<String>,
    },
    /// Atomically publish the supported formats. Custom writes must use the calling package namespace.
    Write { items: Vec<Item> },
}

/// Correlated editor completion values; no native object crosses the component boundary.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ClipboardValue {
    Offer(Offer),
    Data(Item),
    Written,
}

/// Recognized inert formats; custom names have a finite portable ASCII namespace.
pub fn valid_mime(mime: &str) -> bool {
    matches!(
        mime,
        "text/plain;charset=utf-8" | "text/html" | "image/png" | FILES_MIME
    ) || (mime.len() <= 192
        && mime
            .strip_prefix("application/vnd.nanobug.")
            .is_some_and(|name| {
                name.contains('.')
                    && !name.starts_with('.')
                    && !name.ends_with('.')
                    && !name.contains("..")
                    && name.bytes().all(|byte| {
                        byte.is_ascii_lowercase()
                            || byte.is_ascii_digit()
                            || matches!(byte, b'.' | b'-' | b'_')
                    })
            }))
}

impl ClipboardOperation {
    /// Validate shapes, UTF-8, total byte budget and the writer's package namespace before OS effects.
    pub fn validate(&self, package: &str) -> Result<(), crate::api::Failure> {
        use crate::api::{ErrorCode as Code, Failure};
        match self {
            Self::Enumerate => Ok(()),
            Self::Read { preferred, .. } => validate_preferences(preferred),
            Self::Write { items } => {
                if items.is_empty()
                    || items.len() > MAX_FORMATS
                    || items.iter().map(|item| item.bytes.len()).sum::<usize>() > MAX_BYTES
                {
                    return Err(Failure::new(
                        Code::LimitExceeded,
                        "Clipboard item/byte quota exceeded",
                    ));
                }
                let mut names = std::collections::BTreeSet::new();
                for item in items {
                    item.validate()?;
                    if !names.insert(&item.mime) {
                        return Err(Failure::new(
                            Code::InvalidRequest,
                            "Duplicate clipboard format",
                        ));
                    }
                    if item.mime.starts_with("application/")
                        && item
                            .mime
                            .strip_prefix("application/vnd.nanobug.")
                            .and_then(|name| name.rsplit_once('.'))
                            .map(|(owner, _)| owner)
                            != Some(package)
                    {
                        return Err(Failure::new(
                            Code::PermissionDenied,
                            "Custom format requires the calling package namespace",
                        ));
                    }
                }
                Ok(())
            }
        }
    }
}

impl Item {
    /// Validate a complete inert representation before publication or guest delivery.
    /// Native image decoding additionally validates PNG structure and dimension limits.
    pub fn validate(&self) -> Result<(), crate::api::Failure> {
        use crate::api::{ErrorCode as Code, Failure};
        if !valid_mime(&self.mime) || self.mime == FILES_MIME {
            return Err(Failure::new(
                Code::UnsupportedOperation,
                "Unsupported transfer MIME",
            ));
        }
        if self.bytes.len() > MAX_BYTES {
            return Err(Failure::new(
                Code::LimitExceeded,
                "Transfer byte quota exceeded",
            ));
        }
        if self.mime.starts_with("text/") && std::str::from_utf8(&self.bytes).is_err() {
            return Err(Failure::new(
                Code::InvalidRequest,
                "Transfer text must be UTF-8",
            ));
        }
        Ok(())
    }
}

/// Preference order is explicit and bounded; selecting a MIME never reads or executes its payload.
pub fn validate_preferences(preferred: &[String]) -> Result<(), crate::api::Failure> {
    if preferred.is_empty()
        || preferred.len() > MAX_FORMATS
        || preferred.iter().any(|mime| !valid_mime(mime))
    {
        return Err(crate::api::Failure::new(
            crate::api::ErrorCode::InvalidRequest,
            "Invalid transfer format preferences",
        ));
    }
    Ok(())
}
