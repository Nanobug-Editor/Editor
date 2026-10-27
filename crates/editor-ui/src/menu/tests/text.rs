//! Real Windows font metrics detect clipped menu text that headless one-em fonts cannot expose.
use super::*;
use gpui_kit::{TestDispatcher, TextRun, font, size};

/// The actual truncated template labels must retain Latin descenders and Chinese glyphs at display scales.
#[test]
fn popup_menu_icon_labels_keep_native_font_descenders() {
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
    let samples = [("debug", "debug_gyp_"), ("chinese", "插件调试 gyp_")];
    let (_, ui) = cx.add_window_view(move |window, cx| {
        let items = samples
            .iter()
            .map(|(id, label)| MenuItem {
                id: (*id).into(),
                label: (*label).into(),
                disabled: false,
                separator_before: false,
            })
            .collect();
        let content = samples
            .iter()
            .map(|(id, _)| {
                (
                    (*id).into(),
                    MenuRowContent::Icon {
                        icon: crate::Icon::new(gpui_kit::assets::IconName::Close),
                        tooltip: "Template".into(),
                    },
                )
            })
            .collect();
        let view = cx.new(|cx| {
            PopupMenu::new(
                items,
                MenuStyle::current(cx),
                point(px(20.), px(20.)),
                |_, _, _| {},
                window,
                cx,
            )
            .row_content(content)
        });
        Root::new(view, window, cx)
    });
    for scale in [1., 1.25, 1.5, 2.] {
        ui.simulate_scale_factor_change(scale);
        ui.simulate_resize(size(px(500.), px(300.)));
        ui.run_until_parked();
        ui.update(|window, cx| window.draw(cx).clear(cx));
        for (selector, label) in [
            ("native-menu-label-debug", samples[0].1),
            ("native-menu-label-chinese", samples[1].1),
        ] {
            let clip = ui.debug_bounds(selector).expect("painted template label");
            let natural = ui.update(|window, cx| {
                let line = window.text_system().shape_line(
                    label.into(),
                    px(MenuStyle::current(cx).font_size),
                    &[TextRun {
                        len: label.len(),
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
                clip.size.height >= natural,
                "{selector} at {scale}x: menu text clip {:?} cuts native font line {:?}",
                clip.size.height,
                natural
            );
        }
    }
}
