//! File proposals use exactly one revocable root and the shared path/IO proof implementation.
use super::*;
use crate::AuthorizedFile;

impl NativeDocumentRequests {
    /// Only the service's checked language.editing 1.3 path calls this native issuer.
    pub(crate) fn file_for_refactor(&mut self, relative: &str) -> Result<AuthorizedFile, Failure> {
        self.poll();
        if self.roots.application || self.roots.retired || !self.authority.is_live() {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "No live workspace file authority",
            ));
        }
        if self.file_root.is_none() {
            let api::Value::Resource(handle) = self.roots.open(RootKind::Workspace)? else {
                unreachable!()
            };
            self.file_root = Some((handle, Arc::new(AtomicBool::new(true))));
        }
        let (root, _) = self.file_root.as_ref().unwrap();
        let target = FileTarget {
            root: root.clone(),
            path: relative.into(),
        };
        self.authorize_refactor_file(&target)
    }

    /// File handles from another issuer, instance or retired root never borrow this owner's grants.
    fn authorize_refactor_file(&self, target: &FileTarget) -> Result<AuthorizedFile, Failure> {
        target.validate()?;
        let (handle, alive) = self
            .file_root
            .as_ref()
            .filter(|(handle, _)| *handle == target.root)
            .ok_or_else(|| Failure::new(ErrorCode::InvalidHandle, "Unowned refactor root"))?;
        if target.path.is_empty() || !matches!(self.roots.resolve(handle)?, RootKind::Workspace) {
            return Err(Failure::new(
                ErrorCode::InvalidPath,
                "Cannot mutate workspace root",
            ));
        }
        let root = PathBuf::from(&self.workspace)
            .canonicalize()
            .map_err(crate::native_path::io_failure)?;
        let observed = pin_path(&root, false)?.identities;
        let path = normalized_target(&root.join(&target.path))?;
        if !path.starts_with(&root) {
            return Err(Failure::new(
                ErrorCode::InvalidPath,
                "Refactor target escapes workspace",
            ));
        }
        let file = AuthorizedFile::new(
            target.clone(),
            path,
            root,
            observed,
            self.authority.clone(),
            alive.clone(),
            true,
            true,
            false,
        );
        file.guard()?;
        Ok(file)
    }

    /// Recheck complete file authority before allocating any editor request or recovery handle.
    pub(super) fn plan_files(
        &self,
        plan: &plugin_protocol::workspace::Plan,
        capabilities: &BTreeMap<String, semver::Version>,
        permissions: &BTreeSet<String>,
    ) -> Result<Vec<AuthorizedFile>, Failure> {
        let mut files = Vec::new();
        for change in &plan.changes {
            let targets = match change {
                Change::EditDocument { .. } => continue,
                Change::Rename { source, target, .. } => [source, target],
                _ => {
                    return Err(Failure::new(
                        ErrorCode::UnsupportedOperation,
                        "Unsupported language file operation",
                    ));
                }
            };
            if capabilities
                .get("language.editing")
                .is_none_or(|v| *v < semver::Version::new(1, 3, 0))
                || capabilities
                    .get("workspace.files")
                    .is_none_or(|v| *v < semver::Version::new(1, 4, 0))
                || capabilities
                    .get("editor.documents")
                    .is_none_or(|v| *v < semver::Version::new(1, 1, 0))
                || [
                    "workspace.read",
                    "workspace.write",
                    "editor.read",
                    "editor.write",
                ]
                .iter()
                .any(|permission| !permissions.contains(*permission))
            {
                return Err(Failure::new(
                    ErrorCode::PermissionDenied,
                    "Language file operations were not granted",
                ));
            }
            for target in targets {
                files.push(self.authorize_refactor_file(target)?);
            }
        }
        Ok(files)
    }
}
