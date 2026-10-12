//! Project-level consent precedes irreversible cleanup; completion decides home versus application exit.

use super::*;
use gpui_kit::EntityInputHandler as _;
use sha2::{Digest as _, Sha256};

impl ProjectWindow {
    /// Platform close returns false until consent and cleanup have both completed.
    pub(crate) fn request_close(
        &mut self,
        intent: CloseIntent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.close_state == CloseState::Cleaning {
            return false;
        }
        if self.decision.is_some() {
            return false;
        }
        let Some(editor) = self.editor.clone() else {
            cx.quit();
            return true;
        };
        // Begin FIFO replay; the shared evaluation commits IME only after its deferred tail has drained.
        editor.update(cx, |app, cx| app.finish_linked_input(window, cx));
        let owner = cx.entity().downgrade();
        window.defer(cx, move |window, cx| {
            let _ = owner.update(cx, |this, cx| this.review_close(intent, None, window, cx));
        });
        false
    }

    /// Snapshot native text fingerprints so delayed consent cannot discard newer edits.
    fn review_close(
        &mut self,
        intent: CloseIntent,
        settled: Option<DocumentVersions>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.close_state == CloseState::Cleaning || self.decision.is_some() {
            return;
        }
        let Some(editor) = self.editor.clone() else {
            return;
        };
        let app = editor.read(cx);
        if app
            .language_edits
            .bridge
            .as_ref()
            .is_some_and(|bridge| bridge.read(cx).has_pending())
        {
            // Each queued History adds another deferred turn. Keep the original editor mounted
            // until its whole native FIFO is committed, rather than inspecting a transient clean state.
            self.request_close(intent, window, cx);
            return;
        }
        if editor.update(cx, |app, cx| commit_composition(app, window, cx)) {
            // A queued Mark can follow a deferred Undo. Commit it here as well as ordinary preedit,
            // then let its Change event reach DocumentSession before capturing close consent.
            let owner = cx.entity().downgrade();
            window.defer(cx, move |window, cx| {
                let _ = owner.update(cx, |this, cx| this.review_close(intent, None, window, cx));
            });
            return;
        }
        let app = editor.read(cx);
        let documents = document_versions(app, cx);
        if settled.as_ref() != Some(&documents) {
            // The final FIFO replay emits Change effects after clearing its pending flag. Wait for
            // a stable revision/text pair across another effect turn before showing any consent.
            // This prevents a prompt captured from a transient revision from rejecting its first choice.
            let owner = cx.entity().downgrade();
            window.defer(cx, move |window, cx| {
                let _ = owner.update(cx, |this, cx| {
                    this.review_close(intent, Some(documents), window, cx)
                });
            });
            return;
        }
        let dirty = app.tabs.iter().any(OpenTab::is_dirty);
        let running =
            app.run_controls.has_work_in_flight() || app.terminal.read(cx).has_running_programs();
        if dirty || running {
            self.decision = Some(Decision::Leave {
                intent,
                documents,
                dirty,
                running,
            });
            self.modal_focus.focus(window, cx);
            cx.notify();
        } else {
            self.start_cleanup(intent, window, cx);
        }
    }

