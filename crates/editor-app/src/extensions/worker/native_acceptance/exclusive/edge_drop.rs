//! Native editor-edge drops split the upper row while the actual Git viewer spans its full bottom.
use super::*;
use gpui_kit::{MouseMoveEvent, Point};

pub(super) mod grouped;
pub(super) mod nested;
mod one_side;

/// Locate the painted outer allocation around the host title control `id` in `window`.
fn host_frame(window: &Window, id: &str) -> anyhow::Result<Bounds<Pixels>> {
    let nodes = snapshots(window);
    let anchor = named(&nodes, id).with_context(|| format!("host title {id} missing"))?;
    nodes
        .iter()
        .filter(|node| {
            node.visible()
                && matches!(node.path().last(), Some(ElementId::Name(id)) if id.as_ref() == "tab-group")
                && anchor.path().starts_with(node.path())
        })
        .max_by_key(|node| node.path().len())
        .map(|node| node.bounds())
        .context("native host panel frame missing")
}

/// Sample Explorer, Outline and the painted Editor next to them; the right dock is sampled separately.
fn geometry(window: &Window) -> anyhow::Result<[Bounds<Pixels>; 3]> {
    let explorer = host_frame(window, "explorer-title")?;
    let outline = if named(&snapshots(window), "outline-hide").is_some() {
        host_frame(window, "outline-hide")?
    } else {
        Bounds::new(explorer.origin, size(px(0.), px(0.)))
    };
    let editor = snapshots(window)
        .into_iter()
        .filter(|node| {
            let bounds = node.bounds();
            node.visible()
                && matches!(node.path().last(), Some(ElementId::Name(id)) if id.as_ref() == "tab-group")
                && bounds.left() >= explorer.right().max(outline.right()) - px(2.)
                && (bounds.top() - explorer.top().min(outline.top())).abs() < px(2.)
        })
        .min_by(|a, b| a.bounds().left().partial_cmp(&b.bounds().left()).unwrap())
        .context("native Editor frame missing beside the sidebar")?
        .bounds();
    Ok([explorer, outline, editor])
}

/// Locate the actual bottom viewer below Editor, rather than assuming its width from a saved preference.
fn bottom_frame(window: &Window, editor: Bounds<Pixels>) -> anyhow::Result<Bounds<Pixels>> {
    let nodes = snapshots(window);
    let hosts = [
        named(&nodes, "explorer-title"),
        named(&nodes, "outline-hide"),
    ];
    // A left child can align with Editor's bottom after returning to its group. Position alone
    // then mistakes Outline for Git, so exclude actual host-title ancestry before testing geometry.
    nodes.iter().filter(|node| {
        node.visible()
            && matches!(node.path().last(), Some(ElementId::Name(id)) if id.as_ref() == "tab-group")
            && !hosts.iter().flatten().any(|host| host.path().starts_with(node.path()))
            && node.bounds().top() >= editor.bottom() - px(2.)
            && node.bounds().size.height > px(80.)
    }).min_by(|a, b| a.bounds().top().partial_cmp(&b.bounds().top()).unwrap())
        .map(|node| node.bounds()).context("actual bottom Git viewer missing")
}

/// Assert the user's complete row/column relationship from native paint rectangles.
fn check_row(frames: [Bounds<Pixels>; 3], bottom: Bounds<Pixels>) -> anyhow::Result<()> {
    let [explorer, _, editor] = frames;
    ensure!(
        (explorer.bottom() - editor.bottom()).abs() < px(2.),
        "Explorer escaped Editor's row: {frames:?}, bottom={bottom:?}"
    );
    ensure!(
        (bottom.left() - explorer.left()).abs() < px(2.)
            && (bottom.right() - editor.right()).abs() < px(2.),
        "Git must span both upper columns: {frames:?}, bottom={bottom:?}"
    );
    Ok(())
}

