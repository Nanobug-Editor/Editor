//! Native Windows evidence for the generic scrollbar projected onto an independent Git canvas.
use super::*;
use gpui_kit::MouseMoveEvent;

/// Observe the current native canvas bounds, independent of guest business state and tree guesses.
pub(super) fn viewport(h: &Harness, window: &Window, cx: &App) -> anyhow::Result<Bounds<Pixels>> {
    let snapshot = h.app.read(cx).plugin_panels["git/history"]
        .read(cx)
        .native_ui
        .as_ref()
        .context("history native UI missing")?
        .read(cx)
        .native_canvas_input_snapshot("log", window, cx)
        .context("log not painted")?;
    let number = |key: &str| {
        snapshot[key]
            .as_f64()
            .map(|value| px(value as f32))
            .with_context(|| format!("canvas {key} missing"))
    };
    Ok(Bounds::new(
        point(number("origin_x")?, number("origin_y")?),
        gpui_kit::size(number("width")?, number("height")?),
    ))
}

/// Read native quad thickness/alpha at the log's real right edge; hidden zero-area quads are excluded.
fn thumb(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<Option<(f32, f32, Bounds<Pixels>)>> {
    h.main.update(cx, |_, window, cx| -> anyhow::Result<_> {
        let bounds = viewport(h, window, cx)?;
        let scale = window.scale_factor();
        // Scene buffers can retain clipped/covered primitives from cached paint. Verify a
        // candidate against the current D3D target before counting it as a visible thumb.
        let image = window
            .render_to_image()
            .context("scrollbar D3D readback failed")?;
        let pixel = |point: Point<Pixels>| {
            let x = ((point.x / px(1.) * scale).floor() as u32).min(image.width() - 1);
            let y = ((point.y / px(1.) * scale).floor() as u32).min(image.height() - 1);
            image.get_pixel(x, y).0
        };
        Ok(window
            .painted_quads()
            .into_iter()
            .filter_map(|quad| {
                let color = quad.background.as_solid()?;
                let visible = quad.bounds.intersect(&quad.content_mask.bounds);
                let rect = Bounds::new(
                    point(px(visible.left().0 / scale), px(visible.top().0 / scale)),
                    gpui_kit::size(
                        px(visible.size.width.0 / scale),
                        px(visible.size.height.0 / scale),
                    ),
                );
                let width = rect.size.width / px(1.);
                let admitted = color.a > 0.
                    && width > 0.
                    && width <= 8.1
                    && rect.size.height >= px(24.)
                    && (rect.right() - bounds.right() + px(4.)).abs() < px(0.2)
                    && bounds.contains(&rect.center());
                if !admitted {
                    return None;
                }
                let actual = pixel(rect.center());
                let clearance = pixel(point(bounds.right() - px(2.), rect.center().y));
                (0..3)
                    .any(|channel| actual[channel].abs_diff(clearance[channel]) > 3)
                    .then_some((quad.order, width, color.a, rect))
            })
            .max_by_key(|(order, ..)| *order)
            .map(|(_, width, alpha, rect)| (width, alpha, rect)))
    })?
}

/// Drive the original Base rail, then inspect true native frames through the complete idle contraction.
pub(super) async fn exercise(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    // Ordinary acceptance deliberately disables motion. This isolated animation probe needs
    // real motion enabled; production continues to respect the user's reduced-motion setting.
    h.main.update(cx, |_, _, cx| cx.set_reduce_motion(false))?;
    let at = h.main.update(cx, |_, window, cx| -> anyhow::Result<_> {
        let bounds = viewport(h, window, cx)?;
        Ok(point(bounds.left() + px(24.), bounds.center().y))
    })??;
    let before = log_offset(h);
    h.main
        .update(cx, |_, window, cx| input::wheel(window, at, -1., cx))?;
    h.until(
        h.main,
        "scrollbar reveal receipt",
        |_, _| Ok(log_offset(h) == before + 1.),
        cx,
    )
    .await?;
    cx.background_executor()
        .timer(Duration::from_millis(210))
        .await;
    h.frame(h.main, cx).await?;
    let (width, _, rect) = thumb(h, cx)?.context("log scrollbar must paint inside its canvas")?;
    ensure!(
        (width - 6.).abs() < 0.1,
        "unexpected revealed thumb width {width}"
    );
    h.capture(h.main, "history-scrollbar-visible.png", cx)?;
    let selected_before = selected(h);
    let destination = rect.center() + point(px(0.), px(36.));
    h.main.update(cx, |_, window, cx| {
        window.dispatch_event(
            PlatformInput::MouseMove(MouseMoveEvent {
                position: rect.center(),
                pressed_button: None,
                modifiers: Default::default(),
            }),
            cx,
        );
        window.dispatch_event(
            PlatformInput::MouseDown(MouseDownEvent {
                position: rect.center(),
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
                position: destination,
                pressed_button: Some(MouseButton::Left),
                modifiers: Default::default(),
            }),
            cx,
        );
    })?;
    h.until(
        h.main,
        "native scrollbar moves the guest range",
        |_, _| Ok(log_offset(h) > before + 1.),
        cx,
    )
    .await?;
    h.main.update(cx, |_, window, cx| {
        window.dispatch_event(
            PlatformInput::MouseUp(MouseUpEvent {
                position: destination,
                button: MouseButton::Left,
                modifiers: Default::default(),
                click_count: 1,
            }),
            cx,
        );
        window.dispatch_event(
            PlatformInput::MouseMove(MouseMoveEvent {
                position: at,
                pressed_button: None,
                modifiers: Default::default(),
            }),
            cx,
        );
    })?;
    // Range acknowledgements can follow release; wait for the accepted scene before measuring idle.
    h.settle(cx).await?;
    let offset = log_offset(h);
    let started = Instant::now();
    let mut samples = Vec::new();
    let mut intermediate = false;
    let mut hidden = false;
    let mut alpha = None;
    while started.elapsed() < Duration::from_secs(4) {
        h.frame(h.main, cx).await?;
        if let Some((width, opacity, _)) = thumb(h, cx)? {
            if width <= 6.1 {
                let original = *alpha.get_or_insert(opacity);
                ensure!(
                    (opacity - original).abs() < 0.001,
                    "hiding faded the thumb instead of shrinking it"
                );
                if samples.len() < 32 || width < 5.8 {
                    samples.push(serde_json::json!({"ms": started.elapsed().as_millis(), "width": width, "alpha": opacity}));
                }
                if width < 5.8 && !intermediate {
                    intermediate = true;
                    h.capture(h.main, "history-scrollbar-contracting.png", cx)?;
                }
            }
        } else {
            hidden = true;
            break;
        }
        ensure!(
            log_offset(h) == offset,
            "width animation changed the guest scroll position"
        );
    }
    ensure!(
        intermediate && hidden,
        "native frames must show contraction followed by zero width: {samples:?}"
    );
    ensure!(
        selected(h) == selected_before,
        "native scrollbar drag selected another commit"
    );
    h.capture(h.main, "history-scrollbar-hidden.png", cx)?;
    h.record(
        "scrollbar_width",
        serde_json::json!({
            "revealed_width": width, "drag_offset": offset, "hidden": hidden,
            "intermediate_observed": intermediate, "selection_retained": true, "samples": samples,
            "d3d_pixels_verified": true, "reduced_motion_during_probe": false,
        }),
    );
    Ok(())
}