    /// Revalidate the choice before any save or stop; failures retain the editor and its content.
    pub(super) fn accept_leave(&mut self, save: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Decision::Leave {
            intent,
            documents,
            running,
            ..
        }) = &self.decision
        else {
            return;
        };
        let intent = *intent;
        let Some(editor) = self.editor.clone() else {
            return;
        };
        let app = editor.read(cx);
        let new_running = !*running
            && (app.run_controls.has_work_in_flight()
                || app.terminal.read(cx).has_running_programs());
        if document_versions(app, cx) != *documents
            || new_running
            || app
                .language_edits
                .bridge
                .as_ref()
                .is_some_and(|bridge| bridge.read(cx).has_pending())
        {
            self.decision = None;
            self.error = Some(t!("projects.changed_before_close").to_string());
            self.review_close(intent, None, window, cx);
            return;
        }
        if save {
            // Bulk close has no disk-overwrite preview. Recheck disk even if the watcher is late.
            let conflict = editor.read(cx).tabs.iter().find_map(|tab| {
                let text = tab.text.as_ref()?;
                if !text.session.is_dirty() && text.disk_state == DiskState::Synced {
                    return None;
                }
                let unchanged = text.disk_state == DiskState::Synced
                    && std::fs::read(tab.path())
                        .is_ok_and(|bytes| Sha256::digest(bytes).as_slice() == text.disk_digest);
                (!unchanged).then(|| tab.path().display().to_string())
            });
            if let Some(path) = conflict {
                self.error = Some(t!("projects.disk_conflict", path = path).to_string());
                cx.notify();
                return;
            }
            if !editor.update(cx, |app, cx| app.save_dirty_documents(cx)) {
                self.error = Some(editor.read(cx).status.clone());
                cx.notify();
                return;
            }
        }
        self.decision = None;
        self.start_cleanup(intent, window, cx);
    }

    /// Keep the editor alive until its workers acknowledge shutdown; replacing the view occurs last.
    fn start_cleanup(&mut self, intent: CloseIntent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = self.editor.clone() else {
            return;
        };
        let task = match editor.update(cx, |app, cx| app.prepare_project_shutdown(cx)) {
            Ok(task) => task,
            Err(error) => {
                self.close_state = CloseState::Failed;
                self.error = Some(error);
                cx.notify();
                return;
            }
        };
        self.close_state = CloseState::Cleaning;
        self.focus.focus(window, cx);
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |this, window, cx| {
                if let Err(error) = result {
                    this.close_state = CloseState::Failed;
                    this.error = Some(error);
                    cx.notify();
                    return;
                }
                this.close_state = CloseState::Idle;
                // A successful retry retires the previous failure together with the old project.
                this.error = None;
                this.editor = None;
                this.decision = None;
                window.set_window_title(app::APP_NAME);
                match intent {
                    CloseIntent::Home => {
                        this.focus.focus(window, cx);
                        cx.notify();
                    }
                    CloseIntent::Window => {
                        window.remove_window();
                        cx.quit();
                    }
                }
            });
        })
        .detach();
    }
}

/// Base preedit changes native text silently. Commit through the installed input handler so its
/// Change event reaches DocumentSession; unmark alone only ends the native history transaction.
fn commit_composition(
    app: &mut EditorApp,
    window: &mut Window,
    cx: &mut Context<EditorApp>,
) -> bool {
    let composition = app.editor.update(cx, |state, cx| {
        let range = state.marked_text_range(window, cx)?;
        let text = state.text_for_range(range.clone(), &mut None, window, cx)?;
        Some((range, text))
    });
    if let Some((range, text)) = composition {
        // Preserve the platform's UTF-16 range and the semantic adapter's own peer-edit policy.
        // Replacing the displayed preedit with itself commits its existing transaction, not a new buffer.
        if let Some(bridge) = &app.language_edits.bridge {
            bridge.update(cx, |bridge, cx| {
                bridge.replace_text_in_range(Some(range), &text, window, cx)
            });
        } else {
            app.editor.update(cx, |state, cx| {
                state.replace_text_in_range(Some(range), &text, window, cx)
            });
        }
        true
    } else {
        app.editor
            .update(cx, |state, cx| state.unmark_text(window, cx));
        false
    }
}

/// Native text fingerprints also cover pending changes not yet reflected in the document revision.
fn document_versions(app: &EditorApp, cx: &App) -> DocumentVersions {
    app.tabs
        .iter()
        .filter_map(|tab| {
            tab.text.as_ref().map(|text| {
                (
                    tab.path().to_owned(),
                    text.session.revision(),
                    Sha256::digest(text.editor.read(cx).value().to_string().as_bytes()).into(),
                )
            })
        })
        .collect()
}
