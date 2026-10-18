//! Project-level consent precedes resource cleanup; success closes this window or completes application exit.

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
        if matches!(intent, CloseIntent::Application)
            && matches!(self.decision, Some(Decision::Trust { .. }))
        {
            self.decision = None;
            controller::reserve(window.window_handle(), None, cx);
        }
        if self.close_state == CloseState::Cleaning {
            return false;
        }
        if matches!(intent, CloseIntent::Application)
            && let Some(Decision::Leave { intent, .. }) = &mut self.decision
        {
            // The application round adopts an existing local prompt; its ordinary Cancel/Save/Discard now answers that round.
            *intent = CloseIntent::Application;
            self.modal_focus.focus(window, cx);
            cx.notify();
            return false;
        }
        if !matches!(intent, CloseIntent::Application) && controller::is_quitting(cx) {
            controller::cancel_quit(cx);
        }
        if self.decision.is_some() {
            return false;
        }
        let Some(editor) = self.editor.clone() else {
            if self.has_running_clone() {
                self.decision = Some(Decision::Leave {
                    intent,
                    documents: Vec::new(),
                    dirty: false,
                    running: true,
                });
                self.modal_focus.focus(window, cx);
                cx.notify();
                return false;
            }
            if matches!(intent, CloseIntent::Application) {
                controller::approve(
                    window.window_handle(),
                    controller::Consent {
                        documents: Vec::new(),
                        running: false,
                    },
                    cx,
                );
                return false;
            }
            self.start_cleanup(intent, window, cx);
            return false;
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
        let running = app.run_controls.has_work_in_flight()
            || app.terminal.read(cx).has_running_programs()
            || self.has_running_clone();
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
            if matches!(intent, CloseIntent::Application) {
                controller::approve(
                    window.window_handle(),
                    controller::Consent { documents, running },
                    cx,
                );
            } else {
                self.start_cleanup(intent, window, cx);
            }
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
        let approved_running = *running;
        let Some(editor) = self.editor.clone() else {
            self.decision = None;
            if matches!(intent, CloseIntent::Application) {
                controller::approve(
                    window.window_handle(),
                    controller::Consent {
                        documents: Vec::new(),
                        running: approved_running,
                    },
                    cx,
                );
            } else {
                self.start_cleanup(intent, window, cx);
            }
            return;
        };
        let app = editor.read(cx);
        let new_running = !*running
            && (app.run_controls.has_work_in_flight()
                || app.terminal.read(cx).has_running_programs()
                || self.has_running_clone());
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
                (!unchanged).then(|| editor_core::display_path(&tab.path()).to_string())
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
        if matches!(intent, CloseIntent::Application) {
            let app = editor.read(cx);
            controller::approve(
                window.window_handle(),
                controller::Consent {
                    documents: document_versions(app, cx),
                    running: app.run_controls.has_work_in_flight()
                        || app.terminal.read(cx).has_running_programs()
                        || self.has_running_clone(),
                },
                cx,
            );
        } else {
            self.start_cleanup(intent, window, cx);
        }
    }

    /// Previously approved documents can still receive input while another window is deciding.
    pub(super) fn consent_is_current(
        &self,
        consent: &controller::Consent,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        match &self.editor {
            None => consent.documents.is_empty() && consent.running == self.has_running_clone(),
            Some(editor) => {
                let app = editor.read(cx);
                let native = app.editor.clone();
                let current = document_versions(app, cx) == consent.documents
                    && (app.run_controls.has_work_in_flight()
                        || app.terminal.read(cx).has_running_programs()
                        || self.has_running_clone())
                        == consent.running
                    && app
                        .language_edits
                        .bridge
                        .as_ref()
                        .is_none_or(|bridge| !bridge.read(cx).has_pending());
                current
                    && native.update(cx, |state, cx| {
                        state.marked_text_range(window, cx).is_none()
                    })
            }
        }
    }

    /// Called only after the application has validated every collected decision.
    pub(super) fn start_application_cleanup(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.start_cleanup(CloseIntent::Application, window, cx);
    }

    /// Keep the editor alive until its workers acknowledge shutdown; replacing the view occurs last.
    fn start_cleanup(&mut self, intent: CloseIntent, window: &mut Window, cx: &mut Context<Self>) {
        // The decision has been accepted. Only now cancel cloning and close application dialogs.
        self.clone_project = None;
        self.close_global_dialogs(cx);
        let project_cleanup = if let Some(editor) = self.editor.clone() {
            editor.update(cx, |app, cx| app.prepare_project_shutdown(cx))
        } else {
            Ok(cx.background_executor().spawn(async { Ok(()) }))
        };
        let task = match project_cleanup {
            Ok(task) => task,
            Err(error) => {
                self.close_state = CloseState::Failed;
                self.error = Some(error.clone());
                cx.notify();
                if matches!(intent, CloseIntent::Application) {
                    controller::cleaned(window.window_handle(), Err(error), cx);
                } else {
                    controller::local_cleanup_settled(window.window_handle(), cx);
                }
                return;
            }
        };
        let application_cleanup = self
            .extensions
            .as_ref()
            .map(|panel| panel.update(cx, |panel, cx| panel.shutdown_management(cx)));
        self.close_state = CloseState::Cleaning;
        self.focus.focus(window, cx);
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let (project, application) = futures::join!(task, async {
                if let Some(task) = application_cleanup {
                    task.await
                } else {
                    Ok(())
                }
            });
            let errors = [project, application]
                .into_iter()
                .filter_map(Result::err)
                .collect::<Vec<_>>();
            let result = if errors.is_empty() {
                Ok(())
            } else {
                Err(errors.join("\n"))
            };
            let _ = this.update_in(cx, |this, window, cx| {
                if let Err(error) = result {
                    this.close_state = CloseState::Failed;
                    this.error = Some(error.clone());
                    cx.notify();
                    if matches!(intent, CloseIntent::Application) {
                        controller::cleaned(window.window_handle(), Err(error), cx);
                    } else {
                        controller::local_cleanup_settled(window.window_handle(), cx);
                    }
                    return;
                }
                if matches!(intent, CloseIntent::Application) {
                    if let Some(editor) = &this.editor {
                        editor.read(cx).providers.retire();
                    }
                    controller::cleaned(window.window_handle(), Ok(()), cx);
                    return;
                }
                this.close_state = CloseState::Idle;
                // A successful retry retires the previous failure together with the old project.
                this.error = None;
                if let Some(editor) = &this.editor {
                    editor.read(cx).providers.retire();
                }
                this.editor = None;
                this.decision = None;
                controller::reserve(window.window_handle(), None, cx);
                window.set_window_title(app::APP_NAME);
                controller::local_cleanup_settled(window.window_handle(), cx);
                match intent {
                    CloseIntent::Window => {
                        controller::close_window(window.window_handle(), window, cx);
                    }
                    CloseIntent::Application => unreachable!(
                        "application cleanup keeps editors mounted until the coordinator completes"
                    ),
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
