//! Real Windows input verifies free-edge child moves, own-side group fallback and unique occupied sides.
use super::*;
use nested::{frames, place};

/// Reject an editor-edge gesture, preserving all four live allocations before and after release.
async fn reject(
    h: &Harness,
    source: usize,
    pointer: Point<Pixels>,
    name: &str,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    let before = h.main.update(cx, |_, window, _| frames(window))??;
    begin_drag(h, before[source], cx).await?;
    sample_drag(h, pointer, name, cx)?;
    h.main.update(cx, |_, window, cx| {
        let nodes = snapshots(window);
        for name in [
            "local-dock-drop-indicator",
            "local-dock-source-1",
            "local-dock-drag-card-1",
        ] {
            ensure!(
                named(&nodes, name).is_none(),
                "occupied side still advertises {name}"
            );
        }
        ensure!(
            frames(window)? == before,
            "rejected hover changed live allocations"
        );
        window.dispatch_event(
            PlatformInput::MouseUp(MouseUpEvent {
                position: pointer,
                button: MouseButton::Left,
                modifiers: Default::default(),
                click_count: 1,
            }),
            cx,
        );
        Ok::<_, anyhow::Error>(())
    })??;
    h.settle(cx).await?;
    h.main.update(cx, |_, window, cx| {
        ensure!(
            frames(window)? == before,
            "rejected release changed live allocations"
        );
        ensure!(!cx.has_active_drag());
        Ok::<_, anyhow::Error>(())
    })??;
    h.capture(h.main, name, cx)
}

/// A side child can use a free side or join an existing region, then return without moving peers.
pub(super) async fn exercise(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    let before = h.main.update(cx, |_, window, _| frames(window))??;
    grouped::cancel(
        h,
        1,
        gpui_base::Placement::Left,
        &[0, 1],
        "own-left-group-cancelled.png",
        cx,
    )
    .await?;
    reject(
        h,
        1,
        point(before[2].center().x, before[2].bottom() - px(8.)),
        "child-occupied-bottom-rejected.png",
        cx,
    )
    .await?;
    place(
        h,
        1,
        point(before[2].right() - px(8.), before[2].center().y),
        "child-free-right-preview.png",
        cx,
    )
    .await?;
    let right = h.main.update(cx, |_, window, _| frames(window))??;
    ensure!(
        (right[0].left() - before[0].left()).abs() < px(2.)
            && (right[0].size.width - before[0].size.width).abs() < px(2.),
        "left sibling width changed"
    );
    ensure!(
        right[2].right() <= right[1].left() + px(2.)
            && (right[1].size.width - before[1].size.width).abs() < px(2.),
        "only the child must occupy the free right side"
    );
    ensure!(
        right[3] == before[3],
        "moving the child changed the unrelated Git allocation"
    );
    h.capture(h.main, "child-free-right-placed.png", cx)?;
    reject(
        h,
        0,
        point(right[2].right() - px(8.), right[2].center().y),
        "child-center-right-occupied-rejected.png",
        cx,
    )
    .await?;
    place(
        h,
        1,
        point(right[3].left() + px(8.), right[3].center().y),
        "child-join-existing-bottom-preview.png",
        cx,
    )
    .await?;
    let joined = h.main.update(cx, |_, window, _| frames(window))??;
    ensure!(
        joined[1].right() <= joined[3].left() + px(2.)
            && (joined[1].top() - joined[3].top()).abs() < px(2.),
        "child did not join the existing bottom row"
    );
    ensure!((joined[0].size.width - before[0].size.width).abs() < px(2.));
    h.capture(h.main, "child-join-existing-bottom-placed.png", cx)?;
    grouped::cancel(
        h,
        1,
        gpui_base::Placement::Bottom,
        &[1, 3],
        "own-bottom-group-cancelled.png",
        cx,
    )
    .await?;
    place(
        h,
        1,
        point(joined[0].center().x, joined[0].bottom() - px(8.)),
        "child-return-existing-left-preview.png",
        cx,
    )
    .await?;
    let returned = h.main.update(cx, |_, window, _| frames(window))??;
    ensure!(
        (returned[0].left() - returned[1].left()).abs() < px(2.)
            && (returned[0].size.width - returned[1].size.width).abs() < px(2.)
            && returned[0].bottom() <= returned[1].top() + px(2.)
    );
    ensure!(
        returned[2] == before[2] && returned[3] == before[3],
        "returning the child did not restore available editor/bottom space"
    );
    h.capture(h.main, "child-return-existing-left-placed.png", cx)?;
    h.record("single_side_child_placement", serde_json::json!({
        "free_edge_moves_one_window": true, "own_left_and_bottom_preview_the_whole_group": true,
        "free_right_moves_only_child": true, "center_local_right_also_reserves_side": true,
        "remaining_left_width_preserved": true, "invalid_release_preserves_layout": true,
        "concrete_target_joins_existing_bottom_row": true, "group_cancellation_keeps_the_original_region": true,
        "return_to_existing_left_restores_available_space": true,
        "input": "production Window.dispatch_event on WindowsPlatform",
    }));
    Ok(())
}
