//! Paragraph declarations use the same public document admission and version negotiation seam.
use crate::ui::{Canvas, Document, Kind, Node};
use serde_json::json;

/// A Unicode multiline label declares native paragraph layout without changing legacy text boxes.
#[test]
fn paragraph_declaration_negotiates_native_text_version() {
    let mut wire = serde_json::to_value(Canvas::default()).unwrap();
    wire["scene"] = json!({"identity":1,"revision":1,"roots":[{
        "kind":"paragraph","id":"label","layout_key":"label-layout",
        "text":"中文 👩‍💻 e\u{301} אבג office\nSecond line",
        "rect":{"x":10,"y":10,"w":160,"h":100},
        "style":{"font":{"size_px":18.0},"wrap":true}}]});
    let canvas: Canvas =
        serde_json::from_value(wire).expect("native paragraph values should decode");
    let document = Document::new(Node::new("drawing", Kind::Canvas(canvas)));
    document
        .validate()
        .expect("bounded native paragraphs should publish");
    assert_eq!(
        document.required_capabilities()["ui.canvas"],
        semver::Version::new(1, 8, 0)
    );
}

/// A native query response must not expose a caret inside one user-perceived character.
#[test]
fn paragraph_query_rejects_positions_inside_a_grapheme() {
    use crate::ui::{Action, CanvasEvent, UiEvent};
    let mut wire = serde_json::to_value(Canvas::default()).unwrap();
    wire["scene"] = json!({"identity":1,"revision":1,"roots":[{
        "kind":"paragraph","id":"label","layout_key":"label-1","text":"e\u{301}x",
        "rect":{"x":0,"y":0,"w":100,"h":40}}]});
    wire["text_queries"] =
        json!([{"id":"q","paragraph":"label","layout_key":"label-1","points":[{"x":0,"y":0}]}]);
    let canvas: Canvas = serde_json::from_value(wire).unwrap();
    let document = Document::new(Node::new("drawing", Kind::Canvas(canvas)));
    document.validate().unwrap();
    let mut event:UiEvent=serde_json::from_value(json!({"revision":0,"node":"drawing","action":{"type":"canvas","value":{
        "type":"text_layout","identity":1,"scene_revision":1,"results":[{"id":"q","paragraph":"label","layout_key":"label-1",
            "points":[{"x":0,"y":0}],"font_environment":1,"hits":[0]}]}}})).unwrap();
    document
        .validate_event(&event)
        .expect("cluster start is a valid native caret");
    if let Action::Canvas(CanvasEvent::TextLayout { results, .. }) = &mut event.action {
        results[0].hits[0] = 1;
    }
    assert!(
        document.validate_event(&event).is_err(),
        "a scalar boundary inside e + combining acute is not a grapheme boundary"
    );
}

/// Target replacement must never redirect an old logical key to a new cell with the same ID.
#[test]
fn canvas_target_input_rejects_a_replaced_epoch() {
    let mut wire = serde_json::to_value(Canvas::default()).unwrap();
    wire["scene"] = json!({"identity":1,"revision":2,"roots":[],"targets":[{
        "id":"cell","epoch":12,"focusable":true,"tab_index":0,
        "geometry":{"kind":"rect","rect":{"x":0,"y":0,"w":100,"h":40}}}]});
    let canvas: Canvas = serde_json::from_value(wire).expect("stable public hit targets decode");
    let document = Document::new(Node::new("drawing", Kind::Canvas(canvas))).revision(3);
    document.validate().unwrap();
    assert_eq!(
        document.required_capabilities()["ui.canvas"],
        semver::Version::new(1, 8, 0)
    );
    let mut wire = json!({"revision":3,"node":"drawing","action":{"type":"canvas","value":{
        "type":"input","input":{"identity":1,"presented_revision":2,"target":"cell","epoch":12,"sequence":1,
            "kind":{"type":"key","phase":"down","key":"arrowright","code":null,"repeat":false,
                "modifiers":{"ctrl":false,"alt":false,"shift":false,"meta":true,"function":false}}}}}});
    let event: crate::ui::UiEvent = serde_json::from_value(wire.clone()).unwrap();
    document
        .validate_event(&event)
        .expect("current target receives a complete logical key");
    wire["action"]["value"]["input"]["epoch"] = 11.into();
    let stale = serde_json::from_value(wire).unwrap();
    assert!(
        document.validate_event(&stale).is_err(),
        "the same target ID cannot reuse old epoch authority"
    );
}

