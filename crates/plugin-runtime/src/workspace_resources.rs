//! Runtime-issued file and batch leases keep native IO under the original instance and root authority.

use crate::{
    EditorAuthority,
    native_path::{CheckedPath, pin_path},
};
use plugin_protocol::{
    api::{ErrorCode, Failure, ResourceHandle},
    workspace::{BatchState, FileTarget, Receipt},
};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

/// Admit document preconditions and edits under the original capability/grant set. Native language
/// leases and WASM instances share this pure gate before either allocates requests or recovery slots.
/// File targets still require their separate per-root checks in the owning instance.
pub(crate) fn validate_plan_documents(
    plan: &plugin_protocol::workspace::Plan,
    capabilities: &std::collections::BTreeMap<String, semver::Version>,
    permission: impl Fn(&str) -> Result<(), Failure>,
) -> Result<(), Failure> {
    let require = |name: &str, minor| {
        if capabilities
            .get(name)
            .is_none_or(|version| *version < semver::Version::new(1, minor, 0))
        {
            Err(Failure::new(
                ErrorCode::CapabilityUnavailable,
                format!("{name} 1.{minor} was not negotiated"),
            ))
        } else {
            Ok(())
        }
    };
    require("workspace.files", 2)?;
    if plan.origin.is_some() {
        require("workspace.files", 3)?;
        permission("editor.read")?;
    }
    plan.validate()?;
    if plan.changes.iter().any(|change| {
        matches!(
            change,
            plugin_protocol::workspace::Change::EditDocument { .. }
        )
    }) {
        require("editor.edit", 1)?;
        permission("editor.write")?;
    }
    Ok(())
}

/// Bind a result to its immutable native source without fabricating a noop source edit.
/// Both instance and declarative service ingress use the same readonly Plan.origin condition.
pub(crate) fn language_plan(
    document: plugin_protocol::api::DocumentVersion,
    mut plan: plugin_protocol::workspace::Plan,
) -> Result<plugin_protocol::workspace::Plan, Failure> {
    if plan
        .origin
        .as_ref()
        .is_some_and(|origin| origin != &document)
    {
        return Err(Failure::new(
            ErrorCode::InvalidRequest,
            "Language plan origin does not match its result source",
        ));
    }
    plan.origin = Some(document);
    Ok(plan)
}

/// A temporary ancestor pin. Keeping it alive through IO prevents Windows link/rename redirection.
pub struct FileGuard {
    _pins: CheckedPath,
}

/// Native paths are issued only after handle ownership and read/write intent checks; guests cannot create one.
#[derive(Clone)]
pub struct AuthorizedFile {
    target: FileTarget,
    path: PathBuf,
    root: PathBuf,
    observed: Vec<(u64, u64, u64)>,
    authority: EditorAuthority,
    root_alive: Arc<AtomicBool>,
    read: bool,
    write: bool,
    save_exact: bool,
}

impl AuthorizedFile {
    /// Only the serial owning instance constructs proof; paths never cross the guest transport.
    pub(crate) fn new(
        target: FileTarget,
        path: PathBuf,
        root: PathBuf,
        observed: Vec<(u64, u64, u64)>,
        authority: EditorAuthority,
        root_alive: Arc<AtomicBool>,
        read: bool,
        write: bool,
        save_exact: bool,
    ) -> Self {
        Self {
            target,
            path,
            root,
            observed,
            authority,
            root_alive,
            read,
            write,
            save_exact,
        }
    }
    /// Return the original portable target for receipts; this reference grants no new authority.
    pub fn target(&self) -> &FileTarget {
        &self.target
    }
    /// Host-only native target. Keep guard() alive through IO; guests receive only target().
    pub fn path(&self) -> &Path {
        &self.path
    }
    /// Host-only containment root whose immutable identity is rechecked by guard().
    pub fn root(&self) -> &Path {
        &self.root
    }
    /// Whether the originally admitted guest direction includes content reads, excluding Save.
    pub fn can_read(&self) -> bool {
        self.read
    }
    /// Whether this original request was admitted for a previewed mutation.
    pub fn can_write(&self) -> bool {
        self.write
    }
    /// Save grants host validation/recovery of this exact object, never guest content reads.
    pub fn is_save_intent(&self) -> bool {
        self.save_exact
    }
    /// Release, instance retirement or delegated-source revocation invalidates this proof immediately.
    pub fn is_live(&self) -> bool {
        self.authority.is_live() && self.root_alive.load(Ordering::Acquire)
    }

