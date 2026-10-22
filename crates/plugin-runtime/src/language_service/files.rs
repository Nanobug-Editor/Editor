//! Native language file proposals borrow the original workspace grants and ordinary plan IO proofs.
use super::*;
use plugin_protocol::api::{ErrorCode, Failure};

impl LanguageService {
    /// Inspect immutable negotiated authority before advertising standard resource refactors.
    /// This metadata does not issue a file proof; actual calls also recheck retirement and scope.
    pub fn can_edit_workspace_files(&self) -> bool {
        self.provider.workspace_edits
            && self
                .capability("language.editing")
                .is_some_and(|v| v.major == 1 && v.minor >= 3)
            && self
                .capability("workspace.files")
                .is_some_and(|v| v.major == 1 && v.minor >= 4)
            && self
                .capability("editor.documents")
                .is_some_and(|v| v.major == 1 && v.minor >= 1)
            && [
                "workspace.read",
                "workspace.write",
                "editor.read",
                "editor.write",
            ]
            .iter()
            .all(|permission| self.policy.permissions.contains(*permission))
            && !self.policy.application
    }

    /// Issue an exact workspace-relative native file proof for a previewed refactor. `relative`
    /// uses portable slash-separated segments. Keep the proof until ordinary plan admission;
    /// it pins root identity, not file content, and supplies no bypass around disk stamps or preview.
    /// Missing grants return PermissionDenied, retired ownership InvalidHandle, and links, streams
    /// or escapes InvalidPath. Host consumers must use guard() through every subsequent IO.
    pub fn refactor_file(&self, relative: &str) -> Result<crate::AuthorizedFile, Failure> {
        let state = self.state.lock().unwrap();
        if state.retired || !self.authority.is_live() || !self.registered_context_is_live() {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Refactor file owner retired",
            ));
        }
        if !self.can_edit_workspace_files() {
            return Err(Failure::new(
                ErrorCode::PermissionDenied,
                "File refactors were not granted",
            ));
        }
        self.native_requests
            .lock()
            .unwrap()
            .file_for_refactor(relative)
    }
}