/// Start a title gesture on `source` through production pointer dispatch and await its active payload.
pub(super) async fn begin_drag(
    h: &Harness,
    source: Bounds<Pixels>,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    let start = point(
        source.center().x,
        source.top() + px(PANEL_HEADER_HEIGHT / 2.),
    );
    h.main.update(cx, |_, window, cx| {
        let viewport = Bounds::new(point(px(0.), px(0.)), window.viewport_size());
        ensure!(viewport.contains(&start));
        window.activate_window();
        window.dispatch_event(
            PlatformInput::MouseDown(MouseDownEvent {
                position: start,
                button: MouseButton::Left,
                modifiers: Default::default(),
                click_count: 1,
                first_mouse: false,
            }),
            cx,
        );
        window.dispatch_event(
            PlatformInput::MouseMove(MouseMoveEvent {
                position: start + point(px(12.), px(0.)),
                pressed_button: Some(MouseButton::Left),
                modifiers: Default::default(),
            }),
            cx,
        );
        Ok::<_, anyhow::Error>(())
    })??;
    h.until(
        h.main,
        "host title starts Base drag",
        |_, cx| Ok(cx.has_active_drag()),
        cx,
    )
    .await
}

/// Sample injected input and its D3D draw together after the platform frame has drained native events.
/// This keeps unrelated physical mouse motion from replacing the intended test coordinate before assertion.
pub(super) fn sample_drag(
    h: &Harness,
    pointer: Point<Pixels>,
    name: &str,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    h.main.update(cx, |_, window, cx| {
        let observed = window.mouse_position();
        window.dispatch_event(PlatformInput::MouseMove(MouseMoveEvent {
            position: pointer, pressed_button: Some(MouseButton::Left), modifiers: Default::default(),
        }), cx);
        window.draw(cx).clear(cx);
        h.record(&format!("drag_pointer_sample_{name}"), serde_json::json!({
            "requested": format!("{pointer:?}"), "after_platform_frame": format!("{observed:?}"),
            "platform_changed_pointer": observed != pointer,
            "sampled_input": "production Window.dispatch_event followed by native draw in the same update",
        }));
    })?;
    Ok(())
}

/// Hover a target and inspect compact cards plus paint-only source and destination markers.
async fn preview_drag(
    h: &Harness,
    destination: Point<Pixels>,
    preview: &str,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
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
    h.frame(h.main, cx).await?;
    sample_drag(h, destination, preview, cx)?;
    h.main.update(cx, |_, window, _| {
        let nodes = snapshots(window);
        let exact = |name: &str| {
            nodes.iter().find(|node| node.visible()
                && matches!(node.path().last(), Some(ElementId::Name(id)) if id.as_ref() == name))
        };
        let floating = exact("local-dock-floating-preview")
            .context("compact title cards missing")?
            .bounds();
        let source = exact("local-dock-source-0")
            .context("yellow source marker missing")?
            .bounds();
        let target = exact("local-dock-drop-indicator")
            .context("blue destination marker missing")?
            .bounds();
        ensure!(
            floating.size.width < px(200.) && floating.size.height < px(100.),
            "drag cards must remain compact: {floating:?}"
        );
        ensure!(
            source.size.height > px(PANEL_HEADER_HEIGHT)
                && target.size.width > px(0.)
                && target.size.height > px(0.)
        );
        Ok::<_, anyhow::Error>(())
    })??;
    h.capture(h.main, preview, cx)?;
    Ok(())
}

