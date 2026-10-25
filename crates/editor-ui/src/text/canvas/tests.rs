//! GPUI layout checks alignment and Unicode truncation; Windows glyphs have a separate native test.
use super::*;
use gpui_kit::{
    AppContext as _, Context, FontWeight, Render, TestAppContext, div, font, gpui, rgb,
};

mod performance;

/// A test root supplies the same Window/TextSystem interface used by the product's text layout.
struct TextHost;
impl Render for TextHost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl gpui_kit::IntoElement {
        div()
    }
}

/// Proportional/bold/Unicode glyphs anchor to the requested edge and never exceed the text region.
#[gpui::test]
fn canvas_text_regions_align_real_glyphs_and_bound_ellipsis(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::typography::init(cx);
    });
    let (_, visual) =
        cx.add_window_view(|window, cx| gpui_base::Root::new(cx.new(|_| TextHost), window, cx));
    visual.update(|window, _| {
        for content in [
            "iii WWW",
            "中文作者",
            "branch/very-long-reference-中文",
            "👩‍💻 e\u{301}",
        ] {
            for bold in [false, true] {
                let mut face = font("Segoe UI");
                if bold {
                    face.weight = FontWeight::BOLD;
                }
                for width in [0., 5., 40., 240.] {
                    let area = Bounds::new(point(px(10.), px(5.)), size(px(width), px(28.)));
                    for align in [TextAlign::Left, TextAlign::Center, TextAlign::Right] {
                        let layout = CanvasText::new(
                            content.into(),
                            face.clone(),
                            px(12.),
                            rgb(0).into(),
                            area,
                            align,
                            window,
                        );
                        let actual = layout.bounds();
                        assert!(actual.size.width <= area.size.width);
                        match align {
                            TextAlign::Left => assert_eq!(actual.left(), area.left()),
                            TextAlign::Center => {
                                assert!((actual.center().x - area.center().x).abs() < px(0.01))
                            }
                            TextAlign::Right => {
                                assert!((actual.right() - area.right()).abs() < px(0.01))
                            }
                        }
                        assert!(
                            layout.text().is_empty()
                                || layout.text() == content
                                || layout.text().ends_with('…')
                        );
                    }
                }
            }
        }
    });
}
