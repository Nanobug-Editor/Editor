//! Compare actual DirectWrite ink with the icon center through the production tab renderer.

use super::*;
use app::editor_preferences::TabPlacement;
use gpui_kit::{PlatformTextSystem, RenderGlyphParams, ShapedLine, TestDispatcher, TextRun};

/// Return the alpha-weighted device-pixel center and ink top/bottom for glyphs painted at `bounds`.
/// `native` and `line` share font IDs; `scale` follows the window's actual display scale.
fn painted_center(
    native: &dyn PlatformTextSystem,
    line: &ShapedLine,
    bounds: Bounds<Pixels>,
    scale: f32,
) -> (f64, f64, f64) {
    let baseline =
        bounds.top() + (bounds.size.height - line.ascent - line.descent) / 2. + line.ascent;
    let mut alpha_sum = 0_f64;
    let mut weighted_y = 0_f64;
    let mut ink_top = f64::INFINITY;
    let mut ink_bottom = f64::NEG_INFINITY;
    for run in &line.runs {
        for glyph in &run.glyphs {
            let x = f32::from(bounds.left() + glyph.position.x) * scale;
            let y = f32::from(baseline + glyph.position.y) * scale;
            // GPUI rounds positive device coordinates to quarters in x and whole pixels in y.
            let quantized_x = (x * 4. - 0.5).ceil() / 4.;
            let quantized_y = (y - 0.5).ceil();
            let params = RenderGlyphParams {
                font_id: run.font_id,
                glyph_id: glyph.id,
                font_size: line.font_size,
                subpixel_variant: point((quantized_x.fract() * 4.) as u8, 0),
                scale_factor: scale,
                is_emoji: false,
                subpixel_rendering: false,
                dilation: 0,
            };
            let raster = native.glyph_raster_bounds(&params).unwrap();
            if raster.size.width.0 == 0 || raster.size.height.0 == 0 {
                continue;
            }
            let (size, pixels) = native.rasterize_glyph(&params, raster).unwrap();
            let width = size.width.0 as usize;
            for (row, pixels) in pixels.chunks_exact(width).enumerate() {
                let alpha = pixels.iter().map(|alpha| *alpha as u64).sum::<u64>() as f64;
                let y = quantized_y as f64 + raster.origin.y.0 as f64 + row as f64 + 0.5;
                if alpha > 0. {
                    ink_top = ink_top.min(y - 0.5);
                    ink_bottom = ink_bottom.max(y + 0.5);
                }
                alpha_sum += alpha;
                weighted_y += alpha * y;
            }
        }
    }
    assert!(alpha_sum > 0.);
    (weighted_y / alpha_sum, ink_top, ink_bottom)
}

/// Reported, uppercase and CJK captions align their actual ink with icons across tab placements.
#[test]
fn preview_tabs_native_ink_aligns_with_icons() {
    let platform = gpui_platform::current_platform(false);
    let native = platform.text_system();
    let mut cx =
        TestAppContext::build_with_text_system(TestDispatcher::new(0), None, native.clone());
    let (fixture, ui) = mount(&mut cx, true);
    let mut failures = Vec::new();
    for name in [
        "mod.rs",
        "run_file_tabs.rs",
        "README.md",
        "中文预览文件.txt",
        "plugin_development_gyp.rs",
    ] {
        let path = fixture.workspace.root().join(name);
        std::fs::write(&path, "content").unwrap();
        ui.update(|window, cx| {
            fixture
                .app
                .update(cx, |app, cx| app.open_preview_file(path, window, cx));
        });
        for scale in [1., 1.25, 1.5] {
            ui.simulate_scale_factor_change(scale);
            for placement in [
                TabPlacement::Top,
                TabPlacement::Bottom,
                TabPlacement::Left,
                TabPlacement::Right,
                TabPlacement::ToolWindow,
            ] {
                ui.update(|window, cx| {
                    fixture.app.update(cx, |app, cx| {
                        app.apply_tab_placement(placement, window, cx);
                    });
                });
                draw(ui);
                let title = ui.debug_bounds("editor-tab-title-1").unwrap();
                let icon = ui.debug_bounds("editor-tab-file-icon-1").unwrap();
                let line = ui.update(|window, cx| {
                    window.text_system().shape_line(
                        name.into(),
                        cx.theme().font_size * 0.875,
                        &[TextRun {
                            len: name.len(),
                            font: gpui_kit::font(cx.theme().font_family.clone()),
                            color: cx.theme().primary,
                            ..Default::default()
                        }],
                        None,
                    )
                });
                let (ink, ink_top, ink_bottom) =
                    painted_center(native.as_ref(), &line, title, scale);
                let icon_center = f32::from(icon.center().y) as f64 * scale as f64;
                let difference = ink - icon_center;
                let tab = ui.debug_bounds("editor-tab-1").unwrap();
                let clip = title.intersect(&tab);
                let clip_top = (f32::from(clip.top()) * scale).floor() as f64;
                let clip_bottom = (f32::from(clip.bottom()) * scale).ceil() as f64;
                assert!(
                    ink_top >= clip_top && ink_bottom <= clip_bottom,
                    "{name}/{placement:?} at {scale}: native ink {ink_top}..{ink_bottom} exceeds clip {clip_top}..{clip_bottom}"
                );
                if placement == TabPlacement::Top && scale == 1.5 {
                    eprintln!("native caption {name}: {difference:+.2}px at 150%");
                }
                // Native glyph baselines snap to whole device pixels; allow at most one raster row.
                if difference.abs() > 1. {
                    failures.push(format!(
                        "{name}/{placement:?} at {scale}: glyph center offset {difference:+.2}px"
                    ));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
