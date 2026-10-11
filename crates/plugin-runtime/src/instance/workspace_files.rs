//! Admit native file proposals without exposing paths or borrowing private-data/peer-root authority.

use super::*;
use crate::{
    AuthorizedFile, EditorAuthority, WorkspaceBatch, WorkspaceRequest,
    native_path::{normalized_target, pin_path},
};
use api::{ErrorCode, Failure};
use plugin_protocol::{
    interaction::SelectionMode,
    workspace::{Change, FileTarget, Operation},
};
use resource_roots::RootKind;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
mod native;
pub(crate) use native::NativeDocumentRequests;

impl State {
    /// Direct host commands retain the owning installation grant; services must delegate every grant.
    fn file_permission(&self, permission: &str) -> Result<(), Failure> {
        if !self.permissions.contains(permission)
            || self
                .plugin_services
                .context
                .as_ref()
                .is_some_and(|context| {
                    !context.direct_host_selection(&self.plugin_services.principal.instance)
                        && !context.permissions.contains(permission)
                })
        {
            return Err(Failure::new(
                ErrorCode::PermissionDenied,
                format!("{permission} was not granted by the source"),
            ));
        }
        Ok(())
    }

    /// Trusted host ingress belongs to this instance; forwarded consumers retain their own principal.
    fn batch_caller(&self) -> Option<String> {
        self.plugin_services
            .context
            .as_ref()
            .filter(|context| {
                !context.direct_host_selection(&self.plugin_services.principal.instance)
            })
            .map(|context| context.caller.instance.clone())
    }

    /// Roots are immutable native capabilities. A display name or resource number never grants write intent.
    pub(super) fn authorize_file(
        &mut self,
        target: &FileTarget,
        write: bool,
        save_write: bool,
    ) -> Result<AuthorizedFile, Failure> {
        target.validate()?;
        let kind = self.roots.resolve(&target.root)?;
        let (path, root, observed, read, save_exact) = match kind {
            RootKind::Workspace => {
                self.file_authority(kind, false)?;
                self.file_permission(if write {
                    "workspace.write"
                } else {
                    "workspace.read"
                })?;
                if write && target.path.is_empty() {
                    return Err(Failure::new(
                        ErrorCode::InvalidPath,
                        "Cannot mutate the workspace root",
                    ));
                }
                let root = self
                    .workspace
                    .canonicalize()
                    .map_err(crate::native_path::io_failure)?;
                let observed = pin_path(&root, false)?.identities;
                (root.join(&target.path), root, observed, true, false)
            }
            RootKind::Selected => {
                self.check_selection_authority()?;
                let selection = self
                    .selected_resources
                    .get(&target.root.resource)
                    .ok_or_else(|| {
                        Failure::new(ErrorCode::InvalidHandle, "Selection was released")
                    })?;
                if write
                    && (!selection.write
                        || self
                            .api
                            .capabilities
                            .get("files.selection")
                            .is_none_or(|version| *version < semver::Version::new(1, 1, 0)))
                {
                    return Err(Failure::new(
                        ErrorCode::PermissionDenied,
                        "Selection has no explicit write authority",
                    ));
                }
                if selection.mode == SelectionMode::Save && (!write || !save_write) {
                    return Err(Failure::new(
                        ErrorCode::PermissionDenied,
                        "Save intent permits only exact planned replacement",
                    ));
                }
                let exact = selection.mode != SelectionMode::Directory;
                if (exact && !target.path.is_empty()) || (!exact && write && target.path.is_empty())
                {
                    return Err(Failure::new(
                        ErrorCode::InvalidPath,
                        "Selection does not grant this descendant or root mutation",
                    ));
                }
                let pins = pin_path(&selection.path, selection.mode == SelectionMode::Save)?;
                if !pins.identities.starts_with(&selection.observed) {
                    return Err(Failure::new(
                        ErrorCode::InvalidHandle,
                        "Selected resource or ancestor was replaced",
                    ));
                }
                let path = if exact {
                    selection.path.clone()
                } else {
                    selection.path.join(&target.path)
                };
                let root = if exact {
                    selection.path.parent().unwrap().to_path_buf()
                } else {
                    selection.path.clone()
                };
                let observed = pin_path(&root, false)?.identities;
                (
                    path,
                    root,
                    observed,
                    selection.mode != SelectionMode::Save,
                    selection.mode == SelectionMode::Save,
                )
            }
            _ => {
                return Err(Failure::new(
                    ErrorCode::InvalidHandle,
                    "Only workspace and selected file roots may enter a plan",
                ));
            }
        };
        // Native session lookup uses the canonical spelling; an alias must not bypass dirty text.
        let path = normalized_target(&path)?;
        let root_alive = self
            .file_root_lifetimes
            .entry(target.root.resource)
            .or_insert_with(|| Arc::new(AtomicBool::new(true)))
            .clone();
        let file = AuthorizedFile::new(
            target.clone(),
            path,
            root,
            observed,
            EditorAuthority::new(
                self.plugin_services.alive.clone(),
                self.plugin_services.context.as_ref(),
            ),
            root_alive,
            read,
            write,
            save_exact,
        );
        file.guard()?;
        Ok(file)
    }

