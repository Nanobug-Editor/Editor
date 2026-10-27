//! Validate typed editor authority before issuing scoped request handles.
use super::*;
use crate::editor_requests::{EditorRequest, PendingRequest};
use api::{EditorOperation, ErrorCode, Failure, Notification, Value};
use resource_roots::RootKind;
use std::collections::BTreeMap;

#[cfg(test)]
mod comparison_tests;

/// Return the target's required grant for direct and delegated navigation alike.
/// Callers also require editor.read; sharing this mapping prevents stronger provider grants leaking.
pub(super) fn navigation_permission(target: &api::NavigationTarget) -> &'static str {
    match target {
        api::NavigationTarget::PreviewNode { .. } => "editor.read",
        api::NavigationTarget::RelativeDocument { .. } => "workspace.read",
        api::NavigationTarget::ExternalUrl { .. } => "navigation.external",
    }
}

impl State {
    /// Editor access never follows an application's currently selected workspace implicitly.
    pub(super) fn editor_request(
        &mut self,
        operation: EditorOperation,
        timeout_ms: u32,
    ) -> Result<Value, Failure> {
        self.editor_request_with_context(operation, timeout_ms, None)
    }

    /// A selected context follows normal schema/permission admission before it changes source routing.
    pub(super) fn editor_request_with_context(
        &mut self,
        operation: EditorOperation,
        timeout_ms: u32,
        selected: Option<crate::language_contexts::RegisteredContext>,
    ) -> Result<Value, Failure> {
        if !self.active || self.roots.retired || self.roots.application {
            return Err(Failure::new(
                ErrorCode::PermissionDenied,
                "No active workspace editor authority",
            ));
        }
        if timeout_ms == 0 || timeout_ms > 300_000 {
            return Err(Failure::new(
                ErrorCode::InvalidRequest,
                "Deadline must be between 1 and 300000 ms",
            ));
        }
        validate_virtual_source_request(&operation, &self.api)?;
        validate_comparison_request(&operation, &self.api)?;
        // Silent local-source preparation is additive; an older negotiated document contract
        // must not accidentally grant a new presentation or lifetime operation.
        if matches!(
            operation,
            EditorOperation::OpenDocumentSource { .. }
                | EditorOperation::ReleaseDocumentSource { .. }
        ) && self
            .api
            .capabilities
            .get("editor.documents")
            .is_none_or(|version| *version < semver::Version::new(1, 2, 0))
        {
            return Err(Failure::new(
                ErrorCode::CapabilityUnavailable,
                "Document source operations require editor.documents 1.2",
            ));
        }
        validate_comparison_navigation(&operation, &self.api, &self.plugin_services.commands)?;
        let mut comparison_icons = BTreeMap::new();
        if let EditorOperation::OpenDocumentComparison {
            folding: Some(folding),
            ..
        } = &operation
        {
            self.prepare_icon(&folding.collapse, &mut comparison_icons)?;
            self.prepare_icon(&folding.expand, &mut comparison_icons)?;
        }
        if matches!(&operation, EditorOperation::OpenDocumentComparison { .. }) {
            if !direct_editor_owner(
                self.plugin_services.context.as_ref(),
                &self.plugin_services.principal.instance,
            ) {
                return Err(Failure::new(
                    ErrorCode::PermissionDenied,
                    "Comparison views require direct ownership",
                ));
            }
        }
        if matches!(&operation, EditorOperation::ApplyEdits { .. })
            && self
                .api
                .capabilities
                .get("editor.edit")
                .is_none_or(|version| *version < semver::Version::new(1, 1, 0))
        {
            return Err(Failure::new(
                ErrorCode::CapabilityUnavailable,
                "Multiple edits require editor.edit 1.1",
            ));
        }
        // New local-resource and position operations are additive editor.documents 1.1 contracts.
        if matches!(
            &operation,
            EditorOperation::OpenDocument {
                resource: api::ResourceIdentity::Local { .. }
            } | EditorOperation::LocateDocument { .. }
                | EditorOperation::ListDocuments
                | EditorOperation::ReadDocument { .. }
        ) && self
            .api
            .capabilities
            .get("editor.documents")
            .is_none_or(|version| *version < semver::Version::new(1, 1, 0))
        {
            return Err(Failure::new(
                ErrorCode::CapabilityUnavailable,
                "Document resource operations require editor.documents 1.1",
            ));
        }
        let (capability, permission) = match &operation {
            EditorOperation::RefreshLanguageContext { provider, .. } => {
                if self
                    .api
                    .capabilities
                    .get("language.lsp")
                    .is_none_or(|version| *version < semver::Version::new(1, 6, 0))
                    || selected.is_none()
                    || !self.language_providers.contains_key(provider)
                {
                    return Err(Failure::new(
                        ErrorCode::PermissionDenied,
                        "Context refresh requires an owned language.lsp 1.6 provider",
                    ));
                }
                ("language.lsp", "editor.read")
            }
            EditorOperation::LanguageRequest { request } => {
                let _ = request; // Detailed schema/import admission follows shared permission checks.
                ("language.control", "editor.read")
            }
            EditorOperation::Diagnostics { publication } => {
                self.diagnostic_collection(publication)?;
                if !self.permissions.contains("editor.read") {
                    return Err(Failure::new(
                        ErrorCode::PermissionDenied,
                        "Diagnostics require editor.read",
                    ));
                }
                ("editor.diagnostics", "editor.diagnostics")
            }
            EditorOperation::Workspace { .. } => ("workspace.files", ""),
            EditorOperation::Decorations { .. } => ("editor.decorations", "editor.read"),
            EditorOperation::OpenVirtualDocument { .. }
            | EditorOperation::OpenVirtualSource { .. }
            | EditorOperation::RefreshVirtualDocument { .. } => ("editor.virtual", "editor.read"),
            EditorOperation::CompareDocuments { .. }
            | EditorOperation::OpenDocumentComparison { .. } => ("editor.diff", "editor.read"),
            EditorOperation::OpenDocument {
                resource: api::ResourceIdentity::Virtual { .. },
            } => ("editor.virtual", "editor.read"),
            EditorOperation::OpenDocument {
                resource: api::ResourceIdentity::Local { path },
            }
            | EditorOperation::OpenDocumentSource { path } => {
                api::ResourceIdentity::Local { path: path.clone() }.validate()?;
                if !self.permissions.contains("workspace.read") {
                    return Err(Failure::new(
                        ErrorCode::PermissionDenied,
                        "Opening local documents requires workspace.read",
                    ));
                }
                ("editor.documents", "editor.read")
            }
            EditorOperation::ReleaseDocumentSource { .. } => ("editor.documents", "editor.read"),
            EditorOperation::LocateDocument { .. } => ("editor.documents", "editor.read"),
            EditorOperation::ListDocuments | EditorOperation::ReadDocument { .. } => {
                ("editor.documents", "editor.read")
            }
            EditorOperation::Interaction { operation } => {
                operation.validate()?;
                if matches!(
                    operation,
                    plugin_protocol::interaction::Operation::SelectWithAccess { .. }
                ) && self
                    .api
                    .capabilities
                    .get("files.selection")
                    .is_none_or(|version| *version < semver::Version::new(1, 1, 0))
                {
                    return Err(Failure::new(
                        ErrorCode::CapabilityUnavailable,
                        "Explicit selection access requires files.selection 1.1",
                    ));
                }
                if operation.selection().is_some() {
                    self.check_selection_authority()?;
                    ("files.selection", "files.select")
                } else {
                    ("ui.interaction", "ui.interaction")
                }
            }
            EditorOperation::LocateViewport {
                panel,
                target,
                origin,
                ..
            } => {
                target.validate()?;
                if *origin == 0 {
                    return Err(Failure::new(
                        ErrorCode::InvalidRequest,
                        "Viewport location requires a nonzero origin",
                    ));
                }
                self.check_editor_viewport_authority(panel)?;
                ("editor.viewport", "editor.read")
            }
            EditorOperation::NavigateDocument { target, .. } => {
                target.validate()?;
                if !self.permissions.contains("editor.read") {
                    return Err(Failure::new(
                        ErrorCode::PermissionDenied,
                        "Navigation requires editor.read",
                    ));
                }
                if let api::NavigationTarget::PreviewNode { panel, .. } = target {
                    if !self.declared_editor_panels.contains(panel) {
                        return Err(Failure::new(
                            ErrorCode::PermissionDenied,
                            "Navigation panel is not owned by this instance",
                        ));
                    }
                }
                ("editor.navigation", navigation_permission(target))
            }
            EditorOperation::SaveImageInput { .. } => ("editor.images", "workspace.write"),
            EditorOperation::Transfer { .. } => ("ui.transfer", "ui.transfer"),
            EditorOperation::ReadDocumentSelection { .. } => ("editor.edit", "editor.read"),
            EditorOperation::ReplaceDocumentRange { .. } => ("editor.edit", "editor.write"),
            EditorOperation::ApplyEdits { .. } => ("editor.edit", "editor.write"),
            EditorOperation::WriteClipboard { text } if text.len() > 1024 * 1024 => {
                return Err(Failure::new(
                    ErrorCode::LimitExceeded,
                    "Clipboard exceeds 1 MiB",
                ));
            }
            EditorOperation::ReadClipboard
            | EditorOperation::WriteClipboard { .. }
            | EditorOperation::Clipboard { .. } => ("ui.clipboard", "clipboard"),
            EditorOperation::OpenDataFile { path } => {
                if path.is_empty()
                    || path.len() > 4096
                    || path.contains(['\\', ':', '\0'])
                    || path
                        .split('/')
                        .any(|part| part.is_empty() || matches!(part, "." | ".."))
                {
                    return Err(Failure::new(
                        ErrorCode::InvalidPath,
                        "Private file path must be relative",
                    ));
                }
                ("storage.editor", "storage")
            }
            EditorOperation::SetPanelVisibility { panel, .. } => {
                if !self.declared_panels.contains(panel) {
                    return Err(Failure::new(
                        ErrorCode::PermissionDenied,
                        "Panel is not owned by this plugin",
                    ));
                }
                ("ui.panels", "ui.panels")
            }
            EditorOperation::SaveDocument { .. } => ("editor.documents", "editor.write"),
            _ => ("editor.documents", "editor.read"),
        };
        if !self.api.capabilities.contains_key(capability) {
            return Err(Failure::new(
                ErrorCode::CapabilityUnavailable,
                format!("{capability} was not negotiated"),
            ));
        }
        if !permission.is_empty() && !self.permissions.contains(permission) {
            return Err(Failure::new(
                ErrorCode::PermissionDenied,
                format!("{permission} permission required"),
            ));
        }
        if let EditorOperation::OpenDataFile { path } = &operation {
            // Check authority before native path resolution; editor access must also respect Temporary ownership.
            self.validate_data_path(path)?;
        }
        validate_edit_request(&operation)?;
        let diagnostic_collection = if let EditorOperation::Diagnostics { publication } = &operation
        {
            Some(self.diagnostic_collection(publication)?)
        } else {
            None
        };
        // Progress messages modify their existing owner; they cannot allocate or finish another task.
        if let EditorOperation::Interaction {
            operation: interaction,
        } = &operation
        {
            let target = match interaction {
                plugin_protocol::interaction::Operation::UpdateProgress { request, .. }
                | plugin_protocol::interaction::Operation::FinishProgress { request } => {
                    Some(request)
                }
                _ => None,
            };
            if let Some(target) = target {
                self.roots.resolve(target)?;
                let pending = self
                    .editor_requests
                    .get(&target.resource)
                    .ok_or_else(|| Failure::new(ErrorCode::InvalidHandle, "Progress task ended"))?;
                if pending
                    .context
                    .as_ref()
                    .map(|context| &context.caller.instance)
                    != self
                        .plugin_services
                        .context
                        .as_ref()
                        .map(|context| &context.caller.instance)
                {
                    return Err(Failure::new(
                        ErrorCode::PermissionDenied,
                        "Progress belongs to another source",
                    ));
                }
                match interaction {
                    plugin_protocol::interaction::Operation::UpdateProgress {
                        message,
                        percent,
                        ..
                    } => pending.call.update_progress(message.clone(), *percent)?,
                    plugin_protocol::interaction::Operation::FinishProgress { .. } => {
                        pending.call.update_progress(String::new(), Some(100))?;
                        pending.call.finish(Ok(api::EditorValue::Interaction(
                            plugin_protocol::interaction::Value::Finished,
                        )));
                    }
                    _ => unreachable!(),
                }
                return Ok(Value::Unit);
            }
        }
        let image_input = if let EditorOperation::SaveImageInput { input, name } = &operation {
            if timeout_ms > crate::IMAGE_INPUT_TIMEOUT_MS {
                return Err(Failure::new(
                    ErrorCode::InvalidRequest,
                    "Image save deadline must not exceed 30000 ms",
                ));
            }
            Some(self.image_input_payload(input, name)?)
        } else {
            None
        };
        if self.editor_requests.len() >= 32 {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Editor request queue is full",
            ));
        }
        let mut language_control = if let EditorOperation::LanguageRequest { request } = &operation
        {
            Some(self.prepare_language_control(request)?)
        } else {
            None
        };
        if let (Some(selected), EditorOperation::LanguageRequest { request }) =
            (&selected, &operation)
        {
            let metadata = language_control.as_mut().unwrap();
            if selected.reference.descriptor.provider != request.provider {
                return Err(Failure::new(
                    ErrorCode::PermissionDenied,
                    "Context belongs to another provider",
                ));
            }
            let principal = &self.plugin_services.principal;
            metadata.selected_context = Some(self.host_resources.contexts.select(
                &selected.reference.token,
                &principal.instance,
                &principal.scope,
                &request.document.path,
            )?);
            metadata.selection_guard = Some(self.host_resources.contexts.selection_guard(
                metadata.selected_context.as_ref().unwrap(),
                &request.document.path,
            )?);
        }
        let workspace_request = if let EditorOperation::Workspace { operation } = &operation {
            Some(self.prepare_workspace_request(operation)?)
        } else {
            None
        };
        let Value::Resource(handle) = self.roots.open(RootKind::EditorRequest)? else {
            unreachable!()
        };
        let clipboard_offer = match self.prepare_clipboard(&operation) {
            Ok(offer) => offer,
            Err(error) => {
                self.roots.remove(&handle);
                return Err(error);
            }
        };
        let transfer_offer = match self.prepare_transfer(&operation) {
            Ok(offer) => offer,
            Err(error) => {
                self.roots.remove(&handle);
                return Err(error);
            }
        };
        let virtual_document = match self.prepare_virtual_document(&operation) {
            Ok(resource) => resource,
            Err(error) => {
                self.roots.remove(&handle);
                return Err(error);
            }
        };
        let mut call = EditorRequest::new(
            // Decoration admission supplies immutable owner and artwork, never guest native paths.
            handle.clone(),
            operation,
            self.workspace.display().to_string(),
            self.data.clone(),
            timeout_ms,
            self.plugin_services.alive.clone(),
            self.plugin_services.context.as_ref(),
        )
        .with_comparison_icons(comparison_icons);
        if let Some(offer) = clipboard_offer {
            call = call.with_clipboard_offer(offer);
        }
        if let Some(offer) = transfer_offer {
            call = call.with_transfer_offer(offer);
        }
        call.observe(self.host_resources.observations.begin(
            self.observation_identity(),
            crate::observability::Stage::Request,
            self.observed_parent,
        ));
        if let Some(selected) = selected
            && matches!(
                call.operation(),
                EditorOperation::RefreshLanguageContext { .. }
            )
        {
            call = call.with_language_refresh(self.host_resources.contexts.clone(), selected);
        }
        if let Some(input) = image_input {
            self.mark_image_input_pending(&input.input.handle, handle.clone());
            call = call.with_image_input(input);
        }
        if let Some(metadata) = language_control {
            call = call.with_language_control(metadata);
        }
        if let Some(resource) = virtual_document {
            call = call.with_virtual_document(resource);
        }
        if let Some(collection) = diagnostic_collection {
            call = call.with_diagnostic_collection(collection);
        }
        let batch = workspace_request
            .as_ref()
            .and_then(|payload| payload.batch())
            .map(|batch| batch.handle().clone());
        if let Some(payload) = workspace_request {
            call = call.with_workspace_request(payload);
        }
        match self.prepare_decorations(call.operation()) {
            Ok(Some((resource, icons))) => call = call.with_decorations(resource, icons),
            Ok(None) => {}
            Err(error) => {
                self.roots.remove(&handle);
                return Err(error);
            }
        }
        self.editor_requests.insert(
            handle.resource,
            PendingRequest {
                context: self.plugin_services.context.clone(),
                call,
                sent: false,
                reported: 0,
            },
        );
        if matches!(
            self.editor_requests
                .get(&handle.resource)
                .unwrap()
                .call
                .operation(),
            EditorOperation::Workspace {
                operation: plugin_protocol::workspace::Operation::ApplyPlan { .. }
            }
        ) {
            Ok(Value::BatchAccepted {
                request: handle,
                batch: batch.unwrap(),
            })
        } else {
            Ok(Value::Accepted(handle))
        }
    }

    /// Ambient access and exact native commands own their sources; delegated providers cannot borrow them.
    fn prepare_virtual_document(
        &mut self,
        operation: &EditorOperation,
    ) -> Result<Option<std::sync::Arc<crate::VirtualDocumentResource>>, Failure> {
        let check = |document: &api::DocumentVersion| -> Result<Option<std::sync::Arc<crate::VirtualDocumentResource>>, Failure> {
            if !document.path.starts_with("nanobug-virtual://") { return Ok(None); }
            if !direct_editor_owner(self.plugin_services.context.as_ref(), &self.plugin_services.principal.instance) {
                return Err(Failure::new(ErrorCode::PermissionDenied, "Virtual documents are instance-owned"));
            }
            let resource = self.virtual_documents.values().find(|resource| resource.document().is_some_and(|bound| bound.id == document.id))
                .ok_or_else(|| Failure::new(ErrorCode::PermissionDenied, "Virtual document is not owned by this instance"))?;
            self.roots.resolve(resource.handle())?;
            if !resource.is_live() { return Err(Failure::new(ErrorCode::InvalidHandle, "Virtual document was revoked")); }
            if resource.document().as_ref() != Some(document) { return Err(Failure::new(ErrorCode::StaleRevision, "Virtual document revision changed")); }
            Ok(Some(resource.clone()))
        };
        match operation {
            EditorOperation::OpenVirtualDocument {
                title,
                language,
                text,
            }
            | EditorOperation::OpenVirtualSource {
                title,
                language,
                text,
                ..
            } => {
                if !direct_editor_owner(
                    self.plugin_services.context.as_ref(),
                    &self.plugin_services.principal.instance,
                ) {
                    return Err(Failure::new(
                        ErrorCode::PermissionDenied,
                        "Virtual providers require direct ownership",
                    ));
                }
                if title.is_empty()
                    || title.len() > 256
                    || title.chars().any(char::is_control)
                    || language.as_ref().is_some_and(|value| {
                        value.len() > 128 || value.chars().any(char::is_control)
                    })
                {
                    return Err(Failure::new(
                        ErrorCode::InvalidRequest,
                        "Invalid virtual document title or language",
                    ));
                }
                if text.len() > 1024 * 1024 {
                    return Err(Failure::new(
                        ErrorCode::LimitExceeded,
                        "Virtual content exceeds 1 MiB",
                    ));
                }
                if self.virtual_documents.len() >= 32 {
                    return Err(Failure::new(
                        ErrorCode::LimitExceeded,
                        "Virtual document quota exceeded",
                    ));
                }
                let Value::Resource(handle) = self.roots.open(RootKind::VirtualDocument)? else {
                    unreachable!()
                };
                let resource = std::sync::Arc::new(crate::VirtualDocumentResource::new(
                    handle.clone(),
                    self.plugin_services.alive.clone(),
                ));
                self.virtual_documents
                    .insert(handle.resource, resource.clone());
                Ok(Some(resource))
            }
            EditorOperation::OpenDocument {
                resource: api::ResourceIdentity::Virtual { handle },
            } => {
                if !direct_editor_owner(
                    self.plugin_services.context.as_ref(),
                    &self.plugin_services.principal.instance,
                ) {
                    return Err(Failure::new(
                        ErrorCode::PermissionDenied,
                        "Virtual providers require direct ownership",
                    ));
                }
                self.roots.resolve(handle)?;
                let resource = self
                    .virtual_documents
                    .get(&handle.resource)
                    .filter(|resource| resource.is_live())
                    .ok_or_else(|| {
                        Failure::new(ErrorCode::InvalidHandle, "Virtual document was revoked")
                    })?;
                Ok(Some(resource.clone()))
            }
            EditorOperation::RefreshVirtualDocument { document, text } => {
                if text.len() > 1024 * 1024 {
                    return Err(Failure::new(
                        ErrorCode::LimitExceeded,
                        "Virtual content exceeds 1 MiB",
                    ));
                }
                check(document)?.map(Some).ok_or_else(|| {
                    Failure::new(
                        ErrorCode::UnsupportedOperation,
                        "Only virtual documents can be refreshed",
                    )
                })
            }
            EditorOperation::ReadDocument { document, .. }
            | EditorOperation::LocateDocument { document, .. }
            | EditorOperation::SaveDocument { document }
            | EditorOperation::ReadDocumentSelection { document }
            | EditorOperation::ReplaceDocumentRange { document, .. }
            | EditorOperation::ApplyEdits { document, .. } => check(document),
            EditorOperation::CompareDocuments { left, right } => {
                check(left)?;
                check(right)?;
                Ok(None)
            }
            EditorOperation::OpenDocumentComparison {
                left, right, third, ..
            } => {
                check(left)?;
                check(right)?;
                // The added source has exactly the same instance ownership and revision
                // boundary as both existing panes; it must not borrow a foreign virtual.
                if let Some(third) = third {
                    check(third)?;
                }
                Ok(None)
            }
            _ => Ok(None),
        }
    }
}

