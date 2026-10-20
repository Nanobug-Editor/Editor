//! Text-only plans preflight every session, then commit without yielding to other native input.
use super::*;
use crate::editor::text_transactions::AppliedTextEdits;

/// One native mutation's original session and synchronous postimage seal the deferred receipt.
pub(super) struct TextCommit {
    before: NativeVersion,
    session_revision: u64,
    applied: AppliedTextEdits,
}

impl TextCommit {
    /// Pair the pre-write identity/session clock with the receipt returned by that exact write.
    pub(super) fn new(
        before: NativeVersion,
        session_revision: u64,
        applied: AppliedTextEdits,
    ) -> Self {
        Self {
            before,
            session_revision,
            applied,
        }
    }

    /// Read the actual capability version only after native events settle. A second edit/Undo uses
    /// another session revision; notify-only IME alters the Rope identity. Neither can pass as ours.
    pub(super) fn version(&self, app: &EditorApp, cx: &App) -> Result<NativeVersion, Failure> {
        let tab = app
            .tabs
            .iter()
            .find(|tab| native_version(tab).id == self.before.id)
            .ok_or_else(|| failure(ErrorCode::StaleRevision, "Committed target closed"))?;
        let text = tab.text.as_ref().ok_or_else(|| {
            failure(
                ErrorCode::StaleRevision,
                "Committed target lost its text session",
            )
        })?;
        if tab.path() != self.before.path.as_path()
            || Some(text.session.revision())
                != self.applied.session_revision_after(self.session_revision)
            || !self.applied.matches_postimage(text.editor.read(cx).text())
        {
            return Err(failure(
                ErrorCode::StaleRevision,
                "Committed target changed before receipt",
            ));
        }
        Ok(native_version(tab))
    }
}

/// Commit `record` through its original live `request`, after checking every editor before any write.
/// The receipt waits for ordinary native Change events; disk bytes and independent Undo owners remain.
pub(super) async fn apply(
    app: &WeakEntity<EditorApp>,
    cx: &mut AsyncWindowContext,
    request: &EditorRequest,
    record: &mut Record,
) -> Result<(), Failure> {
    let response = update_native(app, cx, |app, window, cx| {
        app.check_workspace_native(record, window, cx)?;
        let mut targets = Vec::new();
        for (operation, edit) in &record.edits {
            let tab = app
                .tabs
                .iter()
                .position(|tab| native_version(tab).id == edit.id)
                .ok_or_else(|| failure(ErrorCode::StaleRevision, "Refactoring target closed"))?;
            let editor = app.text_tab(tab).unwrap().editor.clone();
            editor
                .update(cx, |editor, cx| {
                    edit.prepared.validate_current(editor, window, cx)
                })
                .map_err(|error| failure(ErrorCode::StaleRevision, &error.to_string()))?;
            let previous = record
                .native
                .iter()
                .find(|target| target.current.id == edit.id)
                .unwrap();
            let changed = previous.text.as_ref().unwrap().0 != edit.prepared.result();
            targets.push((
                *operation,
                native_version(&app.tabs[tab]),
                editor,
                app.text_tab(tab).unwrap().session.revision(),
                changed,
            ));
        }
        if !request.enter_side_effect() {
            return Err(failure(
                ErrorCode::Cancelled,
                "Refactoring cancelled before commit",
            ));
        }
        record
            .batch
            .publish(receipt(record, BatchState::Applying, None, None));
        // No await, OS IO or user callbacks separate these writes. Every fallible source/IME/access
        // precondition above was checked for all sessions, and each target appears once in a plan.
        let mut committed = Vec::with_capacity(targets.len());
        for (operation, before, editor, session_revision, changed) in targets {
            let applied = editor
                .update(cx, |editor, cx| {
                    record.edits[&operation].prepared.apply(editor, window, cx)
                })
                .map_err(|error| failure(ErrorCode::StaleRevision, &error.to_string()))?;
            if changed {
                // Consume analysis conditions at the actual write, before native Change callbacks
                // or a provider publication can invalidate the source used to prepare this plan.
                record.batch.record_effect();
            }
            committed.push((TextCommit::new(before, session_revision, applied), changed));
            record.completed.push(operation);
        }
        let (answer, response) = oneshot::channel();
        cx.defer_in(window, move |app, _, cx| {
            let result = committed
                .into_iter()
                .map(|(commit, changed)| commit.version(app, cx).map(|version| (version, changed)))
                .collect::<Result<Vec<_>, Failure>>();
            let _ = answer.send(result);
        });
        Ok(response)
    })?;
    let results = response
        .await
        .map_err(|_| failure(ErrorCode::InvalidState, "Refactoring completion released"))??;
    for (current, changed) in results {
        let target = record
            .native
            .iter_mut()
            .find(|target| target.current.id == current.id)
            .unwrap();
        target.current = current;
        target.changed |= changed;
    }
    Ok(())
}
