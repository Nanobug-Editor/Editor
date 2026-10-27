//! Actual canvas separator drags keep commit selection, vertical scroll and sibling scenes intact.
use super::*;
use gpui_kit::{FontWeight, MouseMoveEvent, font};
use plugin_runtime::plugin_protocol::Paint;
mod cursor;
mod viewport;

/// Observe actual public text positions/contents; invisible hit targets need no test-only paint markers.
fn text_layout(h: &Harness) -> anyhow::Result<Vec<(f32, String)>> {
    let state = h.worker.state.lock().unwrap();
    let node = state.views["git/history"]
        .active_node("log")
        .context("log missing")?;
    let Kind::Canvas(canvas) = &node.kind else {
        anyhow::bail!("log is not a canvas");
    };
    Ok(canvas
        .paint
        .iter()
        .filter_map(|paint| match paint {
            Paint::Text { x, text, .. } => Some((*x, text.clone())),
            Paint::TextBox { rect, text, .. } => Some((rect.x, text.clone())),
            _ => None,
        })
        .collect())
}

/// Native boundaries include current viewport fitting; guest declarations may use a wider reference.
fn targets(h: &Harness, window: &Window, cx: &App) -> anyhow::Result<[f32; 3]> {
    let snapshot = native_canvas(h, window, cx).context("native log missing")?;
    let positions: Vec<f32> = serde_json::from_value(snapshot["column_dividers"].clone())?;
    positions
        .try_into()
        .map_err(|_| anyhow::anyhow!("three native column dividers required"))
}

/// Read actual clip rectangles so glyph measurement observes the same fit as native painting.
fn painted_rects(
    h: &Harness,
    window: &Window,
    cx: &App,
) -> anyhow::Result<Vec<plugin_runtime::plugin_protocol::Rect>> {
    let snapshot = native_canvas(h, window, cx).context("native log missing")?;
    Ok(serde_json::from_value(snapshot["painted_boxes"].clone())?)
}

/// Shape the first public row with the same native typography that actually paints the text.
fn actual_row(h: &Harness, window: &mut Window, cx: &App) -> anyhow::Result<Vec<(f32, f32)>> {
    let painted = painted_rects(h, window, cx)?;
    let mut painted = painted.iter();
    let state = h.worker.state.lock().unwrap();
    let Kind::Canvas(canvas) = &state.views["git/history"].active_node("log").unwrap().kind else {
        anyhow::bail!("log is not a canvas");
    };
    let row = canvas
        .paint
        .iter()
        .filter_map(|paint| {
            let (text, size, bold, family) = match paint {
                Paint::TextBox {
                    text,
                    size,
                    bold,
                    font,
                    ..
                } => (text, size, bold, font),
                _ => return None,
            };
            let mut face = font(
                family
                    .clone()
                    .or(canvas.font.family.clone())
                    .unwrap_or_else(|| "Segoe UI".into()),
            );
            if *bold {
                face.weight = FontWeight::BOLD;
            }
            match paint {
                Paint::TextBox { align, .. } => {
                    let rect = painted.next()?;
                    let align = match align {
                        plugin_runtime::plugin_protocol::ui::Alignment::Start => {
                            gpui_kit::TextAlign::Left
                        }
                        plugin_runtime::plugin_protocol::ui::Alignment::Center => {
                            gpui_kit::TextAlign::Center
                        }
                        plugin_runtime::plugin_protocol::ui::Alignment::End => {
                            gpui_kit::TextAlign::Right
                        }
                    };
                    let layout = editor_ui::text::canvas::CanvasText::new(
                        text.clone().into(),
                        face,
                        px(*size),
                        gpui_kit::rgb(0).into(),
                        Bounds::new(
                            point(px(rect.x), px(rect.y)),
                            gpui_kit::size(px(rect.w), px(rect.h)),
                        ),
                        align,
                        window,
                    );
                    let actual = layout.bounds();
                    Some((actual.left() / px(1.), actual.size.width / px(1.)))
                }
                _ => None,
            }
        })
        .take(4)
        .collect::<Vec<_>>();
    ensure!(row.len() == 4, "history row has no four text columns");
    Ok(row)
}

/// Compare real glyph edges against the current public divider anchors, including clamped drags.
fn alignment(
    h: &Harness,
    targets: [f32; 3],
    window: &mut Window,
    cx: &App,
) -> anyhow::Result<[f32; 3]> {
    let row = actual_row(h, window, cx)?;
    let label_end = targets[1] - 4.;
    let author_start = targets[1] + 4.;
    let time_start = targets[2] + 4.;
    Ok([
        row[1].0 + row[1].1 - label_end,
        row[2].0 - author_start,
        row[3].0 - time_start,
    ])
}