/// Check the original runtime-issued owner before creating or accessing private editor resources.
/// This grants neither a document handle nor filesystem authority; source checks still follow.
fn direct_editor_owner(context: Option<&crate::plugin_services::Context>, instance: &str) -> bool {
    // Host-issued typed callbacks are direct ownership too. Reuse the existing sealed-origin,
    // single-hop and lifetime check; a nested or foreign provider never gains this exception.
    context.is_none_or(|context| context.direct_host_command(instance))
}

/// Adjacent actions can invoke only this instance's declared, schema-checked typed commands.
/// Admission creates no new permission or cross-plugin invocation authority.
fn validate_comparison_navigation(
    operation: &EditorOperation,
    negotiated: &api::Negotiated,
    commands: &BTreeMap<String, plugin_protocol::service::Method>,
) -> Result<(), Failure> {
    let EditorOperation::OpenDocumentComparison {
        navigation: Some(navigation),
        ..
    } = operation
    else {
        return Ok(());
    };
    if !negotiated.capabilities.contains_key("plugin.commands") {
        return Err(Failure::new(
            ErrorCode::CapabilityUnavailable,
            "Comparison navigation requires plugin.commands",
        ));
    }
    for action in navigation.previous.iter().chain(&navigation.next) {
        action.validate()?;
        let signature = commands.get(&action.command).ok_or_else(|| {
            Failure::new(
                ErrorCode::InvalidRequest,
                "Comparison navigation must name a declared typed command",
            )
        })?;
        signature.parameters.accepts(&action.arguments)?;
    }
    Ok(())
}

