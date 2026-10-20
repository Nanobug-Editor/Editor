//! Selected unknown-language completions retain the original source graph throughout real stdio IO.
use super::*;

/// Both a valid main/import pair and every whole-proposal refusal pass the production menu handler.
pub(super) async fn exercise(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    let related = h
        .path
        .parent()
        .unwrap()
        .join("related.linked")
        .canonicalize()?;
    let related_disk = fs::read_to_string(&related)?;
    h.wait(
        "installed completion declaration",
        |app, _| {
            app.providers
                .language_servers()
                .get("resolve-language")
                .and_then(Option::as_deref)
                == Some("delayed-resolve/analysis")
        },
        cx,
    )
    .await?;
    h.main.update(cx, |_, window, cx| {
        h.app.update(cx, |app, cx| {
            app.open_file(related.clone(), window, cx);
            app.open_file(h.path.clone(), window, cx);
        });
    })?;
    h.wait(
        "native completion provider",
        |app, cx| app.editor.read(cx).lsp().completion_provider.is_some(),
        cx,
    )
    .await?;
    let server = h.main.update(cx, |_, _, cx| {
        h.app
            .read(cx)
            .language_server_for_path(&h.path)
            .unwrap()
            .clone()
    })?;
    let plan = server
        .navigation_context_service()
        .context("live original completion plan")?;
    for phase in [
        "positive",
        "source-aba",
        "related-change",
        "invalid",
        "fresh",
        "owner-revoke",
    ] {
        let source = format!("// {phase} 中文😀e\u{301}\r\nres");
        let before = h
            .frames()?
            .iter()
            .filter_map(|event| event["id"].as_u64())
            .max()
            .unwrap_or(0);
        // Actual native text input first changes the header, then types an identifier to open the
        // ordinary menu. No item is injected and no handler is called directly by the test.
        h.main.update(cx, |_, window, cx| {
            window.activate_window();
            h.app.read(cx).editor.clone().update(cx, |editor, cx| {
                editor.focus(window, cx);
                editor.set_selected_range(0..editor.text().len(), cx);
                editor.replace_text_in_range(None, source.strip_suffix("res").unwrap(), window, cx);
                editor.replace_text_in_range(None, "res", window, cx);
            });
        })?;
        h.wait(
            "ordinary selected completion menu",
            |app, cx| {
                let state = app.editor.read(cx);
                state.completion_menu_state().open
                    && state
                        .completion_menu_state()
                        .items
                        .iter()
                        .any(|item| item.label == "resolvedSymbol")
            },
            cx,
        )
        .await?;
        ensure!(
            !h.frames()?
                .iter()
                .any(|event| event["method"] == "completionItem/resolve"
                    && event["id"].as_u64().is_some_and(|id| id > before)),
            "unselected completion was resolved eagerly"
        );
        h.main.update(cx, |_, window, cx| {
            window.dispatch_action(
                Box::new(gpui_base::input::Enter {
                    secondary: false,
                    shift: false,
                }),
                cx,
            );
        })?;
        let request = h
            .wire_event(
                |event| {
                    event["method"] == "completionItem/resolve"
                        && event["id"].as_u64().is_some_and(|id| id > before)
                },
                cx,
            )
            .await?;
        let id = request["id"].clone();
        ensure!(
            !has_reply(&h.frames()?, &id),
            "{phase}: response won the mutation precondition"
        );
        if phase == "source-aba" {
            let before_version = version(h, cx)?;
            h.main.update(cx, |_, window, cx| -> anyhow::Result<()> {
                ensure!(
                    !has_reply(&h.frames()?, &id),
                    "source ABA was not concurrent with resolve"
                );
                h.app.read(cx).editor.clone().update(cx, |editor, cx| {
                    // Position zero is also the pending additional import target. Identical final
                    // bytes and caret must not revive its old source revision after cancelled IME.
                    editor.set_selected_range(0..0, cx);
                    editor.replace_and_mark_text_in_range(None, "拼😀", Some(0..3), window, cx);
                    assert!(editor.marked_text_range(window, cx).is_some());
                    editor.replace_and_mark_text_in_range(None, "", None, window, cx);
                    editor.set_selected_range(source.len()..source.len(), cx);
                });
                Ok(())
            })??;
            let current = version(h, cx)?;
            ensure!(
                current.id == before_version.id && current.revision > before_version.revision,
                "source ABA did not change the native revision"
            );
            ensure!(
                !has_reply(&h.frames()?, &id),
                "resolve returned before the ABA finished"
            );
        } else if phase == "related-change" {
            let target = version(h, cx)?;
            h.main.update(cx, |_, window, cx| -> anyhow::Result<()> {
                ensure!(
                    !has_reply(&h.frames()?, &id),
                    "related input changed after resolve completed"
                );
                let editor = h
                    .app
                    .read(cx)
                    .tabs
                    .iter()
                    .find(|tab| tab.path() == related)
                    .unwrap()
                    .text
                    .as_ref()
                    .unwrap()
                    .editor
                    .clone();
                editor.update(cx, |editor, cx| {
                    editor.set_selected_range(0..0, cx);
                    editor.replace_text_in_range(None, "/* changed required source */", window, cx);
                });
                Ok(())
            })??;
            ensure!(
                version(h, cx)? == target,
                "changing a required source must not mutate the target"
            );
        } else if phase == "owner-revoke" {
            h.actor.tx.send(Work::Disable("delayed-resolve".into()))?;
            h.wait(
                "original completion owner revoked",
                |_, _| !plan.is_active() && plan.process_count() == 0,
                cx,
            )
            .await?;
        }
        if phase != "owner-revoke" {
            let response = h
                .wire_event(
                    |event| event["direction"] == "response" && event["id"] == id,
                    cx,
                )
                .await?;
            ensure!(
                response["result"]["additionalTextEdits"]
                    .as_array()
                    .is_some_and(|edits| !edits.is_empty()),
                "fixture did not return the real import"
            );
            if phase == "invalid" {
                ensure!(
                    response["result"]["additionalTextEdits"]
                        .as_array()
                        .unwrap()
                        .len()
                        == 2,
                    "illegal trailing edit was not on the wire"
                );
                h.wait(
                    "whole invalid completion refusal",
                    |app, _| app.status.to_ascii_lowercase().contains("overlap"),
                    cx,
                )
                .await?;
            }
        }
        if matches!(phase, "positive" | "fresh") {
            let expected = format!(
                "import {{ resolvedSymbol }} from './module';\r\n{}resolvedSymbol",
                source.strip_suffix("res").unwrap()
            );
            h.wait(
                "main and import applied atomically",
                |app, cx| app.editor.read(cx).text().to_string() == expected,
                cx,
            )
            .await?;
            h.main.update(cx, |_, window, cx| {
                window.dispatch_action(Box::new(gpui_base::input::Undo), cx)
            })?;
            h.wait(
                "one Undo restores main and import",
                |app, cx| app.editor.read(cx).text().to_string() == source,
                cx,
            )
            .await?;
        } else {
            for _ in 0..2 {
                h.next_frame(cx).await?;
                h.frame(cx)?;
            }
            ensure!(
                h.main.update(cx, |_, _, cx| h
                    .app
                    .read(cx)
                    .editor
                    .read(cx)
                    .text()
                    .to_string())?
                    == source,
                "{phase}: a rejected proposal applied its main or an import prefix"
            );
        }
        ensure!(
            fs::read_to_string(&h.path)? == h.disk && fs::read_to_string(&related)? == related_disk,
            "completion saved a project source"
        );
        if phase != "owner-revoke" {
            ensure!(
                h.main.update(cx, |_, _, cx| h
                    .app
                    .read(cx)
                    .language_server_for_path(&h.path)
                    .is_some_and(|current| Arc::ptr_eq(current, &server)))?,
                "{phase}: refusal replaced a healthy transport"
            );
            ensure!(
                h.frames()?
                    .iter()
                    .filter(|event| event["method"] == "initialize")
                    .count()
                    == 1,
                "completion refusal consumed a transport retry"
            );
        }
        eprintln!("native completion phase {phase}: source and all import effects verified");
    }
    ensure!(
        h.frames()?
            .iter()
            .any(|event| event["method"] == "fixture/context/documents"
                && event["params"]["arguments"]["candidates"]
                    .as_array()
                    .is_some_and(|values| values.len() == 2)),
        "related-source lifetime was never exercised through the public membership method"
    );
    Ok(())
}

/// Capture the same current DocumentSession identity used by native completion admission.
fn version(
    h: &Harness,
    cx: &mut AsyncApp,
) -> anyhow::Result<plugin_runtime::plugin_protocol::api::DocumentVersion> {
    Ok(h.main.update(cx, |_, _, cx| {
        let app = h.app.read(cx);
        app.plugin_document_version(app.active_text_tab_index().unwrap())
            .unwrap()
    })?)
}

/// Only the matching response ends the original pending resolve interval.
fn has_reply(frames: &[Value], id: &Value) -> bool {
    frames
        .iter()
        .any(|event| event["direction"] == "response" && event["id"] == *id)
}
