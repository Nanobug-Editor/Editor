//! Mixed plans coordinate immutable recovery data with the unique native sessions and secure disk IO.
//! Cancellation and failures preserve actual receipts; neither promises automatic global rollback.

use super::*;
use crate::editor::text_transactions::PreparedTextEdits;
use futures::channel::oneshot;
use plugin_runtime::{
    EditorRequest, WorkspaceBatch, WorkspaceRequest,
    workspace_io::{self, FilePlan},
};
use protocol::{
    api::{EditorOperation, EditorValue, ErrorCode, Failure, TextEdit, TextRange},
    workspace::{self, BatchState, Change, FileTarget, Operation, Plan, Receipt},
};
use std::collections::{BTreeMap, BTreeSet};

mod execution;
mod preparation;
mod presentation;

type BatchKey = (String, String, u64);
fn key(batch: &WorkspaceBatch) -> BatchKey {
    let handle = batch.handle();
    (
        handle.instance.clone(),
        handle.scope.clone(),
        handle.resource,
    )
}

/// Global retention limits apply across plugins, instead of relying only on per-instance quotas.
#[derive(Default)]
pub(crate) struct WorkspaceChanges {
    records: BTreeMap<BatchKey, Record>,
    prompt: Option<Preview>,
    active: bool,
    /// A bounded queue preserves paired external renames during host IO; overflow requests a rescan.
    deferred: Vec<Reconciliation>,
    reconciling: bool,
    /// The host sequence spans shared native watch batches and is unrelated to document revisions.
    file_sequence: u64,
}

struct Preview {
    request: EditorRequest,
    batch: Arc<WorkspaceBatch>,
    title: String,
    source: String,
    details: Vec<String>,
    /// Source-only conditions and edit targets expire the preview before consent; no text is copied.
    documents: Vec<NativeVersion>,
    focus: FocusHandle,
    previous: Option<FocusHandle>,
    answer: Option<oneshot::Sender<Result<bool, Failure>>>,
}

/// A native token captures close/reopen and silent edits as well as ordinary user Undo, even if bytes match.
#[derive(Clone, PartialEq, Eq)]
struct NativeVersion {
    id: String,
    path: PathBuf,
    revision: u64,
}

struct NativeTarget {
    current: NativeVersion,
    /// Only text mutations retain immutable source/selection; rename-only targets retain no text copy.
    text: Option<(String, TextRange)>,
    changed: bool,
}

struct NativeEdit {
    id: String,
    prepared: PreparedTextEdits,
}

struct Record {
    batch: Arc<WorkspaceBatch>,
    title: String,
    source: String,
    /// Readonly language source is a precondition, not a text preimage or recovery target.
    origin: Option<NativeVersion>,
    /// Readonly dependency revisions protect preview-to-commit and never enter recovery targets.
    conditions: Vec<NativeVersion>,
    changes: Vec<Change>,
    native: Vec<NativeTarget>,
    edits: BTreeMap<u16, NativeEdit>,
    watched: BTreeSet<PathBuf>,
    files: Option<FilePlan>,
    completed: Vec<u16>,
    retained_bytes: usize,
    renames: Vec<(PathBuf, PathBuf)>,
}

impl EditorApp {
    /// Publish after the existing workspace reconciliation; private/external native paths are absent.
    pub(crate) fn publish_workspace_file_events(
        &mut self,
        events: Vec<workspace::FileEventKind>,
        cx: &mut Context<Self>,
    ) {
        // File/configuration changes invalidate pending language proposals before publication.
        if !events.is_empty() {
            self.invalidate_language_context_snapshots();
        }
        if !self.session_state.workspace_trusted {
            return;
        }
        self.extensions.update(cx, |panel, _| {
            let mut state = panel.worker.state.lock().unwrap();
            for kind in events {
                self.workspace_changes.file_sequence =
                    self.workspace_changes.file_sequence.saturating_add(1);
                state.file_events.push(workspace::FileEvent {
                    sequence: self.workspace_changes.file_sequence,
                    kind,
                });
            }
        });
    }
    /// Query results overlay unsaved content only with the separate document read capability and permission.
    pub(crate) fn perform_workspace_request(
        &mut self,
        plugin: &str,
        request: EditorRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let EditorOperation::Workspace { operation } = request.operation() else {
            unreachable!()
        };
        let operation = operation.clone();
        let Some(authority) = request.workspace_request().cloned() else {
            request.finish(Err(failure(
                ErrorCode::InvalidHandle,
                "Missing original workspace authority",
            )));
            return;
        };
        match operation {
            Operation::ApplyPlan { plan } => {
                self.prepare_workspace_plan(plugin, request, authority, plan, window, cx)
            }
            Operation::BatchStatus { .. } => {
                request.finish(
                    authority
                        .batch()
                        .ok_or_else(|| failure(ErrorCode::InvalidHandle, "Batch unavailable"))
                        .map(|batch| {
                            EditorValue::Workspace(workspace::Value::Batch(batch.receipt()))
                        }),
                );
            }
            Operation::UndoBatch { .. } => {
                let Some(batch) = authority.batch() else {
                    request.finish(Err(failure(ErrorCode::InvalidHandle, "Batch unavailable")));
                    return;
                };
                self.start_workspace_undo(key(batch), Some(request), window, cx);
            }
            Operation::Rescan => {
                self.file_watch.reconcile();
                request.finish(Ok(EditorValue::Workspace(
                    workspace::Value::RescanRequested,
                )));
            }
            operation => self.query_workspace_files(request, authority, operation, cx),
        }
    }

