//! Declared image geometry and alternative-text semantics are observed on native pixel elements.

use super::*;
use gpui_base::Root;
use gpui_kit::TestAppContext;
use gpui_kit::{gpui, px, size};
use plugin_runtime::{ImageResource, ImageState, plugin_protocol::api};
use std::sync::Arc;

/// A non-square bitmap makes ignored height constraints visible even when the HTML logo is square.
#[gpui::test]
fn explicit_image_dimensions_size_pixels_without_an_alt_caption(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::ui::typography::init(cx);
        crate::ui::theme::apply_theme(crate::ui::theme::builtin_theme(false), cx);
    });
    let bitmap = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        640,
        320,
        image::Rgba([255, 80, 20, 255]),
    ));
    let mut encoded = std::io::Cursor::new(Vec::new());
    bitmap
        .write_to(&mut encoded, image::ImageFormat::Png)
        .unwrap();
    let bytes = Arc::new(encoded.into_inner());
    let pixels = bitmap::decode(&bytes).unwrap();
    let cases = [
        ("both", Some(128.), Some(128.), 128., 128.),
        ("width", Some(128.), None, 128., 64.),
        ("height", None, Some(128.), 256., 128.),
        ("enlarge", Some(800.), Some(400.), 800., 400.),
        ("intrinsic", None, None, 640., 320.),
        ("zero", Some(128.), Some(0.), 128., 0.),
    ];
    let (_, ui) = cx.add_window_view(move |window, cx| {
        let source = api::DocumentVersion {
            id: "source".into(),
            path: "notes.md".into(),
            revision: 1,
        };
        let nodes = cases
            .iter()
            .map(|(id, width, height, _, _)| {
                let mut node = ui::Node::image(*id, "logo.png", "图片说明");
                node.layout.width = *width;
                node.layout.height = *height;
                node
            })
            .collect();
        let mut document = Document::new(ui::Node::column("images", nodes));
        document.source = Some(source.clone());
        let view = cx.new(|cx| {
            let mut view = PluginView::new(
                "geometry-provider".into(),
                document,
                Environment::default(),
                |_, _| {},
                window,
                cx,
            );
            for (id, ..) in cases {
                view.photos.insert(
                    id.into(),
                    Arc::new(images::Photo {
                        resource: Arc::new(ImageResource {
                            source: source.clone().into(),
                            uri: "logo.png".into(),
                            state: ImageState::Ready(bytes.clone()),
                        }),
                        decoded: Ok(Some(pixels.clone())),
                    }),
                );
            }
            view
        });
        Root::new(view, window, cx)
    });
    ui.simulate_resize(size(px(1400.), px(1600.)));
    ui.update(|window, cx| window.draw(cx).clear(cx));
    for (id, _, _, width, height) in cases {
        let pixels_selector = Box::leak(format!("plugin-image-pixels-{id}").into_boxed_str());
        let block_selector = Box::leak(format!("plugin-ui-{id}").into_boxed_str());
        let pixels = ui.debug_bounds(pixels_selector).unwrap();
        assert_eq!(pixels.size.width, px(width), "{id}");
        assert_eq!(pixels.size.height, px(height), "{id}");
        let block = ui.debug_bounds(block_selector).unwrap();
        assert_eq!(
            block.size.height, pixels.size.height,
            "a loaded image must not add visible alternative text: {id}"
        );
    }
}
