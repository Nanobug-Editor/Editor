//! Library navigation preserves provider URI identity without disk files or editable copies.
use super::*;
use gpui_base::input::RopeExt;
use gpui_kit::EntityInputHandler as _;
use plugin_runtime::plugin_protocol::language_content::Location;

impl EditorApp {
    /// Capture the initiating native revision before retrieving one declared non-file definition.
    pub(crate) fn open_language_content(
        &mut self,
        service: &Arc<plugin_runtime::LanguageService>,
        uri: &lsp_types::Uri,
        selection: Option<lsp_types::Range>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(index) = self.active_text_tab_index() else {
            return false;
        };
        let Ok(document) = self.plugin_document_version(index) else {
            return false;
        };
        let Some(server) = self
            .language_server_for_path(self.tabs[index].path())
            .filter(|server| server.uses_service(service))
            .cloned()
        else {
            return false;
        };
        let Some(source_uri) = language_navigation::file_uri(self.tabs[index].path()) else {
            return false;
        };
        let Some(lease) = server.document(&source_uri) else {
            return false;
        };
        let location = Location {
            uri: uri.as_str().into(),
            selection: selection.map(|range| api::DocumentRange::Utf16 {
                start: api::TextPosition {
                    line: range.start.line,
                    character: range.start.character,
                },
                end: api::TextPosition {
                    line: range.end.line,
                    character: range.end.character,
                },
            }),
        };
        let service = service.clone();
        let target = location.uri.clone();
        let title = url::Url::parse(&target)
            .ok()
            .and_then(|url| {
                url.path_segments()
                    .and_then(|mut segments| segments.next_back().map(str::to_owned))
            })
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| service.provider.language.clone());
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_executor()
                .scheduler_executor()
                .spawn_dedicated(move |_| async move { server.content_for(lease, target) })
                .await;
            let _ = this.update_in(cx, |app, _, cx| {
                if !app.session_state.workspace_trusted
                    || !service.is_active()
                    || app
                        .active_text_tab_index()
                        .and_then(|index| app.plugin_document_version(index).ok())
                        .as_ref()
                        != Some(&document)
                    || !app
                        .language_server_for_path(&app.workspace.root().join(&document.path))
                        .is_some_and(|current| current.uses_service(&service))
                {
                    return;
                }
                let result = result.and_then(|text| {
                    service
                        .open_content(document, location, title, text)
                        .map_err(Into::into)
                });
                if let Err(error) = result {
                    app.status = t!("status.open_failed", error = format!("{error:#}")).to_string();
                    cx.notify();
                }
            });
        })
        .detach();
        true
    }

    /// Deferred request dispatch must still observe the exact source revision and active target.
    pub(super) fn check_content_origin(
        &self,
        resource: &plugin_runtime::VirtualDocumentResource,
    ) -> Result<(), Failure> {
        if let Some(origin) = resource.content_origin() {
            if !self.session_state.workspace_trusted
                || self
                    .active_text_tab_index()
                    .and_then(|index| self.plugin_document_version(index).ok())
                    .as_ref()
                    != Some(&origin.source)
            {
                return Err(Failure::new(
                    ErrorCode::StaleRevision,
                    "Library navigation source changed or closed",
                ));
            }
        }
        Ok(())
    }

    /// Repeated jumps reuse a live view only for the exact provider incarnation and original URI.
    pub(super) fn find_language_content_tab(
        &self,
        resource: &plugin_runtime::VirtualDocumentResource,
    ) -> Option<usize> {
        let origin = resource.content_origin()?;
        self.tabs.iter().position(|tab| {
            tab.virtual_document.as_ref().is_some_and(|tab| {
                tab.resource.is_live()
                    && tab.resource.same_language_owner(resource)
                    && tab
                        .resource
                        .content_origin()
                        .is_some_and(|existing| existing.location.uri == origin.location.uri)
            })
        })
    }

    /// Resolve UTF-16 against the actual readonly native text, then reveal the original target.
    pub(super) fn refresh_language_content_tab(
        &mut self,
        index: usize,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let tab = self.tabs[index]
            .text
            .as_mut()
            .expect("Readonly content has a text session");
        if tab.editor.read(cx).text().to_string() != text {
            // Only the native readonly session changes. No extra Rope, writable file or Undo entry exists.
            tab.editor
                .update(cx, |editor, cx| editor.set_value(text, window, cx));
            tab.capability_revision = tab.capability_revision.saturating_add(1);
            tab.session.accept_disk_reload();
            if let Ok(info) = self.plugin_document_info(index, cx) {
                self.tabs[index]
                    .virtual_document
                    .as_ref()
                    .unwrap()
                    .resource
                    .bind(info.document);
            }
        }
    }

    /// Resolve UTF-16 against the actual readonly native text, then reveal the original target.
    pub(super) fn reveal_language_content(
        &mut self,
        resource: &plugin_runtime::VirtualDocumentResource,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), Failure> {
        let Some(selection) = resource
            .content_origin()
            .and_then(|origin| origin.location.selection)
        else {
            return Ok(());
        };
        let index = self
            .active_text_tab_index()
            .ok_or_else(|| Failure::new(ErrorCode::InvalidHandle, "Library view is closed"))?;
        let editor = self.tabs[index].text.as_ref().unwrap().editor.clone();
        let range = selection.resolve(&editor.read(cx).text().to_string())?;
        editor.update(cx, |editor, cx| {
            let position = editor.text().offset_to_position(range.start);
            editor.unfold_at(position, cx);
            editor.set_cursor_position(position, window, cx);
            editor.set_selected_range(range.start..range.end, cx);
        });
        Ok(())
    }
}
