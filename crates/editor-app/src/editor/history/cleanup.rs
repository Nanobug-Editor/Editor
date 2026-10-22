//! Concrete generation-checked cleanup previews require one explicit native confirmation.

use super::model::ClearConfirmation;
use super::*;

impl EditorApp {
    /// Close the selected preview before computing an intentional deletion of its own version.
    pub(super) fn preview_history_clear(
        &mut self,
        scope: editor_history::ClearScope,
        label: String,
        cx: &mut Context<Self>,
    ) {
        self.clear_history_preview(cx);
        self.history.generation = self.history.generation.wrapping_add(1);
        let generation = self.history.generation;
        let Some(worker) = self.history.worker.clone() else {
            return;
        };
        let selection = scope.clone();
        let preview = worker.call(0, move |history| history.preview_clear(&selection));
        if let Some(panel) = &mut self.history.panel {
            panel.busy = true;
        }
        cx.spawn(async move |app, cx| {
            let preview = preview.await;
            let _ = app.update(cx, |app, cx| {
                if app.history.generation != generation {
                    return;
                }
                if let Some(panel) = &mut app.history.panel {
                    panel.busy = false;
                    match preview {
                        Ok(preview) => {
                            panel.clear = Some(ClearConfirmation {
                                label,
                                scope,
                                preview,
                            })
                        }
                        Err(error) => panel.error = Some(error.to_string()),
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// Clearing never touches project files or the native editor text; changed previews are refused.
    pub(super) fn confirm_history_clear(&mut self, cx: &mut Context<Self>) {
        let Some(confirmation) = self
            .history
            .panel
            .as_mut()
            .and_then(|panel| panel.clear.take())
        else {
            return;
        };
        let Some(worker) = self.history.worker.clone() else {
            return;
        };
        let generation = self.history.generation;
        let cleared = worker.call(0, move |history| {
            history.clear_confirmed(&confirmation.scope, confirmation.preview)
        });
        if let Some(panel) = &mut self.history.panel {
            panel.busy = true;
        }
        cx.spawn(async move |app, cx| {
            let result = cleared.await;
            let _ = app.update(cx, |app, cx| {
                if app.history.generation != generation {
                    return;
                }
                match result {
                    Ok(_) => app.refresh_history(cx),
                    Err(error) => app.history_notice(error.to_string(), cx),
                }
            });
        })
        .detach();
        cx.notify();
    }
}
