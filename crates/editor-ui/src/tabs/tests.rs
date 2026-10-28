//! Native font metrics catch clipped file-title descenders that GPUI's no-op test fonts omit.

use super::*;
use gpui_kit::{
    AppContext as _, Context, Render, TestAppContext, TestDispatcher, TextRun, font, size,
};

/// Render the same clipped file title used by the host over the production local tab control.
struct TitleHarness;

impl Render for TitleHarness {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .h(px(crate::PANEL_HEADER_HEIGHT))
            .child(
                file_tab("file".into(), "plugin_development.rs".into(), true, 0, 2)
                    .h_full()
                    .px_2()
                    .child(
                        FileTabTitle::new("file-title", "plugin_development.rs")
                            .debug_selector(|| "file-title".into())
                            .text_sm(),
                    ),
            )
            .child(comparison_tab(
                "comparison".into(),
                "working_changes_gyp.rs".into(),
                false,
                Axis::Horizontal,
                true,
                1,
                2,
                |_, _, _| {},
                |_, _| {},
                cx,
            ))
    }
}

/// DirectWrite metrics must fit the title clip at fractional display scales, without native windows.
#[test]
fn file_tab_title_clip_keeps_font_descenders() {
    // Windows headless mode uses NoopTextSystem, whose one-em metrics hide this regression.
    // Borrow DirectWrite from the native platform, then retain GPUI's deterministic test windows.
    let platform = gpui_platform::current_platform(false);
    let mut cx = TestAppContext::build_with_text_system(
        TestDispatcher::new(0),
        None,
        platform.text_system(),
    );
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::typography::init(cx);
        crate::theme::apply_theme(crate::theme::builtin_theme(false), cx);
    });
    let (_, ui) =
        cx.add_window_view(|window, cx| gpui_base::Root::new(cx.new(|_| TitleHarness), window, cx));
    for (dark, scale) in [(false, 1.), (false, 1.25), (true, 1.5)] {
        ui.update(|window, cx| {
            crate::theme::apply_theme(crate::theme::builtin_theme(dark), cx);
            window.refresh();
        });
        ui.simulate_scale_factor_change(scale);
        ui.simulate_resize(size(px(700.), px(100.)));
        ui.run_until_parked();
        ui.update(|window, cx| window.draw(cx).clear(cx));
        assert!(ui.debug_bounds("comparison-tab-icon").is_some());
        assert!(
            ui.debug_bounds("editor-preview-badge").is_none(),
            "comparison previews use the difference icon, not an extra eye badge"
        );
        assert_eq!(
            ui.debug_bounds("file-title").unwrap().size.height,
            ui.debug_bounds("comparison-tab-title-1")
                .unwrap()
                .size
                .height,
            "ordinary and comparison titles must share their text size and line box"
        );
        for (selector, content, factor) in [
            ("file-title", "plugin_development.rs", 0.875),
            ("comparison-tab-title-1", "working_changes_gyp.rs", 0.875),
        ] {
            let title = ui.debug_bounds(selector).unwrap();
            let natural_height = ui.update(|window, cx| {
                let font_size = cx.theme().font_size * factor;
                let line = window.text_system().shape_line(
                    content.into(),
                    font_size,
                    &[TextRun {
                        len: content.len(),
                        font: font(cx.theme().font_family.clone()),
                        color: cx.theme().foreground,
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    }],
                    None,
                );
                line.ascent + line.descent
            });
            assert!(
                title.size.height >= natural_height,
                "{selector} at scale {scale}: title clip height {:?} cuts a font line of {:?}",
                title.size.height,
                natural_height
            );
        }
    }
}
