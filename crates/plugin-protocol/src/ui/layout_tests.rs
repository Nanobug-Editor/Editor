//! Exercise complete file layouts at the public serialized guest boundary.
use super::*;

/// A plugin must receive the same proportions after a serialized cold-start document roundtrip.
#[test]
fn cold_start_split_proportions_survive_the_public_document() {
    let wire = serde_json::json!({"version":1,"revision":1,"root":{"id":"panes","layout":{"resizable":true,"split_proportions":[0.2,0.5,0.3]},"kind":{"type":"row","children":[{"id":"left","kind":{"type":"text","text":"A"}},{"id":"middle","kind":{"type":"text","text":"B"}},{"id":"right","kind":{"type":"text","text":"C"}}]}}});
    let document: Document = serde_json::from_value(wire).unwrap();
    document.validate().unwrap();
    let value = serde_json::to_value(document).unwrap();
    let weights: Vec<f32> =
        serde_json::from_value(value["root"]["layout"]["split_proportions"].clone()).unwrap();
    assert_eq!(weights, vec![0.2, 0.5, 0.3]);
}

/// Malformed, disabled, stale or unsolicited split receipts never become plugin persistence intent.
#[test]
fn split_proportions_require_opt_in_and_current_normalized_geometry() {
    let mut document = Document::new(
        Node::row(
            "panes",
            vec![Node::text("left", "A"), Node::text("right", "B")],
        )
        .split_proportions(vec![0.4, 0.6]),
    );
    assert_eq!(
        document.required_capabilities()["ui.native"],
        semver::Version::new(1, 3, 0)
    );
    let mut event = UiEvent {
        revision: document.revision,
        node: "panes".into(),
        action: Action::SplitResized(vec![0.3, 0.7]),
    };
    document.validate_event(&event).unwrap();
    for weights in [
        vec![],
        vec![0.2, 0.3],
        vec![-0.1, 1.1],
        vec![0.2, 0.3, 0.5],
        vec![f32::NAN, 0.5],
    ] {
        event.action = Action::SplitResized(weights.clone());
        assert!(document.validate_event(&event).is_err());
        if !weights.is_empty() {
            document.root.layout.split_proportions = Some(weights);
            assert!(document.validate().is_err());
        }
    }
    document.root.layout.split_proportions = Some(vec![0.4, 0.6]);
    event.action = Action::SplitResized(vec![0.3, 0.7]);
    document.root.disabled = true;
    assert!(document.validate_event(&event).is_err());
    document.root.disabled = false;
    document.revision += 1;
    assert!(document.validate_event(&event).is_err());
    event.revision = document.revision;
    document.root.layout.split_proportions = None;
    assert!(document.validate_event(&event).is_err());
    document.root.layout.split_proportions = Some(vec![]);
    document.validate().unwrap();
    document.validate_event(&event).unwrap();
}

/// Pane resizing is explicit portable geometry; wrapping or scalar controls cannot claim it.
#[test]
fn serialized_resizable_containers_keep_their_geometry_contract() {
    let wire = serde_json::json!({"version":1,"revision":1,"root":{"id":"panes","layout":{"resizable":true},"kind":{"type":"row","children":[{"id":"left","kind":{"type":"text","text":"A"}},{"id":"right","kind":{"type":"text","text":"B"}}]}}});
    let document: Document = serde_json::from_value(wire.clone()).unwrap();
    document.validate().unwrap();
    assert_eq!(
        serde_json::to_value(&document).unwrap()["root"]["layout"]["resizable"],
        true
    );
    let mut invalid = wire;
    invalid["root"]["layout"]["wrap"] = true.into();
    assert!(
        serde_json::from_value::<Document>(invalid)
            .unwrap()
            .validate()
            .is_err()
    );
}

/// Content defaults accept portable RGB tokens only, never unbounded or malformed theme data.
#[test]
fn content_defaults_are_bounded_rgb_roles() {
    let mut document = Document::new(Node::text("text", "content"));
    document
        .content_colors
        .insert("rich_text.foreground".into(), 0xabcdef);
    document.validate().unwrap();
    document
        .content_colors
        .insert("rich_text.foreground".into(), 0x1ffffff);
    assert!(document.validate().is_err());
    document.content_colors.clear();
    document.content_colors.insert("invalid role".into(), 0);
    assert!(document.validate().is_err());
    document.content_colors = (0..65)
        .map(|index| (format!("role{index}.foreground"), 0))
        .collect();
    assert!(document.validate().is_err());
}

/// Layout references reuse one exact document; duplicate/cross-file references cannot create sessions.
#[test]
fn native_editor_layout_references_are_exact_and_unique() {
    let wire = serde_json::json!({
        "version":1,"revision":2,"editor_layout":true,
        "source":{"id":"open-text","path":"notes.txt","revision":3},
        "root":{"id":"layout","kind":{"type":"row","children":[
            {"id":"editor","kind":{"type":"editor","source":{"type":"document","document":{"id":"open-text","path":"notes.txt","revision":3}}}},
            {"id":"detail","kind":{"type":"text","text":"Details"}}
        ]}}
    });
    let document: Document = serde_json::from_value(wire.clone()).unwrap();
    document.validate().unwrap();
    assert!(document.active_native_editor().is_some());
    let mut disabled = document.clone();
    disabled.root.disabled = true;
    assert!(disabled.active_native_editor().is_none());
    let mut invalid = wire.clone();
    invalid["root"]["kind"]["children"][0]["kind"]["source"]["document"]["id"] =
        "another-file".into();
    assert!(
        serde_json::from_value::<Document>(invalid)
            .unwrap()
            .validate()
            .is_err()
    );
    let mut duplicate = wire.clone();
    let mut second = duplicate["root"]["kind"]["children"][0].clone();
    second["id"] = "second-editor".into();
    duplicate["root"]["kind"]["children"]
        .as_array_mut()
        .unwrap()
        .push(second);
    assert!(
        serde_json::from_value::<Document>(duplicate)
            .unwrap()
            .validate()
            .is_err()
    );
    let mut unowned = wire;
    unowned["editor_layout"] = false.into();
    assert!(
        serde_json::from_value::<Document>(unowned)
            .unwrap()
            .validate()
            .is_err()
    );
    let mut dialog = document.clone();
    dialog.dialog = Some(Dialog::new(
        "modal",
        "Modal",
        document.root.find("editor").unwrap().clone(),
    ));
    dialog.dialog.as_mut().unwrap().content.id = "dialog-editor".into();
    assert!(dialog.validate().is_err());
}