/// Drag each native boundary across an asynchronous drawing replacement, then inspect actual pixels.
pub(super) async fn exercise(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    h.until(
        h.main,
        "native automatic column widths settled",
        |_, cx| {
            let measured = {
                let state = h.worker.state.lock().unwrap();
                matches!(&state.views["git/history"].active_node("log").unwrap().kind,
                Kind::Canvas(canvas) if canvas.measure_text.is_empty())
            };
            Ok(measured && h.node_current(h.main, "log", cx))
        },
        cx,
    )
    .await?;
    if std::env::var_os("NANOBUG_GIT_NATIVE_BRANCH_COMPACT").is_some() {
        current_branch(h)?;
    }
    let (gaps, glyph_slack) = h.main.update(cx, |_, window, cx| -> anyhow::Result<_> {
        let row = actual_row(h, window, cx)?;
        let rectangles = painted_rects(h, window, cx)?;
        let glyph_slack = (rectangles[2].w - row[2].1).max(0.);
        Ok((
            [
                row[2].0 - row[1].0 - row[1].1,
                row[3].0 - row[2].0 - row[2].1,
            ],
            glyph_slack,
        ))
    })??;
    ensure!(
        (gaps[0] - 8.).abs() < 1. && (gaps[1] - glyph_slack - 8.).abs() < 1.,
        "native metadata gaps are not compact: {gaps:?}"
    );
    h.record("history_compact_gaps", serde_json::json!({"label_author_px": gaps[0], "author_time_px": gaps[1], "author_time_glyph_slack_px":glyph_slack, "native_font_metrics": true}));
    h.capture(h.main, "history-compact-default.png", cx)?;
    if std::env::var_os("NANOBUG_GIT_NATIVE_VIEWPORT_COLUMNS").is_some() {
        viewport::exercise(h, cx).await?;
    }
    let selected_before = selected(h);
    let offset = log_offset(h);
    let staging_revision = h.worker.state.lock().unwrap().views["git/staging"].revision;
    let (origin, y) = h.main.update(cx, |_, window, cx| -> anyhow::Result<_> {
        let canvas = native_canvas(h, window, cx).context("native log missing")?;
        Ok((
            px(canvas["origin_x"].as_f64().context("origin x")? as f32),
            px((canvas["origin_y"].as_f64().context("origin y")? + 42.) as f32),
        ))
    })??;
    // A steady separator hover must advertise horizontal resizing before the guest receives a press.
    let probe_drag = std::env::var_os("NANOBUG_GIT_NATIVE_COLUMN_DRAG").is_some();
    let mut resize_cursor = false;
    let mut drag_times = Vec::new();
    if probe_drag {
        let native_targets = h
            .main
            .update(cx, |_, window, cx| targets(h, window, cx))??;
        cursor::exercise(h, origin, y, native_targets[1], cx).await?;
        resize_cursor = true;
    }
    // Give the compact label column room first; its neighboring subject can already be at minimum.
    for index in [1, 0, 2] {
        let before = text_layout(h)?;
        let native_before = h.main.update(cx, |_, window, cx| {
            native_canvas(h, window, cx).and_then(|snapshot| snapshot.get("painted_boxes").cloned())
        })?;
        let before_targets = h
            .main
            .update(cx, |_, window, cx| targets(h, window, cx))??;
        // Widen labels before the subject; the last gesture then gives extra room to time.
        let start = point(origin + px(before_targets[index]), y);
        let end = start + point(px(if index == 2 { -12. } else { 12. }), px(0.));
        let drag_started = Instant::now();
        h.main.update(cx, |_, window, cx| {
            window.dispatch_event(
                PlatformInput::MouseDown(MouseDownEvent {
                    position: start,
                    button: MouseButton::Left,
                    modifiers: Default::default(),
                    click_count: 1,
                    first_mouse: false,
                }),
                cx,
            );
        })?;
        h.frame(h.main, cx).await?;
        h.main.update(cx, |_, window, cx| {
            window.dispatch_event(
                PlatformInput::MouseMove(MouseMoveEvent {
                    position: end,
                    pressed_button: Some(MouseButton::Left),
                    modifiers: Default::default(),
                }),
                cx,
            );
        })?;
        h.until(
            h.main,
            "history divider drag painted",
            |window, cx| {
                let painted = native_before.as_ref().is_some_and(|before| {
                    native_canvas(h, window, cx)
                        .and_then(|snapshot| snapshot.get("painted_boxes").cloned())
                        .as_ref()
                        != Some(before)
                });
                Ok((painted || text_layout(h)? != before) && h.node_current(h.main, "log", cx))
            },
            cx,
        )
        .await?;
        if probe_drag {
            let owned = h.main.update(cx, |_, window, cx| {
                native_canvas(h, window, cx)
                    .is_some_and(|snapshot| snapshot["column_dragging"] == true)
            })?;
            ensure!(
                owned,
                "divider move did not remain owned by the native column gesture"
            );
        }
        drag_times.push(drag_started.elapsed().as_secs_f64() * 1000.);
        h.main.update(cx, |_, window, cx| {
            window.dispatch_event(
                PlatformInput::MouseUp(MouseUpEvent {
                    position: end,
                    button: MouseButton::Left,
                    modifiers: Default::default(),
                    click_count: 1,
                }),
                cx,
            );
        })?;
        h.settle(cx).await?;
        let after_targets = h
            .main
            .update(cx, |_, window, cx| targets(h, window, cx))??;
        ensure!(
            before_targets.iter().zip(after_targets).enumerate().all(
                |(position, (before, after))| if position == index {
                    (before - after).abs() > 0.1
                } else {
                    (before - after).abs() < 0.1
                }
            ),
            "column drag moved unrelated boundaries"
        );
        ensure!(
            selected(h) == selected_before && log_offset(h) == offset,
            "column drag changed history navigation"
        );
        ensure!(
            h.worker.state.lock().unwrap().views["git/staging"].revision == staging_revision,
            "column drag republished the commit panel"
        );
    }
    {
        let state = h.worker.state.lock().unwrap();
        let Kind::Canvas(canvas) = &state.views["git/history"].active_node("log").unwrap().kind
        else {
            unreachable!()
        };
        ensure!(
            canvas.paint.iter().any(
                |paint| matches!(paint, Paint::TextBox { bold: true, text, .. } if !text.is_empty())
            ),
            "bold author missing"
        );
        ensure!(canvas.paint.iter().any(|paint| matches!(paint, Paint::TextBox { text, .. } if text.as_bytes().get(4) == Some(&b'-') && text.as_bytes().get(10) == Some(&b' '))), "formatted timestamp missing");
        ensure!(
            !canvas
                .paint
                .iter()
                .any(|paint| matches!(paint, Paint::Fill { rect, .. } if rect.w == 1.)),
            "column boundaries must not paint separator lines"
        );
    }
    let errors = h.main.update(cx, |_, window, cx| {
        alignment(h, targets(h, window, cx)?, window, cx)
    })??;
    h.record(
        "history_column_alignment",
        serde_json::json!({
            "label_right_error_px": errors[0], "author_left_error_px": errors[1],
            "time_left_error_px": errors[2], "native_font_metrics": true,
        }),
    );
    ensure!(
        errors.iter().all(|error| error.abs() <= 1.),
        "native column alignment errors (label/right, author/left, time/left): {errors:?}"
    );
    h.capture(h.main, "history-columns.png", cx)?;
    h.record("history_columns", serde_json::json!({"three_native_resize_targets": true, "invisible_boundaries": true, "selection_retained": true,
        "scroll_retained": true, "staging_scene_retained": true, "bold_author": true, "formatted_time": true}));
    if probe_drag {
        h.record("column_drag_probe", serde_json::json!({"horizontal_resize_cursor":resize_cursor, "press_to_paint_ms":drag_times}));
        ensure!(
            resize_cursor,
            "column boundary hover did not display the horizontal resize cursor"
        );
    }
    Ok(())
}

