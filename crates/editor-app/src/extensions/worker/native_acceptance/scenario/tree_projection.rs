//! Capture settled provider pages and visible native leaves after refresh, without repeating input.
use super::*;

/// Git idle can precede lazy-page publication; wait for this staging scene's real metadata and rows.
pub(super) async fn staging(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    h.until(
        h.main,
        "refreshed staging metadata and native leaves",
        |window, cx| {
            if !h.node_current(h.main, "staging", cx) {
                return Ok(false);
            }
            let Some(metadata) = ready_metadata(h, "staging", "staging") else {
                return Ok(false);
            };
            let leaf = input::tree_item(window, "staging", "a-review.txt").is_ok()
                || (0..28).any(|index| {
                    input::tree_item(window, "staging", &format!("z-note-{index:02}.txt")).is_ok()
                });
            if leaf {
                h.record("staging_refresh_native_projection", metadata);
            }
            Ok(leaf)
        },
        cx,
    )
    .await
}

/// Proposals and IME may refresh the sidebar after its earlier selection. The screenshot must
/// show the same selected file again without expanding, refreshing or clicking it a second time.
pub(super) async fn conflicts(
    h: &Harness,
    target: AnyWindowHandle,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    h.until(
        target,
        "refreshed conflict metadata and selected native leaf",
        |window, cx| {
            if !h.node_current(target, "conflicts", cx) {
                return Ok(false);
            }
            let Some(metadata) = ready_metadata(h, "conflicts", "conflicts") else {
                return Ok(false);
            };
            let selected = input::tree_item(window, "conflicts", "merge.txt")
                .is_ok_and(|leaf| leaf.selected() == Some(true));
            if selected {
                h.record("conflict_refresh_native_projection", metadata);
            }
            Ok(selected)
        },
        cx,
    )
    .await
}

/// Read only this current panel's declared resource. Every expanded page must have completed
/// successfully; a missing resource, loading placeholder or error never counts as visible readiness.
fn ready_metadata(h: &Harness, panel: &str, node: &str) -> Option<serde_json::Value> {
    let state = h.worker.state.lock().unwrap();
    let Kind::Tree { handle } = &state
        .views
        .get(&format!("git/{panel}"))?
        .active_node(node)?
        .kind
    else {
        return None;
    };
    let resource = state
        .trees
        .iter()
        .find(|resource| resource.handle() == handle)?;
    let snapshot = resource.snapshot();
    let ready = std::iter::once(None)
        .chain(snapshot.expanded.iter().cloned().map(Some))
        .all(|parent| {
            snapshot
                .branches
                .get(&parent)
                .is_some_and(|branch| branch.loaded && !branch.loading && branch.error.is_none())
        });
    ready.then(|| {
        serde_json::json!({"epoch": snapshot.epoch, "revision": snapshot.revision,
            "expanded": snapshot.expanded, "loaded_expanded_pages": true})
    })
}
