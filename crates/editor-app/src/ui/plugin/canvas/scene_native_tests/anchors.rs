//! Native menu placement follows the actually displayed Canvas and retires with its target.
use super::*;

/// Hover the declared scene rectangle, then move it to the edge and retire the target epoch.
pub(super) fn tooltip_phase(
    view: &Entity<PluginView>,
    phase: u8,
    window: &mut Window,
    cx: &mut App,
) -> anyhow::Result<()> {
    window.draw(cx).clear(cx);
    if phase == 1 || phase == 4 {
        let tooltip = gpui_base::test_support::snapshots(window)
            .into_iter()
            .find(|node| node.visible() && node.label() == Some("重命名 / Rename"))
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "the existing native tooltip must appear over its scene anchor, phase {phase}"
                )
            })?;
        anyhow::ensure!(
            tooltip.bounds().origin.x >= px(0.)
                && tooltip.bounds().right() <= window.viewport_size().width
                && tooltip.bounds().origin.y >= px(0.)
                && tooltip.bounds().bottom() <= window.viewport_size().height,
            "anchored tooltip must clamp inside the native window: {:?}",
            tooltip.bounds()
        );
    } else if phase == 5 {
        super::super::targets::native_text_tests::redraw(
            view,
            |document| {
                if let Kind::Canvas(canvas) = &mut document.root.kind {
                    canvas.scene.as_mut().unwrap().targets[0].epoch = 3;
                }
            },
            window,
            cx,
        )?;
    } else if phase == 6 {
        anyhow::ensure!(
            !gpui_base::test_support::snapshots(window)
                .iter()
                .any(|node| node.visible() && node.label() == Some("重命名 / Rename")),
            "retired tooltip anchor must disappear"
        );
    } else {
        if phase == 2 {
            super::super::targets::native_text_tests::redraw(
                view,
                |document| {
                    if let Kind::Canvas(canvas) = &mut document.root.kind {
                        if let ui::SceneNode::Group { transform, .. } =
                            &mut canvas.scene.as_mut().unwrap().roots[0]
                        {
                            transform.0[4] = 280.;
                        }
                    }
                },
                window,
                cx,
            )?;
            return Ok(());
        }
        if phase == 3 {
            anyhow::ensure!(
                !gpui_base::test_support::snapshots(window)
                    .iter()
                    .any(|node| node.visible() && node.label() == Some("重命名 / Rename")),
                "moving the anchor away must close its old hover popup"
            );
        }
        let snapshot = view
            .read(cx)
            .native_canvas_input_snapshot("drawing", window, cx)
            .unwrap();
        window.dispatch_event(
            gpui_kit::PlatformInput::MouseMove(MouseMoveEvent {
                position: point(
                    px(snapshot["origin_x"].as_f64().unwrap() as f32
                        + if phase == 0 { 30. } else { 290. }),
                    px(snapshot["origin_y"].as_f64().unwrap() as f32 + 25.),
                ),
                pressed_button: None,
                modifiers: Default::default(),
            }),
            cx,
        );
    }
    Ok(())
}

/// Publish an ordinary existing menu, then move its scene space and withdraw its owning target.
pub(super) fn phase(
    view: &Entity<PluginView>,
    phase: u8,
    modal: bool,
    window: &mut Window,
    cx: &mut App,
) -> anyhow::Result<()> {
    window.draw(cx).clear(cx);
    if phase == 0 {
        super::super::targets::native_text_tests::redraw(
            view,
            |document| {
                document.menu=Some(serde_json::from_value(serde_json::json!({"id":"commands","x":0,"y":0,
                "anchor":{"canvas":"drawing","anchor":{"identity":61,"scene_revision":1,
                    "target":{"id":"node","epoch":2},"space":"node-space","rect":{"x":0,"y":0,"w":20,"h":10}}},
                "items":[{"id":"rename-command","label":"Scene anchored command / 场景菜单"}]})).unwrap());
            },
            window,
            cx,
        )?;
    } else if phase == 1 || phase == 3 {
        let row = gpui_base::test_support::snapshots(window)
            .into_iter()
            .find(|node| {
                node.visible()
                    && node
                        .label()
                        .is_some_and(|text| text.contains("Scene anchored command"))
            })
            .ok_or_else(|| {
                anyhow::anyhow!("scene menu must use the existing visible native rows")
            })?;
        // The menu uses 5 px card padding inside a 1 px border. At the right edge its
        // ordinary local widget clamps the 230 px card to the 320 px window.
        let expected = if phase == 1 { 26. } else { 88. };
        anyhow::ensure!(
            (row.bounds().origin.x / px(1.) - expected).abs() < 3.,
            "native menu must follow scene placement and clamp the window edge: {:?}",
            row.bounds()
        );
    } else if phase == 2 {
        super::super::targets::native_text_tests::redraw(
            view,
            |document| {
                if let Kind::Canvas(canvas) = &mut document.root.kind {
                    if let ui::SceneNode::Group { transform, .. } =
                        &mut canvas.scene.as_mut().unwrap().roots[0]
                    {
                        transform.0[4] = 280.;
                    }
                }
            },
            window,
            cx,
        )?;
    } else {
        super::super::targets::native_text_tests::redraw(
            view,
            |document| {
                if modal {
                    let mut content = std::mem::replace(
                        &mut document.root,
                        ui::Node::text("under-modal", "Modal / 模态"),
                    );
                    if let Kind::Canvas(canvas) = &mut content.kind {
                        canvas.scene.as_mut().unwrap().identity = 62;
                    }
                    document
                        .menu
                        .as_mut()
                        .unwrap()
                        .anchor
                        .as_mut()
                        .unwrap()
                        .anchor
                        .identity = 62;
                    document.dialog = Some(ui::Dialog::new("scene-modal", "Modal / 模态", content));
                } else if let Kind::Canvas(canvas) = &mut document.root.kind {
                    canvas.scene.as_mut().unwrap().targets.clear();
                }
            },
            window,
            cx,
        )?;
        window.draw(cx).clear(cx);
        if modal {
            anyhow::ensure!(
                view.read(cx).popup.is_none(),
                "modal takeover must close the original native popup before waiting for new anchor geometry"
            );
        }
        anyhow::ensure!(
            !gpui_base::test_support::snapshots(window)
                .iter()
                .any(|node| node.visible()
                    && node
                        .label()
                        .is_some_and(|text| text.contains("Scene anchored command"))),
            "target retirement must close the native anchored popup"
        );
    }
    Ok(())
}
