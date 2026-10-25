//! Actual plugin settings controls validate, persist and hot-apply independent topology palettes.
use super::*;
use gpui_base::test_support::{ElementSnapshot, snapshots};
use gpui_kit::Role;
use plugin_runtime::plugin_protocol::Paint;

/// Wrapper debug selectors are not native IDs. Use real footer button IDs and the input directly
/// above that footer (the form's 8px gap), so scrolling and typing still hit painted Base controls.
fn field_target(window: &Window, id: &str) -> Option<ElementSnapshot> {
    let nodes = snapshots(window);
    if let Some(field) = id.strip_suffix("-value") {
        let footer = nodes
            .iter()
            .find(|node| input::named(node, &format!("{field}-apply-button")))?;
        return nodes
            .iter()
            .filter(|node| {
                let gap = footer.bounds().top() - node.bounds().bottom();
                node.role() == Some(Role::TextInput) && gap >= px(0.) && gap <= px(32.)
            })
            .max_by(|a, b| {
                a.bounds()
                    .bottom()
                    .partial_cmp(&b.bounds().bottom())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .cloned();
    }
    let actual = if id.ends_with("-apply") || id.ends_with("-reset") {
        format!("{id}-button")
    } else {
        id.to_owned()
    };
    nodes.into_iter().find(|node| input::named(node, &actual))
}

/// Locate an enabled painted settings control, including its native input/button descendants.
fn control(window: &Window, id: &str) -> Option<ElementSnapshot> {
    field_target(window, id).filter(|node| {
        // Base retains snapshots for controls clipped by the scrolling form. Its `visible`
        // metadata alone does not mean the input is below the fixed settings title bar.
        let bounds = node.bounds();
        let inside_form = !id.starts_with("setting-")
            || (bounds.top() >= px(crate::PANEL_HEADER_HEIGHT + 8.)
                && bounds.bottom() <= window.viewport_size().height - px(8.));
        node.visible()
            && node.disabled() != Some(true)
            && inside_form
            && Bounds::new(point(px(0.), px(0.)), window.viewport_size()).contains(&bounds.center())
    })
}

/// Reveal the requested declaration in the real settings scroll container before clicking it.
async fn click(
    h: &Harness,
    target: AnyWindowHandle,
    id: &str,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    h.until(
        target,
        id,
        |window, cx| {
            // This dialog is outside the harness's plugin-window list. Draw its current form
            // before using hit bounds, including focus scrolling and asynchronous Apply receipts.
            window.refresh();
            window.draw(cx).clear(cx);
            if let Some(node) = control(window, id) {
                input::click(window, &node, cx)?;
                return Ok(true);
            }
            let above =
                field_target(window, id).is_some_and(|node| node.bounds().center().y < px(120.));
            let at = point(
                window.viewport_size().width - px(80.),
                window.viewport_size().height / 2.,
            );
            input::wheel(window, at, if above { 180. } else { -180. }, cx);
            Ok(false)
        },
        cx,
    )
    .await?;
    // Closing the settings chrome retires that Window during dispatch; observe the surviving main.
    h.frame(
        if id == "app-dialog-close" {
            h.main
        } else {
            target
        },
        cx,
    )
    .await
}

/// Only the disposable profile is read back; project contents never hold appearance preferences.
fn saved(h: &Harness) -> anyhow::Result<serde_json::Value> {
    let path = extensions::profile_runtime_root(&h.fixture.profile).join("settings/git.json");
    match fs::read(path) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(serde_json::json!({"user":{}}))
        }
        Err(error) => Err(error.into()),
    }
}

/// Observe actual guest vector output while keeping metadata, file colors and other surfaces out.
fn source(h: &Harness) -> anyhow::Result<String> {
    let state = h.worker.state.lock().unwrap();
    let Kind::Canvas(canvas) = &state.views["git/history"]
        .active_node("log")
        .context("log")?
        .kind
    else {
        anyhow::bail!("log is not canvas")
    };
    Ok(canvas
        .paint
        .iter()
        .filter_map(|paint| match paint {
            Paint::Svg { source, .. } => Some(source.as_str()),
            _ => None,
        })
        .collect())
}

