//! Standard LSP language features converge on the same bounded result contract as direct providers.

use super::*;
use plugin_runtime::{
    RequestCancellation,
    plugin_protocol::{
        api::{self, ErrorCode, Failure},
        direct,
        language::SourceSnapshot,
    },
};

mod actions;
mod signatures;

impl LanguageServer {
    /// Read the original service's negotiated native-feature grant without starting a process.
    /// This lets menus and routing omit unsupported methods before initialization. A `true` result
    /// still requires the initialized server's method advertisement and ordinary source checks.
    pub(crate) fn supports_language_features(&self) -> bool {
        self.is_active() && language_features_authorized(&self.service)
    }

    /// Preserve the original service authority when the user selects native navigation or a fix.
    pub(crate) fn fix_origin(&self) -> plugin_runtime::LanguageFixOrigin {
        plugin_runtime::LanguageFixOrigin::Lsp(self.service.clone())
    }

    /// Read a feature from the same synchronized connection used by completion and diagnostics.
    /// `source` and its UTF-8 caret come from the native document before dispatch. `workspace` is
    /// the native workspace, not a service's narrower project root. Cancellation, closed leases,
    /// invalid coordinates, negotiation below `language.lsp 1.4` and unadvertised methods return
    /// typed failures. The native consumer rechecks revision and selected provider again before
    /// presenting the returned plain result.
    /// Code actions support complete current-source WorkspaceEdit text edits. Commands, resource
    /// operations, cross-document edits and lazy resolution are explicitly unsupported; all accepted
    /// edits retain the original service authority and still require native preview/confirmation.
    pub(crate) fn feature_for(
        &self,
        document: DocumentLease,
        source: SourceSnapshot,
        cursor: usize,
        feature: direct::Feature,
        cancellation: &RequestCancellation,
        workspace: &Path,
    ) -> anyhow::Result<direct::Result> {
        self.check_feature_lifetime(&document, cancellation)?;
        // An LSP server's own advertisement cannot grant a host capability that was not negotiated.
        // Reject before source synchronization, connection waiting or first process initialization.
        if !language_features_authorized(&self.service) {
            return Err(Failure::new(
                ErrorCode::UnsupportedOperation,
                "Native language features require negotiated language.lsp 1.4",
            )
            .into());
        }
        let position = source_position(&document, &source, cursor, workspace)?;
        let mut connection = self.feature_connection(&document, cancellation)?;
        let current = connection
            .as_mut()
            .context("LSP provider has been retired")?;
        current.bind_document(&document)?;
        if !current.language_features.contains(&feature) {
            return Err(Failure::new(
                ErrorCode::UnsupportedOperation,
                "LSP did not advertise the requested language feature",
            )
            .into());
        }
        let wire = current.sync_document(document.uri.clone(), source.text.clone())?;
        self.check_feature_lifetime(&document, cancellation)?;
        let (method, params) = match feature {
            direct::Feature::References => (
                "textDocument/references",
                json!({"textDocument":{"uri":wire},"position":position,"context":{"includeDeclaration":true}}),
            ),
            direct::Feature::SignatureHelp => (
                "textDocument/signatureHelp",
                json!({"textDocument":{"uri":wire},"position":position,"context":{"triggerKind":1,"isRetrigger":false}}),
            ),
            direct::Feature::CodeActions => (
                "textDocument/codeAction",
                json!({
                    "textDocument":{"uri":wire},
                    "range":{"start":position,"end":position},
                    // Reuse diagnostics for this synchronized text; never maintain a second store.
                    "context":{"diagnostics":current.diagnostics.snapshot(document.uri.as_str(), &source.text).unwrap_or_default(),"triggerKind":1}
                }),
            ),
        };
        let response = current.request_cancellable(method, params, cancellation);
        self.connection_failed(method, &response, &mut connection);
        self.check_feature_lifetime(&document, cancellation)?;
        let response = response?;
        let current = connection
            .as_ref()
            .context("LSP provider has been retired")?;
        let result = match feature {
            direct::Feature::References => {
                reference_locations(current, response, &document, &source, workspace)?
            }
            direct::Feature::SignatureHelp => signatures::decode(response)?,
            direct::Feature::CodeActions => actions::decode(current, response, &document, &source)?,
        };
        if !result.valid() {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Language result exceeds native presentation limits",
            )
            .into());
        }
        Ok(result)
    }

    /// Waiting for another native request lock is cancellable. Shared first initialization retains
    /// its existing bounded lifetime, since another consumer may be using the same startup.
    fn feature_connection(
        &self,
        document: &DocumentLease,
        cancellation: &RequestCancellation,
    ) -> anyhow::Result<std::sync::MutexGuard<'_, Option<LanguageServerConnection>>> {
        loop {
            self.check_feature_lifetime(document, cancellation)?;
            match self.connection.try_lock() {
                Ok(connection) if connection.is_some() => return Ok(connection),
                Ok(connection) => {
                    drop(connection);
                    self.prepare()?;
                }
                Err(std::sync::TryLockError::WouldBlock) => {
                    std::thread::sleep(Duration::from_millis(10))
                }
                Err(std::sync::TryLockError::Poisoned(_)) => {
                    return Err(anyhow!("LSP connection lock poisoned"));
                }
            }
        }
    }

    /// These delivery guards revoke this request without spending service retry attempts.
    fn check_feature_lifetime(
        &self,
        document: &DocumentLease,
        cancellation: &RequestCancellation,
    ) -> anyhow::Result<()> {
        cancellation.check()?;
        if !self.is_active() || !document.is_active() {
            return Err(Failure::new(
                ErrorCode::Cancelled,
                "Language request source or provider was retired",
            )
            .into());
        }
        Ok(())
    }
}

