//! Native destination selection and staged raw-byte export retain source and target authority separately.

use super::model::PendingCreate;
use super::*;
use platform_windows::file_picker::{FilePickerKind, FilePickerOptions};

impl EditorApp {
    /// Saving a recovered buffer elsewhere retargets its existing session, preserving text and Undo.
    pub(crate) fn relocate_history_recovery(
        &mut self,
        id: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self.tabs.iter().position(|file| file.file_id == id) else {
            return;
        };
        if !self
            .history
            .pending_create
            .get(&id)
            .is_some_and(|recovery| recovery.needs_create)
        {
            return;
        }
        let suggested = self.tabs[index]
            .path()
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("recovered.txt")
            .to_string();
        self.choose_history_destination(
            suggested,
            move |app, path, _, cx| {
                let Some(index) = app.tabs.iter().position(|file| file.file_id == id) else {
                    return;
                };
                if path.exists()
                    || app
                        .tabs
                        .iter()
                        .any(|file| file.file_id != id && file.path() == path)
                {
                    app.history_notice(t!("history.destination_exists").to_string(), cx);
                    return;
                }
                let Some(parent) = path.parent().and_then(|parent| parent.canonicalize().ok())
                else {
                    app.history_notice(t!("history.destination_unavailable").to_string(), cx);
                    return;
                };
                let Some(name) = path.file_name() else {
                    return;
                };
                let path = parent.join(name);
                let previous = app.tabs[index].path().to_owned();
                app.close_language_document(&previous, None, cx);
                let file = &mut app.tabs[index];
                file.path = path.clone();
                file.file_revision = file.file_revision.saturating_add(1);
                if let Some(tab) = &mut file.text {
                    tab.session.rename(path.clone());
                    tab.capability_revision = tab.capability_revision.saturating_add(1);
                    tab.disk_state = DiskState::Deleted;
                    tab.overwrite_confirmed = true;
                }
                if app.active_path.as_ref() == Some(&previous) {
                    app.active_path = Some(path);
                }
                if let Some(recovery) = app.history.pending_create.get_mut(&id) {
                    recovery.file = None;
                    recovery.root = parent;
                }
                app.history.identities.remove(&id);
                app.save_text_tab(index, cx);
            },
            window,
            cx,
        );
    }