/// Native UTF-16 replacements expand only to whole original emoji/combining/CRLF graphemes.
#[test]
fn canvas_native_text_ranges_preserve_original_graphemes() {
    use crate::ui::{CanvasTextRange, canvas_text_range_from_utf16};
    let text = "e\u{301} 👩‍💻\r\n中";
    assert_eq!(
        canvas_text_range_from_utf16(text, 4..6),
        CanvasTextRange { start: 4, end: 15 }
    );
    assert_eq!(
        canvas_text_range_from_utf16(text, 9..10),
        CanvasTextRange { start: 15, end: 17 }
    );
    assert_eq!(
        canvas_text_range_from_utf16(text, 4..4),
        CanvasTextRange { start: 4, end: 4 }
    );
    assert_eq!(
        CanvasTextRange { start: 4, end: 15 }
            .to_utf16(text)
            .unwrap(),
        3..8
    );
    assert!(
        CanvasTextRange { start: 5, end: 15 }
            .to_utf16(text)
            .is_err()
    );
    assert_eq!(
        canvas_text_range_from_utf16(text, 99..100),
        CanvasTextRange { start: 20, end: 20 }
    );
}

/// A newly accepted redraw cannot revoke input from a target still shown in the previous native frame.
#[test]
fn canvas_current_target_accepts_an_older_display_frame() {
    let mut wire = serde_json::to_value(Canvas::default()).unwrap();
    wire["scene"] = json!({"identity":1,"revision":2,"roots":[],"targets":[{
        "id":"cell","epoch":12,"focusable":true,
        "geometry":{"kind":"rect","rect":{"x":0,"y":0,"w":100,"h":40}}}]});
    let document = Document::new(Node::new(
        "drawing",
        Kind::Canvas(serde_json::from_value(wire).unwrap()),
    ))
    .revision(3);
    document.validate().unwrap();
    let mut event = json!({"revision":2,"node":"drawing","action":{"type":"canvas","value":{
        "type":"input","input":{"identity":1,"presented_revision":1,"target":"cell","epoch":12,"sequence":1,
        "kind":{"type":"key","phase":"down","key":"right","code":null,"repeat":false,
          "modifiers":{"ctrl":false,"alt":false,"shift":false,"meta":false,"function":false}}}}}});
    document
        .validate_event(&serde_json::from_value(event.clone()).unwrap())
        .expect("current target admission defers exact shown-frame evidence to the public Manager");
    event["action"]["value"]["input"]["presented_revision"] = 3.into();
    assert!(
        document
            .validate_event(&serde_json::from_value(event).unwrap())
            .is_err(),
        "a future frame is never current"
    );
}

/// A tiny layout rectangle cannot disguise a huge effective native font allocation.
#[test]
fn paragraph_rejects_excessive_effective_font_before_native_shaping() {
    let scene: crate::ui::CanvasScene = serde_json::from_value(json!({
        "identity":1,"revision":1,"roots":[{"kind":"group","id":"g",
        "transform":[10000,0,0,10000,0,0],"children":[{"kind":"paragraph",
        "id":"p","layout_key":"p-1","text":"","rect":{"x":0,"y":0,"w":0.001,"h":0.001},
        "style":{"font":{"size_px":1}}}]}]
    }))
    .unwrap();
    assert!(
        scene.validate().is_err(),
        "font budget must be checked independently of projected box area"
    );
}