/// Enter ordinary native text, press Apply, then wait for the worker's correlated production receipt.
async fn apply(
    h: &Harness,
    dialog: AnyWindowHandle,
    key: &str,
    value: Option<&str>,
    valid: bool,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    let (prior, last_log) = {
        let state = h.worker.state.lock().unwrap();
        (
            state.configuration_result.as_ref().map(|(id, _)| *id),
            state
                .logs
                .records("git")
                .last()
                .map_or(0, |record| record.id),
        )
    };
    if let Some(value) = value {
        let input_id = format!("setting-git-{key}-value");
        click(h, dialog, &input_id, cx).await?;
        dialog.update(cx, |_, window, cx| {
            editor_ui::testing::press_key(window, "ctrl-a", cx);
            editor_ui::testing::press_key(window, "backspace", cx);
            editor_ui::testing::input_text(window, value, cx);
        })?;
        let actual = dialog.update(cx, |_, window, _| {
            field_target(window, &input_id).and_then(|node| node.value().map(str::to_owned))
        })?;
        h.record(
            "graph_palette_last_draft",
            serde_json::json!({"key": key, "expected": value, "actual": actual}),
        );
        if actual.as_deref() != Some(value) {
            h.capture(dialog, "graph-palette-input-failure.png", cx)?;
            anyhow::bail!(
                "Native palette draft mismatch for {key}: expected {value}, actual {actual:?}"
            );
        }
    }
    click(
        h,
        dialog,
        &format!(
            "setting-git-{key}-{}",
            if value.is_some() { "apply" } else { "reset" }
        ),
        cx,
    )
    .await?;
    let receipt = h
        .until_for(
            dialog,
            "palette setting receipt",
            // Each edit validates an isolated real WASM candidate. Debug builds under concurrent
            // compilation can exceed the ordinary 45s UI budget; never click Apply a second time.
            Duration::from_secs(120),
            |window, cx| {
                window.refresh();
                window.draw(cx).clear(cx);
                let state = h.worker.state.lock().unwrap();
                match &state.configuration_result {
                    Some((id, result)) if Some(*id) != prior => {
                        ensure!(
                            result.is_ok() == valid,
                            "unexpected setting result: {result:?}"
                        );
                        if let Err(error) = result {
                            ensure!(
                                error.starts_with("Configuration rejected:")
                                    && error.contains(&format!("\"{key}\"")),
                                "setting failed for an unintended reason: {error}"
                            );
                            // The runner appends the rejected operation after publishing its
                            // correlation receipt. Wait for that exact new log, not any error.
                            let logs = state.logs.records("git");
                            let Some(record) = logs.iter().find(|record| {
                                record.id > last_log
                                    && record.level == plugin_runtime::logs::LogLevel::Error
                                    && record.source == "host.operation"
                                    && record.message == *error
                            }) else {
                                return Ok(false);
                            };
                            h.expect_error_log(record);
                        }
                        Ok(true)
                    }
                    _ => Ok(false),
                }
            },
            cx,
        )
        .await;
    if let Err(error) = receipt {
        h.capture(dialog, "graph-palette-receipt-failure.png", cx)?;
        return Err(error);
    }
    h.frame(dialog, cx).await
}

