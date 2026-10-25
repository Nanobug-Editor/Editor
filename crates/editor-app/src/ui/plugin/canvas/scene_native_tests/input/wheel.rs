//! A publication may update zoom before the previously displayed frame receives its next wheel.
use super::*;

/// Publish a new viewport through the normal UI adapter, then dispatch against the undrawn old frame.
pub(in crate::ui::plugin::canvas::scene_native_tests) fn run(
    view: &Entity<PluginView>,
    window: &mut Window,
    cx: &mut App,
) -> anyhow::Result<()> {
    let snapshot = view
        .read(cx)
        .native_canvas_input_snapshot("drawing", window, cx)
        .unwrap();
    let (mut document, environment) = {
        let current = view.read(cx);
        (current.document.clone(), current.environment.clone())
    };
    document.revision += 1;
    document.root.viewport = Some(ui::VisualViewport {
        content: Some(ui::ContentSize {
            width: 320.,
            height: 240.,
        }),
        transform: Some(ui::ContentTransform {
            scale: 2.,
            x: 0.,
            y: 0.,
            anchor_x: 0.,
            anchor_y: 0.,
        }),
    });
    document.validate().map_err(anyhow::Error::msg)?;
    view.update(cx, |view, cx| {
        view.update_document(document, environment, window, cx)
    });
    window.dispatch_event(
        PlatformInput::ScrollWheel(ScrollWheelEvent {
            position: point(
                px(snapshot["origin_x"].as_f64().unwrap() as f32 + 30.),
                px(snapshot["origin_y"].as_f64().unwrap() as f32 + 30.),
            ),
            delta: gpui_kit::ScrollDelta::Pixels(point(px(0.5), px(-2.25))),
            ..Default::default()
        }),
        cx,
    );
    Ok(())
}

/// Source position, deltas and projection must all describe the same original shown frame.
pub(in crate::ui::plugin::canvas::scene_native_tests) fn verify(
    events: &[ui::UiEvent],
) -> anyhow::Result<()> {
    let wheels = events
        .iter()
        .filter_map(|event| match &event.action {
            ui::Action::Canvas(CanvasEvent::Input { input }) => match &input.kind {
                protocol::ui::CanvasInputKind::Wheel(wheel) => Some((event, input, wheel)),
                _ => None,
            },
            _ => None,
        })
        .collect::<Vec<_>>();
    anyhow::ensure!(
        wheels.len() == 1,
        "one original native wheel required: {wheels:?}"
    );
    let (event, input, wheel) = wheels[0];
    anyhow::ensure!(
        event.revision == 1
            && input.projection.is_none()
            && wheel.x == 30.
            && wheel.y == 30.
            && wheel.delta_x == 0.5
            && wheel.delta_y == -2.25,
        "native wheel must retain original shown projection and delta: {event:?}"
    );
    Ok(())
}