/// Validate inert recognition hints before allocating sources; paths never authorize file access.
fn validate_virtual_source_request(
    operation: &EditorOperation,
    negotiated: &api::Negotiated,
) -> Result<(), Failure> {
    let EditorOperation::OpenVirtualSource { language_path, .. } = operation else {
        return Ok(());
    };
    let minimum = semver::Version::new(1, if language_path.is_some() { 2 } else { 1 }, 0);
    if negotiated
        .capabilities
        .get("editor.virtual")
        .is_none_or(|version| version < &minimum)
    {
        return Err(Failure::new(
            ErrorCode::CapabilityUnavailable,
            format!("Operation requires editor.virtual {minimum}"),
        ));
    }
    if language_path
        .as_ref()
        .is_some_and(|path| path.is_empty() || path.len() > 4096 || path.contains('\0'))
    {
        return Err(Failure::new(
            ErrorCode::InvalidRequest,
            "Invalid virtual source language path",
        ));
    }
    Ok(())
}

/// Validate additive comparison shape before allocating a native request or borrowing sources.
/// Two-pane requests retain 1.1 admission; a third source needs 1.2 and adjacent actions need 1.3.
fn validate_comparison_request(
    operation: &EditorOperation,
    negotiated: &api::Negotiated,
) -> Result<(), Failure> {
    let EditorOperation::OpenDocumentComparison {
        key,
        title,
        left,
        right,
        third,
        navigation,
        folding,
        ..
    } = operation
    else {
        return Ok(());
    };
    let minimum = semver::Version::new(
        1,
        if folding.is_some() {
            4
        } else if navigation.is_some() {
            3
        } else if third.is_some() {
            2
        } else {
            1
        },
        0,
    );
    if negotiated
        .capabilities
        .get("editor.diff")
        .is_none_or(|version| version < &minimum)
    {
        return Err(Failure::new(
            ErrorCode::CapabilityUnavailable,
            format!("Operation requires editor.diff {minimum}"),
        ));
    }
    if key.is_empty()
        || key.len() > 128
        || key.chars().any(char::is_control)
        || title.is_empty()
        || title.len() > 256
        || title.chars().any(char::is_control)
    {
        return Err(Failure::new(
            ErrorCode::InvalidRequest,
            "Invalid comparison key or title",
        ));
    }
    if let Some(folding) = folding {
        folding.validate()?;
    }
    if left.id == right.id
        || third
            .as_ref()
            .is_some_and(|third| third.id == left.id || third.id == right.id)
    {
        return Err(Failure::new(
            ErrorCode::InvalidRequest,
            "Comparison sources must be distinct document sessions",
        ));
    }
    Ok(())
}