/// Inspect the actual admitted tree and package artwork before capturing its native pixels.
fn current_branch(h: &Harness) -> anyhow::Result<()> {
    let state = h.worker.state.lock().unwrap();
    let Kind::Tree { handle } = &state.views["git/history"]
        .active_node("branches")
        .context("branch tree missing")?
        .kind
    else {
        anyhow::bail!("branches is not a tree");
    };
    let tree = state
        .trees
        .iter()
        .find(|tree| tree.handle() == handle)
        .context("branch resource missing")?;
    let snapshot = tree.snapshot();
    let local = snapshot
        .nodes
        .values()
        .find(|node| node.branch && matches!(node.label.as_str(), "本地" | "Local"))
        .context("local group missing")?;
    let children = &snapshot
        .branches
        .get(&Some(local.id.clone()))
        .context("local page not loaded")?
        .children;
    let first = snapshot
        .nodes
        .get(children.first().context("local children missing")?)
        .context("first branch missing")?;
    ensure!(
        first.label == "main" && !first.branch,
        "current branch is not first in its level"
    );
    let icon = first
        .icon
        .as_ref()
        .context("current branch marker missing")?;
    ensure!(
        icon.light == "icons/current-branch.svg" && icon.dark == "icons/current-branch-dark.svg",
        "current branch uses ordinary artwork"
    );
    ensure!(
        snapshot
            .nodes
            .values()
            .filter(|node| node
                .icon
                .as_ref()
                .is_some_and(|icon| icon.light == "icons/current-branch.svg"))
            .count()
            == 1,
        "more than the checked-out branch received the current marker"
    );
    ensure!(
        snapshot.icons.contains_key(&icon.light) && snapshot.icons.contains_key(&icon.dark),
        "current marker artwork was not admitted"
    );
    h.record(
        "current_branch_tree",
        serde_json::json!({"current_first": true, "hierarchy_preserved": true,
        "one_current_marker": true, "light_dark_artwork_admitted": true}),
    );
    Ok(())
}
