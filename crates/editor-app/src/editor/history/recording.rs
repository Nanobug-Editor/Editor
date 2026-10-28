//! Ordered immutable observations share one bounded submission during a native restore/watcher turn.

use super::*;
use editor_history::{Capture, FileId, SkipReason, VersionId};

/// Bound a batch independently of the worker's sixteen submissions and shared 64 MiB reservation.
const BATCH_BYTES: usize = 64 * 1024 * 1024;
const BATCH_RECORDS: usize = 4096;

/// A synchronous scope retains only immutable observations, in their original publication order.
#[derive(Default)]
pub(super) struct Batch {
    records: Vec<Record>,
    /// Includes inputs and a conservative allowance for metadata retained in the reply.
    retained: usize,
}

impl Batch {
    /// Reject only the new observation on exhaustion; previously accepted records remain queued.
    fn push(&mut self, record: Record) -> editor_history::Result<()> {
        let retained = self
            .retained
            .checked_add(record.retained())
            .filter(|bytes| *bytes <= BATCH_BYTES)
            .filter(|_| self.records.len() < BATCH_RECORDS)
            .ok_or(editor_history::Error::Busy)?;
        self.retained = retained;
        self.records.push(record);
        Ok(())
    }
}

/// Background input preserves exact bytes and the native target guard captured by its publisher.
enum Record {
    Capture {
        capture: Capture,
        document: Option<u64>,
        origin: Option<VersionId>,
    },
    Disk {
        path: PathBuf,
        workspace: Option<PathBuf>,
        reason: Reason,
    },
    Removal {
        path: PathBuf,
        root: PathBuf,
        known: bool,
        local: Option<platform_windows::history::FileStamp>,
    },
    Move {
        old: PathBuf,
        new: PathBuf,
        root: PathBuf,
    },
}

/// Small receipts are applied only to the still-open matching native document incarnation.
enum Receipt {
    Captured {
        path: PathBuf,
        document: Option<u64>,
        identity: Option<FileId>,
        reason: Reason,
        skipped: Option<SkipReason>,
    },
    Observed {
        path: PathBuf,
        skipped: Option<SkipReason>,
    },
    Changed,
}

impl Record {
    /// Account for payload, native paths and reply metadata rather than hiding them in a zero-byte job.
    fn retained(&self) -> usize {
        let path_bytes = |path: &Path| path.as_os_str().len().saturating_mul(4);
        let optional = |path: &Option<PathBuf>| path.as_deref().map_or(0, path_bytes);
        let bytes = match self {
            Self::Capture { capture, .. } => capture
                .contents
                .len()
                .saturating_add(path_bytes(&capture.path))
                .saturating_add(optional(&capture.workspace)),
            Self::Disk {
                path, workspace, ..
            } => path_bytes(path).saturating_add(optional(workspace)),
            Self::Removal { path, root, .. } => path_bytes(path).saturating_add(path_bytes(root)),
            Self::Move { old, new, root } => path_bytes(old)
                .saturating_add(path_bytes(new))
                .saturating_add(path_bytes(root)),
        };
        bytes.saturating_add(1024)
    }