    /// Validate all operation grants before the host allocates a native preview or reads preimages.
    pub(super) fn prepare_workspace_request(
        &mut self,
        operation: &Operation,
    ) -> Result<WorkspaceRequest, Failure> {
        if self
            .api
            .capabilities
            .get("workspace.files")
            .is_none_or(|version| *version < semver::Version::new(1, 2, 0))
        {
            return Err(Failure::new(
                ErrorCode::CapabilityUnavailable,
                "Native file plans require workspace.files 1.2",
            ));
        }
        let can_edit = self
            .api
            .capabilities
            .get("editor.edit")
            .is_some_and(|version| *version >= semver::Version::new(1, 1, 0))
            && self.file_permission("editor.write").is_ok();
        let can_read_documents = self
            .api
            .capabilities
            .get("editor.documents")
            .is_some_and(|version| *version >= semver::Version::new(1, 1, 0))
            && self.file_permission("editor.read").is_ok();
        let mut payload = WorkspaceRequest {
            can_edit,
            can_read_documents,
            ..Default::default()
        };
        match operation {
            Operation::Stat { target }
            | Operation::Read { target }
            | Operation::List { target, .. } => {
                payload
                    .files
                    .push(self.authorize_file(target, false, false)?);
            }
            Operation::Discover { root, .. } => {
                payload.files.push(self.authorize_file(
                    &FileTarget {
                        root: root.clone(),
                        path: String::new(),
                    },
                    false,
                    false,
                )?);
            }
            Operation::ApplyPlan { plan } => {
                crate::workspace_resources::validate_plan_documents(
                    plan,
                    &self.api.capabilities,
                    |permission| self.file_permission(permission),
                )?;
                if self.workspace_batches.len() >= 8 || self.roots.len() >= 127 {
                    return Err(Failure::new(
                        ErrorCode::LimitExceeded,
                        "At most eight live recovery batches per instance",
                    ));
                }
                let mut required = BTreeSet::new();
                for change in &plan.changes {
                    let mut add =
                        |target: &FileTarget, write: bool, save: bool| -> Result<(), Failure> {
                            let file = self.authorize_file(target, write, save)?;
                            let selected =
                                matches!(self.roots.resolve(&target.root)?, RootKind::Selected);
                            required.insert(
                                if selected {
                                    "files.select"
                                } else if write {
                                    "workspace.write"
                                } else {
                                    "workspace.read"
                                }
                                .to_string(),
                            );
                            payload.files.push(file);
                            Ok(())
                        };
                    match change {
                        Change::EditDocument { .. } => {
                            required.insert("editor.write".into());
                        }
                        Change::CreateDirectory { target } | Change::Delete { target, .. } => {
                            add(target, true, false)?
                        }
                        Change::Write { target, .. } => add(target, true, true)?,
                        Change::Copy { source, target, .. } => {
                            add(source, false, false)?;
                            add(target, true, false)?;
                        }
                        Change::Rename { source, target, .. } => {
                            add(source, true, false)?;
                            add(target, true, false)?;
                        }
                    }
                }
                if can_edit {
                    required.insert("editor.write".into());
                }
                let api::Value::Resource(handle) = self.roots.open(RootKind::WorkspaceBatch)?
                else {
                    unreachable!()
                };
                let batch = Arc::new(WorkspaceBatch::new(
                    handle.clone(),
                    EditorAuthority::new(
                        self.plugin_services.alive.clone(),
                        self.plugin_services.context.as_ref(),
                    ),
                    self.batch_caller(),
                    required,
                ));
                self.workspace_batches
                    .insert(handle.resource, batch.clone());
                payload.batch = Some(batch);
            }
            Operation::BatchStatus { batch } | Operation::UndoBatch { batch } => {
                if !matches!(self.roots.resolve(batch)?, RootKind::WorkspaceBatch) {
                    return Err(Failure::new(
                        ErrorCode::InvalidHandle,
                        "Not a recovery resource",
                    ));
                }
                let resource = self
                    .workspace_batches
                    .get(&batch.resource)
                    .filter(|resource| resource.is_live())
                    .ok_or_else(|| {
                        Failure::new(ErrorCode::InvalidHandle, "Recovery resource was revoked")
                    })?;
                if resource.caller() != self.batch_caller().as_deref() {
                    return Err(Failure::new(
                        ErrorCode::PermissionDenied,
                        "Recovery resource belongs to another source",
                    ));
                }
                if matches!(operation, Operation::UndoBatch { .. }) {
                    for permission in resource.required() {
                        self.file_permission(permission)?;
                    }
                }
                payload.batch = Some(resource.clone());
            }
            Operation::Rescan => {
                self.file_permission("workspace.read")?;
            }
        }
        if let Operation::List { page_size, .. } | Operation::Discover { page_size, .. } = operation
            && (*page_size == 0 || *page_size > 256)
        {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "File pages require 1..256 entries",
            ));
        }
        Ok(payload)
    }

    /// Revocation is observable immediately by queued preparation, previews and batch Undo.
    pub(super) fn clear_workspace_resources(&mut self) {
        for alive in self.file_root_lifetimes.values() {
            alive.store(false, Ordering::Release);
        }
        for resource in self.workspace_batches.values() {
            resource.revoke();
            self.roots.remove(resource.handle());
        }
        self.file_root_lifetimes.clear();
        self.workspace_batches.clear();
    }
}
