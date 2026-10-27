//! Native overlay children retain ordinary document identity, admission and capability negotiation.
use crate::ui::{self, Canvas, Document, Kind, Node};
use serde_json::json;

/// Publish a normal input bound to an existing scene target through the public wire contract.
fn document() -> Document {
    let mut wire = serde_json::to_value(Canvas::default()).unwrap();
    wire["scene"] = json!({"identity":61,"revision":1,"roots":[
        {"kind":"group","id":"node-space","transform":[1,0,0,1,20,20],"children":[]}
    ],"targets":[{"id":"node","epoch":2,"space":"node-space","focusable":true,
        "geometry":{"kind":"rect","rect":{"x":0,"y":0,"w":140,"h":90}}}]});
    wire["overlays"] = json!([{"id":"rename-box","anchor":{"identity":61,"scene_revision":1,
        "target":{"id":"node","epoch":2},"space":"node-space","rect":{"x":0,"y":0,"w":140,"h":32}},
        "rect":{"x":0,"y":40,"w":140,"h":32},
        "child":Node::input("rename",ui::Input{value:"中文 / Name".into(),..Default::default()})}]);
    let canvas: Canvas = serde_json::from_value(wire).unwrap();
    Document::new(Node::new("drawing", Kind::Canvas(canvas)))
}

/// A scene-anchored existing input participates in the same public tree and ordinary Change events.
#[test]
fn canvas_overlay_keeps_native_children_in_the_public_tree() {
    let document = document();
    document.validate().unwrap();
    assert!(
        document.active_node("rename").is_some(),
        "native child must remain in the ordinary public tree"
    );
    assert_eq!(
        document.required_capabilities()["ui.canvas"],
        semver::Version::new(1, 9, 0)
    );
    document
        .validate_event(&ui::UiEvent {
            revision: 0,
            node: "rename".into(),
            action: ui::Action::Change("新名称".into()),
        })
        .unwrap();
}

/// Moving artwork can reuse a stable ordinary native subtree through the existing delta contract.
#[test]
fn canvas_overlay_native_subtree_survives_incremental_round_trip() {
    let previous = document();
    let mut expected = previous.clone();
    expected.revision = 1;
    if let Kind::Canvas(canvas) = &mut expected.root.kind {
        canvas.scene.as_mut().unwrap().revision = 2;
    }
    let mut compact = expected.clone();
    let reused = ui::incremental::compact(&previous, &mut compact);
    assert_eq!(
        reused
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec!["rename"],
        "native children must retain ordinary delta transport rather than republishing through the scene"
    );
    ui::incremental::restore(&previous, &mut compact, &reused).unwrap();
    compact.validate().unwrap();
    assert_eq!(compact, expected);
}

/// Native widget fonts/layout must not inherit unbounded artwork zoom or projected coordinates.
#[test]
fn canvas_overlay_rejects_unbounded_native_projection() {
    for case in ["group_zoom", "viewport_zoom", "projected_box"] {
        let mut bad = document();
        if let Kind::Canvas(canvas) = &mut bad.root.kind {
            if case == "group_zoom" {
                if let ui::SceneNode::Group { transform, .. } =
                    &mut canvas.scene.as_mut().unwrap().roots[0]
                {
                    transform.0 = [1024., 0., 0., 1024., 20., 20.];
                }
            } else if case == "projected_box" {
                canvas.overlays[0].rect.x = 1_000_000.;
            }
        }
        if case == "viewport_zoom" {
            bad.root.viewport = Some(ui::VisualViewport {
                content: Some(ui::ContentSize {
                    width: 320.,
                    height: 240.,
                }),
                transform: Some(ui::ContentTransform {
                    scale: 1024.,
                    x: 0.,
                    y: 0.,
                    anchor_x: 0.,
                    anchor_y: 0.,
                }),
            });
        }
        assert!(
            bad.validate().is_err(),
            "native projection must reject {case} before layout/font allocation"
        );
    }
}

