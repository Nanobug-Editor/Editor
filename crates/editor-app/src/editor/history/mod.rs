//! Native history integration owns metadata and immutable jobs, never another editable document.

mod profiles;
pub(crate) use profiles::{State, load_policy};
mod saves;
pub(crate) use saves::disk_digest;
mod browsing;
mod cleanup;
mod comparison;
mod export;
mod model;
/// File-operation workers share history preimages without acquiring GPUI entities.
pub(crate) mod observations;
mod panel;
mod recovery;
mod save_sequence;
/// Actual host integration scenarios stay in the shared tests directory without a test-only host API.
#[cfg(test)]
#[path = "../../tests/history.rs"]
pub(crate) mod tests;
use crate::app::messages::MessageLevel;
use crate::*;
use editor_history::{Capture, History, Reason};
use model::{Category, Item, Panel};
use std::io::Read;

/// Supply actual UTC observation times to storage and retention, independent from UI timers.
pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}

/// Read eligible disk bytes with a strict uncompressed bound, then persist the observed state.
/// NotFound is a real absence, not an empty text snapshot. Only successful writes record Saved.
pub(crate) fn observe_disk(
    history: &History,
    path: &Path,
    workspace: Option<&Path>,
    reason: Reason,
) -> editor_history::Result<Option<editor_history::SkipReason>> {
    let policy = history.policy()?;
    if let Some(reason) = policy.skip_path(path) {
        return Ok(Some(reason));
    }
    history.import_known(path, workspace)?;
    let mut file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if !file.metadata()?.is_file() {
        return Ok(Some(editor_history::SkipReason::Unsupported));
    }
    if file.metadata()?.len() > policy.max_file_bytes {
        return Ok(Some(editor_history::SkipReason::TooLarge));
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(policy.max_file_bytes + 1)
        .read_to_end(&mut bytes)?;
    let result = history.capture(Capture {
        file: None,
        path: path.to_owned(),
        workspace: workspace.map(Path::to_owned),
        contents: bytes,
        reason,
        at_ms: now_ms(),
    })?;
    Ok(match result {
        editor_history::CaptureOutcome::Skipped(reason) => Some(reason),
        _ => None,
    })
}

/// Localized skip reasons describe user policy separately from failed persistence.
fn skip_message(path: &Path, reason: editor_history::SkipReason) -> String {
    let reason = match reason {
        editor_history::SkipReason::Disabled => t!("history.skip_disabled"),
        editor_history::SkipReason::Excluded => t!("history.skip_excluded"),
        editor_history::SkipReason::TooLarge => t!("history.skip_size"),
        editor_history::SkipReason::Unsupported => t!("history.skip_format"),
    };
    t!(
        "history.skipped",
        path = editor_core::display_path(path).to_string(),
        reason = reason
    )
    .to_string()
}

impl EditorApp {
    /// Retire document-bound recovery metadata with the actual native session incarnation.
    pub(crate) fn release_history_document(&mut self, id: u64, path: &Path) {
        self.history.pending_create.remove(&id);
        self.history.identities.remove(&id);
        self.history.local_saves.remove(path);
    }

    /// Reliable removal events distinguish known path recreation from this app's own atomic saves.
    pub(crate) fn observe_history_removal(
        &mut self,
        path: PathBuf,
        known_event: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(worker) = self.history.worker.clone() else {
            return;
        };
        let local = self.history.local_saves.get(&path).cloned();
        let root = self.workspace.root().to_owned();
        let observed = worker.call(0, move |history| {
            if !history.policy()?.enabled {
                return Ok(());
            }
            let current = platform_windows::history::FileStamp::read(&path);
            if !known_event && current.as_ref().is_ok_and(Option::is_some) {
                return Ok(());
            }
            if known_event
                && local.is_some()
                && current.as_ref().ok().and_then(Option::as_ref) == local.as_ref()
            {
                return Ok(());
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
            Ok(())
        });
        cx.spawn(async move |app, cx| {
            if let Err(error) = observed.await {
                let _ = app.update(cx, |app, cx| app.history_failure(error.to_string(), cx));
            }
        })
        .detach();
    }

    /// A corrected profile/directory can be retried without restarting or discarding the current view.
    pub(super) fn reconnect_history(&mut self, cx: &mut Context<Self>) {
        let retry = self.history.worker.as_ref().is_none_or(|worker| {
            let status = worker.status();
            status.initialized && !status.available
        });
        if !retry {
            return;
        }
        let profile = self
            .history
            .paths
            .as_ref()
            .and_then(|paths| paths.settings.parent())
            .map(Path::to_owned)
            .or_else(|| {
                self.session_state
                    .file_path()
                    .and_then(|path| path.parent().map(Path::to_owned))
            });
        let recovered = State::new(profile.as_deref(), Some(self.workspace.root()), cx);
        self.history.worker = recovered.worker;
        if recovered.paths.is_some() {
            self.history.paths = recovered.paths;
        }
    }

    /// Drain accepted history writes before project teardown; failed history cannot trap a normal close.
    pub(crate) fn shutdown_history(&mut self, cx: &mut Context<Self>) -> gpui_kit::Task<()> {
        self.history.saves_sealed = true;
        self.history.save_generation = self.history.save_generation.wrapping_add(1);
        let deletion = self.history_delete_cleanup();
        self.history.generation = self.history.generation.wrapping_add(1);
        self.history.picker.take();
        self.clear_history_preview(cx);
        self.history.panel = None;
        let worker = self.history.worker.clone();
        let executor = cx.background_executor().clone();
        cx.background_executor().spawn(async move {
            if let Some(deletion) = deletion {
                let _ = deletion.await;
            }
            if let Some(worker) = worker {
                loop {
                    match worker.call(0, |_| Ok(())).await {
                        Err(editor_history::Error::Busy) => {
                            executor.timer(Duration::from_millis(10)).await
                        }
                        _ => break,
                    }
                }
            }
        })
    }

    /// A failed close permits new explicit saves while its old preparations remain revoked.
    /// Other stopped project resources retain their existing shutdown state.
    pub(crate) fn resume_history_saves(&mut self, cx: &mut Context<Self>) {
        self.history.saves_sealed = false;
        cx.notify();
    }

    /// Reliable native rename pairs and successful host moves transfer retained logical identities.
    pub(crate) fn record_history_move(
        &mut self,
        old: PathBuf,
        new: PathBuf,
        cx: &mut Context<Self>,
    ) {
        let Some(worker) = self.history.worker.clone() else {
            return;
        };
        let root = self.workspace.root().to_owned();
        let changed = worker.call(0, move |history| {
            observations::record_move(history, &old, &new, &root)
        });
        cx.spawn(async move |app, cx| {
            if let Err(error) = changed.await {
                let _ = app.update(cx, |app, cx| app.history_failure(error.to_string(), cx));
            }
        })
        .detach();
    }

    /// Surface initialization failure after the app exists; a failed history never disables Save.
    pub(crate) fn initialize_history(&mut self, cx: &mut Context<Self>) {
        let Some(worker) = self.history.worker.clone() else {
            if let Some(error) = self.history.last_error.take() {
                self.history_failure(error, cx);
            }
            return;
        };
        let initialized = worker.call(0, |_| Ok(()));
        cx.spawn(async move |app, cx| {
            if let Err(error) = initialized.await {
                let _ = app.update(cx, |app, cx| app.history_failure(error.to_string(), cx));
            }
        })
        .detach();
    }

    /// Retain one actionable failure without repeating the same notification for every save.
    pub(crate) fn history_failure(&mut self, error: String, cx: &mut Context<Self>) {
        if self.history.last_error.as_ref() == Some(&error) {
            return;
        }
        self.history.last_error = Some(error.clone());
        self.report_host_message(
            MessageLevel::Warning,
            t!("history.failed", error = error).to_string(),
            cx,
        );
        cx.notify();
    }

    /// Explain a skipped observation once; ordinary saves keep their own successful disk result.
    fn history_skipped(
        &mut self,
        path: PathBuf,
        reason: editor_history::SkipReason,
        cx: &mut Context<Self>,
    ) {
        if self.history.skipped.as_ref() == Some(&(path.clone(), reason)) {
            return;
        }
        let message = skip_message(&path, reason);
        self.history.skipped = Some((path.clone(), reason));
        if let Some(panel) = &mut self.history.panel {
            if matches!(&panel.category, Category::File(target) if target == &path) {
                panel.notice = Some(message.clone());
            }
        }
        self.report_host_message(MessageLevel::Info, message, cx);
        cx.notify();
    }

    /// Existing watcher/open observations stay immutable and never replace an unsaved native buffer.
    pub(crate) fn observe_history_path(
        &mut self,
        path: PathBuf,
        reason: Reason,
        cx: &mut Context<Self>,
    ) {
        let Some(worker) = self.history.worker.clone() else {
            return;
        };
        let workspace = path
            .starts_with(self.workspace.root())
            .then(|| self.workspace.root().to_owned());
        let target = path.clone();
        let observed = worker.call(0, move |history| {
            observe_disk(history, &path, workspace.as_deref(), reason)
        });
        cx.spawn(async move |app, cx| {
            let result = observed.await;
            let _ = app.update(cx, |app, cx| match result {
                Err(error) => app.history_failure(error.to_string(), cx),
                Ok(Some(reason)) => app.history_skipped(target, reason, cx),
                Ok(None) => {}
            });
        })
        .detach();
    }

    /// Enqueue the exact successfully saved bytes immediately, before the next file observation.
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

    /// Immutable save/watcher results preserve the state actually observed, even if disk changes again.
    pub(crate) fn record_history_observation(
        &mut self,
        path: PathBuf,
        value: String,
        reason: Reason,
        cx: &mut Context<Self>,
    ) {
        let Some(worker) = self.history.worker.clone() else {
            return;
        };
        if value.len() > 32 * 1024 * 1024 {
            self.history_skipped(path, editor_history::SkipReason::TooLarge, cx);
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
        let target = path.clone();
        let recorded = worker.call(value.len(), move |history| {
            history.import_known(&path, workspace.as_deref())?;
            let capture = Capture {
                file: identity,
                path: path.clone(),
                workspace,
                contents: value.into_bytes(),
                reason,
                at_ms: now_ms(),
            };
            let outcome = match origin {
                Some(origin) => history.capture_copy(capture, origin)?,
                None => history.capture(capture)?,
            };
            Ok((
                history.file_at(&path)?,
                match outcome {
                    editor_history::CaptureOutcome::Skipped(reason) => Some(reason),
                    _ => None,
                },
            ))
        });
        cx.spawn(async move |app, cx| {
            let _ = app.update(cx, |app, cx| {
                let maintenance = worker.status().error;
                if let Some(error) = maintenance {
                    app.history_failure(error, cx);
                }
            });
            let result = recorded.await;
            let _ = app.update(cx, |app, cx| match result {
                Ok((identity, skipped)) => {
                    if let Some(reason) = skipped {
                        app.history_skipped(target.clone(), reason, cx);
                    } else if app
                        .history
                        .skipped
                        .as_ref()
                        .is_some_and(|(path, _)| *path == target)
                    {
                        app.history.skipped = None;
                    }
                    if let Some((identity, id)) = identity.zip(document.filter(|id| {
                        app.tabs
                            .iter()
                            .any(|file| file.file_id == *id && file.path() == target)
                    })) {
                        if reason == Reason::Saved || !app.history.identities.contains_key(&id) {
                            app.history.identities.insert(id, identity);
                        }
                    }
                }
                Err(error) => app.history_failure(error.to_string(), cx),
            });
        })
        .detach();
    }
}