/// Use one grant rule for both early native routing and the standard LSP initialization handshake.
pub(super) fn language_features_authorized(service: &plugin_runtime::LanguageService) -> bool {
    service
        .capability("language.lsp")
        .is_some_and(|version| version.major == 1 && version.minor >= 4)
}

/// Source identity and caret validation precede any synchronization or native process work.
fn source_position(
    document: &DocumentLease,
    source: &SourceSnapshot,
    cursor: usize,
    workspace: &Path,
) -> anyhow::Result<Position> {
    if !source.valid() {
        return Err(Failure::new(
            ErrorCode::InvalidRequest,
            "Invalid language source snapshot",
        )
        .into());
    }
    api::ResourceIdentity::Local {
        path: source.document.path.clone(),
    }
    .validate()?;
    if file_uri(&workspace.join(&source.document.path)).as_ref() != Some(&document.uri) {
        return Err(Failure::new(
            ErrorCode::InvalidRequest,
            "Language source does not match its native document lease",
        )
        .into());
    }
    caret_position(&source.text, cursor)
}

/// Carets cannot split UTF-8 or CRLF; all supported line breaks use strict UTF-16 columns.
fn caret_position(source: &str, cursor: usize) -> anyhow::Result<Position> {
    if cursor > source.len()
        || !source.is_char_boundary(cursor)
        || (source[..cursor].ends_with('\r') && source[cursor..].starts_with('\n'))
    {
        return Err(Failure::new(
            ErrorCode::InvalidRequest,
            "Language caret is not a text boundary",
        )
        .into());
    }
    let (mut line, mut column, mut previous_cr) = (0, 0, false);
    for character in source[..cursor].chars() {
        match character {
            '\r' => {
                line += 1;
                column = 0;
            }
            '\n' if previous_cr => {}
            '\n' => {
                line += 1;
                column = 0;
            }
            _ => column += character.len_utf16() as u32,
        }
        previous_cr = character == '\r';
    }
    Ok(Position::new(line, column))
}

/// Normalize references without reading another file or granting external URI access. Native
/// navigation rechecks canonical containment and target coordinates when the user selects a row.
fn reference_locations(
    connection: &LanguageServerConnection,
    response: Value,
    document: &DocumentLease,
    source: &SourceSnapshot,
    workspace: &Path,
) -> anyhow::Result<direct::Result> {
    let locations: Vec<Location> = if response.is_null() {
        Vec::new()
    } else {
        serde_json::from_value(response).context("Decode LSP references")?
    };
    if locations.len() > 256 {
        return Err(Failure::new(ErrorCode::LimitExceeded, "Too many language references").into());
    }
    // Normalize drive spelling on both sides. The current dirty source may not yet exist on disk.
    let root_uri = file_uri(workspace).context("Workspace has no local URI")?;
    let root = url::Url::parse(root_uri.as_str())?
        .to_file_path()
        .map_err(|_| anyhow!("Workspace has no local path"))?;
    let locations = locations
        .into_iter()
        .map(|location| {
            let uri = connection.logical_uri(&location.uri);
            let url = url::Url::parse(uri.as_str())?;
            if url.scheme() != "file" || url.query().is_some() || url.fragment().is_some() {
                return Err(Failure::new(
                    ErrorCode::InvalidPath,
                    "Reference target is not a local workspace file",
                )
                .into());
            }
            let target = url
                .to_file_path()
                .map_err(|_| anyhow!("Reference has no local path"))?;
            let relative = target.strip_prefix(&root).map_err(|_| {
                Failure::new(ErrorCode::InvalidPath, "Reference target escaped workspace")
            })?;
            let resource = api::ResourceIdentity::Local {
                path: relative.to_string_lossy().replace('\\', "/"),
            };
            resource.validate()?;
            let position = api::TextPosition {
                line: location.range.start.line,
                character: location.range.start.character,
            };
            if uri == document.uri {
                api::DocumentRange::Utf16 {
                    start: position,
                    end: api::TextPosition {
                        line: location.range.end.line,
                        character: location.range.end.character,
                    },
                }
                .resolve(&source.text)?;
            } else if !connection.service.can_read_workspace() {
                return Err(Failure::new(
                    ErrorCode::PermissionDenied,
                    "Related references require workspace.read",
                )
                .into());
            }
            Ok(direct::Location {
                resource,
                position,
                label: None,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(direct::Result::References { locations })
}

#[cfg(test)]
mod tests;