/// Scene coordinates extend the existing menu and tooltip models without changing text consumers.
#[test]
fn canvas_anchor_positions_existing_menu_and_tooltip() {
    let mut wire = serde_json::to_value(document()).unwrap();
    let declared = document();
    let Kind::Canvas(canvas) = &declared.root.kind else {
        unreachable!()
    };
    let anchor = json!({"canvas":"drawing","anchor":canvas.overlays[0].anchor});
    wire["root"]["tooltip"] = json!("重命名 / Rename");
    wire["root"]["tooltip_anchor"] = anchor.clone();
    wire["menu"] = json!({"id":"commands","x":0,"y":0,"anchor":anchor,"items":[
        {"id":"rename-command","label":"重命名 / Rename"}]});
    let decoded: Document = serde_json::from_value(wire.clone()).unwrap();
    decoded.validate().unwrap();
    let encoded = serde_json::to_value(&decoded).unwrap();
    assert_eq!(
        encoded["menu"]["anchor"], wire["menu"]["anchor"],
        "the existing menu must retain its declared scene anchor"
    );
    assert_eq!(
        encoded["root"]["tooltip_anchor"],
        wire["root"]["tooltip_anchor"]
    );
    wire["menu"]["anchor"]["anchor"]["rect"]["x"] = json!(2_000_000);
    assert!(
        serde_json::from_value::<Document>(wire)
            .unwrap()
            .validate()
            .is_err(),
        "scene popup coordinates share the bounded native anchor contract"
    );
}

/// A source editor in a native subtree must suppress the existing main mount and borrow its one session.
#[test]
fn canvas_foreground_discovers_the_unique_borrowed_document_editor() {
    let mut document = document();
    let source = crate::api::DocumentVersion {
        id: "existing-document".into(),
        path: "a.txt".into(),
        revision: 1,
    };
    document.source = Some(source.clone());
    document.editor_layout = true;
    if let Kind::Canvas(canvas) = &mut document.root.kind {
        canvas.overlays[0].child = Box::new(Node::new(
            "borrowed-editor",
            Kind::Editor(ui::Editor::document(source.clone())),
        ));
    }
    document.validate().unwrap();
    assert_eq!(
        document.active_native_editor(),
        Some(&source),
        "the original main editor must not remain mounted beside its scene-anchored borrowed session"
    );
}

/// Choosing world-space placement cannot revive a native child whose owning target group is hidden.
#[test]
fn canvas_world_anchor_retires_with_the_target_group_visibility() {
    let mut document = document();
    let Kind::Canvas(canvas) = &mut document.root.kind else {
        unreachable!()
    };
    canvas.overlays[0].anchor.space = None;
    if let ui::SceneNode::Group { opacity, .. } = &mut canvas.scene.as_mut().unwrap().roots[0] {
        *opacity = 0.;
    }
    assert!(
        document.active_node("rename").is_none(),
        "world-space positioning does not detach target lifetime"
    );
    let Kind::Canvas(canvas) = &document.root.kind else {
        unreachable!()
    };
    assert!(
        !canvas.overlays[0]
            .anchor
            .placement(canvas.scene.as_ref().unwrap())
            .unwrap()
            .visible
    );
}

/// Native controls cannot silently approximate a curved group clip with its bounding rectangle.
#[test]
fn canvas_native_overlay_explicitly_rejects_unrepresentable_group_clips() {
    let mut document = document();
    let Kind::Canvas(canvas) = &mut document.root.kind else {
        unreachable!()
    };
    if let ui::SceneNode::Group { clip, .. } = &mut canvas.scene.as_mut().unwrap().roots[0] {
        *clip = Some(ui::SceneGeometry::Rect {
            rect: crate::Rect {
                x: 0.,
                y: 0.,
                w: 90.,
                h: 90.,
            },
            radii: [8.; 4],
        });
    }
    assert!(
        document.validate().is_err(),
        "native rectangle-only masks must reject rounded group clips before publication"
    );
}
