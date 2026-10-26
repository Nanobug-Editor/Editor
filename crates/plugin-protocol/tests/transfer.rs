//! Ordinary nodes and Canvas targets share the independently negotiated inert drop contract.
use plugin_protocol::ui::{Document, Node};
use serde_json::json;

/// A normal native node can opt into explicit format priority without acquiring file or clipboard grants.
#[test]
fn ordinary_native_drop_target_requires_the_transfer_capability() {
    let mut wire =
        serde_json::to_value(Document::new(Node::text("drop-zone", "拖入 / Drop"))).unwrap();
    wire["root"]["drop_target"] = json!({"preferred":["text/html","text/plain;charset=utf-8"]});
    let document: Document = serde_json::from_value(wire.clone()).unwrap();
    document.validate().unwrap();
    assert_eq!(
        document.required_capabilities().get("ui.transfer"),
        Some(&semver::Version::new(1, 0, 0))
    );
    assert_eq!(
        serde_json::to_value(document).unwrap()["root"]["drop_target"],
        wire["root"]["drop_target"]
    );
}

/// Canvas uses the exact same priority model under a stable scene target generation.
#[test]
fn canvas_drop_target_requires_canvas_19_and_the_transfer_capability() {
    let mut canvas = serde_json::to_value(plugin_protocol::ui::Canvas::default()).unwrap();
    canvas["scene"] = json!({"identity":91,"revision":1,"roots":[],"targets":[{"id":"image-cell","epoch":1,
        "geometry":{"kind":"rect","rect":{"x":0,"y":0,"w":80,"h":40}},
        "drop_target":{"preferred":["image/png","text/plain;charset=utf-8"]}}]});
    let document = Document::new(Node::new(
        "drawing",
        plugin_protocol::ui::Kind::Canvas(serde_json::from_value(canvas).unwrap()),
    ));
    document.validate().unwrap();
    let capabilities = document.required_capabilities();
    assert_eq!(
        capabilities.get("ui.canvas"),
        Some(&semver::Version::new(1, 9, 0))
    );
    assert_eq!(
        capabilities.get("ui.transfer"),
        Some(&semver::Version::new(1, 0, 0))
    );
}

/// Reading a selected format names an instance-owned offer, never a native path or source permission.
#[test]
fn transfer_selected_read_uses_a_scoped_offer_and_ordered_preferences() {
    let wire = json!({"kind":"transfer","operation":{"type":"read","offer":{"instance":"destination","scope":"workspace","resource":4},
        "preferred":["text/html","text/plain;charset=utf-8"]}});
    let operation: plugin_protocol::api::EditorOperation =
        serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(serde_json::to_value(operation).unwrap(), wire);
}

/// Native file lists select import authority; arbitrary encoded items can never manufacture paths.
#[test]
fn file_lists_require_controlled_imports_instead_of_arbitrary_bytes() {
    let mime = "application/vnd.nanobug.files";
    plugin_protocol::transfer::validate_preferences(&[mime.into()]).unwrap();
    let item = plugin_protocol::transfer::Item {
        mime: mime.into(),
        bytes: b"C:/private.txt".to_vec(),
    };
    assert!(
        item.validate().is_err(),
        "paths cannot enter through encoded items"
    );
    let wire = json!({"kind":"transfer","operation":{"type":"read_file",
        "file":{"offer":{"instance":"destination","scope":"workspace","resource":4},"entry":0},
        "offset":2,"max_bytes":16}});
    let operation: plugin_protocol::api::EditorOperation =
        serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(serde_json::to_value(operation).unwrap(), wire);
}

/// Declaring a bounded source never starts a drag; only the native host may consume a real gesture.
#[test]
fn native_drag_sources_declare_only_bounded_inert_representations() {
    let mut wire = serde_json::to_value(Document::new(Node::text("source", "drag"))).unwrap();
    wire["root"]["drag_source"] =
        json!({"items":[{"mime":"text/plain;charset=utf-8","bytes":[65,9,66]}]});
    let document: Document = serde_json::from_value(wire.clone()).unwrap();
    document.validate().unwrap();
    assert_eq!(
        document.required_capabilities().get("ui.transfer"),
        Some(&semver::Version::new(1, 0, 0))
    );
    assert_eq!(
        serde_json::to_value(document).unwrap()["root"]["drag_source"],
        wire["root"]["drag_source"]
    );
    wire["root"]["drag_source"]["items"][0]["bytes"] = json!([255]);
    assert!(
        serde_json::from_value::<Document>(wire)
            .unwrap()
            .validate()
            .is_err()
    );
}
