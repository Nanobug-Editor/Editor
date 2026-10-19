//! Generic native presentation tests observe glyph geometry and user overrides without a Markdown ID.

use super::*;
use gpui_base::Root;
use gpui_kit::TestAppContext;
use gpui_kit::{gpui, px};
use plugin_runtime::plugin_protocol::{FontStyle, ui};

/// A guest-supplied body/heading/code scale reaches Base glyphs in both themes; user role fonts win.
#[gpui::test]
fn content_styles_control_native_heading_code_and_user_override_geometry(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::ui::typography::init(cx);
        crate::ui::theme::apply_theme(crate::ui::theme::builtin_theme(false), cx);
    });
    let (_, ui) = cx.add_window_view(|window, cx| {
        let mut document = Document::new(ui::Node::column(
            "content",
            vec![
                ui::Node::rich_text("body", "<p>README</p>").role("prose"),
                ui::Node::rich_text("title", "<h1>README</h1>").role("prose"),
                ui::Node::code_block("code", "let x = 1;", None).role("prose"),
                ui::Node::rich_text("custom", "<p>README</p>").role("custom"),
            ],
        ));
        let preset = ui::ContentStyle {
            font: FontStyle {
                size_px: Some(16.),
                ..Default::default()
            },
            line_height: Some(1.5),
            headings: (0..6)
                .map(|_| ui::HeadingStyle {
                    scale: 2.,
                    weight: 600,
                    line_height: 1.25,
                    padding_bottom: 0.3,
                    border_bottom: 1.,
                    muted: false,
                })
                .collect(),
            code: Some(ui::CodeStyle {
                scale: 0.85,
                line_height: 1.45,
                padding: 16.,
                radius: 6.,
                border: 0.,
            }),
            ..Default::default()
        };
        document
            .content_styles
            .insert("prose".into(), preset.clone());
        document.content_styles.insert("custom".into(), preset);
        let mut environment = Environment::default();
        environment.theme_text_styles.insert(
            "document-provider.custom".into(),
            FontStyle {
                size_px: Some(24.),
                ..Default::default()
            },
        );
        let view = cx.new(|cx| {
            PluginView::new(
                "document-provider".into(),
                document,
                environment,
                |_, _| {},
                window,
                cx,
            )
        });
        Root::new(view, window, cx)
    });
    for dark in [false, true] {
        ui.update(|window, cx| {
            crate::ui::theme::apply_theme(crate::ui::theme::builtin_theme(dark), cx);
            window.draw(cx).clear(cx);
        });
        for (id, expected) in [
            ("body", 24.),
            ("title", 50.6),
            ("code", 51.72),
            ("custom", 36.),
        ] {
            let selector = Box::leak(format!("plugin-ui-{id}").into_boxed_str());
            let bounds = ui.debug_bounds(selector).unwrap();
            assert!(
                (bounds.size.height - px(expected)).abs() < px(1.),
                "{id}: expected {expected} px in dark={dark}, got {bounds:?}"
            );
        }
    }
}
