//! Scene publication reuses raster pixels when vector tiles only move with their native canvas.
use super::*;
use plugin_runtime::plugin_protocol::ui::{Canvas, Kind, Node};

/// Build the same opaque vector tile at a requested canvas-local vertical position.
fn scene(y: f32) -> BTreeMap<String, Arc<Document>> {
    let rect = Rect {
        x: 0.,
        y,
        w: 20.,
        h: 28.,
    };
    BTreeMap::from([("chart/view".into(), Arc::new(Document::new(Node::new("plot", Kind::Canvas(Canvas {
        paint: vec![Paint::Svg {
            rect, clip: rect,
            source: "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"20\" height=\"28\"><path fill=\"#ff0000\" d=\"M0 0H20V28H0Z\"/></svg>".into(),
        }], ..Default::default()
    })))))] )
}

/// Scrolling changes placement while source pixels and source-relative clipping stay identical.
#[test]
fn scrolling_vector_tiles_reuses_pixels_but_updates_their_placement() {
    let mut renderer = VectorRenderer::default();
    let before = renderer.prepare(&scene(0.));
    let after = renderer.prepare(&scene(-7.));
    let old = before["chart/view/canvas/plot"][0].as_ref().unwrap();
    let new = after["chart/view/canvas/plot"][0].as_ref().unwrap();
    assert_eq!(new.rect.y, -7.);
    assert!(
        Arc::ptr_eq(&old.image, &new.image),
        "scroll translation must not rasterize identical vector pixels again"
    );
    // A changed visible crop or raster scale must never reuse a formerly correct image.
    let mut clipped = scene(-7.);
    let document = Arc::make_mut(clipped.get_mut("chart/view").unwrap());
    let Kind::Canvas(canvas) = &mut document.root.kind else {
        unreachable!()
    };
    let Paint::Svg { clip, .. } = &mut canvas.paint[0] else {
        unreachable!()
    };
    clip.h = 14.;
    let cropped = renderer.prepare(&clipped);
    assert!(!Arc::ptr_eq(
        &new.image,
        &cropped["chart/view/canvas/plot"][0].as_ref().unwrap().image
    ));
}
