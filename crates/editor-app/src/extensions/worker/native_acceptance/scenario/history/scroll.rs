//! Native range input preserves every pixel and reuses actual published vector images.
use super::*;
use gpui_base::test_support::snapshots;
use std::sync::Arc;
mod scrollbar;

/// Read vector pixels actually supplied to this canvas, without modifying the renderer.
fn vectors(h: &Harness) -> Vec<Arc<gpui_kit::RenderImage>> {
    let state = h.worker.state.lock().unwrap();
    state
        .images
        .get("git/history/canvas/log")
        .into_iter()
        .flat_map(|images| images.iter().flatten().map(|image| image.image.clone()))
        .collect()
}

/// Read the inline identity to ensure range-only input preserves the selected commit.
fn selected(h: &Harness) -> Option<String> {
    h.worker
        .state
        .lock()
        .unwrap()
        .views
        .get("git/history")?
        .active_node("history-oid")
        .and_then(|node| match &node.kind {
            Kind::Text { text } => Some(text.clone()),
            _ => None,
        })
}

/// Exercise the real actor, native wheel listener, immutable images and boundary ordering.
pub(in crate::extensions::worker::native_acceptance::scenario) async fn exercise(
    h: &Harness,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    h.settle(cx).await?;
    let at = h.main.update(cx, |_, window, _| -> anyhow::Result<Point<Pixels>> {
        // Canvas surfaces need not expose a labelled node; use the adjacent painted tree/dock.
        let tree = input::tree(window, "branches")?;
        let bounds = tree.bounds();
        let content = snapshots(window).into_iter().filter(|node| node.visible()
            && tree.path().starts_with(node.path())
            && matches!(node.path().last(), Some(gpui_kit::ElementId::Name(name)) if name.as_ref() == "local-dock-content-frame"))
            .max_by_key(|node| node.path().len()).context("native history dock is not painted")?.bounds();
        ensure!(content.right() - bounds.right() > px(64.) && bounds.size.height > px(56.), "history canvas has no usable viewport");
        window.activate_window();
        Ok(point(bounds.right() + px(24.), bounds.center().y))
    })??;
    let original = selected(h).context("inline commit selection missing")?;
    let before = vectors(h);
    ensure!(!before.is_empty(), "actual graph pixels missing");
    let offset = log_offset(h);
    let started = std::time::Instant::now();
    h.main.update(cx, |_, window, cx| {
        // One displayed scene emits the complete burst before adopting its first repaint.
        for _ in 0..32 {
            input::wheel(window, at, -5., cx);
        }
    })?;
    h.until(
        h.main,
        "all native wheel pixels published",
        |_, cx| Ok(log_offset(h) == offset + 160. && h.node_current(h.main, "log", cx)),
        cx,
    )
    .await?;
    let publication_ms = started.elapsed().as_millis();
    let after = vectors(h);
    let reused = after
        .iter()
        .filter(|new| before.iter().any(|old| Arc::ptr_eq(new, old)))
        .count();
    ensure!(
        reused > 0,
        "native scroll rasterized every unchanged vector tile"
    );
    ensure!(
        selected(h).as_deref() == Some(original.as_str()),
        "wheel replaced inline selection"
    );
    h.capture(h.main, "history-scroll.png", cx)?;
    h.main.update(cx, |_, window, cx| {
        // Clamping followed by reversal is not equivalent to summing these two deltas.
        input::wheel(window, at, 500., cx);
        input::wheel(window, at, -25., cx);
    })?;
    h.until(
        h.main,
        "wheel reversal preserves boundary order",
        |_, _| Ok(log_offset(h) == 25.),
        cx,
    )
    .await?;
    h.record("history_scroll", serde_json::json!({
        "input_count": 32, "expected_pixels": 160., "actual_pixels": 160.,
        "publication_ms": publication_ms, "reused_vector_images": reused,
        "boundary_reversal_offset": log_offset(h), "selection_retained": selected(h).as_deref() == Some(original.as_str()),
        "diagnostic_only": true,
    }));
    scrollbar::exercise(h, cx).await?;
    Ok(())
}
