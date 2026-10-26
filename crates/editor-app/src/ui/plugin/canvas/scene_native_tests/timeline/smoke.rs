//! Check visibility, independent business work, idle invalidations and real two-axis native input.
use super::*;
use gpui_kit::AsyncApp;

/// Pump only complete ordinary publications; waiting itself never invalidates or explicitly draws.
pub(super) async fn pump(
    actor: &Actor,
    view: &Entity<PluginView>,
    window: AnyWindowHandle,
    current: &mut Snapshot,
    duration: Duration,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    let until = Instant::now() + duration;
    while Instant::now() < until {
        if let Some(next) = actor.take() {
            apply(view, window, current, next, cx)?;
        }
        cx.background_executor()
            .timer(Duration::from_millis(8))
            .await;
    }
    Ok(())
}

/// Require real native leases to progress, bounded from package-ready rather than cold compilation.
pub(super) async fn frames(
    actor: &Actor,
    view: &Entity<PluginView>,
    window: AnyWindowHandle,
    current: &mut Snapshot,
    minimum: u64,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(6);
    while current.state["frames"].as_u64().unwrap_or(0) < minimum {
        pump(actor, view, window, current, Duration::from_millis(8), cx).await?;
        anyhow::ensure!(
            Instant::now() < deadline,
            "actual visible GPUI opportunities did not reach timeline: {}",
            current.state
        );
    }
    Ok(())
}

/// OS readback is outside measured idle/performance intervals and proves visible output changed.
fn pixels(window: AnyWindowHandle, cx: &mut AsyncApp) -> anyhow::Result<u64> {
    window.update(cx, |_, window, _| {
        let image = window.render_to_image()?;
        Ok(image
            .as_raw()
            .iter()
            .fold(14695981039346656037_u64, |hash, byte| {
                (hash ^ u64::from(*byte)).wrapping_mul(1099511628211)
            }))
    })?
}

/// The normal view visibility adapter and actual HWND hide together, preserving independent timers.
pub(super) async fn exercise(
    actor: &Actor,
    view: &Entity<PluginView>,
    window: AnyWindowHandle,
    current: &mut Snapshot,
    cx: &mut AsyncApp,
) -> anyhow::Result<usize> {
    let handle = window.update(cx, |_, window, _| uia::owned_handle(window))??;
    eprintln!("native timeline phase: hide; state={}", current.state);
    view.update(cx, |view, cx| view.set_scene_current(false, cx));
    // GPUI's normal minimize/activate pair delivers WM_SIZE in both directions. Raw ShowWindow
    // may leave the locked platform's cached visibility stale despite the OS window being shown.
    window.update(cx, |_, window, _| window.minimize_window())?;
    pump(actor, view, window, current, Duration::from_millis(200), cx).await?;
    anyhow::ensure!(
        !window.update(cx, |_, window, _| window.is_visible())?,
        "actual minimized window still reports visible"
    );
    let hidden_frames = current.state["frames"].as_u64().unwrap();
    let hidden_ticks = current.state["business_ticks"].as_u64().unwrap();
    pump(actor, view, window, current, Duration::from_millis(250), cx).await?;
    anyhow::ensure!(
        current.state["frames"].as_u64() == Some(hidden_frames),
        "hidden Canvas consumed a native frame: {}",
        current.state
    );
    anyhow::ensure!(
        current.state["business_ticks"].as_u64().unwrap() > hidden_ticks,
        "hiding Canvas paused the independent business timer"
    );
    // Publish source authority before native activation can paint or dispatch pointer input.
    view.update(cx, |view, cx| view.set_scene_current(true, cx));
    window.update(cx, |_, window, _| {
        // Restoring visibility is one native state transition. Activate and repaint it once;
        // ordinary frame requests remain non-dirty and idle still asserts zero further draws.
        window.activate_window();
        window.refresh();
    })?;
    let restored = Instant::now() + Duration::from_secs(2);
    while !window.update(cx, |_, window, _| window.is_visible())? {
        pump(actor, view, window, current, Duration::from_millis(8), cx).await?;
        anyhow::ensure!(
            Instant::now() < restored,
            "normal GPUI activation did not restore visible frames"
        );
    }
    cx.background_executor()
        .spawn(async move { super::super::timeline_os::foreground(handle) })
        .await?;
    eprintln!("native timeline phase: resume; state={}", current.state);
    frames(actor, view, window, current, hidden_frames + 2, cx).await?;
    actor
        .port
        .send(Input::Command("stop", serde_json::Value::Null))?;
    eprintln!("native timeline phase: stop/idle; state={}", current.state);
    pump(actor, view, window, current, Duration::from_millis(400), cx).await?;
    let idle_frames = current.state["frames"].as_u64().unwrap();
    let idle_ticks = current.state["business_ticks"].as_u64().unwrap();
    let before = window.update(cx, |_, window, _| {
        window
            .frame_duration_snapshot()
            .draw_duration_histogram
            .len()
    })?;
    pump(actor, view, window, current, Duration::from_millis(300), cx).await?;
    let after = window.update(cx, |_, window, _| {
        window
            .frame_duration_snapshot()
            .draw_duration_histogram
            .len()
    })?;
    anyhow::ensure!(
        current.state["frames"].as_u64() == Some(idle_frames) && before == after,
        "idle Canvas dirtied the window: draws {before}->{after}, state {}",
        current.state
    );
    anyhow::ensure!(
        current.state["business_ticks"].as_u64().unwrap() > idle_ticks,
        "business timer did not continue during zero-draw idle"
    );
    let original = pixels(window, cx)?;
    eprintln!(
        "native timeline phase: native wheels; state={}",
        current.state
    );
    cx.background_executor()
        .spawn(async move { super::super::timeline_os::wheel(handle, false, -480) })
        .await?;
    pump(actor, view, window, current, Duration::from_millis(250), cx).await?;
    cx.background_executor()
        .spawn(async move { super::super::timeline_os::wheel(handle, true, 480) })
        .await?;
    pump(actor, view, window, current, Duration::from_millis(250), cx).await?;
    let offset = current.state["offset"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("offset absent"))?;
    anyhow::ensure!(
        offset[0].as_f64().unwrap() > 0. && offset[1].as_f64().unwrap() > 0.,
        "actual two-axis OS input did not reach guest offset: {}",
        current.state
    );
    anyhow::ensure!(
        pixels(window, cx)? != original,
        "two-axis scroll left actual native pixels unchanged"
    );
    let statistics = current.images.statistics();
    anyhow::ensure!(
        statistics.cached_items > 0 && statistics.cached_items < 10000,
        "viewport/overscan preparation unexpectedly materialized all 100000 logical events: {statistics:?}"
    );
    anyhow::ensure!(
        statistics.paragraph_layouts > 0,
        "visible bilingual labels were not natively shaped"
    );
    anyhow::ensure!(
        current.queue_overflow == 0,
        "timeline native input queue overflow"
    );
    eprintln!(
        "native timeline smoke: visible/resume frames, hidden pause/business timer, idle zero draw, OS x/y scroll/pixels; cache={statistics:?}; state={}",
        current.state
    );
    Ok(handle)
}
