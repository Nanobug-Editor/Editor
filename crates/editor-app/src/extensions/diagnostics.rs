//! Stable collection publication checks the actual native document before changing any markers.
use super::*;
use plugin_runtime::{
    EditorRequest,
    plugin_protocol::{api, diagnostics},
};

impl EditorApp {
    /// A collection can replace or clear only its own complete set at an exact open version.
    pub(super) fn publish_plugin_diagnostics(
        &mut self,
        request: EditorRequest,
        cx: &mut Context<Self>,
    ) {
        let api::EditorOperation::Diagnostics { publication } = request.operation() else {
            unreachable!()
        };
        let result = (|| {
            publication.validate()?;
            let collection = request
                .diagnostic_collection()
                .filter(|collection| {
                    collection.is_live() && collection.handle() == publication.collection()
                })
                .ok_or_else(|| {
                    api::Failure::new(
                        api::ErrorCode::InvalidHandle,
                        "Diagnostic collection was revoked",
                    )
                })?;
            let index = self.plugin_document_target(publication.document())?;
            let tab = self.tabs[index].text.as_ref().unwrap();
            let text = tab.editor.read(cx).text().to_string();
            let key = format!(
                "{}/{}/{}",
                collection.handle().instance,
                collection.handle().scope,
                collection.handle().resource
            );
            let items = match publication {
                diagnostics::Publication::Publish { diagnostics, .. } => diagnostics.clone(),
                diagnostics::Publication::Clear { .. } => Vec::new(),
            };
            for item in &items {
                api::DocumentRange::Bytes {
                    start: item.range.start,
                    end: item.range.end,
                }
                .resolve(&text)?;
                // Foreign related documents are resolved only on the user's navigation action.
                for related in &item.related {
                    if let api::ResourceIdentity::Local { path } = &related.resource
                        && path == &publication.document().path
                    {
                        related.position.offset(&text)?;
                    }
                }
                for fix in &item.fixes {
                    if fix
                        .plan
                        .origin
                        .as_ref()
                        .is_some_and(|origin| origin != publication.document())
                    {
                        return Err(api::Failure::new(
                            api::ErrorCode::StaleRevision,
                            "Fix origin differs from its diagnostic source",
                        ));
                    }
                }
            }
            let total: usize = tab
                .diagnostics
                .direct
                .iter()
                .filter(|(owner, _)| *owner != &key)
                .map(|(_, publication)| publication.items.len())
                .sum();
            if total + items.len() > 1024 {
                return Err(api::Failure::new(
                    api::ErrorCode::LimitExceeded,
                    "Native document diagnostic quota exceeded",
                ));
            }
            if !request.enter_side_effect() {
                return Err(api::Failure::new(
                    api::ErrorCode::Cancelled,
                    "Diagnostic publication cancelled",
                ));
            }
            let tab = self.tabs[index].text.as_mut().unwrap();
            let editor = tab.editor.entity_id();
            if items.is_empty() {
                tab.diagnostics.direct.remove(&key);
            } else {
                tab.diagnostics.direct.insert(
                    key,
                    crate::editor::diagnostics::OwnedDiagnostics {
                        collection: collection.clone(),
                        document: publication.document().clone(),
                        items,
                    },
                );
            }
            self.publish_document_diagnostics(editor, cx);
            Ok(api::EditorValue::Unit)
        })();
        request.finish(result);
    }
}
