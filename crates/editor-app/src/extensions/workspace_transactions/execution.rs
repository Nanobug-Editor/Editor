//! Apply and recover through checked UI/IO boundaries, retaining the real outcome of every started step.

use super::*;
use gpui_kit::AsyncWindowContext;

/// A closed window is a domain failure; no asynchronous result may switch to a different native owner.
pub(super) fn update_native<R>(
    app: &WeakEntity<EditorApp>,
    cx: &mut AsyncWindowContext,
    update: impl FnOnce(&mut EditorApp, &mut Window, &mut Context<EditorApp>) -> Result<R, Failure>,
) -> Result<R, Failure> {
    cx.update(|window, cx| app.update(cx, |app, cx| update(app, window, cx)))
        .map_err(|_| failure(ErrorCode::InvalidState, "Editor window closed"))?
        .map_err(|_| failure(ErrorCode::InvalidState, "Editor owner closed"))?
}

pub(super) async fn apply(
    app: &WeakEntity<EditorApp>,
    cx: &mut AsyncWindowContext,
    request: &EditorRequest,
    record: &mut Record,
) -> Result<(), Failure> {
    let files = record.files.take().expect("prepared file plan");
    let gate = request.clone();
    let owner = record.batch.clone();
    let (files, result) = cx
        .background_executor()
        .spawn(async move {
            let result = files.preflight(&|| stopped(&gate, &owner));
            (files, result)
        })
        .await;
    record.files = Some(files);
    result?;
    update_native(app, cx, |app, window, cx| {
        app.check_workspace_native(record, window, cx)?;
        if !request.enter_side_effect() {
            return Err(failure(
                ErrorCode::Cancelled,
                "Plan stopped before execution",
            ));
        }
        record
            .batch
            .publish(receipt(record, BatchState::Applying, None, None));
        Ok(())
    })?;
    for index in 0..record.changes.len() {
        let index = index as u16;
        if stopped(request, &record.batch) {
            return Err(failure(
                ErrorCode::Cancelled,
                "Further plan operations stopped",
            ));
        }
        update_native(app, cx, |app, window, cx| {
            app.check_workspace_native(record, window, cx)
        })?;
        let mut files = record.files.take().unwrap();
        let gate = request.clone();
        let owner = record.batch.clone();
        let is_document = matches!(record.changes[index as usize], Change::EditDocument { .. });
        let (files, result) = cx
            .background_executor()
            .spawn(async move {
                let result = if is_document {
                    files.check_current(&|| stopped(&gate, &owner))
                } else {
                    files.apply(index, &|| stopped(&gate, &owner))
                };
                (files, result)
            })
            .await;
        record.files = Some(files);
        if let Err(error) = result {
            record.batch.publish(receipt(
                record,
                BatchState::Partial,
                Some(index),
                Some(error.clone()),
            ));
            return Err(error);
        }
        if let Some(edit) = record.edits.get(&index) {
            let changed = record
                .native
                .iter()
                .find(|target| target.current.id == edit.id)
                .unwrap()
                .text
                .as_ref()
                .unwrap()
                .0
                != edit.prepared.result();
            let expected = record
                .native
                .iter()
                .find(|target| target.current.id == edit.id)
                .unwrap()
                .current
                .revision
                + u64::from(changed);
            let response = update_native(app, cx, |app, window, cx| {
                app.check_workspace_native(record, window, cx)?;
                if stopped(request, &record.batch) {
                    return Err(failure(
                        ErrorCode::Cancelled,
                        "Native text operation stopped",
                    ));
                }
                let tab = app
                    .tabs
                    .iter()
                    .position(|tab| native_version(tab).id == edit.id)
                    .unwrap();
                let editor = app.text_tab(tab).unwrap().editor.clone();
                editor
                    .update(cx, |editor, cx| edit.prepared.apply(editor, window, cx))
                    .map_err(|error| Failure::new(ErrorCode::StaleRevision, error.to_string()))?;
                let (answer, response) = oneshot::channel();
                let id = edit.id.clone();
                let result = edit.prepared.result().to_owned();
                cx.defer_in(window, move |app, _, cx| {
                    let version = app
                        .tabs
                        .iter()
                        .find(|tab| native_version(tab).id == id)
                        .and_then(|tab| {
                            let text = tab.text.as_ref()?;
                            (text.capability_revision == expected
                                && text.editor.read(cx).text().to_string() == result)
                                .then(|| native_version(tab))
                        });
                    let _ = answer.send(version.ok_or_else(|| {
                        failure(
                            ErrorCode::StaleRevision,
                            "Native edit was superseded or closed",
                        )
                    }));
                });
                Ok(response)
            })?;
            let current = response.await.map_err(|_| {
                failure(ErrorCode::InvalidState, "Native completion was released")
            })??;
            let target = record
                .native
                .iter_mut()
                .find(|target| target.current.id == current.id)
                .unwrap();
            target.current = current;
            target.changed |= changed;
        }
        if let Change::Rename { source, target, .. } = &record.changes[index as usize] {
            let authority = request.workspace_request().unwrap();
            let from = authority.file(source, true)?.path().to_owned();
            let to = authority.file(target, true)?.path().to_owned();
            update_native(app, cx, |app, window, cx| {
                app.check_workspace_native(record, window, cx)?;
                app.apply_workspace_renames(vec![(from.clone(), to.clone())], window, cx);
                for target in &mut record.native {
                    if let Some(tab) = app
                        .tabs
                        .iter()
                        .find(|tab| native_version(tab).id == target.current.id)
                    {
                        target.current = native_version(tab);
                    }
                }
                Ok(())
            })?;
            record.renames.push((from, to));
        }
        record.completed.push(index);
        record
            .batch
            .publish(receipt(record, BatchState::Applying, None, None));
    }
    Ok(())
}

