//! Instance-owned marker collection lifetime; native EditorState remains the only position/text owner.
use crate::EditorAuthority;
use plugin_protocol::api::{DocumentVersion, ResourceHandle};
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};
/// The runtime shares authority and current document identity, never a second mutable text or Undo state.
pub struct DecorationResource {
    handle: ResourceHandle,
    authority: EditorAuthority,
    revoked: AtomicBool,
    document: Mutex<Option<DocumentVersion>>,
}
impl DecorationResource {
    pub(crate) fn new(handle: ResourceHandle, authority: EditorAuthority) -> Self {
        Self {
            handle,
            authority,
            revoked: AtomicBool::new(false),
            document: Mutex::new(None),
        }
    }
    pub fn handle(&self) -> &ResourceHandle {
        &self.handle
    }
    pub fn is_live(&self) -> bool {
        self.authority.is_live() && !self.revoked.load(Ordering::Acquire)
    }
    /// Native close and runtime release revoke this source's own collection only.
    pub fn revoke(&self) {
        self.revoked.store(true, Ordering::Release);
    }
    /// Current metadata follows native edits without retaining prior revisions or their text.
    pub fn bind(&self, document: DocumentVersion) {
        *self.document.lock().unwrap() = Some(document);
    }
    pub fn document(&self) -> Option<DocumentVersion> {
        self.document.lock().unwrap().clone()
    }
}
