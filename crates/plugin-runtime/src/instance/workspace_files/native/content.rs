//! Native readonly language results use the same revocable virtual-document request path as guests.
use super::*;

impl NativeDocumentRequests {
    /// Issue only admitted content. The native document dispatcher rechecks `source` before opening;
    /// the persistent tab then follows owner authority independently of that source's later edits.
    pub(crate) fn open_content(
        &mut self,
        source: api::DocumentVersion,
        location: plugin_protocol::language_content::Location,
        title: String,
        language: String,
        text: String,
        guards: Vec<Arc<AtomicBool>>,
    ) -> Result<(), Failure> {
        self.poll();
        if self.roots.retired || !self.authority.is_live() {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Language content owner retired",
            ));
        }
        if self.pending.len() >= 32 || self.virtuals.len() >= 32 || self.roots.len() >= 127 {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Readonly language resource quota exceeded",
            ));
        }
        api::ResourceIdentity::Local {
            path: source.path.clone(),
        }
        .validate()?;
        if text.len() > 1024 * 1024
            || title.is_empty()
            || title.len() > 256
            || title.chars().any(char::is_control)
        {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Invalid readonly language content or title",
            ));
        }
        if let Some(selection) = location.selection {
            selection.resolve(&text)?;
        }
        let conditions = Arc::new(PreparationConditions::new(guards));
        if !conditions.is_live() {
            return Err(Failure::new(
                ErrorCode::StaleRevision,
                "Language content source changed",
            ));
        }
        let api::Value::Resource(handle) = self.roots.open(RootKind::VirtualDocument)? else {
            unreachable!()
        };
        let resource = Arc::new(crate::VirtualDocumentResource::from_language(
            handle.clone(),
            self.authority.clone(),
            crate::virtual_documents::ContentOrigin { source, location },
        ));
        let api::Value::Resource(request_handle) = self.roots.open(RootKind::EditorRequest)? else {
            unreachable!()
        };
        let request = EditorRequest::from_authority(
            request_handle.clone(),
            api::EditorOperation::OpenVirtualDocument {
                title,
                language: Some(language),
                text,
            },
            self.workspace.clone(),
            self.data.clone(),
            30_000,
            self.authority.clone(),
        )
        .with_preparation(conditions)
        .with_virtual_document(resource.clone());
        self.virtuals.insert(handle.resource, resource);
        self.pending
            .insert(request_handle.resource, (request, false));
        Ok(())
    }
}
