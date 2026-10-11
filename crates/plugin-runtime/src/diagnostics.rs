//! Persistent diagnostic authority contains metadata and lifetime only; native documents own results.
use crate::EditorAuthority;
use plugin_protocol::api::ResourceHandle;
use std::sync::atomic::{AtomicBool, Ordering};

/// An admitted collection cannot be forged or retargeted by a native consumer or guest.
pub struct DiagnosticCollection {
    handle: ResourceHandle,
    label: String,
    owner: String,
    authority: EditorAuthority,
    released: AtomicBool,
    workspace_read: bool,
}
impl DiagnosticCollection {
    /// Only the original instance can allocate collections after grant and quota checks.
    pub(crate) fn new(
        handle: ResourceHandle,
        label: String,
        owner: String,
        authority: EditorAuthority,
        workspace_read: bool,
    ) -> Self {
        Self {
            handle,
            label,
            owner,
            authority,
            released: AtomicBool::new(false),
            workspace_read,
        }
    }
    /// Stable opaque collection identity; it grants neither text access nor writing.
    pub fn handle(&self) -> &ResourceHandle {
        &self.handle
    }
    /// Display source label as plugin data, without using it for permissions or routing.
    pub fn label(&self) -> &str {
        &self.label
    }
    /// Immutable runtime owner is used to route a native fix back to its original instance.
    pub fn owner(&self) -> &str {
        &self.owner
    }
    /// Native requests borrow the original unforgeable instance lifetime.
    pub fn authority(&self) -> &EditorAuthority {
        &self.authority
    }
    /// A related target can be opened only with the original explicit workspace read grant.
    pub fn can_read_workspace(&self) -> bool {
        self.workspace_read
    }
    /// Released, disabled or replaced collections immediately lose presentation/action authority.
    pub fn is_live(&self) -> bool {
        self.authority.is_live() && !self.released.load(Ordering::Acquire)
    }
    /// Releasing never clears another collection or restores old document text.
    pub(crate) fn revoke(&self) {
        self.released.store(true, Ordering::Release);
    }
}
