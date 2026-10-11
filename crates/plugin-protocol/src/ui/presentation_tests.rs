//! Public wire acceptance for an owned lazy tree, independent of native widget internals.

use super::{Document, Node};
use serde_json::json;

/// A valid tree reference must survive the ordinary public document decoder and capability gate.
#[test]
fn owned_lazy_tree_decodes_as_an_ordinary_native_node() {
    let mut wire = serde_json::to_value(Document::new(Node::text("tree", "placeholder"))).unwrap();
    wire["root"]["kind"] = json!({
        "type": "tree",
        "handle": {"instance":"fixture-instance", "scope":"fixture-workspace", "resource":1}
    });
    let document: Document =
        serde_json::from_value(wire).expect("owned lazy tree is a public node");
    assert!(document.validate().is_ok());
    assert!(document.required_capabilities().contains_key("ui.tree"));
}
