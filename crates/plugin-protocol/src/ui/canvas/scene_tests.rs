//! Public scene admission and negotiation are exercised without private rendering state.
use crate::ui::{Canvas, Document, Kind, Node};
use serde_json::json;

/// Tiny dash periods must be rejected before a renderer expands an otherwise bounded path.
#[test]
fn canvas_scene_rejects_unbounded_dash_expansion() {
    for dash in [json!([0.001, 0.001]), json!([1e-30, 1e-30])] {
        let mut wire = serde_json::to_value(Canvas::default()).unwrap();
        wire["scene"] = json!({"identity":1,"revision":1,"roots":[{
            "kind":"shape","id":"dashes","geometry":{"kind":"path","segments":[
                {"op":"move","x":0,"y":0},{"op":"line","x":500000,"y":0}]},
            "stroke":{"brush":{"kind":"solid","rgba":4294967295u32},"width":1,"dash":dash}}]});
        let canvas: Canvas = serde_json::from_value(wire).unwrap();
        assert!(
            Document::new(Node::new("drawing", Kind::Canvas(canvas)))
                .validate()
                .is_err(),
            "bounded encoded paths must not expand into unbounded native dash work"
        );
    }
}

/// A previously unknown guest can publish curved artwork through the ordinary UI document.
#[test]
fn canvas_scene_admits_paths_and_negotiates_its_version() {
    let mut wire = serde_json::to_value(Canvas::default()).unwrap();
    wire["scene"] = json!({
        "identity": 7, "revision": 1,
        "roots": [{"kind":"group", "id":"layer", "transform":[1,0,0,1,20,30],
            "children":[{"kind":"shape", "id":"curve", "geometry":{"kind":"path",
                "segments":[{"op":"move", "x":0,"y":0},
                    {"op":"cubic","x1":10,"y1":0,"x2":10,"y2":20,"x":20,"y":20}]},
                "stroke":{"brush":{"kind":"solid","rgba":4278190335u32},"width":2}}]}]
    });
    let drawing: Canvas = serde_json::from_value(wire).expect("public scene should decode");
    let document = Document::new(Node::new("diagram", Kind::Canvas(drawing)));
    document.validate().expect("bounded scene should publish");
    assert_eq!(
        document.required_capabilities()["ui.canvas"],
        semver::Version::new(1, 7, 0)
    );
}

/// A drawing reference cannot fabricate pixels from an absent or non-image native node.
#[test]
fn canvas_scene_rejects_unowned_image_node_references() {
    let mut wire = serde_json::to_value(Canvas::default()).unwrap();
    wire["scene"] = json!({"identity": 8,"revision": 1,
        "roots":[{"kind":"image","id":"picture","image":"missing-image",
            "rect":{"x":0,"y":0,"w":100,"h":100}}]});
    let canvas: Canvas = serde_json::from_value(wire).unwrap();
    let document = Document::new(Node::new("diagram", Kind::Canvas(canvas)));
    assert!(
        document.validate().is_err(),
        "scene must reference a declared image in its own UI document"
    );
}

/// Several individually valid canvases cannot multiply a UI document's native allocation budget.
#[test]
fn canvas_scene_document_budget_is_shared_across_canvases() {
    use crate::ui::{CanvasScene, SceneBrush, SceneGeometry, SceneNode};
    let canvases = (1..=5)
        .map(|identity| {
            let scene = CanvasScene {
                semantics: Vec::new(),
                targets: Vec::new(),
                identity,
                revision: 1,
                roots: (0..6500)
                    .map(|index| SceneNode::Shape {
                        id: format!("shape{index}"),
                        geometry: SceneGeometry::Rect {
                            rect: crate::Rect {
                                x: 0.,
                                y: 0.,
                                w: 10.,
                                h: 10.,
                            },
                            radii: [0.; 4],
                        },
                        fill: Some(SceneBrush::Solid { rgba: 0xffffffff }),
                        stroke: None,
                    })
                    .collect(),
            };
            scene
                .validate()
                .expect("each scene is independently within its budget");
            Node::new(
                format!("canvas{identity}"),
                Kind::Canvas(Canvas {
                    scene: Some(scene),
                    ..Default::default()
                }),
            )
        })
        .collect();
    let document = Document::new(Node::column("all", canvases));
    assert!(
        document.validate().is_err(),
        "whole-document scene work must remain bounded"
    );
}

