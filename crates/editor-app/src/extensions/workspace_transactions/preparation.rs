//! Capture native identities and immutable text before background preparation and explicit consent.

use super::*;
use gpui_kit::EntityInputHandler as _;

impl EditorApp {
    pub(super) fn prepare_workspace_plan(
        &mut self,
        source: &str,
        request: EditorRequest,
        authority: WorkspaceRequest,
        plan: Plan,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.prune_workspace_changes(window, cx);
        if self.workspace_changes.active
            || self.plugin_picker_pending
            || self
                .plugin_interactions
                .iter()
                .any(|prompt| prompt.read(cx).is_modal())
        {
            request.finish(Err(failure(
                ErrorCode::Conflict,
                "Another native operation owns confirmation",
            )));
            return;
        }
        if self.workspace_changes.records.len() >= 16 {
            request.finish(Err(failure(
                ErrorCode::LimitExceeded,
                "Release a retained batch before starting another",
            )));
            return;
        }
        let batch = authority.batch().expect("runtime admitted batch").clone();
        let captured =
            self.capture_workspace_plan(source, plan, batch.clone(), &authority, window, cx);
        let (mut record, details) = match captured {
            Ok(record) => record,
            Err(error) => {
                let mut receipt = batch.receipt();
                receipt.state = BatchState::Cancelled;
                receipt.failed = Some(error.clone());
                batch.publish(receipt);
                request.finish(Err(error));
                return;
            }
        };
        self.workspace_changes.active = true;
        let app = cx.entity().downgrade();
        window
            .spawn(cx, async move |cx| {
                let io_changes: Vec<_> = record
                    .changes
                    .iter()
                    .enumerate()
                    .filter(|(_, change)| !matches!(change, Change::EditDocument { .. }))
                    .map(|(index, change)| (index as u16, change.clone()))
                    .collect();
                let buffers = record.edits.keys().copied().collect::<BTreeSet<_>>();
                let io_request = request.clone();
                let io_batch = batch.clone();
                let prepared = cx
                    .background_executor()
                    .spawn(async move {
                        FilePlan::prepare(&io_changes, &authority, &buffers, &|| {
                            stopped(&io_request, &io_batch)
                        })
                    })
                    .await;
                let result = match prepared {
                    Ok(files) => {
                        record.retained_bytes += files.retained_bytes();
                        record.files = Some(files);
                        let (answer, response) = oneshot::channel();
                        let prompt = execution::update_native(&app, cx, |app, window, cx| {
                            app.check_workspace_native(&record, window, cx)?;
                            let retained: usize = app
                                .workspace_changes
                                .records
                                .values()
                                .map(|record| record.retained_bytes)
                                .sum();
                            if retained + record.retained_bytes > 64 * 1024 * 1024 {
                                return Err(failure(
                                    ErrorCode::LimitExceeded,
                                    "Session recovery exceeds 64 MiB; release earlier batches",
                                ));
                            }
                            if stopped(&request, &batch) {
                                return Err(failure(
                                    ErrorCode::Cancelled,
                                    "Plan stopped before preview",
                                ));
                            }
                            let previous = window.focused(cx);
                            let focus = cx.focus_handle();
                            focus.focus(window, cx);
                            app.workspace_changes.prompt = Some(Preview {
                                request: request.clone(),
                                batch: batch.clone(),
                                title: record.title.clone(),
                                source: record.source.clone(),
                                details,
                                documents: record
                                    .origin
                                    .iter()
                                    .chain(&record.conditions)
                                    .chain(record.native.iter().map(|target| &target.current))
                                    .cloned()
                                    .collect(),
                                focus,
                                previous,
                                answer: Some(answer),
                            });
                            batch.publish(receipt(&record, BatchState::Preview, None, None));
                            cx.notify();
                            Ok(())
                        });
                        match prompt {
                            Ok(()) => {
                                // A bounded timer releases a preview even when no native user event occurs.
                                let watched_app = app.clone();
                                let watched_request = request.clone();
                                cx.spawn(async move |cx| {
                                    while !watched_request.status().is_terminal() {
                                        cx.background_executor()
                                            .timer(Duration::from_millis(50))
                                            .await;
                                        if execution::update_native(
                                            &watched_app,
                                            cx,
                                            |app, window, cx| {
                                                app.prune_workspace_changes(window, cx);
                                                Ok(app.workspace_changes.prompt.is_some())
                                            },
                                        )
                                        .unwrap_or(false)
                                            == false
                                        {
                                            break;
                                        }
                                    }
                                    let _ = execution::update_native(
                                        &watched_app,
                                        cx,
                                        |app, window, cx| {
                                            app.prune_workspace_changes(window, cx);
                                            Ok(())
                                        },
                                    );
                                })
                                .detach();
                                match response.await.unwrap_or(Ok(false)) {
                                    Ok(true) => {
                                        execution::apply(&app, cx, &request, &mut record).await
                                    }
                                    Ok(false) => {
                                        Err(failure(ErrorCode::Cancelled, "Preview was dismissed"))
                                    }
                                    Err(error) => Err(error),
                                }
                            }
                            Err(error) => Err(error),
                        }
                    }
                    Err(error) => Err(error),
                };
                execution::finish(&app, cx, request, record, result);
            })
            .detach();
    }