pub(super) fn finish(
    app: &WeakEntity<EditorApp>,
    cx: &mut AsyncWindowContext,
    request: EditorRequest,
    mut record: Record,
    result: Result<(), Failure>,
) {
    let stopped_at = if result.is_err() {
        Some(record.completed.len() as u16)
    } else {
        None
    };
    let state = match &result {
        Ok(()) => BatchState::Complete,
        Err(error)
            if record.completed.is_empty()
                && record
                    .files
                    .as_ref()
                    .is_none_or(|files| files.affected().is_empty())
                && !record.native.iter().any(|target| target.changed) =>
        {
            if error.code == ErrorCode::Cancelled {
                BatchState::Cancelled
            } else {
                BatchState::Partial
            }
        }
        Err(_) => BatchState::Partial,
    };
    let value = receipt(&record, state, stopped_at, result.err());
    record.batch.publish(value.clone());
    record.retained_bytes = record
        .files
        .as_ref()
        .map(FilePlan::retained_bytes)
        .unwrap_or_default()
        + record
            .native
            .iter()
            .filter_map(|target| target.text.as_ref())
            .map(|(text, _)| text.len())
            .sum::<usize>();
    // Generated proposals are no longer required once their actual after-version has been captured.
    record.edits.clear();
    let live = record.batch.is_live();
    let _ = update_native(app, cx, |app, window, cx| {
        app.workspace_changes.active = false;
        if live && value.recoverable {
            app.workspace_changes
                .records
                .insert(key(&record.batch), record);
        }
        app.flush_workspace_reconciliation(window, cx);
        app.file_watch.reconcile();
        cx.notify();
        Ok(())
    });
    request.finish(Ok(EditorValue::Workspace(workspace::Value::Batch(value))));
}

