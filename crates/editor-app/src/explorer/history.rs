//! Required disk preimages and temporary leases precede destructive explorer removal.
#[cfg(test)]
mod tests;

use super::*;
use crate::editor::history::now_ms;
use editor_history::{Capture, Change, FileId, History, Lease, Reason};
use futures::FutureExt as _;
use std::io::Read as _;
use std::sync::atomic::{AtomicBool, Ordering};

/// Guard metadata is bounded independently of a directory's total content size.
const MAX_GUARD_BYTES: usize = 32 * 1024 * 1024;

/// The final background validation checks every observed file and directory membership token.
enum Node {
    /// File bytes were read and their native identity did not change during protection.
    File(
        PathBuf,
        Option<platform_windows::history::FileStamp>,
        Option<FileId>,
    ),
    /// Child creation/removal changes directory metadata, including excluded children.
    Directory(PathBuf, std::time::SystemTime),
}

/// Protection leases remain live until actual deletion outcomes have been reconciled.
struct Guard {
    /// Observed native state, with no copied document contents after capture.
    nodes: Vec<Node>,
    /// No maintenance/clear can reclaim a successfully prepared deletion preimage.
    _leases: Vec<Lease>,
}

/// Protect eligible bytes without inheriting Git/explorer ignore rules or following reparse points.
fn prepare(
    history: Option<&History>,
    root: &Path,
    path: &Path,
    cancel: &AtomicBool,
) -> Result<Guard, String> {
    if path.canonicalize().map_err(|error| error.to_string())? == root
        || !path.starts_with(root)
        || root.canonicalize().map_err(|error| error.to_string())? != root
    {
        return Err(t!("explorer.delete_outside_workspace").to_string());
    }
    let policy = history
        .map(History::policy)
        .transpose()
        .map_err(|error| error.to_string())?;
    let history = history.filter(|_| policy.as_ref().is_some_and(|policy| policy.enabled));
    let mut guard = Guard {
        nodes: Vec::new(),
        _leases: Vec::new(),
    };
    let mut pending = vec![path.to_owned()];
    let mut retained = path.as_os_str().len().saturating_mul(2) + 128;
    while let Some(path) = pending.pop() {
        if cancel.load(Ordering::Acquire) {
            return Err(t!("history.cancelled").to_string());
        }
        let metadata = std::fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
        if is_link(&metadata) {
            return Err(t!("explorer.delete_symlink").to_string());
        }
        if metadata.is_dir() {
            guard.nodes.push(Node::Directory(
                path.clone(),
                metadata.modified().map_err(|error| error.to_string())?,
            ));
            if history.is_some() && policy.as_ref().unwrap().skip_path(&path).is_none() {
                for entry in std::fs::read_dir(&path).map_err(|error| error.to_string())? {
                    let entry = entry.map_err(|error| error.to_string())?;
                    if !is_link(&entry.metadata().map_err(|error| error.to_string())?) {
                        let path = entry.path();
                        retained += path.as_os_str().len().saturating_mul(2) + 128;
                        if retained > MAX_GUARD_BYTES {
                            return Err(t!("history.delete_budget").to_string());
                        }
                        pending.push(path);
                    }
                }
            }
            continue;
        }
        let stamp =
            platform_windows::history::FileStamp::read(&path).map_err(|error| error.to_string())?;
        let mut identity = None;
        if let Some(history) =
            history.filter(|_| policy.as_ref().unwrap().skip_path(&path).is_none())
        {
            let policy = policy.as_ref().unwrap();
            history
                .import_known(&path, Some(root))
                .map_err(|error| error.to_string())?;
            let file = history
                .track(&path, Some(root), crate::editor::history::now_ms())
                .map_err(|error| error.to_string())?;
            identity = Some(file);
            let mut bytes = Vec::new();
            if metadata.len() <= policy.max_file_bytes {
                std::fs::File::open(&path)
                    .and_then(|file| file.take(policy.max_file_bytes + 1).read_to_end(&mut bytes))
                    .map_err(|error| error.to_string())?;
            }
            let lease = if metadata.len() <= policy.max_file_bytes
                && policy.skip(&path, &bytes).is_none()
            {
                history.capture_pinned(Capture {
                    file: Some(file),
                    path: path.clone(),
                    workspace: Some(root.to_owned()),
                    contents: bytes,
                    reason: Reason::BeforeDelete,
                    at_ms: now_ms(),
                })
            } else {
                history
                    .change_without_content(Change {
                        file,
                        path: path.clone(),
                        workspace: Some(root.to_owned()),
                        reason: Reason::BeforeDelete,
                        present: true,
                        at_ms: now_ms(),
                    })
                    .and_then(|version| history.pin(version))
            }
            .map_err(|error| error.to_string())?;
            guard._leases.push(lease);
        }
        if platform_windows::history::FileStamp::read(&path).map_err(|error| error.to_string())?
            != stamp
        {
            return Err(t!("history.stale").to_string());
        }
        guard.nodes.push(Node::File(path, stamp, identity));
    }
    Ok(guard)
}