/// Dispatch one complete real title drag; no DockArea edit substitutes for pointer input.
async fn drag(
    h: &Harness,
    source: Bounds<Pixels>,
    destination: Point<Pixels>,
    preview: &str,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    let before = h.main.update(cx, |_, window, _| geometry(window))??;
    let bottom = h
        .main
        .update(cx, |_, window, _| bottom_frame(window, before[2]))??;
    begin_drag(h, source, cx).await?;
    preview_drag(h, destination, preview, cx).await?;
    h.main.update(cx, |_, window, _| {
        let during = geometry(window)?;
        ensure!(
            during == before && bottom_frame(window, during[2])? == bottom,
            "hover must retain all real allocations: {before:?} -> {during:?}"
        );
        Ok::<_, anyhow::Error>(())
    })??;
    h.main.update(cx, |_, window, cx| {
        ensure!(cx.has_active_drag(), "title drag ended before release");
        window.dispatch_event(
            PlatformInput::MouseUp(MouseUpEvent {
                position: destination,
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

/// A real WASM panel keeps its content allocation while markers follow the gesture and Escape clears them.
async fn cancel_plugin_drag(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    let before = h.main.update(cx, |_, window, _| geometry(window))??;
    let bottom = h
        .main
        .update(cx, |_, window, _| bottom_frame(window, before[2]))??;
    begin_drag(h, bottom, cx).await?;
    h.main.update(cx, |_, window, cx| {
        window.dispatch_event(
            PlatformInput::MouseMove(MouseMoveEvent {
                position: point(px(-30.), px(160.)),
                pressed_button: Some(MouseButton::Left),
                modifiers: Default::default(),
            }),
            cx,
        );
    })?;
    h.frame(h.main, cx).await?;
    let during = h.main.update(cx, |_, window, _| geometry(window))??;
    ensure!(
        during == before,
        "dragging a plugin must retain neighboring allocations: {before:?} -> {during:?}"
    );
    h.main.update(cx, |_, window, _| {
        ensure!(bottom_frame(window, during[2])? == bottom);
        Ok::<_, anyhow::Error>(())
    })??;
    preview_drag(
        h,
        point(before[2].right() - px(10.), before[2].center().y),
        "plugin-source-marker-preview.png",
        cx,
    )
    .await?;
    h.main.update(cx, |_, window, cx| {
        editor_ui::testing::press_key(window, "escape", cx)
    })?;
    h.settle(cx).await?;
    h.main.update(cx, |_, window, cx| {
        ensure!(!cx.has_active_drag() && !h.app.read(cx).dock_ui.is_dragging());
        let restored = bottom_frame(window, geometry(window)?[2])?;
        ensure!(
            (restored.left() - bottom.left()).abs() < px(2.)
                && (restored.top() - bottom.top()).abs() < px(2.)
                && (restored.size.width - bottom.size.width).abs() < px(2.)
                && (restored.size.height - bottom.size.height).abs() < px(2.),
            "plugin cancellation changed its allocation: {bottom:?} -> {restored:?}"
        );
        Ok::<_, anyhow::Error>(())
    })??;
    h.capture(h.main, "plugin-source-cancelled.png", cx)?;
    h.record(
        "plugin_source_marker_drag",
        serde_json::json!({
            "source_bottom_unchanged": true, "compact_title_cards": true,
            "escape_keeps_original_allocation": true, "yellow_source_and_blue_target": true,
        }),
    );
    Ok(())
}

/// A child keeps one marker/card at free editor edges without moving siblings or saving on Escape.
async fn cancel_child_drag(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    for top in [false, true] {
        let before = h.main.update(cx, |_, window, _| geometry(window))??;
        begin_drag(h, before[1], cx).await?;
        let pointer = if top {
            point(before[2].center().x, before[2].top() + px(8.))
        } else {
            point(before[2].right() - px(10.), before[2].center().y)
        };
        preview_drag(
            h,
            pointer,
            if top {
                "child-top-preview.png"
            } else {
                "child-right-preview.png"
            },
            cx,
        )
        .await?;
        h.main.update(cx, |_, window, _| {
            let nodes = snapshots(window);
            for name in [
                "local-dock-source-1",
                "local-dock-target-1",
                "local-dock-drag-card-1",
            ] {
                ensure!(
                    named(&nodes, name).is_none(),
                    "a child title included an unrelated member: {name}"
                );
            }
            let exact = |name: &str| {
                nodes.iter().find(|node| node.visible()
                && matches!(node.path().last(), Some(ElementId::Name(id)) if id.as_ref() == name))
            };
            let first = exact("local-dock-target-0")
                .context("first member marker missing")?
                .bounds();
            if top {
                ensure!(
                    (first.left() - before[2].left()).abs() < px(2.)
                        && (first.size.width - before[2].size.width).abs() < px(2.)
                );
            } else {
                ensure!(
                    (first.top() - before[2].top()).abs() < px(2.)
                        && (first.size.height - before[2].size.height).abs() < px(2.)
                );
            }
            ensure!(
                geometry(window)? == before,
                "child preview resized the real layout"
            );
            Ok::<_, anyhow::Error>(())
        })??;
        h.main.update(cx, |_, window, cx| {
            editor_ui::testing::press_key(window, "escape", cx)
        })?;
        h.settle(cx).await?;
        h.main.update(cx, |_, window, _| {
            ensure!(geometry(window)? == before);
            ensure!(named(&snapshots(window), "local-dock-drop-indicator").is_none());
            Ok::<_, anyhow::Error>(())
        })??;
    }
    h.record(
        "child_marker_drag",
        serde_json::json!({
            "source_and_target_have_one_member": true, "one_compact_title_card": true,
            "real_layout_unchanged": true, "escape_clears_markers": true,
            "top_and_right_accept_single_child": true,
        }),
    );
    Ok(())
}

/// An unsplit region rejects a forbidden first-level fork, including after a valid preview.
async fn regional_limits(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    for side in [false, true] {
        if side {
            // An existing cell now permits nested splits. Hide the peer to exercise the root rule.
            sidebar::toggle(h, "outline-toggle", cx).await?;
            h.settle(cx).await?;
        }
        let before = h.main.update(cx, |_, window, _| geometry(window))??;
        let bottom = h
            .main
            .update(cx, |_, window, _| bottom_frame(window, before[2]))??;
        let (source, target) = if side {
            (bottom, before[0])
        } else {
            (before[0], bottom)
        };
        begin_drag(h, source, cx).await?;
        let valid = if side {
            point(target.center().x, target.top() + px(8.))
        } else {
            point(target.left() + px(8.), target.center().y)
        };
        preview_drag(
            h,
            valid,
            if side {
                "side-axis-valid.png"
            } else {
                "bottom-axis-valid.png"
            },
            cx,
        )
        .await?;
        let invalid = if side {
            point(target.left() + px(8.), target.center().y)
        } else {
            point(target.center().x, target.top() + px(8.))
        };
        h.main.update(cx, |_, window, cx| {
            window.dispatch_event(
                PlatformInput::MouseMove(MouseMoveEvent {
                    position: invalid,
                    pressed_button: Some(MouseButton::Left),
                    modifiers: Default::default(),
                }),
                cx,
            );
        })?;
        h.frame(h.main, cx).await?;
        sample_drag(
            h,
            invalid,
            if side {
                "side-forbidden"
            } else {
                "bottom-forbidden"
            },
            cx,
        )?;
        h.main.update(cx, |_, window, cx| {
            ensure!(
                named(&snapshots(window), "local-dock-drop-indicator").is_none(),
                "forbidden region edge still displays a target"
            );
            ensure!(geometry(window)? == before && bottom_frame(window, before[2])? == bottom);
            window.dispatch_event(
                PlatformInput::MouseUp(MouseUpEvent {
                    position: invalid,
                    button: MouseButton::Left,
                    modifiers: Default::default(),
                    click_count: 1,
                }),
                cx,
            );
            Ok::<_, anyhow::Error>(())
        })??;
        h.settle(cx).await?;
        h.main.update(cx, |_, window, _| {
            ensure!(geometry(window)? == before && bottom_frame(window, before[2])? == bottom);
            Ok::<_, anyhow::Error>(())
        })??;
        if side {
            sidebar::toggle(h, "outline-toggle", cx).await?;
            h.settle(cx).await?;
        }
    }
    h.record(
        "regional_split_limits",
        serde_json::json!({
            "bottom_rejects_top_edge": true, "side_rejects_left_edge": true,
            "valid_preview_cleared": true, "invalid_release_preserves_allocation": true,
        }),
    );
    Ok(())
}

/// Toggle the right dock through its footer while both explicitly separated left columns keep pixels.
async fn check_right(
    h: &Harness,
    before: [Bounds<Pixels>; 3],
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    for open in [true, false] {
        sidebar::toggle(h, "host-messages-toggle", cx).await?;
        h.settle(cx).await?;
        h.main.update(cx, |_, window, cx| {
            ensure!(h.app.read(cx).messages_visible(cx) == open);
            let after = geometry(window)?;
            check_row(after, bottom_frame(window, after[2])?)?;
            for index in [0, 1] {
                ensure!(
                    (after[index].left() - before[index].left()).abs() < px(2.)
                        && (after[index].size.width - before[index].size.width).abs() < px(2.),
                    "right open={open} resized left column: {:?} -> {:?}",
                    before[index],
                    after[index]
                );
            }
            ensure!((after[2].left() - before[2].left()).abs() < px(2.));
            if open {
                ensure!(after[2].size.width < before[2].size.width - px(100.));
            } else {
                ensure!((after[2].size.width - before[2].size.width).abs() < px(2.));
            }
            Ok::<_, anyhow::Error>(())
        })??;
        h.capture(
            h.main,
            if open {
                "editor-edge-right-open.png"
            } else {
                "editor-edge-right-closed.png"
            },
            cx,
        )?;
    }
    Ok(())
}

/// Exercise the user's failing drag, right-side reveal and drag back in the active ordinary combination.
pub(super) async fn exercise(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    cancel_plugin_drag(h, cx).await?;
    cancel_child_drag(h, cx).await?;
    one_side::exercise(h, cx).await?;
    regional_limits(h, cx).await?;
    // Match the reported complete left Explorer, upper Editor and bottom Git arrangement.
    sidebar::toggle(h, "outline-toggle", cx).await?;
    h.settle(cx).await?;
    h.until(
        h.main,
        "complete Explorer sidebar before the row drop",
        |window, cx| {
            Ok(!h.app.read(cx).session_state.outline_visible
                && named(&snapshots(window), "outline-hide").is_none())
        },
        cx,
    )
    .await?;
    let before = h.main.update(cx, |_, window, _| geometry(window))??;
    let before_bottom = h
        .main
        .update(cx, |_, window, _| bottom_frame(window, before[2]))??;
    let expected_width = before[0].size.width;
    let destination = point(before[2].left() + px(10.), before[2].center().y);
    drag(h, before[0], destination, "editor-edge-preview.png", cx).await?;
    h.until(
        h.main,
        "Explorer commits the previewed left column",
        |window, _| {
            let after = geometry(window)?;
            check_row(after, bottom_frame(window, after[2])?)?;
            Ok((after[0].size.width - expected_width).abs() < px(2.)
                && after[0].left() >= after[1].right() - px(2.)
                && (after[1].size.width - before[1].size.width).abs() < px(2.))
        },
        cx,
    )
    .await?;
    let separated = h.main.update(cx, |_, window, _| geometry(window))??;
    let separated_bottom = h
        .main
        .update(cx, |_, window, _| bottom_frame(window, separated[2]))??;
    h.capture(h.main, "editor-edge-placed.png", cx)?;
    check_right(h, separated, cx).await?;
    sidebar::toggle(h, "outline-toggle", cx).await?;
    h.settle(cx).await?;
    let rejoining = h.main.update(cx, |_, window, _| geometry(window))??;
    let destination = point(
        rejoining[1].center().x,
        rejoining[1].top() + px(PANEL_HEADER_HEIGHT + 8.),
    );
    drag(
        h,
        rejoining[0],
        destination,
        "editor-edge-return-preview.png",
        cx,
    )
    .await?;
    h.until(
        h.main,
        "Explorer rejoins Outline and releases the extra column",
        |window, _| {
            let after = geometry(window)?;
            Ok((after[0].size.width - before[0].size.width).abs() < px(2.)
                && (after[1].size.width - before[0].size.width).abs() < px(2.)
                && after[0].bottom() <= after[1].top() + px(2.)
                && (after[2].left() - before[2].left()).abs() < px(2.))
        },
        cx,
    )
    .await?;
    h.capture(h.main, "editor-edge-returned.png", cx)?;
    h.record("editor_left_drop_complete", serde_json::json!({
        "before": format!("{before:?}"), "separated": format!("{separated:?}"),
        "before_bottom":format!("{before_bottom:?}"), "separated_bottom":format!("{separated_bottom:?}"),
        "bottom_spans_both_upper_columns":true, "explorer_stays_in_editor_row":true,
        "previewed_width": expected_width / px(1.), "placed": true,
        "right_open_close_preserves_both_columns": true, "rejoin_releases_column": true,
        "input_method": "production Window.dispatch_event on WindowsPlatform", "os_sendinput": false,
    }));
    Ok(())
}
