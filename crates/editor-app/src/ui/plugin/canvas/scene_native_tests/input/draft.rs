//! Observe affine native preedit geometry and changed D3D pixels through the real input endpoint.
use super::*;
use gpui_kit::EntityInputHandler;

/// Capture the initial public native image without inspecting the component's private draft.
#[derive(Default)]
pub(in crate::ui::plugin::canvas::scene_native_tests) struct Evidence {
    before: Option<image::RgbaImage>,
}

/// Keep an active native composition while the bounded complex-scene renderer catches up.
pub(in crate::ui::plugin::canvas::scene_native_tests) fn phase(
    view: &Entity<PluginView>,
    evidence: &mut Evidence,
    phase: u8,
    window: &mut Window,
    cx: &mut App,
) -> anyhow::Result<()> {
    if phase == 0 {
        let snapshot = view
            .read(cx)
            .native_canvas_input_snapshot("drawing", window, cx)
            .unwrap();
        let position = point(
            px(snapshot["origin_x"].as_f64().unwrap() as f32 + 160.),
            px(snapshot["origin_y"].as_f64().unwrap() as f32 + 35.),
        );
        window.dispatch_event(
            PlatformInput::MouseDown(MouseDownEvent {
                position,
                button: MouseButton::Left,
                modifiers: Default::default(),
                click_count: 1,
                first_mouse: false,
            }),
            cx,
        );
        window.dispatch_event(
            PlatformInput::MouseUp(MouseUpEvent {
                position,
                button: MouseButton::Left,
                modifiers: Default::default(),
                click_count: 1,
            }),
            cx,
        );
        window.draw(cx).clear(cx);
        evidence.before = Some(window.render_to_image()?);
    } else if phase == 1 {
        let input = super::super::super::targets::native_text_tests::endpoint(view, cx);
        input.update(cx, |input, cx| -> anyhow::Result<()> {
            input.replace_and_mark_text_in_range(None, "中文", Some(2..2), window, cx);
            anyhow::ensure!(
                input.marked_text_range(window, cx) == Some(0..2),
                "affine native preedit missing"
            );
            let caret = input.bounds_for_range(2..2, Bounds::default(), window, cx);
            anyhow::ensure!(
                caret.is_some(),
                "affine input must have mapped native candidate geometry"
            );
            let caret = caret.unwrap();
            let byte = input.character_index_for_point(caret.center(), window, cx);
            anyhow::ensure!(byte == Some(2), "affine native point inverse: {byte:?}");
            Ok(())
        })?;
        window.draw(cx).clear(cx);
    } else {
        window.draw(cx).clear(cx);
        let after = window.render_to_image()?;
        let before = evidence.before.as_ref().unwrap();
        let scale = window.scale_factor();
        let mut changed = 0;
        for y in (20. * scale) as u32..(140. * scale) as u32 {
            for x in (100. * scale) as u32..(200. * scale) as u32 {
                if before.get_pixel(x, y) != after.get_pixel(x, y) {
                    changed += 1;
                }
            }
        }
        anyhow::ensure!(
            changed >= 150,
            "complex scene still displays committed source instead of native preedit: {changed} changed pixels"
        );
    }
    Ok(())
}
