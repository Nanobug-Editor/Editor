//! Text-only plans preflight every session, then commit without yielding to other native input.
use super::*;

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
                edit.id.clone(),
                editor,
                previous.current.revision + u64::from(changed),
                changed,
                edit.prepared.result().to_owned(),
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
        for (operation, _, editor, _, _, _) in &targets {
            editor
                .update(cx, |editor, cx| {
                    record.edits[operation].prepared.apply(editor, window, cx)
                })
                .map_err(|error| failure(ErrorCode::StaleRevision, &error.to_string()))?;
            record.completed.push(*operation);
        }
        let (answer, response) = oneshot::channel();
        cx.defer_in(window, move |app, _, cx| {
            let result = targets
                .into_iter()
                .map(|(_, id, _, revision, changed, text)| {
                    let tab = app
                        .tabs
                        .iter()
                        .find(|tab| native_version(tab).id == id)
                        .ok_or_else(|| {
                            failure(ErrorCode::StaleRevision, "Committed target closed")
                        })?;
                    let current = tab.text.as_ref().unwrap();
                    if current.capability_revision != revision
                        || current.editor.read(cx).text().to_string() != text
                    {
                        return Err(failure(
                            ErrorCode::StaleRevision,
                            "Committed target changed before receipt",
                        ));
                    }
                    Ok((native_version(tab), changed))
                })
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