/// Target and semantic geometry share a named group's transform/clip; missing references are atomic errors.
#[test]
fn canvas_semantics_and_targets_resolve_shared_group_space() {
    let mut wire = json!({"identity":1,"revision":1,"roots":[{"kind":"group","id":"group",
        "transform":[2,0,0,2,10,20],"children":[]}],"targets":[{"id":"cell","epoch":1,
        "space":"group","focusable":true,"geometry":{"kind":"rect","rect":{"x":0,"y":0,"w":40,"h":20}}}],
        "semantics":[{"id":"cell-name","role":"grid_cell","target":"cell","name":{"en":"A1","zh_cn":"甲一"},
            "row":1,"column":1,"row_count":100000,"column_count":4,"actions":["focus","activate"]}]});
    let scene: crate::ui::CanvasScene =
        serde_json::from_value(wire.clone()).expect("bounded semantic declarations decode");
    scene.validate().unwrap();
    assert_eq!(
        scene.minimum_canvas_version(),
        semver::Version::new(1, 8, 0)
    );
    wire["targets"][0]["space"] = "missing".into();
    let scene: crate::ui::CanvasScene = serde_json::from_value(wire).unwrap();
    assert!(
        scene.validate().is_err(),
        "a dangling group cannot grant target geometry"
    );
}

/// Retired-target cleanup grants no business input; the Manager still requires an active old frame lease.
#[test]
fn canvas_retired_target_only_admits_bounded_cleanup_observations() {
    let canvas: Canvas =
        serde_json::from_value(json!({"paint":[],"scene":{"identity":1,"revision":2,"roots":[]}}))
            .unwrap();
    let document = Document::new(Node::new("drawing", Kind::Canvas(canvas))).revision(3);
    let mut wire = json!({"revision":2,"node":"drawing","action":{"type":"canvas","value":{"type":"input","input":{
        "identity":1,"presented_revision":1,"target":"old","epoch":1,"sequence":9,
        "kind":{"type":"composition","phase":"cancel","text_revision":1,"base_sequence":0,"range":{"start":4,"end":15},"text":"","selection":{"start":0,"end":0}}
    }}}});
    document
        .validate_event(&serde_json::from_value(wire.clone()).unwrap())
        .expect("only cleanup is deferred to the exact old-frame lease gate");
    wire["action"]["value"]["input"]["kind"] = json!({"type":"text","text_revision":1,"base_sequence":0,"range":{"start":0,"end":0},"text":"late","selection":{"start":4,"end":4}});
    assert!(
        document
            .validate_event(&serde_json::from_value(wire).unwrap())
            .is_err(),
        "old text cannot borrow cleanup authority"
    );
}

/// Disabled semantic containers also deny child accessibility actions through the public input gate.
#[test]
fn canvas_semantic_disabled_parent_rejects_child_focus() {
    let canvas: Canvas = serde_json::from_value(json!({"paint":[],"scene":{"identity":1,"revision":1,"roots":[],
        "targets":[{"id":"cell","epoch":1,"focusable":true,"geometry":{"kind":"rect","rect":{"x":0,"y":0,"w":20,"h":20}}}],
        "semantics":[{"id":"group","role":"group","disabled":true,"name":{"en":"Group","zh_cn":"分组"},"rect":{"x":0,"y":0,"w":20,"h":20}},
            {"id":"label","parent":"group","target":"cell","role":"text_input","name":{"en":"Cell","zh_cn":"单元格"},"actions":["focus"]}]
    }})).unwrap();
    let document = Document::new(Node::new("drawing", Kind::Canvas(canvas))).revision(1);
    document.validate().unwrap();
    let event: crate::ui::UiEvent = serde_json::from_value(json!({"revision":1,"node":"drawing","action":{"type":"canvas","value":{"type":"input","input":{
        "identity":1,"presented_revision":1,"target":"cell","epoch":1,"sequence":1,
        "kind":{"type":"a11y","semantic":"label","action":"focus","value":null}
    }}}})).unwrap();
    assert!(document.validate_event(&event).is_err());
}
