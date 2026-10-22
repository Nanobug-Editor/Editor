//! Actual Windows/D3D input verifies whole combinations on every side, including a real plugin member.
use super::*;
use gpui_base::Placement;
use nested::{frames, place};

/// Resolve an editor edge in window-space logical pixels, retaining the original real allocations.
fn edge(bounds: Bounds<Pixels>, side: Placement) -> Point<Pixels> {
    match side {
        Placement::Left => point(bounds.left() + px(8.), bounds.center().y),
        Placement::Right => point(bounds.right() - px(8.), bounds.center().y),
        Placement::Top => point(bounds.center().x, bounds.top() + px(8.)),
        Placement::Bottom => point(bounds.center().x, bounds.bottom() - px(8.)),
    }
}

/// Compare actual source marks with `members`' original rectangles and require matching cards/targets.
fn check_members(
    window: &Window,
    before: &[Bounds<Pixels>; 4],
    members: &[usize],
) -> anyhow::Result<()> {
    let nodes = snapshots(window);
    for (index, member) in members.iter().enumerate() {
        for kind in ["source", "target", "drag-card"] {
            let name = format!("local-dock-{kind}-{index}");
            let node = named(&nodes, &name).with_context(|| format!("group {name} missing"))?;
            if kind == "source" {
                ensure!(
                    node.bounds() == before[*member],
                    "group source order/allocation changed"
                );
            }
        }
    }
    ensure!(named(&nodes, &format!("local-dock-source-{}", members.len())).is_none());
    ensure!(
        frames(window)? == *before,
        "group preview changed real allocations"
    );
    Ok(())
}

/// Preview the whole same-side group from `source`, then Escape without changing any real panel.
pub(super) async fn cancel(
    h: &Harness,
    source: usize,
    side: Placement,
    members: &[usize],
    name: &str,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    let before = h.main.update(cx, |_, window, _| frames(window))??;
    begin_drag(h, before[source], cx).await?;
    preview_drag(h, edge(before[2], side), name, cx).await?;
    h.main.update(cx, |_, window, cx| {
        check_members(window, &before, members)?;
        editor_ui::testing::press_key(window, "escape", cx);
        Ok::<_, anyhow::Error>(())
    })??;
    h.settle(cx).await?;
    h.main.update(cx, |_, window, cx| {
        ensure!(
            frames(window)? == before,
            "cancelled group changed real allocations"
        );
        ensure!(!cx.has_active_drag() && !h.app.read(cx).dock_ui.is_dragging());
        ensure!(named(&snapshots(window), "local-dock-drop-indicator").is_none());
        Ok::<_, anyhow::Error>(())
    })??;
    Ok(())
}

/// Release an own-side whole group after proving its source, target and compact preview membership.
async fn commit(
    h: &Harness,
    source: usize,
    side: Placement,
    members: &[usize],
    name: &str,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    let before = h.main.update(cx, |_, window, _| frames(window))??;
    let pointer = edge(before[2], side);
    begin_drag(h, before[source], cx).await?;
    preview_drag(h, pointer, &format!("group-{name}-preview.png"), cx).await?;
    h.main.update(cx, |_, window, cx| {
        check_members(window, &before, members)?;
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
        let after = frames(window)?;
        ensure!(!cx.has_active_drag() && !h.app.read(cx).dock_ui.is_dragging());
        for pair in members.windows(2) {
            let (first, second) = (after[pair[0]], after[pair[1]]);
            if matches!(side, Placement::Left | Placement::Right) {
                ensure!((first.left() - second.left()).abs() < px(2.));
                ensure!(first.bottom() <= second.top() + px(2.));
            } else {
                ensure!((first.top() - second.top()).abs() < px(2.));
                ensure!(first.right() <= second.left() + px(2.));
            }
        }
        for member in members {
            let moved = after[*member];
            let editor = after[2];
            ensure!(match side {
                Placement::Left => moved.right() <= editor.left() + px(2.),
                Placement::Right => editor.right() <= moved.left() + px(2.),
                Placement::Top => moved.bottom() <= editor.top() + px(2.),
                Placement::Bottom => editor.bottom() <= moved.top() + px(2.),
            });
        }
        Ok::<_, anyhow::Error>(())
    })??;
    h.capture(h.main, &format!("group-{name}-placed.png"), cx)?;
    Ok(())
}

/// Assemble combinations exclusively through real title input, then verify own-side fallback in all directions.
/// The bottom case contains Explorer, Outline and the real Git plugin, so grabbing the plugin also moves the group.
pub(in super::super) async fn exercise(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    let before = h.main.update(cx, |_, window, _| frames(window))??;
    place(
        h,
        1,
        edge(before[0], Placement::Bottom),
        "group-prepare-left.png",
        cx,
    )
    .await?;
    cancel(
        h,
        0,
        Placement::Left,
        &[0, 1],
        "group-left-first-cancel.png",
        cx,
    )
    .await?;
    commit(h, 1, Placement::Left, &[0, 1], "left", cx).await?;

    let left = h.main.update(cx, |_, window, _| frames(window))??;
    place(
        h,
        0,
        edge(left[2], Placement::Right),
        "group-prepare-right-first.png",
        cx,
    )
    .await?;
    let right = h.main.update(cx, |_, window, _| frames(window))??;
    place(
        h,
        1,
        edge(right[0], Placement::Bottom),
        "group-prepare-right-second.png",
        cx,
    )
    .await?;
    cancel(
        h,
        1,
        Placement::Right,
        &[0, 1],
        "group-right-second-cancel.png",
        cx,
    )
    .await?;
    commit(h, 0, Placement::Right, &[0, 1], "right", cx).await?;

    let right = h.main.update(cx, |_, window, _| frames(window))??;
    place(
        h,
        0,
        edge(right[2], Placement::Top),
        "group-prepare-top-first.png",
        cx,
    )
    .await?;
    let top = h.main.update(cx, |_, window, _| frames(window))??;
    place(
        h,
        1,
        edge(top[0], Placement::Right),
        "group-prepare-top-second.png",
        cx,
    )
    .await?;
    cancel(
        h,
        0,
        Placement::Top,
        &[0, 1],
        "group-top-first-cancel.png",
        cx,
    )
    .await?;
    commit(h, 1, Placement::Top, &[0, 1], "top", cx).await?;

    let top = h.main.update(cx, |_, window, _| frames(window))??;
    place(
        h,
        0,
        edge(top[3], Placement::Left),
        "group-prepare-bottom-first.png",
        cx,
    )
    .await?;
    let bottom = h.main.update(cx, |_, window, _| frames(window))??;
    place(
        h,
        1,
        edge(bottom[0], Placement::Right),
        "group-prepare-bottom-second.png",
        cx,
    )
    .await?;
    for source in [0, 1] {
        cancel(
            h,
            source,
            Placement::Bottom,
            &[0, 1, 3],
            &format!("group-bottom-{source}-cancel.png"),
            cx,
        )
        .await?;
    }
    commit(h, 3, Placement::Bottom, &[0, 1, 3], "bottom-plugin", cx).await?;
    h.record(
        "four_side_group_fallback",
        serde_json::json!({
            "left": true, "right": true, "top": true, "bottom": true,
            "any_first_level_member": true, "real_plugin_member": true,
            "source_target_cards_share_members": true, "preview_keeps_real_allocations": true,
            "escape_keeps_original_layout": true, "single_window_moves_remain_available": true,
            "input": "production Window.dispatch_event on WindowsPlatform",
        }),
    );
    Ok(())
}
