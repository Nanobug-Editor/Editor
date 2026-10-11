//! Admit marker changes at one exact native version and publish only the initiating source's layer.
use super::*;
use plugin_runtime::plugin_protocol::decorations::Operation;
impl EditorApp {
    pub(super) fn perform_plugin_decorations(
        &mut self,
        operation: &Operation,
        request: &EditorRequest,
        cx: &mut Context<Self>,
    ) -> Result<Value, Failure> {
        let resource = request
            .decoration()
            .filter(|resource| resource.is_live())
            .ok_or_else(|| {
                Failure::new(
                    ErrorCode::InvalidHandle,
                    "Missing native decoration authority",
                )
            })?;
        let requested = match operation {
            Operation::Create { document, .. } | Operation::Replace { document, .. } => {
                document.clone()
            }
            Operation::Read { .. } => resource.document().ok_or_else(|| {
                Failure::new(ErrorCode::InvalidHandle, "Decoration was not attached")
            })?,
        };
        let index = (0..self.tabs.len())
            .find(|index| {
                self.plugin_document_version(*index)
                    .is_ok_and(|version| version.id == requested.id)
            })
            .ok_or_else(|| {
                resource.revoke();
                Failure::new(ErrorCode::InvalidHandle, "Decoration document closed")
            })?;
        let current = self.plugin_document_version(index)?;
        if !matches!(operation, Operation::Read { .. }) && current != requested {
            return Err(Failure::new(
                ErrorCode::StaleRevision,
                "Decoration document changed",
            ));
        }
        if resource
            .document()
            .as_ref()
            .is_some_and(|bound| bound.id != current.id)
        {
            return Err(Failure::new(
                ErrorCode::Conflict,
                "Decoration cannot target another document",
            ));
        }
        let text = self.tabs[index]
            .text
            .as_ref()
            .expect("validated native text");
        let markers = text.highlighter.markers();
        if let Operation::Create {
            markers: submitted, ..
        }
        | Operation::Replace {
            markers: submitted, ..
        } = operation
        {
            markers
                .borrow()
                .validate(resource, submitted, text.editor.read(cx).text())?;
            if !request.enter_side_effect() {
                return Err(Failure::new(
                    ErrorCode::Cancelled,
                    "Decoration publication cancelled",
                ));
            }
            let paint = if markers.borrow().contains(resource) {
                None
            } else {
                Some(text.editor.update(cx, |editor, cx| {
                    editor.create_range_decorations_collection(Vec::new(), cx)
                }))
            };
            markers.borrow_mut().replace(
                resource.clone(),
                submitted.clone(),
                request.decoration_icons().clone(),
                paint,
            );
        }
        resource.bind(current.clone());
        markers.borrow_mut().synchronize(Some(&current), cx);
        let positions = markers.borrow().positions(resource)?;
        Ok(Value::Decorations {
            handle: resource.handle().clone(),
            document: current,
            positions,
        })
    }
}
