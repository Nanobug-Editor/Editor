//! Save-dependent actions resume only after the same native sessions have committed successfully.

use super::*;

/// UI-only continuation; it retains metadata/intent rather than copied mutable document text.
type Resume = Box<dyn FnOnce(&mut EditorApp, &mut Window, &mut Context<EditorApp>)>;

impl EditorApp {
    /// Return false when already clean. Otherwise save sequentially and run resume after success.
    /// A conflict, changed revision or failed write leaves the requested action unexecuted.
    pub(crate) fn defer_history_saves(
        &mut self,
        resume: impl FnOnce(&mut Self, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.tabs.iter().any(|file| {
            file.text
                .as_ref()
                .is_some_and(|tab| tab.session.is_dirty() || tab.disk_state != DiskState::Synced)
        }) {
            return false;
        }
        self.queue_history_save(Box::new(resume), cx);
        true
    }

    /// Each successful acknowledgement re-evaluates the actual current document set.
    fn queue_history_save(&mut self, resume: Resume, cx: &mut Context<Self>) {
        let Some(index) = self.tabs.iter().position(|file| {
            file.text
                .as_ref()
                .is_some_and(|tab| tab.session.is_dirty() || tab.disk_state != DiskState::Synced)
        }) else {
            return;
        };
        let tab = self.text_tab(index).expect("selected text session");
        if tab.disk_state != DiskState::Synced && !tab.overwrite_confirmed {
            self.report_host_message(
                MessageLevel::Warning,
                t!("status.confirm_disk_overwrite").to_string(),
                cx,
            );
            cx.notify();
            return;
        }
        self.save_history_document(
            index,
            move |app, saved, window, cx| {
                if !saved || app.history.saves_sealed {
                    return;
                }
                if app.tabs.iter().any(|file| {
                    file.text.as_ref().is_some_and(|tab| {
                        tab.session.is_dirty() || tab.disk_state != DiskState::Synced
                    })
                }) {
                    app.queue_history_save(resume, cx);
                } else {
                    resume(app, window, cx);
                }
            },
            cx,
        );
    }
}
