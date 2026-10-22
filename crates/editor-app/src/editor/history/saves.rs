//! All user/pre-launch saves prepare history and bytes off-thread before the session's final write.

use super::super::native_text::NativeTextIdentity;
use super::*;
use platform_windows::PreparedFileStore;

/// A prepared replacement retains the exact disk state observed before any destructive write.
struct Prepared {
    /// Single-use sibling staging resource, consumed only through DocumentSession.
    store: PreparedFileStore,
    /// Final publication checks metadata/native identity; full content reads stay in preparation.
    stamp: Option<platform_windows::history::FileStamp>,
    /// History failure is visible, but does not turn an otherwise valid ordinary save into failure.
    history_error: Option<String>,
}

/// Preparation failures distinguish conflict confirmation from an ordinary I/O failure.
enum SaveFailure {
    Conflict(bool),
    Other(String),
}

/// A bounded-buffer fingerprint avoids retaining another complete disk file during verification.
pub(crate) fn disk_digest(path: &Path) -> std::io::Result<Option<[u8; 32]>> {
    let mut file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(Some(digest.finalize().into()))
}

impl EditorApp {
    /// Share the same asynchronous save and completion seam with user commands and pre-launch saves.
    /// Completion receives the owning window only after revision, text identity and disk revalidation.
    pub(crate) fn save_history_document(
        &mut self,
        index: usize,
        finished: impl FnOnce(&mut Self, bool, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) {
        if self.history.saves_sealed {
            self.status = t!("projects.closing").to_string();
            cx.notify();
            return;
        }
        let save_generation = self.history.save_generation;
        self.reconnect_history(cx);
        if let Some(file) = self.tabs.get(index) {
            if self
                .history
                .pending_create
                .get(&file.file_id)
                .is_some_and(|recovery| {
                    recovery.needs_create
                        && (file.path().exists()
                            || !recovery.root.is_dir()
                            || file
                                .path()
                                .parent()
                                .is_some_and(|parent| parent.exists() && !parent.is_dir()))
                })
            {
                let id = file.file_id;
                let owner = cx.entity().downgrade();
                let handle = self.owner_window;
                cx.defer(move |cx| {
                    let _ = handle.update(cx, |_, window, cx| {
                        let _ = owner
                            .update(cx, |app, cx| app.relocate_history_recovery(id, window, cx));
                    });
                });
                return;
            }
        }
        let Some(file) = self.tabs.get(index) else {
            return;
        };
        let Some(tab) = &file.text else {
            return;
        };
        if file.virtual_document.is_some() || tab.session.is_readonly() {
            self.report_host_message(
                MessageLevel::Warning,
                t!("status.readonly_document").to_string(),
                cx,
            );
            return;
        }
        if self.plugin_saves.contains(tab.path()) {
            self.status = t!("history.saving").to_string();
            cx.notify();
            return;
        }
        if !tab.session.is_dirty() && tab.disk_state == DiskState::Synced {
            self.status = t!("status.no_changes_to_save").to_string();
            cx.notify();
            return;
        }
        if tab.disk_state != DiskState::Synced && !tab.overwrite_confirmed {
            let deleted = tab.disk_state == DiskState::Deleted;
            self.text_tab_mut(index).unwrap().overwrite_confirmed = true;
            self.report_host_message(
                MessageLevel::Warning,
                if deleted {
                    t!("status.confirm_disk_restore").to_string()
                } else {
                    t!("status.confirm_disk_overwrite").to_string()
                },
                cx,
            );
            cx.notify();
            return;
        }
        let id = file.file_id;
        let path = tab.path().to_owned();
        let recovery = self.history.pending_create.get(&id);
        let create_required = recovery.is_some_and(|recovery| recovery.needs_create);
        let revision = tab.session.revision();
        let capability = tab.capability_revision;
        let identity = NativeTextIdentity::new(tab.editor.read(cx).text());
        let snapshot = identity.clone();
        let expected = tab.disk_digest;
        let confirmed = tab.overwrite_confirmed;
        let workspace = path
            .starts_with(self.workspace.root())
            .then(|| self.workspace.root().to_owned());
        let create_root = recovery
            .map(|recovery| recovery.root.clone())
            .or_else(|| workspace.clone())
            .or_else(|| path.parent().map(Path::to_owned));
        let worker = self.history.worker.clone();
        let save_path = path.clone();
        let window_handle = self.owner_window;
        self.plugin_saves.insert(path.clone());
        self.publish_document_save(index, None, cx);
        cx.spawn(async move |app, cx| {
            let prepared = cx
                .background_executor()
                .spawn(async move {
                    let stamp = platform_windows::history::FileStamp::read(&save_path)
                        .map_err(|error| SaveFailure::Other(error.to_string()))?;
                    let before = disk_digest(&save_path)
                        .map_err(|error| SaveFailure::Other(error.to_string()))?;
                    if create_required && before.is_some() {
                        return Err(SaveFailure::Conflict(false));
                    }
                    if !confirmed && before != Some(expected) {
                        return Err(SaveFailure::Conflict(before.is_none()));
                    }
                    let history_error = if let Some(worker) = worker {
                        let observed_path = save_path.clone();
                        worker
                            .call(0, move |history| {
                                observe_disk(
                                    history,
                                    &observed_path,
                                    workspace.as_deref(),
                                    Reason::BeforeOverwrite,
                                )
                            })
                            .await
                            .err()
                            .map(|error| error.to_string())
                    } else {
                        None
                    };
                    if disk_digest(&save_path)
                        .map_err(|error| SaveFailure::Other(error.to_string()))?
                        != before
                    {
                        return Err(SaveFailure::Conflict(false));
                    }
                    if platform_windows::history::FileStamp::read(&save_path)
                        .map_err(|error| SaveFailure::Other(error.to_string()))?
                        != stamp
                    {
                        return Err(SaveFailure::Conflict(false));
                    }
                    // Waiting for required disk-history work retains only the native COW snapshot.
                    // A second complete string is materialized only once staging can proceed.
                    let value = snapshot.contents();
                    let store = if before.is_none() {
                        let root = create_root.ok_or_else(|| {
                            SaveFailure::Other("Recovery root unavailable".into())
                        })?;
                        platform_windows::history::prepare_recovery_parent(&root, &save_path)
                            .map_err(|error| SaveFailure::Other(error.to_string()))?;
                        PreparedFileStore::prepare_new(root, save_path, value)
                    } else {
                        PreparedFileStore::prepare(save_path, value)
                    }
                    .map_err(|error| SaveFailure::Other(error.to_string()))?;
                    Ok(Prepared {
                        store,
                        stamp,
                        history_error,
                    })
                })
                .await;
            let _ = window_handle.update(cx, |_, window, cx| {
                let _ = app.update(cx, |app, cx| {
                    app.plugin_saves.remove(&path);
                    let index = app
                        .tabs
                        .iter()
                        .position(|file| file.file_id == id && file.path() == path);
                    let Some(index) = index else {
                        return;
                    };
                    let unchanged = !app.history.saves_sealed
                        && app.history.save_generation == save_generation
                        && app.text_tab(index).is_some_and(|tab| {
                            tab.session.revision() == revision
                                && tab.capability_revision == capability
                                && identity.matches(tab.editor.read(cx).text())
                        });
                    let result = if !unchanged {
                        Err(SaveFailure::Other(t!("history.stale").to_string()))
                    } else {
                        prepared.and_then(|prepared| app.commit_history_save(index, prepared, cx))
                    };
                    let ok = result.is_ok();
                    let event = result.map_err(|failure| {
                        let message = match failure {
                            SaveFailure::Conflict(deleted) => {
                                if let Some(tab) = app.text_tab_mut(index) {
                                    tab.disk_state = if deleted {
                                        DiskState::Deleted
                                    } else {
                                        DiskState::Conflict
                                    };
                                    tab.overwrite_confirmed = true;
                                }
                                if deleted {
                                    t!("status.confirm_disk_restore").to_string()
                                } else {
                                    t!("status.confirm_disk_overwrite").to_string()
                                }
                            }
                            SaveFailure::Other(error) => {
                                t!("status.save_failed", error = error).to_string()
                            }
                        };
                        app.report_host_message(MessageLevel::Error, message.clone(), cx);
                        plugin_runtime::plugin_protocol::api::Failure::new(
                            plugin_runtime::plugin_protocol::api::ErrorCode::OperationFailed,
                            message,
                        )
                    });
                    app.publish_document_save(index, Some(event), cx);
                    finished(app, ok, window, cx);
                    cx.notify();
                });
            });
        })
        .detach();
    }

    /// Atomic publication remains a DocumentSession write; only its success marks the revision saved.
    fn commit_history_save(
        &mut self,
        index: usize,
        prepared: Prepared,
        cx: &mut Context<Self>,
    ) -> std::result::Result<(), SaveFailure> {
        let path = self.tabs[index].path().to_owned();
        let current = platform_windows::history::FileStamp::read(&path)
            .map_err(|error| SaveFailure::Other(error.to_string()))?;
        if current != prepared.stamp {
            return Err(SaveFailure::Conflict(current.is_none()));
        }
        let tab = self
            .text_tab_mut(index)
            .ok_or_else(|| SaveFailure::Other(t!("history.stale").to_string()))?;
        tab.session
            .save(&prepared.store, prepared.store.contents())
            .map_err(|error| SaveFailure::Other(error.to_string()))?;
        tab.disk_digest = Sha256::digest(prepared.store.contents().as_bytes()).into();
        tab.last_saved_at = Instant::now();
        tab.disk_state = DiskState::Synced;
        tab.overwrite_confirmed = false;
        let value = prepared.store.contents().to_owned();
        self.record_history_save(path.clone(), value.clone(), cx);
        self.notify_language_document_saved(&path, value, cx);
        self.status = t!("status.saved", path = editor_core::display_path(&path)).to_string();
        if let Some(error) = prepared.history_error {
            self.history_failure(error, cx);
        }
        Ok(())
    }
}
