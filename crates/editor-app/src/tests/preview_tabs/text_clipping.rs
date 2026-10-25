//! File titles must retain room for the font's ascenders and descenders inside their clipping box.

use super::*;
use app::editor_preferences::TabPlacement;

/// The real title box must contain a full font line, including underscores below the baseline.
#[gpui::test]
fn preview_tabs_titles_keep_descenders_inside_clip_bounds(cx: &mut TestAppContext) {
    let (fixture, ui) = mount(cx, true);
    let path = fixture.workspace.root().join("plugin_development_gyp.rs");
    std::fs::write(&path, "content").unwrap();
    ui.update(|window, cx| {
        fixture
            .app
            .update(cx, |app, cx| app.open_file(path, window, cx));
    });
    for (dark, scale) in [(false, 1.), (false, 1.25), (true, 1.5)] {
        ui.update(|window, cx| {
            apply_theme(builtin_theme(dark), cx);
            window.refresh();
        });
        ui.simulate_scale_factor_change(scale);
        for placement in [
            TabPlacement::Top,
            TabPlacement::Bottom,
            TabPlacement::Left,
            TabPlacement::Right,
            TabPlacement::ToolWindow,
        ] {
            ui.update(|window, cx| {
                fixture
                    .app
                    .update(cx, |app, cx| app.apply_tab_placement(placement, window, cx));
            });
            draw(ui);
            let title = ui.debug_bounds("editor-tab-title-1").unwrap();
            let natural_line_height = ui.update(|window, cx| {
                let text = window.text_system();
                let font = text.resolve_font(&gpui_kit::font(cx.theme().font_family.clone()));
                let font_size = cx.theme().font_size * 0.875;
                text.ascent(font, font_size) + text.descent(font, font_size).abs()
            });
            assert!(
                title.size.height >= natural_line_height,
                "{placement:?} at scale {scale}: title clip height {:?} cuts a font line of {:?}",
                title.size.height,
                natural_line_height
            );
            // A font line may extend through blank leading after optical adjustment. The native
            // ink regression checks actual glyph pixels against both the title and tab clips.
        }
    }
}

/// Artwork and trailing controls share one center; native ink alignment is checked separately.
#[gpui::test]
fn preview_tabs_icons_and_controls_share_vertical_center(cx: &mut TestAppContext) {
    let (fixture, ui) = mount(cx, true);
    let path = fixture.workspace.root().join("centered_preview.rs");
    std::fs::write(&path, "content").unwrap();
    ui.update(|window, cx| {
        fixture
            .app
            .update(cx, |app, cx| app.open_preview_file(path, window, cx));
    });
    for (dark, scale) in [(false, 1.), (false, 1.25), (true, 1.5)] {
        ui.update(|window, cx| {
            apply_theme(builtin_theme(dark), cx);
            window.refresh();
        });
        ui.simulate_scale_factor_change(scale);
        for placement in [
            TabPlacement::Top,
            TabPlacement::Bottom,
            TabPlacement::Left,
            TabPlacement::Right,
            TabPlacement::ToolWindow,
        ] {
            ui.update(|window, cx| {
                fixture
                    .app
                    .update(cx, |app, cx| app.apply_tab_placement(placement, window, cx));
            });
            draw(ui);
            let tab = ui.debug_bounds("editor-tab-1").unwrap();
            for selector in [
                "editor-tab-file-icon-1",
                "editor-preview-badge",
                "editor-tab-close-1",
            ] {
                let bounds = ui.debug_bounds(selector).unwrap();
                assert!(
                    (bounds.center().y - tab.center().y).abs() <= px(0.75),
                    "{selector} is not centered in {placement:?} at scale {scale}: {bounds:?} / {tab:?}"
                );
            }
            let preview = ui.debug_bounds("editor-preview-badge").unwrap();
            let close = ui.debug_bounds("editor-tab-close-1").unwrap();
            let gap = close.left() - preview.right();
            assert!(gap >= px(0.) && gap <= px(2.1), "controls gap: {gap:?}");
        }
    }
}
