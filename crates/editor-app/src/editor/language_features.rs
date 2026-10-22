//! Shared native language actions validate source/owner before navigation or ordinary plan admission.
use crate::*;
use plugin_runtime::{
    LanguageFixOrigin,
    plugin_protocol::{api, diagnostics, direct},
};
use std::io::Read as _;
mod requests;
pub(crate) use requests::State;

impl EditorApp {
    /// Opening a reference never reads disk instead of an already open dirty native document.
    /// The sealed original owner must still be live; foreign workspace files need its explicit grant.
    pub(crate) fn navigate_language_location(
        &mut self,
        origin: &LanguageFixOrigin,
        document: &api::DocumentVersion,
        location: direct::Location,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), api::Failure> {
        if !self.session_state.workspace_trusted || !origin.is_live() {
            return Err(api::Failure::new(
                api::ErrorCode::PermissionDenied,
                "Language result owner was revoked",
            ));
        }
        if !(0..self.tabs.len())
            .any(|index| self.plugin_document_version(index).ok().as_ref() == Some(document))
        {
            return Err(api::Failure::new(
                api::ErrorCode::StaleRevision,
                "Language source changed or closed",
            ));
        }
        location.resource.validate()?;
        let api::ResourceIdentity::Local { path } = location.resource else {
            return Err(api::Failure::new(
                api::ErrorCode::UnsupportedOperation,
                "Language targets must be local workspace documents",
            ));
        };
        if path != document.path && !origin.can_read_workspace() {
            return Err(api::Failure::new(
                api::ErrorCode::PermissionDenied,
                "Related document requires workspace.read",
            ));
        }
        let proposed = self.workspace.root().join(&path);
        let existing = self.tabs.iter().position(|tab| tab.path() == proposed);
        let target = if existing.is_some() {
            proposed
        } else {
            let target = proposed.canonicalize().map_err(|error| {
                api::Failure::new(api::ErrorCode::OperationFailed, error.to_string())
            })?;
            if !target.starts_with(self.workspace.root()) || !target.is_file() {
                return Err(api::Failure::new(
                    api::ErrorCode::InvalidPath,
                    "Language target escaped workspace",
                ));
            }
            target
        };
        // Check the actual target before moving focus. Open buffers retain unsaved text; only a
        // previously unopened file needs one bounded native read through the normal open path.
        if let Some(index) = existing {
            let tab = self.tabs[index].text.as_ref().ok_or_else(|| {
                api::Failure::new(
                    api::ErrorCode::UnsupportedOperation,
                    "Language target is not text",
                )
            })?;
            location
                .position
                .offset_chars(tab.editor.read(cx).text().chars())?;
        } else {
            let mut bytes = Vec::new();
            std::fs::File::open(&target)
                .and_then(|file| file.take(1024 * 1024 + 1).read_to_end(&mut bytes))
                .map_err(|error| {
                    api::Failure::new(api::ErrorCode::OperationFailed, error.to_string())
                })?;
            if bytes.len() > 1024 * 1024 {
                return Err(api::Failure::new(
                    api::ErrorCode::LimitExceeded,
                    "Unopened reference target exceeds 1 MiB",
                ));
            }
            let text = std::str::from_utf8(&bytes).map_err(|_| {
                api::Failure::new(
                    api::ErrorCode::UnsupportedOperation,
                    "Language target must be UTF-8",
                )
            })?;
            location.position.offset(text)?;
        }
        if let LanguageFixOrigin::Lsp(service) | LanguageFixOrigin::GuardedLsp { service, .. } =
            origin
        {
            self.inherit_source_language_context(service, &target, cx)?;
        }
        self.open_file(target, window, cx);
        let index = self.active_text_tab_index().ok_or_else(|| {
            api::Failure::new(
                api::ErrorCode::OperationFailed,
                "Language target could not be opened",
            )
        })?;
        let editor = self.tabs[index].text.as_ref().unwrap().editor.clone();
        // Opening can observe a newer disk value. Re-resolve against the actual native session.
        let offset = location
            .position
            .offset_chars(editor.read(cx).text().chars())?;
        editor.update(cx, |state, cx| {
            let position = state.text().offset_to_position(offset);
            state.unfold_at(position, cx);
            state.set_cursor_position(position, window, cx);
        });
        editor.focus_handle(cx).focus(window, cx);
        Ok(())
    }

    /// User-selected proposals return to the original instance's normal permissions and versioned
    /// workspace plan queue. This bridge neither edits text nor supplies host-owned file grants.
    pub(crate) fn submit_native_language_fix(
        &mut self,
        origin: LanguageFixOrigin,
        document: api::DocumentVersion,
        fix: diagnostics::Fix,
        cx: &mut Context<Self>,
    ) -> Result<(), api::Failure> {
        if !self.session_state.workspace_trusted || !origin.is_live() {
            return Err(api::Failure::new(
                api::ErrorCode::PermissionDenied,
                "Language fix owner was revoked",
            ));
        }
        if !(0..self.tabs.len())
            .any(|index| self.plugin_document_version(index).ok().as_ref() == Some(&document))
        {
            return Err(api::Failure::new(
                api::ErrorCode::StaleRevision,
                "Language fix source changed or closed",
            ));
        }
        fix.plan.validate()?;
        self.extensions
            .read(cx)
            .submit_language_fix(origin, document, fix)
    }
}
