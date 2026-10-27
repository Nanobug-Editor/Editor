//! Two-axis retained scroll preserves an explicit content-to-viewport coordinate contract.
use plugin_protocol::{Rect, ui};
use serde_json::json;

/// Scroll precedes zoom, and the inverse uses the same actual native logical basis.
#[test]
fn retained_two_axis_scroll_precedes_visual_zoom_and_requires_110() {
    let scroll: ui::SceneScroll2D = serde_json::from_value(json!({
        "content":{"width":10000,"height":24000},"offset":{"x":100,"y":200},
        "buffer_rect":{"x":50,"y":150,"w":600,"h":500},"epoch":7
    }))
    .unwrap();
    let viewport: ui::VisualViewport =
        serde_json::from_value(json!({"content":{"width":400,"height":300},
        "transform":{"scale":2,"x":10,"y":20,"anchor_x":0,"anchor_y":0}}))
        .unwrap();
    let rect = scroll.project(
        Rect {
            x: 120.,
            y: 230.,
            w: 20.,
            h: 10.,
        },
        scroll.offset,
        Some(&viewport),
        ui::ContentSize {
            width: 400.,
            height: 300.,
        },
    );
    assert_eq!(
        rect,
        Rect {
            x: 50.,
            y: 80.,
            w: 40.,
            h: 20.
        }
    );
    let mut wire = serde_json::to_value(ui::Canvas::default()).unwrap();
    wire["scene"] = json!({"identity":81,"revision":1,"roots":[]});
    wire["scroll_2d"] = serde_json::to_value(scroll).unwrap();
    let document = ui::Document::new(ui::Node::new(
        "drawing",
        ui::Kind::Canvas(serde_json::from_value(wire).unwrap()),
    ));
    document.validate().unwrap();
    assert_eq!(
        document.required_capabilities().get("ui.canvas"),
        Some(&semver::Version::new(1, 10, 0))
    );
}

/// A painting-only document update cannot rewind a pending content movement or grant a new epoch.
#[test]
fn retained_two_axis_motion_crosses_only_the_same_content_epoch() {
    let mut wire = serde_json::to_value(
        ui::Document::new(ui::Node::new(
            "drawing",
            ui::Kind::Canvas(ui::Canvas::default()),
        ))
        .revision(2),
    )
    .unwrap();
    wire["root"]["kind"]["scroll_2d"] = json!({"content":{"width":1000,"height":2000},
        "offset":{"x":0,"y":0},"buffer_rect":{"x":0,"y":0,"w":600,"h":500},"epoch":7});
    wire["root"]["kind"]["scene"] = json!({"identity":81,"revision":2,"roots":[]});
    wire["root"]["viewport"] = json!({"content":{"width":400,"height":300}});
    let document: ui::Document = serde_json::from_value(wire.clone()).unwrap();
    let event = ui::UiEvent {
        revision: 1,
        node: "drawing".into(),
        action: ui::Action::ViewportInput(ui::ViewportInput {
            content: ui::ContentSize {
                width: 200.,
                height: 150.,
            },
            event: ui::CanvasEvent::Scroll2D {
                identity: 81,
                source: None,
                file: None,
                epoch: 7,
                offset: ui::SceneOffset { x: 120., y: 340. },
            },
        }),
    };
    document.validate_event(&event).unwrap();
    let mut changed = wire.clone();
    changed["root"]["kind"]["scene"]["identity"] = 82.into();
    let changed: ui::Document = serde_json::from_value(changed).unwrap();
    assert!(
        changed.validate_event(&event).is_err(),
        "same epoch cannot borrow a replacement scene"
    );
    let mut changed = wire.clone();
    changed["source"] = json!({"id":"source","path":"scene.native","revision":1});
    let changed: ui::Document = serde_json::from_value(changed).unwrap();
    assert!(
        changed.validate_event(&event).is_err(),
        "same epoch cannot borrow a replacement source"
    );
    wire["root"]["kind"]["scroll_2d"]["epoch"] = 8.into();
    let replaced: ui::Document = serde_json::from_value(wire).unwrap();
    assert!(replaced.validate_event(&event).is_err());
}