    /// Execute independently: one failed observation must not discard later successful states.
    fn apply(self, history: &History) -> editor_history::Result<Receipt> {
        match self {
            Self::Capture {
                capture,
                document,
                origin,
            } => {
                let path = capture.path.clone();
                let reason = capture.reason;
                history.import_known(&path, capture.workspace.as_deref())?;
                let outcome = match origin {
                    Some(origin) => history.capture_copy(capture, origin)?,
                    None => history.capture(capture)?,
                };
                Ok(Receipt::Captured {
                    identity: history.file_at(&path)?,
                    path,
                    document,
                    reason,
                    skipped: match outcome {
                        editor_history::CaptureOutcome::Skipped(reason) => Some(reason),
                        _ => None,
                    },
                })
            }
            Self::Disk {
                path,
                workspace,
                reason,
            } => {
                let skipped = observe_disk(history, &path, workspace.as_deref(), reason)?;
                Ok(Receipt::Observed { path, skipped })
            }
            Self::Move { old, new, root } => {
                observations::record_move(history, &old, &new, &root)?;
                Ok(Receipt::Changed)
            }
            Self::Removal {
                path,
                root,
                known,
                local,
            } => {
                if !history.policy()?.enabled {
                    return Ok(Receipt::Changed);
                }
                let current = platform_windows::history::FileStamp::read(&path);
                if !known && current.as_ref().is_ok_and(Option::is_some) {
                    return Ok(Receipt::Changed);
                }
                if known
                    && local.is_some()
                    && current.as_ref().ok().and_then(Option::as_ref) == local.as_ref()
                {
                    return Ok(Receipt::Changed);
                }
                for (file, path) in history.bindings_under(&path)? {
                    let location = history.location(file)?;
                    history.change(editor_history::Change {
                        file,
                        path,
                        workspace: location.workspace,
                        reason: Reason::Deleted,
                        present: false,
                        at_ms: now_ms(),
                    })?;
                }
                if path.is_file() {
                    observe_disk(
                        history,
                        &path,
                        path.starts_with(&root).then_some(root.as_path()),
                        Reason::External,
                    )?;
                }
                Ok(Receipt::Changed)
            }
        }
    }
}

impl EditorApp {
    /// Begin a synchronous scope; nested callers leave flushing to the scope that created it.
    pub(crate) fn begin_history_batch(&mut self) -> bool {
        if self.history.batch.is_some() {
            return false;
        }
        self.history.batch = Some(Batch::default());
        true
    }

    /// Eagerly submit accepted observations before queries or shutdown can pass them in the FIFO.
    pub(crate) fn finish_history_batch(&mut self, owner: bool, cx: &mut Context<Self>) {
        if owner && let Some(batch) = self.history.batch.take() {
            self.submit_history_batch(batch, cx);
        }
    }

    /// Scope-free callers retain the existing eager admission and visible resource-limit failures.
    fn enqueue_history_record(&mut self, record: Record, cx: &mut Context<Self>) {
        if self.history.worker.is_none() {
            return;
        }
        if let Some(batch) = &mut self.history.batch {
            if let Err(error) = batch.push(record) {
                self.history_failure(error.to_string(), cx);
            }
        } else {
            let mut batch = Batch::default();
            match batch.push(record) {
                Ok(()) => self.submit_history_batch(batch, cx),
                Err(error) => self.history_failure(error.to_string(), cx),
            }
        }
    }

    /// Reserve the complete batch once; each item keeps its own outcome and target validation.
    fn submit_history_batch(&mut self, batch: Batch, cx: &mut Context<Self>) {
        if batch.records.is_empty() {
            return;
        }
        let Some(worker) = self.history.worker.clone() else {
            return;
        };
        let recorded = worker.call(batch.retained, move |history| {
            Ok(batch
                .records
                .into_iter()
                .map(|record| record.apply(history))
                .collect::<Vec<_>>())
        });
        cx.spawn(async move |app, cx| {
            let _ = app.update(cx, |app, cx| {
                if let Some(error) = worker.status().error {
                    app.history_failure(error, cx);
                }
            });
            let result = recorded.await;
            let _ = app.update(cx, |app, cx| match result {
                Err(error) => app.history_failure(error.to_string(), cx),
                Ok(results) => {
                    for result in results {
                        match result {
                            Err(error) => app.history_failure(error.to_string(), cx),
                            Ok(Receipt::Changed) => {}
                            Ok(Receipt::Observed { path, skipped }) => {
                                if let Some(reason) = skipped {
                                    app.history_skipped(path, reason, cx);
                                }
                            }
                            Ok(Receipt::Captured {
                                path,
                                document,
                                identity,
                                reason,
                                skipped,
                            }) => {
                                if let Some(reason) = skipped {
                                    app.history_skipped(path.clone(), reason, cx);
                                } else if app
                                    .history
                                    .skipped
                                    .as_ref()
                                    .is_some_and(|(target, _)| *target == path)
                                {
                                    app.history.skipped = None;
                                }
                                if let Some((identity, id)) = identity.zip(document.filter(|id| {
                                    app.tabs
                                        .iter()
                                        .any(|file| file.file_id == *id && file.path() == path)
                                })) {
                                    if reason == Reason::Saved
                                        || !app.history.identities.contains_key(&id)
                                    {
                                        app.history.identities.insert(id, identity);
                                    }
                                }
                            }
                        }
                    }
                }
            });
        })
        .detach();
    }

