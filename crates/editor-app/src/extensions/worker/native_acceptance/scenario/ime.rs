//! Chinese composition, cancellation, commit and Undo all use the original native EditorState session.
use super::*;
use gpui_kit::EntityInputHandler as _;

/// Direct composition callbacks are the public native IME seam. This verifies its rendered caret and
/// original session, while explicitly leaving the operating system's candidate popup unclaimed.
pub(super) async fn exercise(
    h: &Harness,
    target: AnyWindowHandle,
    editor: &Entity<EditorState>,
    comparison: bool,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    let original = cx.update(|cx| editor.read(cx).value().to_string());
    let mode = if comparison { "comparison" } else { "conflict" };
    checkpoint(h, target, editor, mode, "before_preedit", &original, cx)?;
    target.update(cx, |_, window, cx| {
        editor.update(cx, |editor, cx| {
            // The scenario must have focused the painted source through native pointer input.
            // Forcing focus here would conceal a broken result-pane input route.
            ensure!(
                editor.focus_handle(cx).is_focused(window),
                "IME source did not retain native editing focus"
            );
            editor.set_selected_range(0..0, cx);
            editor.replace_and_mark_text_in_range(None, "拼😀", Some(0..3), window, cx);
            ensure!(
                editor.marked_text_range(window, cx) == Some(0..3),
                "UTF-16 preedit range is incorrect"
            );
            Ok::<_, anyhow::Error>(())
        })
    })??;
    h.frame(target, cx).await?;
    checkpoint(h, target, editor, mode, "preedit", &original, cx)?;
    target.update(cx, |_, window, cx| {
        ensure!(editor.read(cx).value().starts_with("拼😀"), "original document did not receive preedit");
        let bounds = if comparison {
            h.app.read(cx).comparison_alignment().and_then(|group| group.project_range(1, 0..0, cx)).context("composition caret is not projected")?
        } else {
            editor.read(cx).cursor_layout().map(|(bounds, _)| bounds).context("native result caret not measured")?
        };
        ensure!(bounds.size.height > px(0.), "composition caret is empty");
        ensure!(Bounds::new(point(px(0.), px(0.)), window.viewport_size()).intersects(&bounds), "composition caret is outside native window: {bounds:?}");
        h.record(if comparison { "comparison_ime" } else { "conflict_ime" }, serde_json::json!({"original_entity": format!("{:?}", editor.entity_id()), "caret": format!("{bounds:?}"), "utf16_preedit": [0, 3], "os_candidates_verified": false}));
        Ok::<_, anyhow::Error>(())
    })??;
    if comparison {
        h.capture(target, "workbench-ime.png", cx)?;
    }
    target.update(cx, |_, window, cx| {
        editor.update(cx, |editor, cx| {
            editor.replace_and_mark_text_in_range(None, "", None, window, cx);
        })
    })?;
    checkpoint(h, target, editor, mode, "cancelled", &original, cx)?;
    target.update(cx, |_, window, cx| {
        editor.update(cx, |editor, cx| {
            ensure!(
                editor.marked_text_range(window, cx).is_none(),
                "composition cancellation left a mark"
            );
            ensure!(
                editor.value().as_ref() == original,
                "composition cancellation changed committed text"
            );
            Ok::<_, anyhow::Error>(())
        })
    })??;
    h.frame(target, cx).await?;
    eprintln!("native {}: IME {mode} commit", h.case.name);
    target.update(cx, |_, window, cx| {
        editor.update(cx, |editor, cx| {
            editor.replace_and_mark_text_in_range(None, "确认", Some(0..2), window, cx);
            editor.replace_text_in_range(None, "确认", window, cx);
            ensure!(
                editor.marked_text_range(window, cx).is_none(),
                "composition commit left a mark"
            );
            Ok::<_, anyhow::Error>(())
        })
    })??;
    h.frame(target, cx).await?;
    checkpoint(h, target, editor, mode, "committed", &original, cx)?;
    eprintln!("native {}: IME {mode} native Undo", h.case.name);
    target.update(cx, |_, window, cx| {
        // Prove that the composition committed before testing Undo: two ignored commands
        // must not pass merely because the document stayed equal to its original bytes.
        ensure!(
            editor.read(cx).value().as_ref() == format!("确认{original}"),
            "IME commit did not insert the confirmed text into the original session"
        );
        ensure!(
            editor.focus_handle(cx).is_focused(window),
            "IME commit lost the original editor focus before Undo"
        );
        editor_ui::testing::press_key(window, "ctrl-z", cx);
        Ok::<_, anyhow::Error>(())
    })??;
    h.frame(target, cx).await?;
    checkpoint(h, target, editor, mode, "after_undo", &original, cx)?;
    ensure!(
        cx.update(|cx| editor.read(cx).value().as_ref() == original),
        "native Undo did not restore the original session"
    );
    h.checkpoint(
        &format!("{mode}_composition_cancel_commit_undo"),
        true.into(),
    )?;
    Ok(())
}

/// Snapshot only public IME/selection/focus metadata at each mutation boundary. Text is compared
/// in memory, so a failed assertion is distinguishable without persisting editor contents.
fn checkpoint(
    h: &Harness,
    target: AnyWindowHandle,
    editor: &Entity<EditorState>,
    mode: &str,
    phase: &str,
    original: &str,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    let state = target.update(cx, |_, window, cx| {
        editor.update(cx, |editor, cx| {
            let selection = editor.selected_text_range(false, window, cx);
            serde_json::json!({
                "phase": phase,
                "original_entity": format!("{:?}", cx.entity_id()),
                "focused": editor.focus_handle(cx).is_focused(window),
                "original_bytes": original.len(),
                "actual_bytes": editor.value().len(),
                "equals_original": editor.value().as_ref() == original,
                "marked_utf16": editor.marked_text_range(window, cx),
                "selection_utf16": selection.as_ref().map(|selection| selection.range.clone()),
                "selection_reversed": selection.as_ref().map(|selection| selection.reversed),
            })
        })
    })?;
    eprintln!("native {}: IME {mode} {state}", h.case.name);
    h.checkpoint(&format!("{mode}_ime_{phase}"), state)
}
