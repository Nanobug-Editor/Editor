//! Controlled native language owners issue the same document-plan request and recovery resources
//! as guests. This bounded arena retains no text, editable document model or separate Undo history.
use super::*;
use crate::EditorRequest;
use std::collections::BTreeMap;

/// The original service incarnation owns both transient requests and separately retained receipts.
/// RootKinds remain private to the normal resource issuer; no caller can manufacture file grants.
pub(crate) struct NativeDocumentRequests {
    roots: resource_roots::ResourceRoots,
    workspace: String,
    data: PathBuf,
    authority: EditorAuthority,
    pending: BTreeMap<u64, (EditorRequest, bool)>,
    batches: BTreeMap<u64, Arc<WorkspaceBatch>>,
}

impl NativeDocumentRequests {
    /// Create one real service owner, with its immutable workspace and installation data metadata.
    /// It issues only document handles; zero private-file quota cannot become file-root authority.
    pub(crate) fn new(
        workspace: &Path,
        data: PathBuf,
        application: bool,
        authority: EditorAuthority,
    ) -> Self {
        let workspace = workspace.display().to_string();
        Self {
            roots: resource_roots::ResourceRoots::new(&workspace, application, 0),
            workspace,
            data,
            authority,
            pending: BTreeMap::new(),
            batches: BTreeMap::new(),
        }
    }

    /// Current-document text plans use original negotiated contracts and consent, including the
    /// readonly source condition. Resource, foreign-document and application-scope effects fail
    /// before allocation; the native host still validates revision, previews and asks for consent.
    pub(crate) fn submit(
        &mut self,
        plan: plugin_protocol::workspace::Plan,
        capabilities: &BTreeMap<String, semver::Version>,
        permissions: &BTreeSet<String>,
    ) -> Result<api::Value, Failure> {
        self.poll();
        self.check_plan(&plan, capabilities, permissions)?;
        self.issue(plan)
    }

    /// Shared document admission precedes native-only scope and shape restrictions; none of
    /// these checks allocates a request, file proof or recovery resource on rejection.
    fn check_plan(
        &self,
        plan: &plugin_protocol::workspace::Plan,
        capabilities: &BTreeMap<String, semver::Version>,
        permissions: &BTreeSet<String>,
    ) -> Result<(), Failure> {
        if self.roots.retired || !self.authority.is_live() {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Native language owner retired",
            ));
        }
        if self.roots.application || self.workspace.is_empty() {
            return Err(Failure::new(
                ErrorCode::PermissionDenied,
                "No workspace document authority",
            ));
        }
        if capabilities
            .get("language.lsp")
            .is_none_or(|version| *version < semver::Version::new(1, 4, 0))
        {
            return Err(Failure::new(
                ErrorCode::CapabilityUnavailable,
                "Language actions require language.lsp 1.4",
            ));
        }
        crate::workspace_resources::validate_plan_documents(plan, capabilities, |permission| {
            if permissions.contains(permission) {
                Ok(())
            } else {
                Err(Failure::new(
                    ErrorCode::PermissionDenied,
                    format!("{permission} was not granted"),
                ))
            }
        })?;
        let document = plan.origin.as_ref().ok_or_else(|| {
            Failure::new(
                ErrorCode::InvalidRequest,
                "Language action requires its original source",
            )
        })?;
        api::ResourceIdentity::Local {
            path: document.path.clone(),
        }
        .validate()?;
        if plan.changes.iter().any(|change| {
            !matches!(change,
            Change::EditDocument { document: target, .. } if target == document)
        }) {
            return Err(Failure::new(
                ErrorCode::UnsupportedOperation,
                "Native language actions support only their current document",
            ));
        }
        Ok(())
    }

    /// Allocate both handles only after the complete proposal is admitted and capacity preflight
    /// guarantees the pair. The returned ordinary request owns preview and native group recovery.
    fn issue(&mut self, plan: plugin_protocol::workspace::Plan) -> Result<api::Value, Failure> {
        if self.pending.len() >= 32 || self.batches.len() >= 8 || self.roots.len() >= 127 {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Native owner request or recovery quota exceeded",
            ));
        }
        let api::Value::Resource(batch_handle) = self.roots.open(RootKind::WorkspaceBatch)? else {
            unreachable!()
        };
        let batch = Arc::new(WorkspaceBatch::new(
            batch_handle.clone(),
            self.authority.clone(),
            None,
            BTreeSet::from(["editor.read".into(), "editor.write".into()]),
        ));
        let api::Value::Resource(handle) = self.roots.open(RootKind::EditorRequest)? else {
            unreachable!()
        };
        let request = EditorRequest::from_authority(
            handle.clone(),
            api::EditorOperation::Workspace {
                operation: Operation::ApplyPlan { plan },
            },
            self.workspace.clone(),
            self.data.clone(),
            120_000,
            self.authority.clone(),
        )
        .with_workspace_request(WorkspaceRequest {
            batch: Some(batch.clone()),
            can_edit: true,
            // The origin permits a version condition, not arbitrary document reads.
            can_read_documents: false,
            files: Vec::new(),
        });
        self.batches.insert(batch_handle.resource, batch);
        self.pending.insert(handle.resource, (request, false));
        Ok(api::Value::BatchAccepted {
            request: handle,
            batch: batch_handle,
        })
    }

    /// Transfer each request exactly once through the public Manager actor path.
    pub(crate) fn take(&mut self) -> Vec<EditorRequest> {
        self.poll();
        self.pending
            .values_mut()
            .filter_map(|(request, sent)| {
                if *sent {
                    None
                } else {
                    *sent = true;
                    Some(request.clone())
                }
            })
            .collect()
    }

    /// Terminal calls release transient slots. A recovery stays alive while a native receipt or
    /// request retains it; dropping its last native holder releases that owner-only resource.
    pub(crate) fn poll(&mut self) {
        if !self.authority.is_live() {
            self.retire();
            return;
        }
        self.pending.retain(|_, (request, _)| {
            if request.status().is_terminal() {
                self.roots.remove(request.handle());
                false
            } else {
                true
            }
        });
        self.batches.retain(|_, batch| {
            if !batch.is_live() || Arc::strong_count(batch) == 1 {
                batch.revoke();
                self.roots.remove(batch.handle());
                false
            } else {
                true
            }
        });
    }

    /// Retirement seals queued work and native group recovery immediately, without requiring a
    /// manager tick or a guest notification. Published clones observe the same completion gate.
    pub(crate) fn retire(&mut self) {
        for (request, _) in self.pending.values() {
            request.retire();
        }
        for batch in self.batches.values() {
            batch.revoke();
        }
        self.pending.clear();
        self.batches.clear();
        self.roots.retire();
    }

    /// Shutdown diagnostics count this real owner in addition to its controlled process.
    pub(crate) fn resource_count(&self) -> usize {
        self.roots.len()
    }
}

impl Drop for NativeDocumentRequests {
    fn drop(&mut self) {
        self.retire();
    }
}
