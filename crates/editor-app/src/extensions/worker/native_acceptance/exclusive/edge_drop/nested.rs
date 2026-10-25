//! Actual D3D windows verify a regional row, its nested column and a child-only editor drop.
use super::*;

/// Resolve the four visible dock allocations without assuming Explorer remains beside Editor.
/// Editor is the sole titleless dock leaf; the remaining titled leaf is the actual Git viewer.
pub(super) fn frames(window: &Window) -> anyhow::Result<[Bounds<Pixels>; 4]> {
    let explorer = host_frame(window, "explorer-title")?;
    let outline = host_frame(window, "outline-hide")?;
    let nodes = snapshots(window);
    let groups = nodes.iter().filter(|node| node.visible()
        && matches!(node.path().last(), Some(ElementId::Name(id)) if id.as_ref() == "tab-group")).collect::<Vec<_>>();
    let editor = groups
        .iter()
        .find(|group| {
            !nodes.iter().any(|node| {
                node.visible()
        && node.path().starts_with(group.path())
        && node.path().iter().skip(group.path().len()).any(|part|
            matches!(part, ElementId::Name(id) if id.as_ref() == "local-dock-title-bar"))
            })
        })
        .context("titleless Editor allocation missing")?
        .bounds();
    let remaining = groups
        .iter()
        .filter(|group| ![explorer, outline, editor].contains(&group.bounds()))
        .collect::<Vec<_>>();
    ensure!(
        remaining.len() == 1,
        "expected the actual Git viewer after the three host leaves"
    );
    Ok([explorer, outline, editor, remaining[0].bounds()])
}

/// Preview then release one real title gesture, checking that all actual views stay put until release.
/// This helper targets free editor edges or concrete windows, requiring one source, target and card.
pub(super) async fn place(
    h: &Harness,
    source: usize,
    pointer: Point<Pixels>,
    name: &str,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    let before = h.main.update(cx, |_, window, _| frames(window))??;
    begin_drag(h, before[source], cx).await?;
    preview_drag(h, pointer, name, cx).await?;
    h.main.update(cx, |_, window, cx| {
        ensure!(
            frames(window)? == before,
            "nested preview changed actual allocations"
        );
        let nodes = snapshots(window);
        for name in [
            "local-dock-source-1",
            "local-dock-target-1",
            "local-dock-drag-card-1",
        ] {
            ensure!(
                named(&nodes, name).is_none(),
                "a local move included an unrelated member: {name}"
            );
        }
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
    h.settle(cx).await
}

/// Build the user's illustrated nesting through production input, then move only Window1 to Editor.
/// Run after the exclusive reopen/close checks because this intentionally changes the isolated layout.
pub(in super::super) async fn exercise(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    let before = h.main.update(cx, |_, window, _| frames(window))??;
    ensure!(
        before[3].size.width == before[2].size.width,
        "native fixture must exercise equality"
    );
    begin_drag(h, before[3], cx).await?;
    let equal = point(before[2].right() - px(8.), before[2].center().y);
    preview_drag(h, equal, "equal-dimension-preview.png", cx).await?;
    h.main.update(cx, |_, window, cx| {
        let nodes = snapshots(window);
        let target = nodes.iter().find(|node| node.visible()
            && matches!(node.path().last(), Some(ElementId::Name(id)) if id.as_ref() == "local-dock-drop-indicator"))
            .context("equal-dimension marker missing")?.bounds();
        ensure!((target.size.width - before[2].size.width / 2.).abs() < px(1.));
        editor_ui::testing::press_key(window, "escape", cx);
        Ok::<_, anyhow::Error>(())
    })??;
    h.settle(cx).await?;

    place(
        h,
        0,
        point(before[3].left() + px(8.), before[3].center().y),
        "nested-bottom-row-preview.png",
        cx,
    )
    .await?;
    let row = h.main.update(cx, |_, window, _| frames(window))??;
    ensure!(
        row[0].right() <= row[3].left() + px(2.) && (row[0].top() - row[3].top()).abs() < px(2.)
    );
    place(
        h,
        1,
        point(row[0].center().x, row[0].bottom() - px(8.)),
        "nested-cell-preview.png",
        cx,
    )
    .await?;
    let nested = h.main.update(cx, |_, window, _| frames(window))??;
    ensure!(
        (nested[0].left() - nested[1].left()).abs() < px(2.)
            && nested[0].bottom() <= nested[1].top() + px(2.)
    );
    ensure!(nested[0].right() <= nested[3].left() + px(2.));
    h.capture(h.main, "nested-cell-placed.png", cx)?;

    place(
        h,
        0,
        point(nested[2].left() + px(8.), nested[2].center().y),
        "nested-child-editor-preview.png",
        cx,
    )
    .await?;
    let after = h.main.update(cx, |_, window, _| frames(window))??;
    ensure!(after[3] == nested[3], "unrelated bottom Git cell changed");
    ensure!(
        (after[1].top() - nested[0].top()).abs() < px(2.)
            && (after[1].bottom() - nested[1].bottom()).abs() < px(2.)
    );
    ensure!(
        (after[0].size.width - nested[0].size.width).abs() < px(2.)
            && after[0].right() <= after[2].left() + px(2.)
    );
    h.capture(h.main, "nested-child-editor-placed.png", cx)?;

    // Direct bottom children also remain individual at an empty editor side.
    begin_drag(h, after[1], cx).await?;
    preview_drag(
        h,
        point(after[2].center().x, after[2].top() + px(8.)),
        "nested-surviving-child-preview.png",
        cx,
    )
    .await?;
    h.main.update(cx, |_, window, cx| {
        let nodes = snapshots(window);
        for name in [
            "local-dock-source-1",
            "local-dock-target-1",
            "local-dock-drag-card-1",
        ] {
            ensure!(
                named(&nodes, name).is_none(),
                "a first-level child expanded into its remaining combination"
            );
        }
        ensure!(frames(window)? == after);
        editor_ui::testing::press_key(window, "escape", cx);
        Ok::<_, anyhow::Error>(())
    })??;
    h.settle(cx).await?;
    h.record("nested_dock_placement", serde_json::json!({
        "equal_dimension_uses_half": true, "regional_row_kept": true, "cell_accepts_vertical_layout": true,
        "nested_child_has_one_source_target_and_card": true, "real_layout_unchanged_before_release": true,
        "sibling_reclaims_original_cell": true, "unrelated_git_cell_unchanged": true,
        "first_level_child_also_moves_alone": true, "input": "production Window.dispatch_event on WindowsPlatform",
    }));
    Ok(())
}
