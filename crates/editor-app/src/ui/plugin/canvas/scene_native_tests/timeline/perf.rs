//! An explicit Release run replays one fixed wall-clock input trajectory in either publication mode.
use super::*;
use gpui_kit::AsyncApp;

/// No performance claim is produced by default; opt in with seconds, mode and an optional raw path.
pub(super) async fn run(
    actor: &Actor,
    view: &Entity<PluginView>,
    window: AnyWindowHandle,
    current: &mut Snapshot,
    handle: usize,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    let Some(seconds) = std::env::var("NANOBUG_CANVAS_TIMELINE_SECONDS").ok() else {
        return Ok(());
    };
    let seconds = seconds.parse::<u64>()?;
    anyhow::ensure!(
        seconds > 0 && seconds <= 3600,
        "explicit timeline duration must be 1..3600 seconds"
    );
    anyhow::ensure!(
        !cfg!(debug_assertions),
        "timeline performance must run the Release test binary"
    );
    let mode = std::env::var("NANOBUG_CANVAS_TIMELINE_MODE").unwrap_or_else(|_| "patch".into());
    anyhow::ensure!(
        matches!(mode.as_str(), "full" | "patch"),
        "publication mode must be full or patch"
    );
    actor.port.send(Input::Command(
        "publication-mode",
        serde_json::json!({"mode":mode}),
    ))?;
    actor.port.send(Input::Command(
        "set-offset",
        serde_json::json!({"x":0.,"y":0.}),
    ))?;
    actor
        .port
        .send(Input::Command("animate", serde_json::Value::Null))?;
    smoke::pump(actor, view, window, current, Duration::from_millis(500), cx).await?;
    anyhow::ensure!(
        current.state["seed"].as_u64() == Some(1729),
        "timeline seed changed"
    );
    anyhow::ensure!(
        current.state["full_publication"].as_bool() == Some(mode == "full"),
        "guest did not apply requested publication mode: {}",
        current.state
    );
    let mut recorder = window.update(cx, |_, window, _| {
        super::super::timeline_perf::Recorder::start(&mode, seconds, current, window)
    })??;
    let initial_bytes = current.bytes;
    let start = Instant::now();
    let mut frame_sample = start;
    let mut counter_sample = start;
    let mut step = 0_u128;
    let mut input_count = 0_u64;
    let mut last_offset = [0_f64; 2];
    let mut increased = [false; 2];
    let mut decreased = [false; 2];
    let measured = async {
        while start.elapsed() < Duration::from_secs(seconds) {
            smoke::pump(actor, view, window, current, Duration::from_millis(8), cx).await?;
            for axis in 0..2 {
                if let Some(offset) = current.state["offset"][axis].as_f64() {
                    increased[axis] |= offset > last_offset[axis] + 0.25;
                    decreased[axis] |= offset < last_offset[axis] - 0.25;
                    last_offset[axis] = offset;
                }
            }
            let desired = start.elapsed().as_millis() / 400;
            if desired > step {
                // Every 9.6 seconds both axes return. Slow hosts never receive a burst of stale inputs.
                anyhow::ensure!(
                    desired - step == 1,
                    "native benchmark missed its fixed input cadence"
                );
                step = desired;
                let horizontal = step % 2 == 0;
                let outward = ((step - 1) / 12) % 2 == 0;
                let delta = if horizontal == outward { 480 } else { -480 };
                cx.background_executor()
                    .spawn(
                        async move { super::super::timeline_os::wheel(handle, horizontal, delta) },
                    )
                    .await?;
                input_count += 1;
            }
            if frame_sample.elapsed() >= Duration::from_millis(100) {
                recorder.frames()?;
                frame_sample = Instant::now();
            }
            if counter_sample.elapsed() >= Duration::from_secs(1) {
                window.update(cx, |_, window, _| recorder.sample(current, window, false))??;
                anyhow::ensure!(
                    current.queue_overflow == 0,
                    "native benchmark input queue overflowed"
                );
                anyhow::ensure!(
                    current.images.statistics().cached_items < 10000,
                    "native benchmark materialized the entire logical dataset"
                );
                counter_sample = Instant::now();
            }
        }
        anyhow::ensure!(seconds < 12 || (increased.into_iter().all(|value| value)
                && decreased.into_iter().all(|value| value)),
            "physical input did not move both actual guest axes out and back: increased={increased:?}, decreased={decreased:?}");
        Ok::<(), anyhow::Error>(())
    }
    .await;
    // Preserve the final partial sample even when a real input, resource or timing assertion failed.
    window.update(cx, |_, window, _| recorder.sample(current, window, true))??;
    eprintln!(
        "actual timeline {mode} run: {:.3}s, {input_count} physical wheel inputs, result={measured:?}",
        start.elapsed().as_secs_f64()
    );
    measured?;
    anyhow::ensure!(
        current.bytes > initial_bytes,
        "native animation produced no observed publication bytes"
    );
    anyhow::ensure!(
        current.missing_publication_bytes == 0
            && current.publications == current.expected_publications,
        "publication byte observation is incomplete: observed={}, expected={}, missing_bytes={}",
        current.publications,
        current.expected_publications,
        current.missing_publication_bytes
    );
    Ok(())
}
