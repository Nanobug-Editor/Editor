//! Native document publications retain the sealed owner and project into existing Base markers.
use super::*;
use plugin_runtime::{
    DiagnosticCollection,
    plugin_protocol::{api::DocumentVersion, diagnostics},
};

/// One collection's complete immutable result; the native editor remains the only text authority.
pub(crate) struct OwnedDiagnostics {
    pub(crate) collection: Arc<DiagnosticCollection>,
    pub(crate) document: DocumentVersion,
    pub(crate) items: Vec<diagnostics::Diagnostic>,
}

impl OwnedDiagnostics {
    /// Distinct collections remain distinguishable even if both guests choose the same source label.
    pub(crate) fn source(&self) -> String {
        format!(
            "{} · {} #{}",
            self.collection.owner(),
            self.collection.label(),
            self.collection.handle().resource
        )
    }

    /// Render coordinates are derived from the current native Rope only after the revision gate.
    pub(crate) fn project(&self, text: &gpui_base::input::Rope) -> Vec<Diagnostic> {
        self.items
            .iter()
            .map(|item| {
                let severity = match item.severity {
                    diagnostics::Severity::Error => gpui_base::input::DiagnosticSeverity::Error,
                    diagnostics::Severity::Warning => gpui_base::input::DiagnosticSeverity::Warning,
                    diagnostics::Severity::Information => {
                        gpui_base::input::DiagnosticSeverity::Info
                    }
                    diagnostics::Severity::Hint => gpui_base::input::DiagnosticSeverity::Hint,
                };
                let mut projected = Diagnostic::new(
                    text.offset_to_position(item.range.start)
                        ..text.offset_to_position(item.range.end),
                    item.message.clone(),
                )
                .with_severity(severity)
                .with_source(self.source());
                projected.code = item.code.clone().map(Into::into);
                projected
            })
            .collect()
    }
}

impl DocumentDiagnostics {
    /// Preserve existing fallback semantics without merging unrelated checker identities.
    pub(super) fn legacy_projection(&self) -> Vec<Diagnostic> {
        let mut merged = self.semantic.clone();
        merged.extend(
            self.syntax
                .iter()
                .filter(|syntax| {
                    !self.semantic.iter().any(|semantic| {
                        semantic.severity == gpui_base::input::DiagnosticSeverity::Error
                            && semantic.range.start < syntax.range.end
                            && syntax.range.start < semantic.range.end
                    })
                })
                .cloned(),
        );
        merged.sort_by_key(|item| (item.range.start, item.range.end));
        merged.dedup();
        merged
    }
}

impl EditorApp {
    /// Revoke native presentation after close, edits, instance replacement, release or trust change.
    pub(crate) fn prune_direct_diagnostics(&mut self, cx: &mut Context<Self>) {
        let trusted = self.session_state.workspace_trusted;
        let mut changed = Vec::new();
        for index in 0..self.tabs.len() {
            let version = self.plugin_document_version(index).ok();
            if let Some(tab) = self.tabs[index].text.as_mut() {
                let before = tab.diagnostics.direct.len();
                tab.diagnostics.direct.retain(|_, publication| {
                    trusted
                        && publication.collection.is_live()
                        && version.as_ref() == Some(&publication.document)
                });
                if before != tab.diagnostics.direct.len() {
                    changed.push(tab.editor.entity_id());
                }
            }
        }
        for editor in changed {
            self.publish_document_diagnostics(editor, cx);
        }
    }
}