    fn query_workspace_files(
        &mut self,
        request: EditorRequest,
        authority: WorkspaceRequest,
        operation: Operation,
        cx: &mut Context<Self>,
    ) {
        let result = (|| {
            let (target, contents) = match &operation {
                Operation::Stat { target } => (Some(target), false),
                Operation::Read { target } => (Some(target), true),
                Operation::List { target, .. } => (Some(target), false),
                Operation::Discover { .. } => (None, false),
                _ => unreachable!(),
            };
            let mut buffer = None;
            if let Some(target) = target {
                let file = authority.file(target, false)?;
                if let Some(index) = self.tabs.iter().position(|tab| tab.path() == file.path()) {
                    if contents {
                        if !authority.can_read_documents() {
                            return Err(failure(
                                ErrorCode::PermissionDenied,
                                "Unsaved text requires editor.documents and editor.read",
                            ));
                        }
                        let tab = self.text_tab(index).ok_or_else(|| {
                            failure(
                                ErrorCode::UnsupportedOperation,
                                "Opened opaque content has no text read",
                            )
                        })?;
                        let bytes = tab.editor.read(cx).text().to_string().into_bytes();
                        if bytes.len() > 1024 * 1024 {
                            return Err(failure(
                                ErrorCode::LimitExceeded,
                                "Unsaved read exceeds 1 MiB",
                            ));
                        }
                        buffer = Some((native_version(&self.tabs[index]), bytes));
                    }
                }
            }
            Ok(buffer)
        })();
        let buffer = match result {
            Ok(buffer) => buffer,
            Err(error) => {
                request.finish(Err(error));
                return;
            }
        };
        cx.spawn(async move |app, cx| {
            let io_request = request.clone();
            let result = cx
                .background_executor()
                .spawn(async move {
                    if io_request.status().is_terminal() {
                        return Err(failure(ErrorCode::Cancelled, "Query stopped"));
                    }
                    match operation {
                        Operation::Stat { target } | Operation::Read { target } => {
                            let contents = matches!(
                                io_request.operation(),
                                EditorOperation::Workspace {
                                    operation: Operation::Read { .. }
                                }
                            );
                            let (metadata, bytes) = workspace_io::read(
                                authority.file(&target, false)?,
                                contents && buffer.is_none(),
                            )?;
                            Ok((metadata, bytes, buffer, contents, None))
                        }
                        Operation::List {
                            target,
                            page_size,
                            cursor,
                        } => {
                            let page = workspace_io::discover(
                                authority.file(&target, false)?,
                                None,
                                page_size,
                                cursor.as_ref(),
                            )?;
                            Ok((
                                workspace::Metadata {
                                    target,
                                    stamp: workspace::FileStamp::Missing,
                                    document: None,
                                },
                                Vec::new(),
                                None,
                                false,
                                Some(page),
                            ))
                        }
                        Operation::Discover {
                            root,
                            query,
                            page_size,
                            cursor,
                        } => {
                            let target = FileTarget {
                                root,
                                path: String::new(),
                            };
                            let page = workspace_io::discover(
                                authority.file(&target, false)?,
                                Some(&query),
                                page_size,
                                cursor.as_ref(),
                            )?;
                            Ok((
                                workspace::Metadata {
                                    target,
                                    stamp: workspace::FileStamp::Missing,
                                    document: None,
                                },
                                Vec::new(),
                                None,
                                false,
                                Some(page),
                            ))
                        }
                        _ => unreachable!(),
                    }
                })
                .await;
            let result = app
                .update(cx, |app, _| {
                    let (mut metadata, mut bytes, buffer, contents, page) = result?;
                    if let Some(page) = page {
                        return Ok(EditorValue::Workspace(workspace::Value::Page(page)));
                    }
                    if let Some((version, value)) = buffer {
                        if !app.tabs.iter().any(|tab| native_version(tab) == version) {
                            return Err(failure(
                                ErrorCode::StaleRevision,
                                "Opened text changed during read",
                            ));
                        }
                        bytes = value;
                        metadata.document = app
                            .tabs
                            .iter()
                            .position(|tab| native_version(tab) == version)
                            .and_then(|index| app.plugin_document_version(index).ok());
                    }
                    Ok(EditorValue::Workspace(if contents {
                        workspace::Value::Read { metadata, bytes }
                    } else {
                        workspace::Value::Metadata(metadata)
                    }))
                })
                .unwrap_or_else(|_| Err(failure(ErrorCode::InvalidState, "Editor window closed")));
            request.finish(result);
        })
        .detach();
    }
}

fn native_version(tab: &OpenTab) -> NativeVersion {
    NativeVersion {
        id: tab.text.as_ref().map_or_else(
            || format!("file-{}", tab.file_id),
            |text| format!("{:?}", text.editor.entity_id()),
        ),
        path: tab.path().to_owned(),
        revision: tab
            .text
            .as_ref()
            .map_or(tab.file_revision, |text| text.capability_revision),
    }
}
fn failure(code: ErrorCode, message: &str) -> Failure {
    Failure::new(code, message)
}

fn stopped(request: &EditorRequest, batch: &WorkspaceBatch) -> bool {
    request.status().is_terminal() || !batch.is_live()
}

/// Receipt publication is independent of a canceled wait and always includes the actual disk targets.
fn receipt(
    record: &Record,
    state: BatchState,
    stopped_at: Option<u16>,
    error: Option<Failure>,
) -> Receipt {
    Receipt {
        batch: record.batch.handle().clone(),
        state,
        completed: record.completed.clone(),
        stopped_at,
        affected: record
            .files
            .as_ref()
            .map(FilePlan::affected)
            .unwrap_or_default(),
        failed: error,
        conflicts: Vec::new(),
        recoverable: record.files.as_ref().is_some_and(FilePlan::recoverable)
            && (!record.completed.is_empty()
                || record
                    .files
                    .as_ref()
                    .is_some_and(|files| !files.affected().is_empty())),
    }
}
