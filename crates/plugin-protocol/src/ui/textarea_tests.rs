//! Native row sizing is negotiated and validated without weakening the existing draft contract.
use super::*;

/// Mixed ordinary/bounded fields require the highest native version regardless of traversal order.
#[test]
fn textarea_rows_negotiate_native_1_2_and_roundtrip() {
    let bounded = Node::textarea("bounded", Input::default()).textarea_rows(2, 4);
    let document = Document::new(Node::column(
        "form",
        vec![bounded, Node::textarea("ordinary", Input::default())],
    ));
    document.validate().unwrap();
    assert_eq!(
        document.required_capabilities()["ui.native"],
        semver::Version::new(1, 2, 0)
    );
    let decoded: Document =
        serde_json::from_str(&serde_json::to_string(&document).unwrap()).unwrap();
    assert_eq!(document, decoded);
    let ordinary = Document::new(Node::textarea("ordinary", Input::default()));
    assert_eq!(
        ordinary.required_capabilities()["ui.native"],
        semver::Version::new(1, 1, 0)
    );
}

/// Ambiguous pixel/row sizing, non-textareas and unbounded line ranges are rejected before allocation.
#[test]
fn textarea_rows_reject_invalid_ranges_and_control_kinds() {
    for (min, max) in [(0, 4), (4, 2), (2, 65)] {
        assert!(
            Document::new(Node::textarea("field", Input::default()).textarea_rows(min, max))
                .validate()
                .is_err()
        );
    }
    for node in [
        Node::button("button", "button").textarea_rows(2, 4),
        Node::textarea("field", Input::default())
            .textarea_rows(2, 4)
            .height(88.),
    ] {
        assert!(Document::new(node).validate().is_err());
    }
}
