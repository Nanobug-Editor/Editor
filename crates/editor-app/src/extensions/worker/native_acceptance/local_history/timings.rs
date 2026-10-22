//! Separate durable native saves, bounded diff preparation and frame-visible history measurements.

use super::*;

/// Percentiles use observed real operations, including native event/frame scheduling where applicable.
fn summary(mut values: Vec<f64>) -> serde_json::Value {
    values.sort_by(f64::total_cmp);
    let percentile = |fraction: f64| values[((values.len() - 1) as f64 * fraction).ceil() as usize];
    serde_json::json!({"samples":values.len(),"p50_ms":percentile(0.5),"p95_ms":percentile(0.95),"max_ms":values.last()})
}

/// Real Ctrl-S saves use the same small document with recording disabled/enabled; no project baseline is bypassed.
pub(super) async fn save_latency(h: &Native, cx: &mut AsyncApp) -> anyhow::Result<()> {
    let (worker, editor) = cx.update(|cx| {
        let app = h.app.read(cx);
        (app.history.worker.clone().unwrap(), app.editor.clone())
    });
    let mut results = serde_json::Map::new();
    for enabled in [false, true] {
        worker
            .call(0, move |history| {
                let mut policy = history.policy()?;
                policy.enabled = enabled;
                history.set_policy(policy)
            })
            .await?;
        let mut samples = Vec::new();
        for _ in 0..12 {
            h.window.update(cx, |_, window, cx| {
                let end = editor.read(cx).value().encode_utf16().count();
                editor.focus_handle(cx).focus(window, cx);
                editor.update(cx, |editor, cx| {
                    editor.replace_text_in_range(Some(end..end), "x", window, cx)
                });
            })?;
            h.until(
                "save latency input",
                |app, _| app.tabs.iter().any(OpenTab::is_dirty),
                cx,
            )
            .await?;
            let started = Instant::now();
            h.window.update(cx, |_, window, cx| {
                editor_ui::testing::press_key(window, "ctrl-s", cx)
            })?;
            h.until(
                "save latency publication",
                |app, _| !app.tabs.iter().any(OpenTab::is_dirty),
                cx,
            )
            .await?;
            // The dirty marker clears at file publication; the worker barrier also includes its Saved history commit.
            worker.call(0, |_| Ok(())).await?;
            samples.push(started.elapsed().as_secs_f64() * 1000.);
        }
        results.insert(
            if enabled { "enabled" } else { "disabled" }.into(),
            summary(samples),
        );
    }
    h.evidence.borrow_mut()["observations"]["native_small_save_including_history_ack"] =
        results.into();
    Ok(())
}

/// Time the actual shared diff implementation on the background executor at its independent byte/line bounds.
pub(super) async fn diff_preparation(h: &Native, cx: &mut AsyncApp) -> anyhow::Result<()> {
    let measured = cx
        .background_executor()
        .spawn(async move {
            let compare = crate::editor::comparison::changed_byte_ranges;
            let mut cases = Vec::new();
            for (name, left, right, accepted) in [
                ("1MiB", "x".repeat(1024 * 1024), "y".into(), true),
                ("1MiB+1", "x".repeat(1024 * 1024 + 1), "y".into(), false),
                ("2000-lines", "x\n".repeat(2_000), "y\n".repeat(2_000), true),
                ("2001-lines", "x\n".repeat(2_001), "y".into(), false),
            ] {
                let mut times = Vec::new();
                for _ in 0..9 {
                    let started = Instant::now();
                    let result = compare(&left, &right);
                    times.push(started.elapsed().as_secs_f64() * 1000.);
                    ensure!(result.is_ok() == accepted, "diff limit changed for {name}");
                }
                cases.push(
                    serde_json::json!({"case":name,"accepted":accepted,"timing":summary(times)}),
                );
            }
            Ok::<_, anyhow::Error>(cases)
        })
        .await?;
    h.evidence.borrow_mut()["observations"]["background_diff_preparation"] = measured.into();
    Ok(())
}
