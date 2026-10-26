//! The dock title must not add a second stroke below the window caption separator.

use super::*;

/// Inspect clipped paint at the title's top edge and its actual one-pixel bottom separator.
#[gpui::test]
fn host_messages_header_has_one_caption_separator(cx: &mut TestAppContext) {
    with_messages(cx, |form, _, _| {
        for (dark, scale) in [(false, 1.), (true, 1.5), (false, 2.)] {
            form.update(|window, cx| {
                apply_theme(builtin_theme(dark), cx);
                window.refresh();
            });
            form.simulate_scale_factor_change(scale);
            draw(form);
            let caption = form.debug_bounds("custom-title-bar").unwrap();
            let content = form.debug_bounds("host-messages-panel").unwrap();
            form.update(|window, cx| {
                let scale = window.scale_factor();
                let x = f32::from(content.center().x) * scale;
                let top = point(
                    gpui_kit::ScaledPixels(x),
                    gpui_kit::ScaledPixels(f32::from(caption.bottom()) * scale + 0.25),
                );
                let bottom = point(
                    gpui_kit::ScaledPixels(x),
                    gpui_kit::ScaledPixels((f32::from(content.top()) - 0.5) * scale),
                );
                let quads = window.painted_quads();
                assert!(
                    !quads.iter().any(|quad| {
                        quad.bounds.contains(&top)
                            && quad.content_mask.bounds.contains(&top)
                            && quad.border_color.a > 0.
                            && quad.border_widths.bottom.0 > 0.
                    }),
                    "bottom-only title border must not paint an antialiased top edge"
                );
                let style = component_styles(cx, ThemeComponent::DockTitleBar).base;
                let color = style.border.unwrap_or(cx.theme().border);
                // GPUI snaps both ends: one logical pixel can cover one or two device pixels at 150%.
                let bottom_edge = f32::from(content.top()) * scale;
                let height = bottom_edge.round() - (bottom_edge - scale).round();
                assert!(
                    quads.iter().any(|quad| {
                        quad.bounds.contains(&bottom)
                            && quad.content_mask.bounds.contains(&bottom)
                            && (quad.bounds.size.height.0 - height).abs() < 0.01
                            && quad.background == color.into()
                    }),
                    "dock title must paint a single logical-pixel separator at scale {scale}"
                );
            });
        }
    });
}
