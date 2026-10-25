//! Deterministic authorized image inputs enter the ordinary worker and native view together.
use super::*;

/// Attach one declared source selected by the fixture; no predecoded pixels bypass admission.
pub(super) fn attach(
    fixture: &serde_json::Value,
    document: &mut Document,
    resources: &mut BTreeMap<String, Arc<plugin_runtime::ImageResource>>,
) {
    let Some(source) = fixture["image_source"].as_str() else {
        return;
    };
    let bytes = match source {
        "noise_png" => {
            let mut seed = 1_u32;
            let pixels = image::RgbaImage::from_fn(300,300,|x,y| {
                if (100..200).contains(&x) && (100..200).contains(&y) {
                    image::Rgba([0,153,255,255])
                } else {
                    seed ^= seed << 13; seed ^= seed >> 17; seed ^= seed << 5;
                    image::Rgba([seed as u8,(seed >> 8) as u8,(seed >> 16) as u8,255])
                }
            });
            let mut output = std::io::Cursor::new(Vec::new());
            image::DynamicImage::ImageRgba8(pixels).write_to(&mut output,image::ImageFormat::Png).unwrap();
            output.into_inner()
        }
        "padded_svg" => format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"4\" height=\"4\"><!--{}--><rect width=\"4\" height=\"4\" fill=\"#0099ff\"/></svg>","x".repeat(256*1024)).into_bytes(),
        _ => panic!("unknown native image source"),
    };
    let version = protocol::api::DocumentVersion {
        id: "scene-source".into(),
        path: "scene.fixture".into(),
        revision: 1,
    };
    let mut declared = ui::Node::new(
        "pixels",
        Kind::Image {
            source: "image.fixture".into(),
            alt: "authorized scene pixels".into(),
        },
    );
    declared.layout.width = Some(0.);
    declared.layout.height = Some(0.);
    document.root = ui::Node::column("root", vec![declared, document.root.clone()]).grow();
    document.source = Some(version.clone());
    resources.insert(
        "ordinary/diagram/image/pixels".into(),
        Arc::new(plugin_runtime::ImageResource {
            source: version.into(),
            uri: "image.fixture".into(),
            state: plugin_runtime::ImageState::Ready(Arc::new(bytes)),
        }),
    );
}