    /// One cancellable native save picker belongs to this view's current generation.
    pub(super) fn choose_history_destination(
        &mut self,
        suggested_name: String,
        selected: impl FnOnce(&mut Self, PathBuf, &mut Window, &mut Context<Self>) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.history.picker.take();
        let picker = match platform_windows::file_picker::pick_files(FilePickerOptions {
            owner_hwnd: 0,
            title: t!("history.choose_destination").to_string(),
            kind: FilePickerKind::Save,
            multiple: false,
            suggested_name: Some(suggested_name),
        }) {
            Ok(picker) => picker,
            Err(error) => {
                self.history_notice(error.to_string(), cx);
                return;
            }
        };
        self.history.generation = self.history.generation.wrapping_add(1);
        let generation = self.history.generation;
        self.history.picker = Some(picker.control);
        if let Some(panel) = &mut self.history.panel {
            panel.busy = true;
        }
        cx.spawn_in(window, async move |app, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    picker
                        .result
                        .recv()
                        .map_err(|_| std::io::Error::other("File selection closed"))?
                })
                .await;
            let _ = app.update_in(cx, |app, window, cx| {
                if app.history.generation != generation {
                    return;
                }
                app.history.picker.take();
                if let Some(panel) = &mut app.history.panel {
                    panel.busy = false;
                }
                match result {
                    Ok(Some(paths)) => {
                        if let Some(path) = paths.into_iter().next() {
                            selected(app, path, window, cx);
                        }
                    }
                    Ok(None) => {}
                    Err(error) => app.history_notice(error.to_string(), cx),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// A source-path collision creates an independent pending document only after another location is chosen.
    pub(super) fn choose_history_copy(
        &mut self,
        contents: String,
        origin: editor_history::VersionId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let active_document = self
            .active_tab_index()
            .map(|index| self.tabs[index].file_id);
        let suggested = self
            .history
            .panel
            .as_ref()
            .and_then(|panel| panel.selection.as_ref())
            .and_then(|selection| selection.location.as_ref())
            .and_then(|location| location.path.file_name())
            .and_then(|name| name.to_str())
            .unwrap_or("recovered.txt")
            .to_string();
        self.choose_history_destination(
            suggested,
            move |app, path, window, cx| {
                if app.active_tab_index().map(|index| app.tabs[index].file_id) != active_document {
                    app.history_notice(t!("history.stale").to_string(), cx);
                    return;
                }
                if path.exists() || app.tabs.iter().any(|file| file.path() == path) {
                    app.history_notice(t!("history.destination_exists").to_string(), cx);
                    return;
                }
                let Some(parent) = path.parent().and_then(|parent| parent.canonicalize().ok())
                else {
                    app.history_notice(t!("history.destination_unavailable").to_string(), cx);
                    return;
                };
                let Some(name) = path.file_name() else {
                    return;
                };
                let path = parent.join(name);
                let Some(worker) = app.history.worker.clone() else {
                    return;
                };
                let allowed = worker.call(0, |history| {
                    if !history.policy()?.enabled {
                        return Err(editor_history::Error::ProtectionSkipped(
                            editor_history::SkipReason::Disabled,
                        ));
                    }
                    Ok(())
                });
                let generation = app.history.generation;
                cx.spawn_in(window, async move |owner, cx| {
                    let result = allowed.await;
                    let _ = owner.update_in(cx, |app, window, cx| {
                        if app.history.generation != generation {
                            return;
                        }
                        // Creating a writable recovery document still belongs to the initiating tab.
                        if app.active_tab_index().map(|index| app.tabs[index].file_id)
                            != active_document
                        {
                            app.history_notice(t!("history.stale").to_string(), cx);
                            return;
                        }
                        if let Err(error) = result {
                            app.history_notice(error.to_string(), cx);
                            return;
                        }
                        if path.exists() || app.tabs.iter().any(|file| file.path() == path) {
                            app.history_notice(t!("history.destination_exists").to_string(), cx);
                            return;
                        }
                        let index = app.open_pending_history_document(path, contents, window, cx);
                        app.history.pending_create.insert(
                            app.tabs[index].file_id,
                            PendingCreate {
                                needs_create: true,
                                root: parent,
                                file: None,
                                origin,
                            },
                        );
                        app.close_history(window, cx);
                        app.editor.focus_handle(cx).focus(window, cx);
                        app.status = t!("history.restored_pending").to_string();
                        cx.notify();
                    });
                })
                .detach();
            },
            window,
            cx,
        );
    }

    /// Raw legacy bytes, including oversized/binary files, stream directly into a private sibling stage.
    pub(super) fn export_history_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(panel) = &self.history.panel else {
            return;
        };
        let Some(index) = panel.selected else {
            return;
        };
        let Some(item) = panel.rows.get(index).cloned() else {
            return;
        };
        if !item.has_content() {
            return;
        }
        let suggested = match &item {
            Item::Version(row) => row
                .path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("history.txt")
                .to_string(),
            Item::Legacy(row) => row
                .filename
                .split_once('-')
                .map(|(_, name)| name)
                .unwrap_or(&row.filename)
                .to_string(),
        };
        self.choose_history_destination(
            suggested,
            move |app, path, window, cx| {
                let Some(worker) = app.history.worker.clone() else {
                    return;
                };
                let generation = app.history.generation;
                let staged = worker.call(0, move |history| {
                    let root = path
                        .parent()
                        .ok_or(editor_history::Error::InvalidPath)?
                        .canonicalize()?;
                    let path =
                        root.join(path.file_name().ok_or(editor_history::Error::InvalidPath)?);
                    let stage =
                        platform_windows::PreparedNewFile::prepare(&root, &path, |output| {
                            use std::io::Write as _;
                            let result = match item {
                                Item::Version(row) => history
                                    .read(row.id)
                                    .and_then(|bytes| output.write_all(&bytes).map_err(Into::into)),
                                Item::Legacy(row) => history.copy_legacy(&row, output),
                            };
                            result.map_err(|error| std::io::Error::other(error.to_string()))
                        })?;
                    Ok(stage)
                });
                if let Some(panel) = &mut app.history.panel {
                    panel.busy = true;
                }
                cx.spawn_in(window, async move |owner, cx| {
                    let result = staged.await;
                    let _ = owner.update_in(cx, |app, _, cx| {
                        if app.history.generation != generation {
                            return;
                        }
                        if let Some(panel) = &mut app.history.panel {
                            panel.busy = false;
                        }
                        match result.and_then(|stage| stage.commit().map_err(Into::into)) {
                            Ok(()) => {
                                if let Some(panel) = &mut app.history.panel {
                                    panel.notice = Some(t!("history.exported").to_string());
                                }
                            }
                            Err(error) => app.history_notice(error.to_string(), cx),
                        }
                        cx.notify();
                    });
                })
                .detach();
            },
            window,
            cx,
        );
    }
}