/// Recheck prepared observations, perform removal, then publish only paths actually absent.
fn remove(history: Option<&History>, root: &Path, path: &Path, guard: Guard) -> Result<(), String> {
    for node in &guard.nodes {
        let unchanged = match node {
            Node::File(path, stamp, _) => {
                platform_windows::history::FileStamp::read(path)
                    .map_err(|error| error.to_string())?
                    == *stamp
            }
            Node::Directory(path, modified) => {
                std::fs::symlink_metadata(path).is_ok_and(|metadata| {
                    metadata.is_dir()
                        && !is_link(&metadata)
                        && metadata.modified().ok().as_ref() == Some(modified)
                })
            }
        };
        if !unchanged {
            return Err(t!("history.stale").to_string());
        }
    }
    let result = files::delete(root, path);
    let mut metadata_errors = Vec::new();
    if let Some(history) = history {
        for node in &guard.nodes {
            if let Node::File(path, _, Some(file)) = node {
                if std::fs::symlink_metadata(path)
                    .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
                    && let Err(error) = history.change(Change {
                        file: *file,
                        path: path.clone(),
                        workspace: Some(root.to_owned()),
                        reason: Reason::Deleted,
                        present: false,
                        at_ms: now_ms(),
                    })
                {
                    metadata_errors.push(error.to_string());
                }
            }
        }
    }
    match (result, metadata_errors.is_empty()) {
        (Ok(()), true) => Ok(()),
        (Err(error), true) => Err(error),
        (result, false) => {
            Err([result.err().unwrap_or_default(), metadata_errors.join("\n")].join("\n"))
        }
    }
}

/// Windows junctions are reparse points even when their metadata does not report a Unix-style symlink.
fn is_link(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

impl EditorApp {
    /// Normal Delete requires successful protection; bypass is available only through its explicit failure action.
    pub(super) fn begin_history_delete(&mut self, unprotected: bool, cx: &mut Context<Self>) {
        let Some(delete) = &mut self.explorer_delete else {
            return;
        };
        if delete.busy || (unprotected && !delete.backup_failed) {
            return;
        }
        let path = delete.path.clone();
        if self.tabs.iter().any(|tab| tab.path().starts_with(&path)) {
            self.report_host_message(
                MessageLevel::Warning,
                t!("explorer.close_before_delete").to_string(),
                cx,
            );
            return;
        }
        delete.busy = true;
        delete.error = None;
        delete.backup_failed = false;
        let cancel = delete.cancel.clone();
        let marker = cancel.clone();
        let root = self.workspace.root().to_owned();
        let worker = self.history.worker.clone();
        cx.spawn(async move |app, cx| {
            let prepared = if unprotected {
                let target = path.clone();
                let workspace = root.clone();
                let cancel = cancel.clone();
                cx.background_executor()
                    .spawn(async move { prepare(None, &workspace, &target, &cancel) })
                    .await
            } else if let Some(worker) = &worker {
                let target = path.clone();
                let workspace = root.clone();
                let cancel = cancel.clone();
                worker
                    .call(0, move |history| {
                        prepare(Some(history), &workspace, &target, &cancel).map_err(|error| {
                            editor_history::Error::Io(std::io::Error::other(error))
                        })
                    })
                    .await
                    .map_err(|error| error.to_string())
            } else {
                Err(t!("history.unavailable_delete").to_string())
            };
            let _ = app.update(cx, |app, cx| {
                let current = app
                    .explorer_delete
                    .as_ref()
                    .is_some_and(|delete| Arc::ptr_eq(&delete.cancel, &marker));
                if !current || cancel.load(Ordering::Acquire) || app.shutting_down {
                    return;
                }
                let guard = match prepared {
                    Ok(guard) => guard,
                    Err(error) => {
                        let message =
                            t!("history.delete_protection_failed", error = error).to_string();
                        let delete = app.explorer_delete.as_mut().unwrap();
                        delete.busy = false;
                        delete.backup_failed = true;
                        delete.error = Some(message.clone());
                        app.report_host_message(MessageLevel::Error, message, cx);
                        cx.notify();
                        return;
                    }
                };
                if app.tabs.iter().any(|tab| tab.path().starts_with(&path)) {
                    app.cancel_explorer_delete(cx);
                    return;
                }
                app.explorer_delete.as_mut().unwrap().committing = true;
                let task = cx
                    .background_executor()
                    .spawn(async move {
                        if !unprotected && let Some(worker) = worker {
                            worker
                                .call(0, move |history| {
                                    remove(Some(history), &root, &path, guard).map_err(|error| {
                                        editor_history::Error::Io(std::io::Error::other(error))
                                    })
                                })
                                .await
                                .map_err(|error| error.to_string())
                        } else {
                            remove(None, &root, &path, guard)
                        }
                    })
                    .shared();
                app.explorer_delete.as_mut().unwrap().completion = Some(task.clone());
                cx.spawn(async move |owner, cx| {
                    let result = task.await;
                    let _ = owner.update(cx, |app, cx| {
                        if !app
                            .explorer_delete
                            .as_ref()
                            .is_some_and(|delete| Arc::ptr_eq(&delete.cancel, &marker))
                        {
                            return;
                        }
                        match result {
                            Ok(()) => app.explorer_delete = None,
                            Err(error) => {
                                let message =
                                    t!("explorer.delete_failed", error = error).to_string();
                                let delete = app.explorer_delete.as_mut().unwrap();
                                delete.busy = false;
                                delete.committing = false;
                                delete.error = Some(message.clone());
                                app.report_host_message(MessageLevel::Error, message, cx);
                            }
                        }
                        app.file_watch.reconcile();
                        app.refresh_files(cx);
                        cx.notify();
                    });
                })
                .detach();
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// Cancellation seals preparation; an already authorized recursive removal is awaited on shutdown.
    pub(crate) fn history_delete_cleanup(
        &mut self,
    ) -> Option<futures::future::Shared<gpui_kit::Task<Result<(), String>>>> {
        self.explorer_delete.as_ref().and_then(|delete| {
            delete.cancel.store(true, Ordering::Release);
            delete.completion.clone()
        })
    }
}