    fn capture_workspace_plan(
        &self,
        source: &str,
        plan: Plan,
        batch: Arc<WorkspaceBatch>,
        authority: &WorkspaceRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(Record, Vec<String>), Failure> {
        plan.validate()?;
        let origin = plan
            .origin
            .as_ref()
            .map(|document| {
                self.plugin_document_target(document)
                    .map(|index| native_version(&self.tabs[index]))
            })
            .transpose()?;
        let mut record = Record {
            conditions: plan
                .conditions
                .iter()
                .map(|document| {
                    self.plugin_document_target(document)
                        .map(|index| native_version(&self.tabs[index]))
                })
                .collect::<Result<_, _>>()?,
            batch,
            title: plan.title,
            source: source.into(),
            origin,
            changes: plan.changes,
            native: Vec::new(),
            edits: BTreeMap::new(),
            watched: BTreeSet::new(),
            files: None,
            completed: Vec::new(),
            retained_bytes: 0,
            renames: Vec::new(),
        };
        let mut paths: Vec<_> = self.tabs.iter().map(|tab| tab.path().to_owned()).collect();
        let mut details = Vec::new();
        for (index, change) in record.changes.clone().iter().enumerate() {
            match change {
                Change::EditDocument {
                    document,
                    edits,
                    selection,
                } => {
                    if edits.len() > 256 {
                        return Err(failure(
                            ErrorCode::LimitExceeded,
                            "A native preview supports at most 256 text ranges per document",
                        ));
                    }
                    let tab = self.plugin_document_target(document)?;
                    record.watched.insert(paths[tab].clone());
                    self.capture_native_edit(
                        &mut record,
                        index as u16,
                        tab,
                        edits,
                        *selection,
                        window,
                        cx,
                    )?;
                    let source = self
                        .text_tab(tab)
                        .unwrap()
                        .editor
                        .read(cx)
                        .text()
                        .to_string();
                    details.push(
                        t!(
                            "workspace_plan.edit",
                            path = document.path,
                            ranges = edits.len()
                        )
                        .to_string(),
                    );
                    for edit in edits {
                        let old =
                            source
                                .get(edit.range.start..edit.range.end)
                                .ok_or_else(|| {
                                    failure(ErrorCode::InvalidRequest, "Invalid source boundary")
                                })?;
                        details.push(format!("− {}\n+ {}", excerpt(old), excerpt(&edit.text)));
                    }
                }
                Change::Write { target, bytes, .. } => {
                    let file = authority.file(target, true)?;
                    record.watched.insert(file.path().to_owned());
                    if let Some(tab) = paths.iter().position(|path| path == file.path()) {
                        if !authority.can_edit_documents() {
                            return Err(failure(
                                ErrorCode::PermissionDenied,
                                "Opened writes require editor.edit 1.1 and editor.write",
                            ));
                        }
                        let text = String::from_utf8(bytes.clone()).map_err(|_| {
                            failure(
                                ErrorCode::UnsupportedOperation,
                                "Opened text requires UTF-8 replacement",
                            )
                        })?;
                        let source = self
                            .text_tab(tab)
                            .ok_or_else(|| {
                                failure(
                                    ErrorCode::UnsupportedOperation,
                                    "Opened opaque files cannot be overwritten",
                                )
                            })?
                            .editor
                            .read(cx)
                            .text()
                            .to_string();
                        self.capture_native_edit(
                            &mut record,
                            index as u16,
                            tab,
                            &[TextEdit {
                                range: TextRange {
                                    start: 0,
                                    end: source.len(),
                                },
                                text,
                            }],
                            TextRange { start: 0, end: 0 },
                            window,
                            cx,
                        )?;
                        details.push(
                            t!(
                                "workspace_plan.buffer_write",
                                path = display_target(target),
                                bytes = bytes.len()
                            )
                            .to_string(),
                        );
                        details.push(format!(
                            "− {}\n+ {}",
                            excerpt(&source),
                            excerpt(std::str::from_utf8(bytes).unwrap())
                        ));
                    } else {
                        details.push(
                            t!(
                                "workspace_plan.disk_write",
                                path = display_target(target),
                                bytes = bytes.len()
                            )
                            .to_string(),
                        );
                    }
                }
                Change::CreateDirectory { target } | Change::Delete { target, .. } => {
                    let file = authority.file(target, true)?;
                    record.watched.insert(file.path().to_owned());
                    if paths.iter().any(|path| path.starts_with(file.path())) {
                        return Err(failure(
                            ErrorCode::Conflict,
                            "Close opened targets before creating or deleting resources",
                        ));
                    }
                    details.push(
                        t!(
                            if matches!(change, Change::Delete { .. }) {
                                "workspace_plan.delete"
                            } else {
                                "workspace_plan.mkdir"
                            },
                            path = display_target(target)
                        )
                        .to_string(),
                    );
                }
                Change::Copy { source, target, .. } | Change::Rename { source, target, .. } => {
                    let rename = matches!(change, Change::Rename { .. });
                    let from = authority.file(source, rename)?.path().to_owned();
                    let to = authority.file(target, true)?.path().to_owned();
                    record.watched.insert(from.clone());
                    record.watched.insert(to.clone());
                    if paths.iter().any(|path| path.starts_with(&to)) {
                        return Err(failure(
                            ErrorCode::Conflict,
                            "Destination contains an opened document",
                        ));
                    }
                    for tab in 0..paths.len() {
                        if let Ok(relative) = paths[tab].strip_prefix(&from) {
                            if !rename {
                                return Err(failure(
                                    ErrorCode::Conflict,
                                    "Opened copy sources require an explicit snapshot-to-write plan",
                                ));
                            }
                            ensure_native(&mut record, &self.tabs[tab]);
                            paths[tab] = if relative.as_os_str().is_empty() {
                                to.clone()
                            } else {
                                to.join(relative)
                            };
                        }
                    }
                    details.push(
                        t!(
                            if rename {
                                "workspace_plan.rename"
                            } else {
                                "workspace_plan.copy"
                            },
                            source = display_target(source),
                            target = display_target(target)
                        )
                        .to_string(),
                    );
                }
            }
        }
        Ok((record, details))
    }

    fn capture_native_edit(
        &self,
        record: &mut Record,
        index: u16,
        tab: usize,
        edits: &[TextEdit],
        selection: TextRange,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), Failure> {
        let native = native_version(&self.tabs[tab]);
        if record.edits.values().any(|edit| edit.id == native.id) {
            return Err(failure(
                ErrorCode::Conflict,
                "Combine one document's ranges into a single EditDocument operation",
            ));
        }
        let text = self.text_tab(tab).ok_or_else(|| {
            failure(
                ErrorCode::UnsupportedOperation,
                "Target has no native text session",
            )
        })?;
        text.editor.update(cx, |editor, cx| {
            if !editor.is_editable() || editor.marked_text_range(window, cx).is_some() {
                return Err(failure(
                    ErrorCode::InvalidState,
                    "Finish composition and check readonly state",
                ));
            }
            let source = editor.text().to_string();
            let selected = editor.selected_range();
            let prepared = PreparedTextEdits::new(source.clone(), edits, selection)
                .map_err(|error| Failure::new(ErrorCode::InvalidRequest, error.to_string()))?;
            record.retained_bytes += source.len() + prepared.result().len();
            if record.retained_bytes > 32 * 1024 * 1024 {
                return Err(failure(
                    ErrorCode::LimitExceeded,
                    "Native preimages exceed 32 MiB",
                ));
            }
            let target = ensure_native(record, &self.tabs[tab]);
            target.text = Some((
                source,
                TextRange {
                    start: selected.start,
                    end: selected.end,
                },
            ));
            record.edits.insert(
                index,
                NativeEdit {
                    id: native.id,
                    prepared,
                },
            );
            Ok(())
        })
    }

    pub(super) fn check_workspace_native(
        &self,
        record: &Record,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), Failure> {
        if !self.session_state.workspace_trusted || !record.batch.is_live() {
            return Err(failure(
                ErrorCode::PermissionDenied,
                "Plan authority was revoked",
            ));
        }
        // Readonly source/input conditions protect preview-to-apply without constraining group Undo.
        // After the first completed effect, ordinary changed-target checks govern the remaining steps.
        if record.completed.is_empty()
            && matches!(
                record.batch.receipt().state,
                BatchState::Preparing | BatchState::Preview | BatchState::Applying
            )
            && record
                .origin
                .iter()
                .chain(&record.conditions)
                .any(|origin| !self.tabs.iter().any(|tab| native_version(tab) == *origin))
        {
            return Err(failure(
                ErrorCode::StaleRevision,
                "The plan's readonly source or input changed or closed",
            ));
        }
        for target in &record.native {
            let tab = self
                .tabs
                .iter()
                .find(|tab| native_version(tab).id == target.current.id)
                .ok_or_else(|| failure(ErrorCode::StaleRevision, "A target document was closed"))?;
            if native_version(tab) != target.current {
                return Err(failure(
                    ErrorCode::StaleRevision,
                    "A target document changed, renamed or used local Undo",
                ));
            }
            if target.text.is_some() {
                let text = tab.text.as_ref().ok_or_else(|| {
                    failure(
                        ErrorCode::UnsupportedOperation,
                        "Native text is unavailable",
                    )
                })?;
                text.editor.update(cx, |editor, cx| {
                    if !editor.is_editable() || editor.marked_text_range(window, cx).is_some() {
                        Err(failure(
                            ErrorCode::InvalidState,
                            "Target is readonly or composing",
                        ))
                    } else {
                        Ok(())
                    }
                })?;
            }
        }
        for tab in &self.tabs {
            let version = native_version(tab);
            if record
                .watched
                .iter()
                .any(|path| tab.path().starts_with(path))
                && !record
                    .native
                    .iter()
                    .any(|target| target.current.id == version.id)
            {
                return Err(failure(
                    ErrorCode::Conflict,
                    "A planned disk target was opened after preview",
                ));
            }
        }
        Ok(())
    }
}

fn ensure_native<'a>(record: &'a mut Record, tab: &OpenTab) -> &'a mut NativeTarget {
    let version = native_version(tab);
    let index = record
        .native
        .iter()
        .position(|target| target.current.id == version.id)
        .unwrap_or_else(|| {
            record.native.push(NativeTarget {
                current: version.clone(),
                text: None,
                changed: false,
            });
            record.native.len() - 1
        });
    &mut record.native[index]
}

fn display_target(target: &FileTarget) -> String {
    if target.path.is_empty() {
        t!("workspace_plan.selected_target").to_string()
    } else {
        target.path.clone()
    }
}
fn excerpt(text: &str) -> String {
    let mut chars = text.chars();
    let mut result: String = chars.by_ref().take(256).collect();
    if chars.next().is_some() {
        result.push_str(&t!("workspace_plan.excerpt"));
    }
    result
}