/// Real settings editing preserves the running tree owner, current selection, scroll and commit form.
pub(super) async fn exercise(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    let initial = source(h)?;
    let original = selected(h);
    let offset = log_offset(h);
    let (staging, branch_owner) = {
        let state = h.worker.state.lock().unwrap();
        let Kind::Tree { handle } = &state.views["git/history"]
            .active_node("branches")
            .context("branches")?
            .kind
        else {
            anyhow::bail!("branches missing")
        };
        (state.views["git/staging"].revision, handle.clone())
    };
    click(h, h.main, "open-settings", cx).await?;
    let dialog: AnyWindowHandle = cx
        .update(|cx| {
            h.app
                .read(cx)
                .dialog_window
                .context("settings did not open")
        })?
        .into();
    h.dpi(dialog, cx).await?;
    // Settings navigation uses plain clickable Divs, which Base's control snapshots omit.
    // Use the existing 214px sidebar, 8px inset, 34px rows and 4px gaps from app/settings.rs;
    // the ensuing declaration snapshots verify that the real fourth-row click took effect.
    dialog.update(cx, |_, window, cx| {
        let at = point(
            px(107.),
            px(crate::PANEL_HEADER_HEIGHT + 8. + 3. * 38. + 17.),
        );
        input::point_click(window, at, cx)
    })??;
    h.frame(dialog, cx).await?;
    h.until(
        dialog,
        "Git palette declarations visible",
        |window, _| Ok(field_target(window, "setting-git-graph.colors.dark.1-apply").is_some()),
        cx,
    )
    .await?;
    let (active, other) = if h.case.dark {
        ("dark", "light")
    } else {
        ("light", "dark")
    };
    let key = format!("graph.colors.{active}.1");
    let other_key = format!("graph.colors.{other}.1");
    apply(h, dialog, &other_key, Some("#A145B8"), true, cx).await?;
    ensure!(
        source(h)? == initial,
        "inactive palette changed the current graph"
    );
    apply(h, dialog, &key, Some("#12AB34"), true, cx).await?;
    let stored = saved(h)?;
    h.record("graph_palette_saved", stored.clone());
    ensure!(
        stored["user"][&key] == "#12AB34" && stored["user"][&other_key] == "#A145B8",
        "theme palettes were not saved independently: {stored}"
    );
    h.capture(dialog, "graph-palette-settings.png", cx)?;
    h.until(
        h.main,
        "custom graph colors painted",
        |_, cx| Ok(source(h)?.contains("#12ab34") && h.node_current(h.main, "log", cx)),
        cx,
    )
    .await?;
    h.capture(h.main, "graph-palette-custom.png", cx)?;
    apply(h, dialog, &key, Some("#GGGGGG"), false, cx).await?;
    ensure!(
        saved(h)? == stored && source(h)?.contains("#12ab34"),
        "rejected color changed durable or painted state"
    );
    apply(h, dialog, &key, None, true, cx).await?;
    h.until(
        h.main,
        "default graph palette restored",
        |_, _| Ok(source(h)? == initial),
        cx,
    )
    .await?;
    ensure!(
        saved(h)?["user"].get(&key).is_none() && saved(h)?["user"][&other_key] == "#A145B8",
        "reset changed the other theme"
    );
    click(h, dialog, "app-dialog-close", cx).await?;
    h.until(
        h.main,
        "settings dismissed",
        |_, cx| Ok(h.app.read(cx).dialog_window.is_none()),
        cx,
    )
    .await?;
    h.settle(cx).await?;
    let state = h.worker.state.lock().unwrap();
    let Kind::Tree { handle } = &state.views["git/history"]
        .active_node("branches")
        .unwrap()
        .kind
    else {
        unreachable!()
    };
    ensure!(
        handle == &branch_owner && state.views["git/staging"].revision == staging,
        "palette update restarted the owner or commit scene"
    );
    drop(state);
    ensure!(
        selected(h) == original && log_offset(h) == offset,
        "palette update changed history navigation"
    );
    h.record("graph_palette_settings", serde_json::json!({
        "native_input_apply_and_reset": true, "light_dark_saved_independently": true,
        "inactive_theme_keeps_current_graph": true, "invalid_color_keeps_saved_value": true,
        "owner_retained": true, "commit_scene_retained": true, "selection_and_scroll_retained": true,
    }));
    Ok(())
}
