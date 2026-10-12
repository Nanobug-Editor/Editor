//! Stable collection creation and versioned publication preserve direct instance ownership.
use super::*;
use api::{ErrorCode, Failure, Value};
use resource_roots::RootKind;

impl State {
    /// Collections are workspace resources, with an independent grant and no delegated authority.
    pub(super) fn create_diagnostics(
        &mut self,
        operation: plugin_protocol::diagnostics::Operation,
    ) -> Result<Value, Failure> {
        if !self.active
            || self.roots.application
            || self.roots.retired
            || self.plugin_services.context.is_some()
        {
            return Err(Failure::new(
                ErrorCode::PermissionDenied,
                "Diagnostic collections require direct workspace ownership",
            ));
        }
        if !self.api.capabilities.contains_key("editor.diagnostics") {
            return Err(Failure::new(
                ErrorCode::CapabilityUnavailable,
                "editor.diagnostics was not negotiated",
            ));
        }
        if !["editor.read", "editor.diagnostics"]
            .iter()
            .all(|grant| self.permissions.contains(*grant))
        {
            return Err(Failure::new(
                ErrorCode::PermissionDenied,
                "Diagnostic collection grants are missing",
            ));
        }
        let plugin_protocol::diagnostics::Operation::Create { label } = operation;
        if label.trim().is_empty() || label.len() > 256 || label.chars().any(char::is_control) {
            return Err(Failure::new(
                ErrorCode::InvalidRequest,
                "Invalid diagnostic source label",
            ));
        }
        if self.diagnostic_collections.len() >= 16 {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Diagnostic collection quota exceeded",
            ));
        }
        let Value::Resource(handle) = self.roots.open(RootKind::DiagnosticCollection)? else {
            unreachable!()
        };
        let collection = std::sync::Arc::new(crate::DiagnosticCollection::new(
            handle.clone(),
            label,
            self.plugin_services.principal.plugin.clone(),
            crate::EditorAuthority::new(self.plugin_services.alive.clone(), None),
            self.permissions.contains("workspace.read"),
        ));
        self.diagnostic_collections
            .insert(handle.resource, collection);
        Ok(Value::Resource(handle))
    }

    /// Every publication resolves the original opaque collection; guests cannot forge another source.
    pub(super) fn diagnostic_collection(
        &self,
        publication: &plugin_protocol::diagnostics::Publication,
    ) -> Result<std::sync::Arc<crate::DiagnosticCollection>, Failure> {
        publication.validate()?;
        if self.plugin_services.context.is_some() {
            return Err(Failure::new(
                ErrorCode::PermissionDenied,
                "Diagnostic collections cannot be delegated",
            ));
        }
        let handle = publication.collection();
        if !matches!(self.roots.resolve(handle)?, RootKind::DiagnosticCollection) {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Not a diagnostic collection",
            ));
        }
        self.diagnostic_collections
            .get(&handle.resource)
            .filter(|collection| collection.is_live())
            .cloned()
            .ok_or_else(|| {
                Failure::new(
                    ErrorCode::InvalidHandle,
                    "Diagnostic collection was released",
                )
            })
    }
}
