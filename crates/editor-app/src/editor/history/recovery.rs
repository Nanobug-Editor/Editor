//! Durable preimage protection precedes the one native undoable historical replacement.

use super::super::text_transactions::PreparedTextEdits;
use super::model::{PendingCreate, Target};
use super::*;
use gpui_kit::EntityInputHandler as _;

/// Match both session incarnation and native COW content, including notify-only IME changes.
pub(super) fn target_matches(app: &EditorApp, target: &Target, cx: &App) -> Option<usize> {
    app.tabs.iter().position(|file| {
        file.file_id == target.id
            && file.text.as_ref().is_some_and(|tab| {
                tab.session.revision() == target.revision
                    && tab.capability_revision == target.capability
                    && target.identity.matches(tab.editor.read(cx).text())
            })
    })
}

impl EditorApp {
    /// Restore remains an editor edit; the disk is not written until a later ordinary Save.
    pub(crate) fn restore_history_selection(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(panel) = &self.history.panel else {
            return;
        };
        if panel.busy {
            return;
        }
        let Some(selection) = &panel.selection else {
            return;
        };
        if selection.active_document
            != self
                .active_tab_index()
                .map(|index| self.tabs[index].file_id)
        {
            self.history_notice(t!("history.stale").to_string(), cx);
            return;
        }
        let Item::Version(version) = &selection.item else {
            return;
        };
        let Some(preview) = &panel.preview else {
            return;
        };
        let Some(location) = selection.location.clone() else {
            return;
        };
        let version = version.clone();
        let expected_disk = selection.disk.clone();
        let expected_target = selection.target.clone();
        let result = preview.read(cx).value().to_string();
        if result.len() > 32 * 1024 * 1024 {
            self.history_notice(t!("history.too_large").to_string(), cx);
            return;
        }
        let current_disk = match platform_windows::history::FileStamp::read(&location.path) {
            Ok(stamp) => stamp,
            Err(error) => {
                self.history_notice(error.to_string(), cx);
                return;
            }
        };
        if current_disk != expected_disk {
            self.history_notice(t!("history.stale").to_string(), cx);
            return;
        }
        let existing = self
            .tabs
            .iter()
            .position(|file| file.path() == location.path);
        if let Some(target) = &expected_target {
            if target_matches(self, target, cx) != existing {
                self.history_notice(t!("history.stale").to_string(), cx);
                return;
            }
        } else if existing.is_some() {
            self.history_notice(t!("history.stale").to_string(), cx);
            return;
        }
        let deleted = !location.present || expected_disk.is_none();
        if deleted && expected_disk.is_some() {
            // A known independent path claimant is never adopted as this deleted file's buffer.
            self.choose_history_copy(result, version.id, window, cx);
            return;
        }
        let root = location
            .workspace
            .clone()
            .or_else(|| location.path.parent().map(Path::to_owned));
        if deleted
            && (root.as_ref().is_none_or(|root| !root.is_dir())
                || location
                    .path
                    .parent()
                    .is_some_and(|parent| parent.exists() && !parent.is_dir()))
        {
            self.choose_history_copy(result, version.id, window, cx);
            return;
        }
        let index = match existing {
            Some(index) => index,
            None if deleted => {
                // There is no pre-existing mutable buffer to protect; recheck history is enabled off-thread.
                self.open_deleted_history(version, location, result, root, window, cx);
                return;
            }
            None => {
                self.open_file(location.path.clone(), window, cx);
                let Some(index) = self
                    .tabs
                    .iter()
                    .position(|file| file.path() == location.path)
                else {
                    return;
                };
                index
            }
        };
        let Some(tab) = self.text_tab(index) else {
            self.history_notice(t!("history.no_text_target").to_string(), cx);
            return;
        };
        let editor = tab.editor.clone();
        if tab.session.is_readonly()
            || !editor.read(cx).is_editable()
            || editor.update(cx, |editor, cx| {
                editor.marked_text_range(window, cx).is_some()
            })
        {
            self.history_notice(t!("history.readonly_ime").to_string(), cx);
            return;
        }
        let source = editor.read(cx).value().to_string();
        if source.len() > 32 * 1024 * 1024 {
            self.history_notice(t!("history.too_large").to_string(), cx);
            return;
        }
        let target = Target {
            id: self.tabs[index].file_id,
            revision: tab.session.revision(),
            capability: tab.capability_revision,
            identity: super::super::native_text::NativeTextIdentity::new(editor.read(cx).text()),
        };
        self.activate_tab(index, window, cx);
        self.history.generation = self.history.generation.wrapping_add(1);
        let generation = self.history.generation;
        if let Some(panel) = &mut self.history.panel {
            panel.busy = true;
            panel.error = None;
        }
        let Some(worker) = self.history.worker.clone() else {
            return;
        };
        let path = location.path.clone();
        let expected = expected_disk.clone();
        let protected = worker.call(source.len() + result.len(), move |history| {
            if platform_windows::history::FileStamp::read(&path)? != expected {
                return Err(editor_history::Error::Stale);
            }
            let plan = PreparedTextEdits::restore(
                source.clone(),
                result,
                plugin_runtime::plugin_protocol::api::TextRange { start: 0, end: 0 },
            )
            .map_err(|error| editor_history::Error::Io(std::io::Error::other(error.to_string())))?;
            let lease = history.protect(Capture {
                file: Some(version.file),
                path: path.clone(),
                workspace: location.workspace.clone(),
                contents: source.into_bytes(),
                reason: Reason::BeforeRestore,
                at_ms: now_ms(),
            })?;
            if platform_windows::history::FileStamp::read(&path)? != expected {
                return Err(editor_history::Error::Stale);
            }
            Ok((plan, lease))
        });
        cx.spawn_in(window, async move |app, cx| {
            let prepared = protected.await;
            let _ = app.update_in(cx, |app, window, cx| {
                if app.history.generation != generation {
                    return;
                }
                if let Some(panel) = &mut app.history.panel {
                    panel.busy = false;
                }
                let applied = (|| -> anyhow::Result<()> {
                    let (plan, _lease) = prepared?;
                    let index = target_matches(app, &target, cx)
                        .ok_or_else(|| anyhow::anyhow!(t!("history.stale")))?;
                    anyhow::ensure!(
                        app.active_tab_index() == Some(index),
                        "{}",
                        t!("history.stale")
                    );
                    anyhow::ensure!(
                        platform_windows::history::FileStamp::read(&location.path)?
                            == expected_disk,
                        "{}",
                        t!("history.stale")
                    );
                    editor.update(cx, |editor, cx| plan.apply(editor, window, cx))?;
                    if deleted {
                        app.history.pending_create.insert(
                            target.id,
                            PendingCreate {
                                needs_create: true,
                                root: root
                                    .ok_or_else(|| anyhow::anyhow!("Missing recovery root"))?,
                                file: Some(version.file),
                                origin: version.id,
                            },
                        );
                        let tab = app.text_tab_mut(index).unwrap();
                        tab.disk_state = DiskState::Deleted;
                        tab.overwrite_confirmed = true;
                    }
                    app.close_history(window, cx);
                    editor.read(cx).focus_handle(cx).focus(window, cx);
                    app.status = t!("history.restored_pending").to_string();
                    Ok(())
                })();
                if let Err(error) = applied {
                    app.history_notice(error.to_string(), cx);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// A missing file opens as a dirty native document, retaining original lineage until create-only Save.
    fn open_deleted_history(
        &mut self,
        version: editor_history::Version,
        location: editor_history::Location,
        contents: String,
        root: Option<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(worker) = self.history.worker.clone() else {
            return;
        };
        self.history.generation = self.history.generation.wrapping_add(1);
        let generation = self.history.generation;
        let active_document = self
            .active_tab_index()
            .map(|index| self.tabs[index].file_id);
        let allowed = worker.call(0, |history| {
            if !history.policy()?.enabled {
                return Err(editor_history::Error::ProtectionSkipped(
                    editor_history::SkipReason::Disabled,
                ));
            }
            Ok(())
        });
        if let Some(panel) = &mut self.history.panel {
            panel.busy = true;
        }
        cx.spawn_in(window, async move |app, cx| {
            let result = allowed.await;
            let _ = app.update_in(cx, |app, window, cx| {
                if app.history.generation != generation {
                    return;
                }
                if let Some(panel) = &mut app.history.panel {
                    panel.busy = false;
                }
                // A late policy acknowledgement cannot take focus from a newly selected document.
                if app.active_tab_index().map(|index| app.tabs[index].file_id) != active_document {
                    app.history_notice(t!("history.stale").to_string(), cx);
                    return;
                }
                if let Err(error) = result {
                    app.history_notice(error.to_string(), cx);
                    return;
                }
                if location.path.exists()
                    || app.tabs.iter().any(|file| file.path() == location.path)
                {
                    app.history_notice(t!("history.stale").to_string(), cx);
                    return;
                }
                let Some(root) = root.filter(|root| root.is_dir()) else {
                    app.choose_history_copy(contents, version.id, window, cx);
                    return;
                };
                let index = app.open_pending_history_document(location.path, contents, window, cx);
                app.history.pending_create.insert(
                    app.tabs[index].file_id,
                    PendingCreate {
                        needs_create: true,
                        root,
                        file: Some(version.file),
                        origin: version.id,
                    },
                );
                app.close_history(window, cx);
                app.editor.focus_handle(cx).focus(window, cx);
                app.status = t!("history.restored_pending").to_string();
                cx.notify();
            });
        })
        .detach();
    }

    /// History-local diagnostics preserve the current editor and provide refresh/retry actions.
    pub(super) fn history_notice(&mut self, message: String, cx: &mut Context<Self>) {
        if let Some(panel) = &mut self.history.panel {
            panel.error = Some(message);
            panel.busy = false;
        } else {
            self.report_host_message(MessageLevel::Warning, message, cx);
        }
        cx.notify();
    }
}
