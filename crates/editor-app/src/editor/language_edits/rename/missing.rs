//! Closed refactor targets become bounded native sources before a fresh semantic request is allowed.
use super::*;
use plugin_runtime::plugin_protocol::api::{ErrorCode, Failure};

impl EditorApp {
    /// Discovery never supplies an editable preimage. Admit all targets first, retain their normal
    /// native sessions, then wait for exact transport ownership before recomputing the complete rename.
    pub(super) fn rename_missing_sources(
        &mut self,
        round: Round,
        targets: Vec<lsp_types::Uri>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), Failure> {
        if self.language_edits.rename_discoveries >= 4 || targets.len() + round.revisions.len() > 64
        {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Workspace refactor discovery budget exceeded",
            ));
        }
        let plugin_runtime::LanguageFixOrigin::Lsp(service) = round.form.server.fix_origin() else {
            unreachable!()
        };
        if !service.can_read_workspace() {
            return Err(Failure::new(
                ErrorCode::PermissionDenied,
                "Related source opens require workspace.read",
            ));
        }
        let mut files = Vec::new();
        let mut paths = Vec::new();
        let mut bytes = round.source_bytes;
        for uri in targets {
            let path = url::Url::parse(uri.as_str())
                .ok()
                .and_then(|url| url.to_file_path().ok())
                .and_then(|path| path.canonicalize().ok())
                .ok_or_else(|| {
                    Failure::new(ErrorCode::InvalidPath, "Invalid refactor source URI")
                })?;
            let relative = path
                .strip_prefix(self.workspace.root())
                .map_err(|_| {
                    Failure::new(ErrorCode::InvalidPath, "Refactor source escapes workspace")
                })?
                .to_string_lossy()
                .replace('\\', "/");
            if !self.tabs.iter().any(|tab| tab.path() == path) {
                let file = service.read_source(&relative)?;
                bytes += file.text().len();
                if bytes > 8 * 1024 * 1024 {
                    return Err(Failure::new(
                        ErrorCode::LimitExceeded,
                        "Refactor source budget exceeded",
                    ));
                }
                files.push(file);
            }
            paths.push(path);
        }
        for file in &files {
            self.inherit_source_language_context(&service, file.path(), cx)?;
            self.open_refactor_source(file, window, cx)?;
        }
        drop(files);
        self.language_edits.rename_discoveries += 1;
        self.language_edits.rename_pending = true;
        let mut form = Some(round.form);
        let request = round.request;
        let deadline = Instant::now() + Duration::from_secs(30);
        cx.spawn_in(window, async move |this, cx| {
            loop {
                let done = this
                    .update_in(cx, |app, window, cx| {
                        if app.language_edits.rename_request != request {
                            return true;
                        }
                        let original = form.as_ref().unwrap();
                        if !original.server.is_active()
                            || !(0..app.tabs.len()).any(|index| {
                                app.plugin_document_version(index).ok().as_ref()
                                    == Some(&original.document)
                            })
                        {
                            app.language_edits.rename_pending = false;
                            return true;
                        }
                        let all_ready = paths.iter().all(|path| {
                            app.language_server_for_path(path).is_some_and(|server| {
                                Arc::ptr_eq(server, &original.server)
                                    && navigation::file_uri(path)
                                        .is_some_and(|uri| server.document(&uri).is_some())
                            })
                        });
                        if all_ready {
                            app.language_edits.rename = form.take();
                            app.confirm_rename(window, cx);
                            return true;
                        }
                        if Instant::now() >= deadline {
                            app.language_edits.rename_pending = false;
                            app.status = t!(
                                "editor.editing_failed",
                                error = "Related source context is unavailable"
                            )
                            .to_string();
                            cx.notify();
                            return true;
                        }
                        false
                    })
                    .unwrap_or(true);
                if done {
                    break;
                }
                cx.background_executor()
                    .timer(Duration::from_millis(25))
                    .await;
            }
        })
        .detach();
        Ok(())
    }
}