/// Check bounded request shape; only the host can check the target revision and UTF-8 boundaries.
fn validate_edit_request(operation: &EditorOperation) -> Result<(), Failure> {
    if let EditorOperation::ApplyEdits {
        edits, selection, ..
    } = operation
    {
        api::validate_text_edits(edits, *selection)?;
    }
    if let EditorOperation::ReplaceDocumentRange {
        range,
        text,
        selection,
        expected_selection,
        ..
    } = operation
    {
        if text.len() > 1024 * 1024 {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Replacement text exceeds 1 MiB",
            ));
        }
        // Result selections address the resulting full document, so there is no replacement-size end bound.
        if [Some(range), Some(selection), expected_selection.as_ref()]
            .into_iter()
            .flatten()
            .any(|range| range.start > range.end)
        {
            return Err(Failure::new(
                ErrorCode::InvalidRequest,
                "Text range start must not exceed its end",
            ));
        }
    }
    Ok(())
}

impl Instance {
    /// The worker transfers only typed owned calls; the GPUI thread never touches a WASM store.
    pub fn take_editor_requests(&mut self) -> Vec<EditorRequest> {
        self.store
            .data_mut()
            .editor_requests
            .values_mut()
            .filter_map(|request| {
                if request.sent {
                    return None;
                }
                request.sent = true;
                Some(request.call.clone())
            })
            .collect()
    }
    /// Terminal results release slots even if the guest fails while consuming its notification.
    pub(super) fn poll_editor_requests(&mut self) -> anyhow::Result<()> {
        self.reconcile_transfer_offers();
        let cancelled = std::mem::take(&mut self.store.data_mut().transfer_cancellations);
        for (panel, event) in cancelled {
            self.notify(Some(panel), event)?;
        }
        self.store.data_mut().prune_clipboard_offers();
        // Native close, failed publication and delegated retirement release this source's quota.
        let revoked = self
            .store
            .data()
            .decorations
            .iter()
            .filter(|(_, resource)| !resource.is_live())
            .map(|(slot, resource)| (*slot, resource.handle().clone()))
            .collect::<Vec<_>>();
        for (slot, handle) in revoked {
            self.store.data_mut().decorations.remove(&slot);
            self.store.data_mut().roots.remove(&handle);
            self.store
                .data_mut()
                .plugin_services
                .resources
                .remove(&slot);
        }
        // A native tab close revokes its shared authority; release the persistent root before new work.
        let revoked = self
            .store
            .data()
            .virtual_documents
            .iter()
            .filter(|(_, resource)| !resource.is_live())
            .map(|(slot, resource)| (*slot, resource.handle().clone()))
            .collect::<Vec<_>>();
        for (slot, handle) in revoked {
            self.store.data_mut().virtual_documents.remove(&slot);
            self.store.data_mut().roots.remove(&handle);
        }
        let updates = self
            .store
            .data_mut()
            .editor_requests
            .iter_mut()
            .filter_map(|(slot, pending)| {
                let (version, update) = pending.call.update();
                if version == pending.reported {
                    return None;
                }
                pending.reported = version;
                Some((
                    *slot,
                    pending.call.handle().clone(),
                    update,
                    pending.context.clone(),
                ))
            })
            .collect::<Vec<_>>();
        for (slot, handle, mut update, context) in updates {
            // A preceding guest callback may explicitly release another request in this batch.
            if !self.store.data().editor_requests.contains_key(&slot) {
                continue;
            }
            // An earlier callback in this batch may reselect this source after collection.
            // Recheck at the actual guest-delivery boundary, including already completed results.
            update = self.store.data().editor_requests[&slot]
                .call
                .check_language_update(update);
            if update.is_terminal() {
                if let Some(pending) = self.store.data_mut().editor_requests.remove(&slot) {
                    self.store
                        .data_mut()
                        .finish_clipboard_enumeration(&pending.call, Some(&update));
                    if matches!(
                        pending.call.operation(),
                        EditorOperation::Decorations {
                            operation: plugin_protocol::decorations::Operation::Create { .. }
                        }
                    ) && !matches!(&update, api::RequestUpdate::Completed { result: Ok(_) })
                    {
                        if let Some(resource) = pending.call.decoration() {
                            resource.revoke();
                            let handle = resource.handle().clone();
                            // Release before notifying the guest, which may immediately retry a failed create.
                            self.store.data_mut().decorations.remove(&handle.resource);
                            self.store.data_mut().roots.remove(&handle);
                            self.store
                                .data_mut()
                                .plugin_services
                                .resources
                                .remove(&handle.resource);
                        }
                    }
                    if matches!(
                        pending.call.operation(),
                        EditorOperation::OpenVirtualDocument { .. }
                            | EditorOperation::OpenVirtualSource { .. }
                    ) && !matches!(&update, api::RequestUpdate::Completed { result: Ok(_) })
                    {
                        if let Some(resource) = pending.call.virtual_document() {
                            resource.revoke();
                        }
                    }
                    if let api::RequestUpdate::Completed { result: Ok(_) } = &update {
                        if let EditorOperation::Interaction { operation } = pending.call.operation()
                            && let Some(selection) = operation.selection()
                        {
                            let result = pending
                                .call
                                .take_selection()
                                .ok_or_else(|| {
                                    Failure::new(
                                        ErrorCode::InvalidRequest,
                                        "Missing native selection",
                                    )
                                })
                                .and_then(|paths| {
                                    // Materialization keeps the same authenticated source that
                                    // admitted the picker, including its effective permission set.
                                    let state = self.store.data_mut();
                                    let previous = std::mem::replace(
                                        &mut state.plugin_services.context,
                                        context.clone(),
                                    );
                                    let result = state.grant_selection(
                                        paths,
                                        selection.mode,
                                        selection.multiple,
                                        selection.access,
                                    );
                                    state.plugin_services.context = previous;
                                    result
                                });
                            update = api::RequestUpdate::Completed { result };
                        }
                    }
                    self.store
                        .data_mut()
                        .finish_image_input_request(&pending.call, &update);
                }
                self.store.data_mut().roots.remove(&handle);
                self.store
                    .data_mut()
                    .plugin_services
                    .resources
                    .remove(&handle.resource);
            }
            self.call_with_service_context(
                context,
                api::Input::Event {
                    panel: None,
                    event: Notification::Request { handle, update },
                },
            )?;
        }
        Ok(())
    }
}

impl Drop for Instance {
    /// Manager removal also revokes calls retained by a queued UI callback.
    fn drop(&mut self) {
        self.stop();
    }
}
