//! Unified resource operations keep virtual text inside ordinary native document tabs.
use super::tabs::VirtualTab;
use crate::*;
use editor_core::DocumentSession;
use plugin_runtime::{
    EditorRequest,
    plugin_protocol::api::{self, EditorOperation as Op, EditorValue as Value, ErrorCode, Failure},
};
mod content;
mod sources;

impl EditorApp {
    /// Global edit/save commands use actual pane focus and reject provider-owned readonly sessions.
    pub(crate) fn reject_readonly_document_action(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let readonly = self.active_text_tab_index().is_some_and(|index| {
            self.tabs[index]
                .text
                .as_ref()
                .unwrap()
                .session
                .is_readonly()
        }) || self.comparison_readonly_has_focus(window, cx);
        if readonly {
            self.report_host_message(
                app::messages::MessageLevel::Warning,
                t!("status.readonly_document").to_string(),
                cx,
            );
        }
        readonly
    }

    /// A retained request must still own every virtual target when it reaches the UI thread.
    pub(crate) fn check_virtual_request(&self, request: &EditorRequest) -> Result<(), Failure> {
        let check = |document: &api::DocumentVersion| {
            if let Some(tab) = self
                .tabs
                .iter()
                .find(|tab| tab.text_document_id().as_ref() == Some(&document.id))
            {
                if let Some(virtual_tab) = &tab.virtual_document {
                    let handle = virtual_tab.resource.handle();
                    if !virtual_tab.resource.is_live() {
                        return Err(Failure::new(
                            ErrorCode::InvalidHandle,
                            "Virtual document was revoked",
                        ));
                    }
                    if handle.instance != request.handle().instance
                        || handle.scope != request.handle().scope
                    {
                        return Err(Failure::new(
                            ErrorCode::PermissionDenied,
                            "Virtual document belongs to another instance",
                        ));
                    }
                }
            }
            Ok(())
        };
        match request.operation() {
            Op::ReadDocument { document, .. }
            | Op::RefreshVirtualDocument { document, .. }
            | Op::LocateDocument { document, .. }
            | Op::SaveDocument { document }
            | Op::ReadDocumentSelection { document }
            | Op::ReplaceDocumentRange { document, .. } => check(document),
            Op::CompareDocuments { left, right } => {
                check(left)?;
                check(right)
            }
            Op::OpenDocumentComparison {
                left, right, third, ..
            } => {
                check(left)?;
                check(right)?;
                if let Some(third) = third {
                    check(third)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Local opens recheck canonical physical containment; virtual opens require issued live authority.
    pub(crate) fn open_plugin_resource(
        &mut self,
        resource: &api::ResourceIdentity,
        request: &EditorRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Value, Failure> {
        let index = match resource {
            api::ResourceIdentity::Local { path } => {
                resource.validate()?;
                let target = self
                    .workspace
                    .root()
                    .join(path)
                    .canonicalize()
                    .map_err(|error| Failure::new(ErrorCode::NotFound, error.to_string()))?;
                if !target.starts_with(self.workspace.root()) {
                    return Err(Failure::new(
                        ErrorCode::PermissionDenied,
                        "Document escapes its workspace",
                    ));
                }
                if !target.is_file() {
                    return Err(Failure::new(
                        ErrorCode::UnsupportedOperation,
                        "Resource is not a text file",
                    ));
                }
                if !request.enter_side_effect() {
                    return Err(Failure::new(
                        ErrorCode::Cancelled,
                        "Document open did not execute",
                    ));
                }
                self.open_file(target.clone(), window, cx);
                self.tabs
                    .iter()
                    .position(|tab| tab.path() == target)
                    .ok_or_else(|| {
                        Failure::new(ErrorCode::OperationFailed, "Document could not be opened")
                    })?
            }
            api::ResourceIdentity::Virtual { handle } => {
                let authority = request
                    .virtual_document()
                    .filter(|authority| authority.handle() == handle && authority.is_live())
                    .ok_or_else(|| {
                        Failure::new(ErrorCode::InvalidHandle, "Virtual resource was revoked")
                    })?;
                self.tabs
                    .iter()
                    .position(|tab| {
                        tab.virtual_document.as_ref().is_some_and(|virtual_tab| {
                            virtual_tab.resource.handle() == authority.handle()
                        })
                    })
                    .ok_or_else(|| {
                        Failure::new(ErrorCode::InvalidHandle, "Virtual document is closed")
                    })?
            }
        };
        if matches!(resource, api::ResourceIdentity::Virtual { .. }) && !request.enter_side_effect()
        {
            return Err(Failure::new(
                ErrorCode::Cancelled,
                "Document open did not execute",
            ));
        }
        if let Some(virtual_tab) = &mut self.tabs[index].virtual_document {
            virtual_tab.hidden = false;
        }
        self.activate_tab(index, window, cx);
        self.plugin_document_info(index, cx)
            .map(Value::DocumentOpened)
    }

    /// Provider text has no disk file, watcher, history entry, separate analysis session or restoration record.
    pub(crate) fn open_plugin_virtual(
        &mut self,
        title: &str,
        language: Option<&str>,
        text: &str,
        request: &EditorRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Value, Failure> {
        let resource = request
            .virtual_document()
            .filter(|resource| resource.is_live())
            .ok_or_else(|| Failure::new(ErrorCode::InvalidHandle, "Virtual resource was revoked"))?
            .clone();
        self.check_content_origin(&resource)?;
        if !request.enter_side_effect() {
            return Err(Failure::new(
                ErrorCode::Cancelled,
                "Virtual open did not execute",
            ));
        }
        if let Some(index) = self.find_language_content_tab(&resource) {
            self.refresh_language_content_tab(index, text, window, cx);
            self.activate_tab(index, window, cx);
            self.reveal_language_content(&resource, window, cx)?;
            return self
                .plugin_document_info(index, cx)
                .map(Value::DocumentOpened);
        }
        let path = PathBuf::from(resource.uri());
        // The resource URI is opaque and has no source extension. Keep recognition separate from
        // identity, without reading that path or attaching language servers to a readonly snapshot.
        let language_path = match request.operation() {
            Op::OpenVirtualSource { language_path, .. } if language.is_none() => {
                language_path.clone()
            }
            _ => None,
        };
        let language = language.map(str::to_owned).unwrap_or_else(|| {
            language_path.as_ref().map_or_else(
                || "text".into(),
                |path| self.language_for_path(Path::new(path)),
            )
        });
        let cached = matches!(request.operation(), Op::OpenVirtualSource { .. })
            .then(|| {
                self.comparison_source_cache.take(
                    request.authority(),
                    self.providers.feature_generation(),
                    &language,
                    text,
                    cx,
                )
            })
            .flatten();
        let _timing = super::comparison::timing::Span::new(if cached.is_some() {
            "source-reused"
        } else {
            "source-new"
        });
        let native = if let Some(mut native) = cached {
            // A fresh document identity owns the existing readonly buffer; old handles remain revoked.
            native.session = DocumentSession::readonly_resource(&path, text.to_owned()).session;
            native.capability_revision = 0;
            native.editor.update(cx, |editor, cx| {
                editor.set_selected_range(0..0, cx);
                editor.set_scroll_offset(point(px(0.), px(0.)), cx);
            });
            native
        } else {
            let native = self.create_native_text_tab(
                DocumentSession::readonly_resource(&path, text.to_owned()),
                window,
                cx,
            );
            native.editor.update(cx, |editor, cx| {
                editor.set_highlighter(language.clone(), cx)
            });
            native
        };
        self.tabs.push(OpenTab {
            preview: false,
            source_only: None,
            path,
            file_id: NEXT_FILE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            file_revision: 0,
            opened_at: Instant::now(),
            file_digest: None,
            text: Some(native),
            file_error: None,
            virtual_document: Some(VirtualTab {
                owner: request.authority().clone(),
                hidden: matches!(request.operation(), Op::OpenVirtualSource { .. }),
                resource: resource.clone(),
                title: title.into(),
                language,
                language_path,
            }),
        });
        let index = self.tabs.len() - 1;
        if !matches!(request.operation(), Op::OpenVirtualSource { .. }) {
            self.activate_tab(index, window, cx);
        }
        let info = self.plugin_document_info(index, cx)?;
        resource.bind(info.document.clone());
        self.reveal_language_content(&resource, window, cx)?;
        Ok(Value::DocumentOpened(info))
    }

    /// Silent readonly replacement advances the session/version explicitly because Base set_value suppresses Change.
    pub(crate) fn refresh_plugin_virtual(
        &mut self,
        request: EditorRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Op::RefreshVirtualDocument { document, text } = request.operation().clone() else {
            unreachable!()
        };
        let result = (|| {
            let index = (0..self.tabs.len())
                .find(|index| {
                    self.plugin_document_version(*index)
                        .is_ok_and(|version| version == document)
                })
                .ok_or_else(|| {
                    Failure::new(
                        ErrorCode::StaleRevision,
                        "Virtual document changed or closed",
                    )
                })?;
            if self.tabs[index].virtual_document.is_none() {
                return Err(Failure::new(
                    ErrorCode::UnsupportedOperation,
                    "Only virtual content may be refreshed",
                ));
            }
            if !request.enter_side_effect() {
                return Err(Failure::new(
                    ErrorCode::Cancelled,
                    "Refresh did not execute",
                ));
            }
            let tab = self.tabs[index].text.as_mut().unwrap();
            // Base clears the readonly display's IME/selection/undo state, without emitting a user edit.
            // The refresh still returns a new capability version, but equal bytes keep layout and grammar.
            if !tab.editor.read(cx).text().chars().eq(text.chars()) {
                tab.editor
                    .update(cx, |editor, cx| editor.set_value(text, window, cx));
            }
            tab.capability_revision = tab.capability_revision.saturating_add(1);
            // Publish one stable version for this silent refresh before its observer runs.
            tab.capability_content =
                super::native_text::NativeTextIdentity::new(tab.editor.read(cx).text());
            tab.session.accept_disk_reload();
            let info = self.plugin_document_info(index, cx)?;
            self.tabs[index]
                .virtual_document
                .as_ref()
                .unwrap()
                .resource
                .bind(info.document.clone());
            self.sync_plugin_documents(cx);
            Ok(Value::DocumentOpened(info))
        })();
        request.finish(result);
        cx.notify();
    }

    /// Runtime retirement revokes retained authority immediately; the next native frame removes its tab.
    pub(crate) fn sync_virtual_documents(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.comparison_source_cache
            .prune(self.providers.feature_generation());
        let revoked = self
            .tabs
            .iter()
            .enumerate()
            .filter(|(_, tab)| {
                tab.virtual_document
                    .as_ref()
                    .is_some_and(|virtual_tab| !virtual_tab.resource.is_live())
            })
            .map(|(index, tab)| (index, tab.path().to_path_buf()))
            .collect::<Vec<_>>();
        if !revoked.is_empty() {
            self.editor_panel.update(cx, |_, cx| cx.notify());
        }
        for (index, path) in revoked.into_iter().rev() {
            self.remove_tab(index, path, window, cx);
        }
        self.sync_prepared_document_sources(window, cx);
    }

    /// Strict UTF-16 conversion rejects split surrogate positions before changing native selection.
    pub(crate) fn locate_plugin_resource(
        &mut self,
        document: &api::DocumentVersion,
        position: api::TextPosition,
        request: &EditorRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Value, Failure> {
        let index = (0..self.tabs.len())
            .find(|index| {
                self.plugin_document_version(*index)
                    .is_ok_and(|version| version == *document)
            })
            .ok_or_else(|| Failure::new(ErrorCode::StaleRevision, "Document changed or closed"))?;
        let editor = self.tabs[index].text.as_ref().unwrap().editor.clone();
        let offset = position.offset_chars(editor.read(cx).text().chars())?;
        if !request.enter_side_effect() {
            return Err(Failure::new(
                ErrorCode::Cancelled,
                "Location did not execute",
            ));
        }
        self.activate_tab(index, window, cx);
        editor.update(cx, |editor, cx| {
            editor.set_selected_range(offset..offset, cx)
        });
        Ok(Value::DocumentLocated {
            document: document.clone(),
        })
    }
}