    /// Reliable removals distinguish known recreation from this application's own atomic saves.
    pub(crate) fn observe_history_removal(
        &mut self,
        path: PathBuf,
        known_event: bool,
        cx: &mut Context<Self>,
    ) {
        let local = self.history.local_saves.get(&path).cloned();
        self.enqueue_history_record(
            Record::Removal {
                path,
                root: self.workspace.root().to_owned(),
                known: known_event,
                local,
            },
            cx,
        );
    }

    /// Reliable native rename pairs and successful host moves retain the original logical identity.
    pub(crate) fn record_history_move(
        &mut self,
        old: PathBuf,
        new: PathBuf,
        cx: &mut Context<Self>,
    ) {
        self.enqueue_history_record(
            Record::Move {
                old,
                new,
                root: self.workspace.root().to_owned(),
            },
            cx,
        );
    }

    /// Existing watcher/open reads remain immutable and never replace an unsaved native buffer.
    pub(crate) fn observe_history_path(
        &mut self,
        path: PathBuf,
        reason: Reason,
        cx: &mut Context<Self>,
    ) {
        let workspace = path
            .starts_with(self.workspace.root())
            .then(|| self.workspace.root().to_owned());
        self.enqueue_history_record(
            Record::Disk {
                path,
                workspace,
                reason,
            },
            cx,
        );
    }

    /// Enqueue exact successfully saved bytes before the next file observation can be admitted.
    pub(crate) fn record_history_save(
        &mut self,
        path: PathBuf,
        value: String,
        cx: &mut Context<Self>,
    ) {
        if let Ok(Some(stamp)) = platform_windows::history::FileStamp::read(&path) {
            self.history.local_saves.insert(path.clone(), stamp);
        }
        if let Some(file) = self.tabs.iter().find(|file| file.path() == path) {
            if let Some(recovery) = self.history.pending_create.get_mut(&file.file_id) {
                recovery.needs_create = false;
            }
        }
        self.record_history_observation(path, value, Reason::Saved, cx);
    }

    /// Retain each observed state, including A/B/A, without merging paths or rereading later disk bytes.
    pub(crate) fn record_history_observation(
        &mut self,
        path: PathBuf,
        value: String,
        reason: Reason,
        cx: &mut Context<Self>,
    ) {
        if self.history.worker.is_none() {
            return;
        }
        if value.len() > 32 * 1024 * 1024 {
            self.history_skipped(path, SkipReason::TooLarge, cx);
            return;
        }
        let workspace = path
            .starts_with(self.workspace.root())
            .then(|| self.workspace.root().to_owned());
        let document = self
            .tabs
            .iter()
            .find(|file| file.path() == path)
            .map(|file| file.file_id);
        let recovery = document.and_then(|id| self.history.pending_create.get(&id));
        let identity = if reason == Reason::Saved {
            recovery
                .and_then(|recovery| recovery.file)
                .or_else(|| document.and_then(|id| self.history.identities.get(&id).copied()))
        } else {
            None
        };
        let origin = recovery
            .filter(|recovery| recovery.file.is_none() && reason == Reason::Saved)
            .map(|recovery| recovery.origin);
        self.enqueue_history_record(
            Record::Capture {
                capture: Capture {
                    file: identity,
                    path,
                    workspace,
                    contents: value.into_bytes(),
                    reason,
                    at_ms: now_ms(),
                },
                document,
                origin,
            },
            cx,
        );
    }
}