/// Small local coordinates cannot hide an enormous transformed stroke allocation.
#[test]
fn canvas_scene_bounds_projected_strokes_before_native_tessellation() {
    let mut wire = serde_json::to_value(Canvas::default()).unwrap();
    wire["scene"] = json!({"identity":9,"revision":1,"roots":[
        {"kind":"group","id":"scaled","transform":[1000000,0,0,1000000,0,0],"children":[
            {"kind":"shape","id":"line","geometry":{"kind":"path","segments":[{"op":"move","x":0,"y":0},{"op":"line","x":1,"y":1}]},
                "stroke":{"brush":{"kind":"solid","rgba":0xffffffffu32},"width":2}}]}]});
    let canvas: Canvas = serde_json::from_value(wire).unwrap();
    assert!(
        canvas.validate().is_err(),
        "projected stroke must share the finite drawing extent"
    );
}

/// Native display receipts cannot acknowledge a replacement identity or a merely published revision.
#[test]
fn canvas_scene_presentation_receipts_are_exactly_addressed() {
    use crate::ui::{Action, CanvasEvent, UiEvent};
    let mut wire = serde_json::to_value(Canvas::default()).unwrap();
    wire["scene"] = json!({"identity":10,"revision":3,"roots":[]});
    let drawing: Canvas = serde_json::from_value(wire).unwrap();
    let document = Document::new(Node::new("drawing", Kind::Canvas(drawing))).revision(5);
    let receipt: CanvasEvent =
        serde_json::from_value(json!({"type":"scene_presented","identity":10,"scene_revision":3}))
            .expect("native display receipts must cross the public transport");
    document
        .validate_event(&UiEvent {
            revision: 5,
            node: "drawing".into(),
            action: Action::Canvas(receipt),
        })
        .unwrap();
    let foreign: CanvasEvent =
        serde_json::from_value(json!({"type":"scene_presented","identity":11,"scene_revision":3}))
            .unwrap();
    assert!(
        document
            .validate_event(&UiEvent {
                revision: 5,
                node: "drawing".into(),
                action: Action::Canvas(foreign)
            })
            .is_err()
    );
}

/// A small declared content size cannot hide huge scene coordinates from viewport admission.
#[test]
fn canvas_scene_viewport_bounds_the_projected_artwork() {
    let mut wire = serde_json::to_value(Canvas::default()).unwrap();
    wire["scene"] = json!({"identity":11,"revision":1,"roots":[{"kind":"shape","id":"far",
        "geometry":{"kind":"rect","rect":{"x":500000,"y":0,"w":100,"h":100}},
        "fill":{"kind":"solid","rgba":0xffffffffu32}}]});
    let drawing: Canvas = serde_json::from_value(wire).unwrap();
    let mut node = Node::new("drawing", Kind::Canvas(drawing));
    node.viewport = Some(crate::ui::VisualViewport {
        content: Some(crate::ui::ContentSize {
            width: 10.,
            height: 10.,
        }),
        transform: Some(crate::ui::ContentTransform {
            scale: 100.,
            anchor_x: 0.,
            anchor_y: 0.,
            ..Default::default()
        }),
    });
    assert!(
        Document::new(node).validate().is_err(),
        "projected scene coordinates must share the native extent budget"
    );
}

/// A display observation survives an unrelated repaint, while source replacement revokes it.
#[test]
fn canvas_scene_receipt_survives_repaint_without_crossing_source_authority() {
    use crate::ui::{Action, CanvasEvent, UiEvent};
    let mut wire = serde_json::to_value(Canvas::default()).unwrap();
    wire["scene"] = json!({"identity":12,"revision":3,"roots":[]});
    let mut document = Document::new(Node::new(
        "drawing",
        Kind::Canvas(serde_json::from_value(wire).unwrap()),
    ))
    .revision(8);
    document.source = Some(crate::api::DocumentVersion {
        id: "source".into(),
        path: "diagram.fixture".into(),
        revision: 10,
    });
    let receipt: CanvasEvent = serde_json::from_value(
        json!({"type":"scene_presented","identity":12,"scene_revision":3,"source":document.source}),
    )
    .unwrap();
    let event = UiEvent {
        revision: 7,
        node: "drawing".into(),
        action: Action::Canvas(receipt),
    };
    document
        .validate_event(&event)
        .expect("same-source drawing observations must survive UI repaint");
    document.source.as_mut().unwrap().revision = 11;
    assert!(
        document.validate_event(&event).is_err(),
        "old source cannot acknowledge the replacement source"
    );
}
