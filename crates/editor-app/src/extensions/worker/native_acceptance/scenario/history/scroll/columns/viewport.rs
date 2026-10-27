//! Real split drags fit narrow columns immediately and keep that frame stable across guest replies.
use super::*;

/// Both positions and widths matter: a fixed right edge alone can conceal a second layout pass.
#[derive(Clone, Copy)]
struct Geometry {
    width: f32,
    metadata: [[f32; 2]; 3],
    revision: u64,
}

/// Read the rectangles actually painted by CanvasView, rather than the guest's last snapshot.
fn metadata(h: &Harness, window: &Window, cx: &App) -> anyhow::Result<Geometry> {
    let snapshot = native_canvas(h, window, cx).context("history canvas not painted")?;
    let origin = snapshot["origin_x"]
        .as_f64()
        .context("canvas origin missing")? as f32;
    let width = snapshot["width"].as_f64().context("canvas width missing")? as f32;
    let boxes = snapshot["painted_boxes"]
        .as_array()
        .context("painted columns missing")?;
    ensure!(boxes.len() >= 4, "four painted columns required");
    let mut positions = [[0.; 2]; 3];
    for (index, rect) in boxes.iter().skip(1).take(3).enumerate() {
        positions[index] = [
            origin + rect["x"].as_f64().context("column x missing")? as f32,
            rect["w"].as_f64().context("column width missing")? as f32,
        ];
    }
    Ok(Geometry {
        width,
        metadata: positions,
        revision: snapshot["paint_revision"]
            .as_u64()
            .context("paint revision missing")?,
    })
}

/// Legal compression may move text when bounds change; a reply at identical bounds must not move it.
fn stable(before: Geometry, after: Geometry) -> anyhow::Result<Vec<[f32; 2]>> {
    let errors: Vec<_> = after
        .metadata
        .iter()
        .zip(before.metadata)
        .map(|(after, before)| [after[0] - before[0], after[1] - before[1]])
        .collect();
    ensure!(
        (after.width - before.width).abs() <= 0.01
            && errors.iter().flatten().all(|error| error.abs() <= 1.),
        "guest reply changed native fitted columns at the same bounds: width={}→{}, errors={errors:?}",
        before.width,
        after.width
    );
    Ok(errors)
}

/// Move the real branches/history divider back and forth, observing every native frame and reply.
/// The temporary fixture is restored to its initial split; no user's workspace is opened here.
pub(super) async fn exercise(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    let selected_before = selected(h);
    let offset_before = log_offset(h);
    let staging_revision = h.worker.state.lock().unwrap().views["git/staging"].revision;
    let (start, baseline) = h.main.update(cx, |_, window, cx| -> anyhow::Result<_> {
        let branch_bounds = input::tree(window, "branches")?.bounds();
        let canvas = scrollbar::viewport(h, window, cx)?;
        let divider_x = (branch_bounds.right() + canvas.left()) / 2.;
        Ok((
            point(divider_x, canvas.center().y),
            metadata(h, window, cx)?,
        ))
    })??;
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
    let mut samples = Vec::new();
    let mut changed = false;
    for delta in [
        8., 16., 24., 40., 64., 80., 56., 32., 16., 0., -16., -32., -16., 0.,
    ] {
        let immediate = h.main.update(cx, |_, window, cx| -> anyhow::Result<_> {
            window.dispatch_event(
                PlatformInput::MouseMove(MouseMoveEvent {
                    position: start + point(px(delta), px(0.)),
                    pressed_button: Some(MouseButton::Left),
                    modifiers: Default::default(),
                }),
                cx,
            );
            // Draw before the harness polls the worker: this must use the new bounds with the
            // previously admitted artwork, exposing the original one-frame translation defect.
            window.draw(cx).clear(cx);
            metadata(h, window, cx)
        })??;
        changed |= (immediate.width - baseline.width).abs() > 4.;
        let right = |geometry: Geometry| geometry.metadata[2][0] + geometry.metadata[2][1];
        ensure!(
            (right(immediate) - right(baseline)).abs() <= 1.,
            "first native frame lost the fixed right edge before guest polling"
        );
        h.frame(h.main, cx).await?;
        let polled = h
            .main
            .update(cx, |_, window, cx| metadata(h, window, cx))??;
        let errors = stable(immediate, polled)?;
        samples.push(
            serde_json::json!({"pointer_delta":delta,"width":immediate.width,
                "first_paint_revision":immediate.revision, "polled_revision":polled.revision,
                "metadata_error_px":errors}),
        );
        if delta == 80. {
            // Keep capture while the worker catches up: a correct first frame alone cannot rule
            // out the original snap when its asynchronous Resize drawing finally arrives.
            h.settle(cx).await?;
            let settled = h
                .main
                .update(cx, |_, window, cx| metadata(h, window, cx))??;
            stable(immediate, settled)?;
            // Narrow reference artwork may be identical and legitimately produce no new scene.
            samples.push(serde_json::json!({"held_pointer_delta":delta,
                "first_paint_revision":immediate.revision,"settled_revision":settled.revision,
                "metadata_error_px":stable(immediate, settled)?}));
        }
    }
    h.main.update(cx, |_, window, cx| {
        window.dispatch_event(
            PlatformInput::MouseUp(MouseUpEvent {
                position: start,
                button: MouseButton::Left,
                modifiers: Default::default(),
                click_count: 1,
            }),
            cx,
        );
    })?;
    h.settle(cx).await?;
    ensure!(changed, "outer divider never changed the history viewport");
    ensure!(
        selected(h) == selected_before && log_offset(h) == offset_before,
        "outer divider changed selection or vertical scroll"
    );
    ensure!(
        h.worker.state.lock().unwrap().views["git/staging"].revision == staging_revision,
        "outer divider republished the commit panel"
    );
    let final_geometry = h
        .main
        .update(cx, |_, window, cx| metadata(h, window, cx))??;
    ensure!(
        (final_geometry.width - baseline.width).abs() <= 1.,
        "split did not restore its original width"
    );
    stable(baseline, final_geometry)?;
    h.record("history_viewport_columns", serde_json::json!({
        "native_split_drag":true, "samples":samples, "selection_retained":true,
        "scroll_retained":true, "staging_scene_retained":true, "restored_width":final_geometry.width,
    }));
    h.capture(h.main, "history-viewport-columns.png", cx)?;
    Ok(())
}