    /// Revalidate every existing component immediately before IO and retain stable ancestor handles.
    /// Planned creations may have a missing tail; directory/stream/device aliases are still rejected.
    pub fn guard(&self) -> Result<FileGuard, Failure> {
        if !self.is_live() {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "File authority was revoked",
            ));
        }
        let mut pins = pin_path(&self.path, true)?;
        if !pins.identities.starts_with(&self.observed) {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "File root or ancestor was replaced",
            ));
        }
        // The exact leaf's content/identity is separately checked by the plan's disk stamp.
        // Its sharing lock must not obstruct a legitimate atomic replacement or resource rename.
        if self.path != self.root {
            pins.release_leaf();
        }
        Ok(FileGuard { _pins: pins })
    }
}

/// Current-session receipts survive cancellation of the wait; release and retirement revoke the resource.
pub struct WorkspaceBatch {
    handle: ResourceHandle,
    authority: EditorAuthority,
    released: AtomicBool,
    caller: Option<String>,
    required: std::collections::BTreeSet<String>,
    receipt: Mutex<Receipt>,
}

impl WorkspaceBatch {
    pub(crate) fn new(
        handle: ResourceHandle,
        authority: EditorAuthority,
        caller: Option<String>,
        required: std::collections::BTreeSet<String>,
    ) -> Self {
        let receipt = Receipt {
            batch: handle.clone(),
            state: BatchState::Preparing,
            completed: Vec::new(),
            stopped_at: None,
            affected: Vec::new(),
            failed: None,
            conflicts: Vec::new(),
            recoverable: false,
        };
        Self {
            handle,
            authority,
            released: AtomicBool::new(false),
            caller,
            required,
            receipt: Mutex::new(receipt),
        }
    }
    /// Return the current-session guest slot used for BatchStatus, UndoBatch and CloseResource.
    pub fn handle(&self) -> &ResourceHandle {
        &self.handle
    }
    /// Recovery cannot outlive its original instance, delegated source or explicitly released slot.
    pub fn is_live(&self) -> bool {
        self.authority.is_live() && !self.released.load(Ordering::Acquire)
    }
    pub(crate) fn caller(&self) -> Option<&str> {
        self.caller.as_deref()
    }
    pub(crate) fn required(&self) -> &std::collections::BTreeSet<String> {
        &self.required
    }
    /// A read is an immutable receipt copy; no native text or separate Undo history is stored here.
    pub fn receipt(&self) -> Receipt {
        self.receipt.lock().unwrap().clone()
    }
    /// A native coordinator reports actual effects even when the original wait already stopped.
    pub fn publish(&self, receipt: Receipt) {
        *self.receipt.lock().unwrap() = receipt;
    }
    pub(crate) fn revoke(&self) {
        self.released.store(true, Ordering::Release);
    }
}

/// Every native operation receives only the targets admitted under its original checked permission set.
#[derive(Clone, Default)]
pub struct WorkspaceRequest {
    pub(crate) files: Vec<AuthorizedFile>,
    pub(crate) batch: Option<Arc<WorkspaceBatch>>,
    pub(crate) can_edit: bool,
    pub(crate) can_read_documents: bool,
}

impl WorkspaceRequest {
    /// File roots do not implicitly grant access to unsaved native document text or history.
    pub fn can_edit_documents(&self) -> bool {
        self.can_edit
    }
    pub fn can_read_documents(&self) -> bool {
        self.can_read_documents
    }
    /// A lookup enforces the exact admitted target and required direction, without reconstructing authority.
    pub fn file(&self, target: &FileTarget, write: bool) -> Result<&AuthorizedFile, Failure> {
        self.files
            .iter()
            .find(|file| {
                file.target() == target
                    && if write {
                        file.can_write()
                    } else {
                        file.can_read()
                    }
            })
            .ok_or_else(|| {
                Failure::new(
                    ErrorCode::PermissionDenied,
                    "File target was not admitted for this access",
                )
            })
    }
    /// Plans/status/recovery own a batch; read-only discovery requests deliberately do not.
    pub fn batch(&self) -> Option<&Arc<WorkspaceBatch>> {
        self.batch.as_ref()
    }
    /// Native preparation receives a readonly slice of the original sealed proofs, including Save intent.
    pub fn files(&self) -> &[AuthorizedFile] {
        &self.files
    }
}
