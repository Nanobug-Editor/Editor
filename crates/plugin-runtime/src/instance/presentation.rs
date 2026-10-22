//! Capability and ownership admission for domain-neutral workspace presentation.
use super::*;
use api::{ErrorCode, Failure, Value};
use plugin_protocol::{presentation, ui};
use resource_roots::RootKind;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

impl State {
    /// Each operation checks current consent; the one owned resource follows ordinary close/revoke.
    pub(super) fn presentation_request(
        &mut self,
        operation: presentation::Operation,
    ) -> Result<Value, Failure> {
        if !self.active
            || self.roots.retired
            || self.roots.application
            || self.workspace.as_os_str().is_empty()
        {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "Presentation requires an active workspace instance",
            ));
        }
        if !self.permissions.contains("ui.panels") {
            return Err(Failure::new(
                ErrorCode::PermissionDenied,
                "ui.panels grant required",
            ));
        }
        let capability = match &operation {
            presentation::Operation::Files { .. } => "explorer.decorations",
            presentation::Operation::Entries { .. } => "ui.workspace",
            presentation::Operation::Create => {
                if self.api.capabilities.contains_key("explorer.decorations") {
                    "explorer.decorations"
                } else {
                    "ui.workspace"
                }
            }
        };
        if !self.api.capabilities.contains_key(capability)
            || !self.api.capabilities.contains_key("ui.native")
            || !self.api.capabilities.contains_key("ui.panels")
        {
            return Err(Failure::new(
                ErrorCode::CapabilityUnavailable,
                "Workspace presentation capability, ui.native and ui.panels must be negotiated",
            ));
        }
        match operation {
            presentation::Operation::Create => {
                if !self.presentations.is_empty() {
                    return Err(Failure::new(
                        ErrorCode::LimitExceeded,
                        "One workspace presentation collection per instance",
                    ));
                }
                let Value::Resource(handle) = self.roots.open(RootKind::WorkspacePresentation)?
                else {
                    unreachable!()
                };
                let authority = crate::EditorAuthority::new(
                    self.plugin_services.alive.clone(),
                    self.plugin_services.context.as_ref(),
                );
                self.presentations.insert(
                    handle.resource,
                    Arc::new(crate::WorkspacePresentationResource::new(
                        handle.clone(),
                        self.plugin_services.principal.plugin.clone(),
                        self.workspace.clone(),
                        authority,
                    )),
                );
                Ok(Value::Resource(handle))
            }
            presentation::Operation::Files { resource, files } => {
                if !self.permissions.contains("workspace.read") {
                    return Err(Failure::new(
                        ErrorCode::PermissionDenied,
                        "Filename decorations require workspace.read",
                    ));
                }
                self.owned_presentation(&resource)?.publish_files(files)?;
                Ok(Value::Unit)
            }
            presentation::Operation::Entries { resource, entries } => {
                let owned = self.owned_presentation(&resource)?;
                if entries.len() > 8
                    || serde_json::to_vec(&entries)
                        .map_or(true, |bytes| bytes.len() > 2 * 1024 * 1024 - 4096)
                {
                    return Err(Failure::new(
                        ErrorCode::LimitExceeded,
                        "Workspace entry quota exceeded",
                    ));
                }
                let mut ids = BTreeSet::new();
                let mut icons = BTreeMap::new();
                for entry in &entries {
                    entry.validate()?;
                    if !ids.insert(&entry.id) {
                        return Err(Failure::new(
                            ErrorCode::Conflict,
                            "Duplicate workspace entry",
                        ));
                    }
                    self.prepare_icon(&entry.icon, &mut icons)?;
                    check_artwork_budget(&icons)?;
                    if let Some(command) = &entry.on_open {
                        self.validate_presentation_command(command)?;
                    }
                    let mut nodes = Vec::new();
                    for item in &entry.menu {
                        item.visit(&mut |item| nodes.push(item));
                    }
                    self.prepare_action_menu(&nodes, &mut icons)?;
                }
                owned.publish_entries(entries, icons)?;
                Ok(Value::Unit)
            }
        }
    }
    /// Validate all hidden actions and artwork before publishing any native menu.
    pub(super) fn prepare_action_menu(
        &self,
        nodes: &[&ui::ActionMenuItem],
        icons: &mut BTreeMap<String, Arc<[u8]>>,
    ) -> Result<(), Failure> {
        for item in nodes {
            if let ui::ActionMenuKind::Action { command } = &item.kind {
                self.validate_presentation_command(command)?;
            }
            if let ui::ActionMenuKind::Submenu {
                on_open: Some(command),
                ..
            } = &item.kind
            {
                self.validate_presentation_command(command)?;
            }
            if let Some(icon) = &item.icon {
                self.prepare_icon(icon, icons)?;
                // Stop after this bounded icon pair, before reading any later menu artwork.
                check_artwork_budget(icons)?;
            }
        }
        Ok(())
    }
    /// Menus reuse the package's existing typed command schemas, never a second command channel.
    fn validate_presentation_command(
        &self,
        command: &plugin_protocol::tree::Command,
    ) -> Result<(), Failure> {
        if !self.api.capabilities.contains_key("plugin.commands") {
            return Err(Failure::new(
                ErrorCode::CapabilityUnavailable,
                "Action menus require plugin.commands",
            ));
        }
        command.validate()?;
        self.plugin_services
            .commands
            .get(&command.command)
            .ok_or_else(|| {
                Failure::new(
                    ErrorCode::InvalidRequest,
                    "Action must name an owned typed command",
                )
            })?
            .parameters
            .accepts(&command.arguments)
    }
    /// A foreign scope, retired incarnation or unrelated root kind cannot address this collection.
    fn owned_presentation(
        &self,
        handle: &api::ResourceHandle,
    ) -> Result<Arc<crate::WorkspacePresentationResource>, Failure> {
        if !matches!(self.roots.resolve(handle)?, RootKind::WorkspacePresentation) {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Not a workspace presentation collection",
            ));
        }
        self.presentations
            .get(&handle.resource)
            .filter(|resource| resource.is_live())
            .cloned()
            .ok_or_else(|| {
                Failure::new(
                    ErrorCode::InvalidHandle,
                    "Workspace presentation was revoked",
                )
            })
    }
}

/// Bound transient admission as well as the retained snapshot for both native menu consumers.
fn check_artwork_budget(icons: &BTreeMap<String, Arc<[u8]>>) -> Result<(), Failure> {
    if icons.values().map(|bytes| bytes.len()).sum::<usize>() > 2 * 1024 * 1024 {
        return Err(Failure::new(
            ErrorCode::LimitExceeded,
            "Menu artwork exceeds its budget",
        ));
    }
    Ok(())
}

impl Instance {
    /// Native shells borrow only resources from this live instance; no guest execution on UI reads.
    pub fn workspace_presentations(&self) -> Vec<Arc<crate::WorkspacePresentationResource>> {
        self.store
            .data()
            .presentations
            .values()
            .filter(|resource| resource.is_live())
            .cloned()
            .collect()
    }
}
