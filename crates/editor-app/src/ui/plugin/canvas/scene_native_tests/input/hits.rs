//! Native pixels and native pointer dispatch must agree on independently clamped corner geometry.
use super::*;

/// Probe the visible left-middle of an oversized corner on both a target and an ancestor clip.
pub(in crate::ui::plugin::canvas::scene_native_tests) fn run(
    view: &Entity<PluginView>,
    window: &mut Window,
    cx: &mut App,
) -> anyhow::Result<()> {
    let snapshot = view
        .read(cx)
        .native_canvas_input_snapshot("drawing", window, cx)
        .unwrap();
    let origin = point(
        px(snapshot["origin_x"].as_f64().unwrap() as f32),
        px(snapshot["origin_y"].as_f64().unwrap() as f32),
    );
    let image = window.render_to_image()?;
    let scale = window.scale_factor();
    for x in [30., 170.] {
        let position = origin + point(px(x), px(70.));
        let pixel = image
            .get_pixel(
                ((position.x / px(1.)) * scale) as u32,
                ((position.y / px(1.)) * scale) as u32,
            )
            .0;
        anyhow::ensure!(
            pixel[0] < 50 && pixel[1] > 120 && pixel[2] < 50,
            "native rounded target probe must be painted green: {pixel:?}"
        );
        window.dispatch_event(
            PlatformInput::MouseDown(MouseDownEvent {
                position,
                button: MouseButton::Left,
                click_count: 1,
                modifiers: Default::default(),
                first_mouse: false,
            }),
            cx,
        );
        window.dispatch_event(
            PlatformInput::MouseUp(MouseUpEvent {
                position,
                button: MouseButton::Left,
                click_count: 1,
                modifiers: Default::default(),
            }),
            cx,
        );
    }
    Ok(())
}
/// Observe only the public target envelopes produced by those genuine native dispatches.
pub(in crate::ui::plugin::canvas::scene_native_tests) fn verify(
    events: &[ui::UiEvent],
) -> anyhow::Result<()> {
    let targets=events.iter().filter_map(|event|match &event.action {
        ui::Action::Canvas(CanvasEvent::Input{input}) if matches!(&input.kind,protocol::ui::CanvasInputKind::Pointer(pointer) if pointer.phase==protocol::ui::CanvasPointerPhase::Down)=>Some(input.target.as_str()),_=>None,
    }).collect::<Vec<_>>();
    anyhow::ensure!(
        targets == ["round", "clipped"],
        "native pixels and rounded hit/clip geometry must agree: {targets:?}"
    );
    Ok(())
}
