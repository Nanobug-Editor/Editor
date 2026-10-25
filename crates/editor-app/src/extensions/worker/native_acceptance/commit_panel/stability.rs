//! Inspect every native frame of a real checkbox write, including its transient busy presentation.
use super::*;
use gpui_kit::{Bounds, Pixels};

/// Painted geometry and native ownership must survive an index write without remount or selection loss.
#[derive(Debug, PartialEq)]
struct Observation {
    /// The native tree viewport cannot shrink when feedback first appears.
    tree: Bounds<Pixels>,
    /// The existing multiline draft stays at the same position and size.
    draft: Bounds<Pixels>,
    /// The production PluginView remains mounted across every receipt.
    viewer: String,
    /// A checkbox changes index intent, never the native file selection.
    selected: Option<String>,
}

/// Read the actual rendered controls and existing host view; no alternate tree model is introduced.
fn observe(h: &Harness, window: &Window, cx: &App) -> anyhow::Result<Observation> {
    let app = h.app.read(cx);
    let panel = app
        .plugin_panels
        .get("git/staging")
        .context("commit panel missing")?
        .read(cx);
    ensure!(
        panel
            .current_document()
            .is_some_and(|document| document.active_node("staging").is_some()),
        "native commit tree was disabled during a checkbox operation"
    );
    let view = panel
        .native_ui
        .as_ref()
        .context("native commit view missing")?;
    Ok(Observation {
        tree: input::tree(window, "staging")?.bounds(),
        draft: input::field(window, "commit-message")?.bounds(),
        viewer: format!("{:?}", view.entity_id()),
        selected: view
            .read(cx)
            .native_tree_selected_id("staging", cx)
            .context("native tree missing")?,
    })
}

/// Click the named painted checkbox and check each frame until its guest scene has changed and settled.
/// The caller separately verifies the actual Git result, including expected lock failures.
pub(super) async fn click(h: &Harness, label: &str, cx: &mut AsyncApp) -> anyhow::Result<()> {
    let before = h
        .main
        .update(cx, |_, window, cx| observe(h, window, cx))??;
    let previous = h.main.update(cx, |_, window, _| {
        let node = checkbox(window, label)?;
        Ok::<_, anyhow::Error>((node.checked(), node.indeterminate()))
    })??;
    let started = std::time::Instant::now();
    let mut first_idle_ms = None;
    let mut first_feedback_ms = None;
    h.main.update(cx, |_, window, cx| {
        input::click(window, &checkbox(window, label)?, cx)
    })??;
    let mut stable = 0;
    let mut frames = 0;
    h.until(
        h.main,
        "checkbox retains commit tree and composer",
        |window, cx| {
            frames += 1;
            ensure!(
                observe(h, window, cx)? == before,
                "checkbox moved/remounted the commit UI"
            );
            let node = checkbox(window, label)?;
            if (node.checked(), node.indeterminate()) != previous {
                first_feedback_ms.get_or_insert_with(|| started.elapsed().as_millis());
            }
            let idle = {
                let state = h.worker.state.lock().unwrap();
                let document = &state.views["git/staging"];
                ensure!(
                    document.active_node("staging").is_some(),
                    "guest withdrew the whole commit tree"
                );
                first_feedback_ms.is_some() && state.processes.get("git").copied().unwrap_or(0) == 0
            };
            stable = if idle && h.node_current(h.main, "staging", cx) {
                first_idle_ms.get_or_insert_with(|| started.elapsed().as_millis());
                stable + 1
            } else {
                0
            };
            Ok(stable >= 8)
        },
        cx,
    )
    .await?;
    h.record(
        "checkbox_stable_frames",
        serde_json::json!({"last_checked": label, "frames": frames, "first_feedback_ms": first_feedback_ms, "first_idle_ms": first_idle_ms, "settled_ms": started.elapsed().as_millis()}),
    );
    h.checkpoint(
        &format!("checkbox_latency_{label}"),
        serde_json::json!({"feedback_ms": first_feedback_ms, "idle_ms": first_idle_ms}),
    )?;
    ensure!(
        first_feedback_ms.is_some_and(|millis| millis < 350),
        "checkbox feedback exceeded 350 ms: {first_feedback_ms:?}"
    );
    Ok(())
}
