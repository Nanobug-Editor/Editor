//! Shared authority revokes native readonly tabs without retaining their text.
use plugin_protocol::api::{DocumentVersion, ResourceHandle};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

/// An instance-owned resource binds to one native document incarnation and version.
pub struct VirtualDocumentResource {
    handle: ResourceHandle,
    alive: Arc<AtomicBool>,
    revoked: AtomicBool,
    document: Mutex<Option<DocumentVersion>>,
    /// Native language owners retain their full original authority chain without storing text.
    authority: Option<crate::EditorAuthority>,
    content: Option<ContentOrigin>,
}

/// Provenance and initial navigation coordinates for a native-issued readonly library resource.
pub struct ContentOrigin {
    /// Source incarnation and revision checked before deferred navigation opens a view.
    pub source: DocumentVersion,
    /// Provider identity and original target coordinates; neither implies a backing file.
    pub location: plugin_protocol::language_content::Location,
}
impl VirtualDocumentResource {
    /// Only admitted instances allocate this persistent authority.
    pub(crate) fn new(handle: ResourceHandle, alive: Arc<AtomicBool>) -> Self {
        Self {
            handle,
            alive,
            revoked: AtomicBool::new(false),
            document: Mutex::new(None),
            authority: None,
            content: None,
        }
    }
    /// Native issuers share the same virtual resource shape and original service incarnation.
    pub(crate) fn from_language(
        handle: ResourceHandle,
        authority: crate::EditorAuthority,
        content: ContentOrigin,
    ) -> Self {
        let mut resource = Self::new(handle, Arc::new(AtomicBool::new(true)));
        resource.authority = Some(authority);
        resource.content = Some(content);
        resource
    }
    /// Provider URI and source revision are metadata only; no filesystem backing file is implied.
    pub fn content_origin(&self) -> Option<&ContentOrigin> {
        self.content.as_ref()
    }
    /// Exact owner comparison prevents another provider from replacing an existing library view.
    pub fn same_language_owner(&self, other: &Self) -> bool {
        self.authority
            .as_ref()
            .zip(other.authority.as_ref())
            .is_some_and(|(a, b)| a.same_owner(b))
    }
    /// Opaque identity never grants filesystem access.
    pub fn handle(&self) -> &ResourceHandle {
        &self.handle
    }
    /// Instance cutover and close seal every native clone.
    pub fn is_live(&self) -> bool {
        self.alive.load(Ordering::Acquire)
            && !self.revoked.load(Ordering::Acquire)
            && self
                .authority
                .as_ref()
                .is_none_or(crate::EditorAuthority::is_live)
    }
    /// Native close uses the same revocation path as explicit release.
    pub fn revoke(&self) {
        self.revoked.store(true, Ordering::Release);
    }
    /// Publish metadata without a second text or undo state.
    pub fn bind(&self, document: DocumentVersion) {
        *self.document.lock().unwrap() = Some(document);
    }
    /// Runtime checks metadata ownership, not editable content.
    pub(crate) fn document(&self) -> Option<DocumentVersion> {
        self.document.lock().unwrap().clone()
    }
    /// This URI is not a workspace path and has no temporary backing file.
    pub fn uri(&self) -> String {
        format!(
            "nanobug-virtual://{}/{}",
            self.handle.instance, self.handle.resource
        )
    }
}
