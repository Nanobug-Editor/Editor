//! Native refactors acquire bounded, pinned local preimages under the original workspace read grant.
use super::*;
use crate::native_path::{CheckedPath, io_failure, normalized_target, pin_path};
use plugin_protocol::api::{ErrorCode, Failure, ResourceIdentity};
use std::{fs::File, io::Read, path::Path};

/// A temporary immutable source snapshot; no writable document or alternate history is retained.
/// Keep it alive through native session construction. Windows also excludes concurrent writers.
pub struct LanguageSource {
    path: PathBuf,
    text: String,
    authority: crate::EditorAuthority,
    _pins: CheckedPath,
    _file: File,
}
impl LanguageSource {
    /// Canonical checked path, valid only with this original live authority and retained pins.
    pub fn path(&self) -> &Path {
        &self.path
    }
    /// At most 1 MiB of immutable UTF-8, transferred into the ordinary native document model.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Retirement never permits a pending open to create another native source session.
    pub fn is_live(&self) -> bool {
        self.authority.is_live()
    }
}

impl LanguageService {
    /// Read an existing workspace-relative source under language.editing 1.2, editor.read and
    /// workspace.read. Pins reject links, streams, aliases and boundary changes; reads reject
    /// excessive data, non-UTF-8 and concurrent changes. No guest file handle or write grant is issued.
    pub fn read_source(&self, relative: &str) -> Result<LanguageSource, Failure> {
        let state = self.state.lock().unwrap();
        if state.retired || !self.authority.is_live() || !self.registered_context_is_live() {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Refactor source owner retired",
            ));
        }
        if !self.provider.workspace_edits
            || self
                .capability("language.editing")
                .is_none_or(|v| v.major != 1 || v.minor < 2)
            || !self.policy.permissions.contains("editor.read")
            || !self.can_read_workspace()
        {
            return Err(Failure::new(
                ErrorCode::PermissionDenied,
                "Workspace refactor reads were not granted",
            ));
        }
        ResourceIdentity::Local {
            path: relative.into(),
        }
        .validate()?;
        let root = self.policy.workspace.canonicalize().map_err(io_failure)?;
        let target = root.join(relative);
        let pins = pin_path(&target, false)?;
        let path = normalized_target(&target)?;
        if !path.starts_with(&root) {
            return Err(Failure::new(
                ErrorCode::InvalidPath,
                "Refactor source escapes workspace",
            ));
        }
        let mut options = std::fs::OpenOptions::new();
        options.read(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(1);
        }
        let file = options.open(&path).map_err(io_failure)?;
        let before = file.metadata().map_err(io_failure)?;
        if !before.is_file() || before.len() > 1024 * 1024 {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Refactor source exceeds 1 MiB",
            ));
        }
        let mut bytes = Vec::new();
        (&file)
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(io_failure)?;
        let after = file.metadata().map_err(io_failure)?;
        if bytes.len() > 1024 * 1024 {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Refactor source exceeds 1 MiB",
            ));
        }
        if before.len() != after.len() || before.modified().ok() != after.modified().ok() {
            return Err(Failure::new(
                ErrorCode::StaleRevision,
                "Refactor source changed during read",
            ));
        }
        let text = String::from_utf8(bytes).map_err(|_| {
            Failure::new(
                ErrorCode::UnsupportedOperation,
                "Refactor source must be UTF-8",
            )
        })?;
        Ok(LanguageSource {
            path,
            text,
            authority: self.authority.clone(),
            _pins: pins,
            _file: file,
        })
    }
}
