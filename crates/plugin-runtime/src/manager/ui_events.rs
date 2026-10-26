//! Validate panel ownership, input revisions and preview authority before invoking a guest.
use super::*;

impl Manager {
    /// Route a notification with an explicit owning panel; never infer ownership from guest identity.
    pub fn event(
        &mut self,
        id: &str,
        panel: Option<String>,
        event: api::Notification,
    ) -> anyhow::Result<()> {
        if matches!(
            &event,
            api::Notification::ImageInput { .. }
                | api::Notification::Transfer(_)
                | api::Notification::DragFinished(_)
        ) {
            return Err(api::Failure::new(
                api::ErrorCode::InvalidRequest,
                "Native image/transfer notifications require runtime-owned native offers",
            )
            .into());
        }
        self.refresh_services();
        if let api::Notification::PanelVisibility { panel: target, .. } = &event {
            let entry = self.installed.get(id).ok_or_else(|| {
                api::Failure::new(api::ErrorCode::NotFound, "Unknown visibility owner")
            })?;
            validate_panel_visibility(
                entry,
                panel.as_deref(),
                target,
                self.trusted,
                self.workspace_open,
            )?;
            // Observation cannot demand a cold instance or restart one that retired after sampling.
            let instance = self.live.get(id).ok_or_else(|| {
                api::Failure::new(api::ErrorCode::InvalidState, "Visibility owner is not live")
            })?;
            if !instance
                .negotiated()
                .capabilities
                .contains_key("ui.visibility")
            {
                return Err(api::Failure::new(
                    api::ErrorCode::CapabilityUnavailable,
                    "ui.visibility was not negotiated",
                )
                .into());
            }
        }
        // Never deliver document text to a guest without the same editor permission as native reads.
        let inner = &event;
        if let api::Notification::SourceViewport(position) = inner {
            position.validate()?;
            let instance = self.live.get(id).ok_or_else(|| {
                api::Failure::new(api::ErrorCode::InvalidState, "Viewport owner is disabled")
            })?;
            let scene = panel
                .as_ref()
                .and_then(|panel| instance.views.get(panel))
                .ok_or_else(|| {
                    api::Failure::new(
                        api::ErrorCode::InvalidHandle,
                        "Viewport panel is unavailable",
                    )
                })?;
            if scene.editor_viewport.is_none()
                || scene.dialog.is_some()
                || scene.menu.is_some()
                || scene.source.as_ref() != Some(&position.document)
                || scene.revision != position.ui_revision
                || panel
                    .as_ref()
                    .and_then(|panel| instance.preview_sources.get(panel))
                    .and_then(Option::as_ref)
                    != Some(&position.document)
            {
                return Err(api::Failure::new(
                    api::ErrorCode::StaleRevision,
                    "Viewport source or scene has changed",
                )
                .into());
            }
        }
        if let api::Notification::FilePreview { file } = inner {
            let entry = self
                .installed
                .get(id)
                .ok_or_else(|| api::Failure::new(api::ErrorCode::NotFound, "Unknown plugin"))?;
            let panel_id = panel.as_ref().ok_or_else(|| {
                api::Failure::new(
                    api::ErrorCode::InvalidRequest,
                    "File preview requires a panel",
                )
            })?;
            if !self.trusted
                || !self.workspace_open
                || !entry.grants.contains("editor.read")
                || entry.manifest.scope != api::InstanceScope::Workspace
                || !entry
                    .manifest
                    .panels
                    .iter()
                    .any(|descriptor| descriptor.id == *panel_id && descriptor.position == "editor")
            {
                return Err(api::Failure::new(
                    api::ErrorCode::PermissionDenied,
                    "File preview requires a trusted workspace-owned surface and editor.read",
                )
                .into());
            }
            if let Some(file) = file {
                file.version.validate()?;
                if file.file_type.len() > 32
                    || !file.file_type.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'+')
                    })
                {
                    return Err(api::Failure::new(
                        api::ErrorCode::InvalidRequest,
                        "Invalid file type",
                    )
                    .into());
                }
            }
            let instance = self.live.get_mut(id).ok_or_else(|| {
                api::Failure::new(api::ErrorCode::InvalidState, "File provider is disabled")
            })?;
            if !instance
                .negotiated()
                .capabilities
                .contains_key("editor.files")
            {
                return Err(api::Failure::new(
                    api::ErrorCode::CapabilityUnavailable,
                    "editor.files was not negotiated",
                )
                .into());
            }
            if let (Some(Some(current)), Some(next)) = (instance.file_sources.get(panel_id), file)
                && current.version.id == next.version.id
                && next.version.revision < current.version.revision
            {
                return Err(api::Failure::new(
                    api::ErrorCode::StaleRevision,
                    "File context is obsolete",
                )
                .into());
            }
            instance.file_sources.insert(panel_id.clone(), file.clone());
            // A file-only provider must never retain authority over the preceding text document.
            instance.preview_sources.insert(
                panel_id.clone(),
                file.as_ref().and_then(|context| context.text.clone()),
            );
        }
        if let api::Notification::Preview { document, text } = inner {
            let panel_id = panel.as_ref();
            let entry = self
                .installed
                .get(id)
                .ok_or_else(|| api::Failure::new(api::ErrorCode::NotFound, "Unknown plugin"))?;
            if panel.is_none()
                || !entry.grants.contains("editor.read")
                || entry.manifest.scope != api::InstanceScope::Workspace
                || !entry
                    .manifest
                    .panels
                    .iter()
                    .any(|panel| Some(&panel.id) == panel_id && panel.position == "editor")
            {
                return Err(api::Failure::new(
                    api::ErrorCode::PermissionDenied,
                    "Preview requires a workspace-owned declared surface and editor.read grant",
                )
                .into());
            }
            if text.len() > 1024 * 1024 || (document.is_none() && !text.is_empty()) {
                return Err(api::Failure::new(
                    api::ErrorCode::LimitExceeded,
                    "Invalid or oversized preview document",
                )
                .into());
            }
            let instance = self.live.get_mut(id).ok_or_else(|| {
                api::Failure::new(api::ErrorCode::InvalidState, "Preview plugin is disabled")
            })?;
            // Late memory notifications must not roll back source authority or retire a healthy guest.
            if let (Some(Some(current)), Some(next)) = (
                instance
                    .preview_sources
                    .get(panel.as_ref().expect("validated panel")),
                document,
            ) && current.id == next.id
                && next.revision < current.revision
            {
                return Err(api::Failure::new(
                    api::ErrorCode::StaleRevision,
                    "Preview document is obsolete",
                )
                .into());
            }
            instance.preview_sources.insert(
                panel.as_ref().expect("validated panel").clone(),
                document.clone(),
            );
            // Revoke old offers before the new Preview hook can attempt a save using their handles.
            // Accepted writers retain their original payload and may still report completed files.
            instance.reconcile_image_inputs();
        }
        if let api::Notification::Ui(event) = inner {
            if panel.is_none() {
                return Err(api::Failure::new(
                    api::ErrorCode::InvalidRequest,
                    "UI events require one owning panel",
                )
                .into());
            }
            let document = self
                .live
                .get(id)
                .and_then(|instance| instance.views.get(panel.as_ref().expect("validated panel")))
                .ok_or_else(|| {
                    api::Failure::new(api::ErrorCode::InvalidHandle, "UI panel is unavailable")
                })?;
            // Reject stale host callbacks before calling WASM; they are not plugin crashes.
            if let Err(failure) = document.validate_event(event) {
                if failure.code == api::ErrorCode::StaleRevision
                    && let ui::Action::Tree(plugin_protocol::tree::Event::Request(request)) =
                        &event.action
                    && let Some(instance) = self.live.get(id)
                {
                    // Keep rejecting obsolete input. The provider never received this page, so
                    // its native owner must be free to create a fresh scene-bound request instead
                    // of waiting for a false provider timeout. No business action is replayed.
                    instance.release_undelivered_tree_page(request);
                }
                return Err(failure.into());
            }
            if !self
                .live
                .get_mut(id)
                .expect("validated UI owner")
                .accept_scene_event(panel.as_deref().expect("validated panel"), event)?
            {
                return Ok(());
            }
            if let ui::Action::Tree(tree_event) = &event.action {
                let instance = self.live.get(id).expect("validated UI owner");
                instance.validate_tree_event(tree_event)?;
                if let plugin_protocol::tree::Event::Invoke {
                    tree,
                    epoch,
                    node,
                    action,
                } = tree_event
                {
                    let command = instance.tree_command(tree, *epoch, node, action)?;
                    // A native node menu uses the same declared schema, broker and incarnation as ticket 02.
                    // The node's opaque identifier remains plugin data and grants no path authority.
                    return self.invoke_command(id, &command.command, command.arguments);
                }
            }
        }
        if let api::Notification::Tool(event) = inner {
            let instance = self.live.get(id).ok_or_else(|| {
                api::Failure::new(api::ErrorCode::InvalidState, "Tool owner is disabled")
            })?;
            let panel = panel.as_ref().ok_or_else(|| {
                api::Failure::new(
                    api::ErrorCode::InvalidRequest,
                    "Tool requires one owning panel",
                )
            })?;
            // Captured buttons outlive their publication; installed consent, trust and ownership
            // must still hold now, rather than relying on the instance's initialization snapshot.
            let entry = self
                .installed
                .get(id)
                .ok_or_else(|| api::Failure::new(api::ErrorCode::NotFound, "Unknown tool owner"))?;
            let owned = entry.manifest.panels.iter().any(|descriptor| {
                descriptor.id == *panel
                    && match &event.target {
                        ui::ToolTarget::File { .. } => descriptor.position == "editor",
                        ui::ToolTarget::Window { panel: target } => {
                            descriptor.position != "editor" && target == panel
                        }
                    }
            });
            let file_authorized = !matches!(event.target, ui::ToolTarget::File { .. })
                || (self.trusted
                    && self.workspace_open
                    && entry.grants.contains("editor.read")
                    && entry.manifest.scope == api::InstanceScope::Workspace);
            if !owned || !file_authorized {
                return Err(api::Failure::new(
                    api::ErrorCode::PermissionDenied,
                    "Tool target requires current consent and an owned declared surface",
                )
                .into());
            }
            let document = instance.views.get(panel).ok_or_else(|| {
                api::Failure::new(api::ErrorCode::InvalidHandle, "Tool surface was withdrawn")
            })?;
            document.validate_tool_event(event)?;
            if let ui::ToolTarget::File { version } = &event.target
                && instance
                    .file_sources
                    .get(panel)
                    .and_then(Option::as_ref)
                    .map(|file| &file.version)
                    != Some(version)
            {
                return Err(api::Failure::new(
                    api::ErrorCode::StaleRevision,
                    "File function target is obsolete",
                )
                .into());
            }
        }
        let instance = self
            .live
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("Plugin is disabled"))?;
        if let Err(error) = instance.notify(panel, event) {
            if instance.preserve_rejected_scene(&error) {
                return Err(error);
            }
            instance.stop();
            if let Some(entry) = self.installed.get_mut(id) {
                entry.error = Some(format!("{error:#}"));
            }
            return Err(error);
        }
        // Replacing or withdrawing a declaration cancels its resource before returning to publication.
        self.reconcile_images();
        Ok(())
    }
}

/// Host observations retain the same ownership, consent and scope boundary as their native surface.
fn validate_panel_visibility(
    entry: &Installed,
    routed_panel: Option<&str>,
    target: &str,
    trusted: bool,
    workspace_open: bool,
) -> Result<(), api::Failure> {
    if routed_panel != Some(target) {
        return Err(api::Failure::new(
            api::ErrorCode::InvalidRequest,
            "Visibility requires its exact routed panel",
        ));
    }
    if !entry.enabled
        || entry.error.is_some()
        || !entry.grants.contains("ui.panels")
        || !entry.manifest.panels.iter().any(|panel| panel.id == target)
        || (entry.manifest.scope == api::InstanceScope::Workspace && (!trusted || !workspace_open))
    {
        return Err(api::Failure::new(
            api::ErrorCode::PermissionDenied,
            "Visibility requires an enabled declared surface and current ui.panels consent",
        ));
    }
    if !entry.supports_capability("ui.visibility") {
        return Err(api::Failure::new(
            api::ErrorCode::CapabilityUnavailable,
            "ui.visibility was not negotiated",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod visibility_tests;