impl EditorApp {
    pub(super) fn start_workspace_undo(
        &mut self,
        batch_key: BatchKey,
        request: Option<EditorRequest>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.workspace_changes.active {
            if let Some(request) = request {
                request.finish(Err(failure(ErrorCode::Conflict, "Another batch is active")));
            }
            return;
        }
        let Some(mut record) = self.workspace_changes.records.remove(&batch_key) else {
            if let Some(request) = request {
                request.finish(Err(failure(
                    ErrorCode::InvalidHandle,
                    "Current-session recovery is unavailable",
                )));
            }
            return;
        };
        if !record.batch.is_live() || record.batch.receipt().state == BatchState::Undone {
            if let Some(request) = request {
                request.finish(Err(failure(
                    ErrorCode::InvalidHandle,
                    "Recovery resource was released or already undone",
                )));
            }
            return;
        }
        self.workspace_changes.active = true;
        let app = cx.entity().downgrade();
        window
            .spawn(cx, async move |cx| {
                let files = record.files.take().unwrap();
                let (mut files, disk_conflicts) = cx
                    .background_executor()
                    .spawn(async move {
                        let conflicts = files.undo_conflicts();
                        (files, conflicts)
                    })
                    .await;
                record.files = None;
                let mut conflicts: Vec<_> = disk_conflicts
                    .into_iter()
                    .map(|(target, reason)| workspace::Conflict {
                        target: target.path,
                        reason,
                    })
                    .collect();
                let native = update_native(&app, cx, |app, window, cx| {
                    let mut conflicts = Vec::new();
                    for target in &record.native {
                        if app
                            .tabs
                            .iter()
                            .find(|tab| native_version(tab).id == target.current.id)
                            .is_none_or(|tab| native_version(tab) != target.current)
                        {
                            conflicts.push(workspace::Conflict {
                                target: editor_core::display_path(
                                    target
                                        .current
                                        .path
                                        .strip_prefix(app.workspace.root())
                                        .unwrap_or(&target.current.path),
                                )
                                .to_string(),
                                reason: t!("workspace_plan.document_conflict").to_string(),
                            });
                        }
                    }
                    if let Err(error) = app.check_workspace_native(&record, window, cx) {
                        if conflicts.is_empty() {
                            conflicts.push(workspace::Conflict {
                                target: record.title.clone(),
                                reason: error.message,
                            });
                        }
                    }
                    let mut restores = Vec::new();
                    if conflicts.is_empty() {
                        for target in &record.native {
                            if !target.changed {
                                continue;
                            }
                            let tab = app
                                .tabs
                                .iter()
                                .find(|tab| native_version(tab).id == target.current.id)
                                .unwrap();
                            let (before, selection) = target.text.as_ref().unwrap();
                            let source = tab
                                .text
                                .as_ref()
                                .unwrap()
                                .editor
                                .read(cx)
                                .text()
                                .to_string();
                            let prepared =
                                PreparedTextEdits::restore(source, before.clone(), *selection)
                                    .map_err(|error| {
                                        Failure::new(ErrorCode::InvalidState, error.to_string())
                                    })?;
                            restores.push((target.current.id.clone(), prepared));
                        }
                    }
                    Ok((conflicts, restores))
                });
                let result = match native {
                    Ok((native_conflicts, restores)) => {
                        conflicts.extend(native_conflicts);
                        if !conflicts.is_empty() {
                            Err(failure(
                                ErrorCode::Conflict,
                                "All-target recovery check found conflicts",
                            ))
                        } else if request
                            .as_ref()
                            .is_some_and(|request| !request.enter_side_effect())
                        {
                            Err(failure(
                                ErrorCode::Cancelled,
                                "Recovery stopped before execution",
                            ))
                        } else {
                            let owner = record.batch.clone();
                            let gate = request.clone();
                            let (next, result) = cx
                                .background_executor()
                                .spawn(async move {
                                    let result = files.undo(&|| {
                                        !owner.is_live()
                                            || gate.as_ref().is_some_and(|request| {
                                                request.status().is_terminal()
                                            })
                                    });
                                    (files, result)
                                })
                                .await;
                            files = next;
                            result.and_then(|_| {
                                update_native(&app, cx, |app, window, cx| {
                                    app.check_workspace_native(&record, window, cx)?;
                                    app.apply_workspace_renames(
                                        record
                                            .renames
                                            .iter()
                                            .rev()
                                            .map(|(source, target)| {
                                                (target.clone(), source.clone())
                                            })
                                            .collect(),
                                        window,
                                        cx,
                                    );
                                    for (id, prepared) in restores {
                                        let tab = app
                                            .tabs
                                            .iter()
                                            .find(|tab| native_version(tab).id == id)
                                            .ok_or_else(|| {
                                                failure(
                                                    ErrorCode::StaleRevision,
                                                    "Recovery document closed",
                                                )
                                            })?;
                                        tab.text
                                            .as_ref()
                                            .unwrap()
                                            .editor
                                            .update(cx, |editor, cx| {
                                                prepared.apply(editor, window, cx)
                                            })
                                            .map_err(|error| {
                                                Failure::new(
                                                    ErrorCode::OperationFailed,
                                                    error.to_string(),
                                                )
                                            })?;
                                    }
                                    Ok(())
                                })
                            })
                        }
                    }
                    Err(error) => Err(error),
                };
                record.files = Some(files);
                let state = if result.is_ok() {
                    BatchState::Undone
                } else if !conflicts.is_empty() {
                    record.batch.receipt().state
                } else {
                    BatchState::UndoPartial
                };
                let mut value = receipt(&record, state, None, result.err());
                value.conflicts = conflicts;
                if state == BatchState::Undone {
                    value.recoverable = false;
                }
                record.batch.publish(value.clone());
                let _ = update_native(&app, cx, |app, window, cx| {
                    app.workspace_changes.active = false;
                    if record.batch.is_live() {
                        app.workspace_changes.records.insert(batch_key, record);
                    }
                    app.flush_workspace_reconciliation(window, cx);
                    app.file_watch.reconcile();
                    cx.notify();
                    Ok(())
                });
                if let Some(request) = request {
                    request.finish(Ok(EditorValue::Workspace(workspace::Value::Batch(value))));
                }
            })
            .detach();
    }

    fn apply_workspace_renames(
        &mut self,
        renames: Vec<(PathBuf, PathBuf)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.workspace_changes.reconciling = true;
        self.apply_reconciliation(
            Reconciliation {
                file_events: Vec::new(),
                snapshot: None,
                documents: Vec::new(),
                renames,
                native: true,
            },
            window,
            cx,
        );
        self.workspace_changes.reconciling = false;
    }

    pub(crate) fn defer_workspace_reconciliation(
        &mut self,
        update: Reconciliation,
    ) -> Option<Reconciliation> {
        if self.workspace_changes.active && !self.workspace_changes.reconciling {
            if self.workspace_changes.deferred.len() < 8 {
                self.workspace_changes.deferred.push(update);
            } else {
                self.file_watch.reconcile();
            }
            None
        } else {
            Some(update)
        }
    }

    fn flush_workspace_reconciliation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for update in std::mem::take(&mut self.workspace_changes.deferred) {
            self.apply_reconciliation(update, window, cx);
        }
    }
}
