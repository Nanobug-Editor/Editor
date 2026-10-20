//! Wire admission, native IME revisions and one fresh range distinguish rejection from a dead service.
use super::*;

/// Preedit/cancellation uses the native text input handler, without claiming to exercise an OS candidate window.
pub(super) async fn exercise(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    h.wait(
        "installed formatter declaration",
        |app, _| {
            app.providers
                .formatters()
                .get("range-language")
                .and_then(Option::as_deref)
                == Some("delayed-range/format")
        },
        cx,
    )
    .await?;
    h.main.update(cx, |_, window, cx| {
        h.app
            .update(cx, |app, cx| app.open_file(h.path.clone(), window, cx));
    })?;
    h.wait(
        "normal native text session",
        |app, _| {
            app.active_text_tab_index()
                .is_some_and(|index| app.plugin_document_version(index).is_ok())
        },
        cx,
    )
    .await?;
    let start = h.disk.find("const").context("fixture const selection")?;
    let end = h.disk[start..]
        .find('\r')
        .context("fixture CRLF selection")?
        + start;
    h.select_and_format(start..end, cx)?;
    let request = h
        .wire_event(
            |event| event["method"] == "textDocument/rangeFormatting",
            cx,
        )
        .await?;
    let id = request["id"].clone();
    ensure!(!id.is_null(), "range wire request has no identity");
    ensure!(
        request["params"]["range"]
            == json!({"start":{"line":1,"character":13},"end":{"line":1,"character":31}}),
        "incorrect original UTF-16 range: {request}"
    );
    let observed = Instant::now();
    let (before, server) = h.main.update(cx, |_, _, cx| {
        let app = h.app.read(cx);
        (
            app.plugin_document_version(app.active_text_tab_index().unwrap())
                .unwrap(),
            app.language_edits.formatters["range-language"].clone(),
        )
    })?;
    let plan = server
        .navigation_context_service()
        .context("live original formatter plan")?;
    ensure!(
        plan.process_count() == 1,
        "pending range needs one actual managed formatter"
    );
    // Recheck wire evidence in the exact native callback that mutates the composition. A reply
    // that won this race is a failed test precondition, never a product stale-result RED.
    h.main.update(cx, |_, window, cx| -> anyhow::Result<()> {
        ensure!(
            !has_response(&h.frames()?, &id),
            "IME precondition failed: reply already returned before preedit"
        );
        h.app
            .read(cx)
            .editor
            .clone()
            .update(cx, |editor, cx| -> anyhow::Result<()> {
                editor.set_selected_range(h.disk.len()..h.disk.len(), cx);
                editor.replace_and_mark_text_in_range(None, "拼😀", Some(0..3), window, cx);
                ensure!(
                    editor.marked_text_range(window, cx).is_some(),
                    "native preedit did not mark text"
                );
                Ok(())
            })
    })??;
    h.main.update(cx, |_, window, cx| -> anyhow::Result<()> {
        ensure!(
            !has_response(&h.frames()?, &id),
            "IME precondition failed: reply returned before cancellation"
        );
        h.app
            .read(cx)
            .editor
            .clone()
            .update(cx, |editor, cx| -> anyhow::Result<()> {
                editor.replace_and_mark_text_in_range(None, "", None, window, cx);
                ensure!(
                    editor.marked_text_range(window, cx).is_none(),
                    "native cancellation retained marked text"
                );
                ensure!(
                    editor.text().to_string() == h.disk,
                    "IME cancellation did not restore identical bytes"
                );
                Ok(())
            })
    })??;
    let after = h.main.update(cx, |_, _, cx| {
        let app = h.app.read(cx);
        app.plugin_document_version(app.active_text_tab_index().unwrap())
            .unwrap()
    })?;
    ensure!(
        after.id == before.id && after.revision > before.revision,
        "IME cancellation failed to advance the same native document revision"
    );
    ensure!(
        !has_response(&h.frames()?, &id),
        "IME precondition failed: response was not pending through native cancellation"
    );
    eprintln!(
        "native range phase: actual pending request {id}, IME/cancel {:.6}s, revision {} -> {}",
        observed.elapsed().as_secs_f64(),
        before.revision,
        after.revision
    );
    h.wire_event(
        |event| event["direction"] == "response" && event["id"] == id,
        cx,
    )
    .await?;
    // The reply has crossed the real pipe. Foreground frames let its application callback run
    // before another user command can cancel it, so a fresh request cannot hide a stale edit.
    for _ in 0..2 {
        h.next_frame(cx).await?;
        h.frame(cx)?;
    }
    h.main.update(cx, |_, _, cx| -> anyhow::Result<()> {
        ensure!(
            h.app.read(cx).editor.read(cx).text().to_string() == h.disk,
            "late range result revived after IME cancellation"
        );
        Ok(())
    })??;
    h.select_and_format(start..end, cx)?;
    let fresh = h
        .wire_event(
            |event| event["method"] == "textDocument/rangeFormatting" && event["id"] != id,
            cx,
        )
        .await?;
    ensure!(
        fresh["params"]["range"] == request["params"]["range"],
        "fresh request changed the selected UTF-16 range"
    );
    let expected = h
        .disk
        .replacen("const value={x:1};", "/* range */ const value={x:1};", 1);
    h.wait(
        "fresh selected edit",
        |app, cx| app.editor.read(cx).text().to_string() == expected,
        cx,
    )
    .await?;
    h.main.update(cx, |_, _, cx| -> anyhow::Result<()> {
        ensure!(
            Arc::ptr_eq(
                &server,
                &h.app.read(cx).language_edits.formatters["range-language"]
            ),
            "stale reply restarted the formatter adapter"
        );
        Ok(())
    })??;
    let frames = h.frames()?;
    ensure!(
        frames
            .iter()
            .filter(|event| event["method"] == "initialize")
            .count()
            == 1,
        "healthy formatter was reinitialized"
    );
    ensure!(
        frames
            .iter()
            .filter(|event| event["method"] == "textDocument/rangeFormatting")
            .count()
            == 2,
        "native range command was retried or duplicated"
    );
    ensure!(
        frames
            .iter()
            .all(|event| event["method"] != "textDocument/formatting"),
        "selected formatting fell back to whole-document formatting"
    );
    ensure!(
        plan.process_count() == 1,
        "fresh format lost the original managed transport"
    );
    ensure!(
        fs::read_to_string(&h.path)? == h.disk,
        "formatting implicitly saved source bytes"
    );
    h.main.update(cx, |_, window, cx| {
        window.dispatch_action(Box::new(gpui_base::input::Undo), cx)
    })?;
    h.wait(
        "one native Undo",
        |app, cx| app.editor.read(cx).text().to_string() == h.disk,
        cx,
    )
    .await?;
    eprintln!(
        "native range phase: fresh UTF-16 range, same server, initialize1, single Undo and unchanged disk"
    );
    Ok(())
}

/// Only this request's matching reply ends its pending interval.
fn has_response(frames: &[Value], id: &Value) -> bool {
    frames
        .iter()
        .any(|event| event["direction"] == "response" && event["id"] == *id)
}
