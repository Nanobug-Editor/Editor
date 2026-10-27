//! Unified Editor actions and scene overlays compose through the same public tree and delta walkers.
use plugin_protocol::{api, ui};
use serde_json::json;

/// One borrowed source owns a Canvas edge whose native overlay contains a readonly Editor action.
fn document() -> ui::Document {
    let source = api::DocumentVersion {
        id: "source".into(),
        path: "source.txt".into(),
        revision: 1,
    };
    let mut snapshot = ui::Editor::snapshot("old");
    snapshot.line_actions = Some(ui::EditorActions {
        left: Vec::new(),
        right: vec![ui::EditorAction {
            offset: 0,
            content: Box::new(ui::Node::button("action", "Run")),
        }],
    });
    let canvas: ui::Canvas = serde_json::from_value(json!({
        "scene":{"identity":61,"revision":1,"roots":[],"targets":[{
            "id":"anchor","epoch":1,
            "geometry":{"kind":"rect","rect":{"x":0,"y":0,"w":100,"h":100}}
        }]},
        "overlays":[{"id":"overlay","anchor":{"identity":61,"scene_revision":1,
            "target":{"id":"anchor","epoch":1},"rect":{"x":0,"y":0,"w":100,"h":100}},
            "rect":{"x":0,"y":0,"w":100,"h":100},
            "child":ui::Node::new("snapshot-editor",ui::Kind::Editor(snapshot))}]
    }))
    .unwrap();
    let mut editor = ui::Editor::document(source.clone());
    editor.line_actions = Some(ui::EditorActions {
        left: vec![ui::EditorAction {
            offset: 0,
            content: Box::new(ui::Node::new("drawing", ui::Kind::Canvas(canvas))),
        }],
        right: Vec::new(),
    });
    let mut document =
        ui::Document::new(ui::Node::new("source-editor", ui::Kind::Editor(editor))).revision(1);
    document.source = Some(source);
    document.editor_layout = true;
    document
}

/// Neither action nor overlay boundaries hide capabilities, source credentials or retained mutations.
#[test]
fn unified_editor_canvas_nested_actions_preserve_public_tree_and_delta_admission() {
    let previous = document();
    previous.validate().unwrap();
    let mut ids = Vec::new();
    previous.root.visit(&mut |node| ids.push(node.id.clone()));
    assert_eq!(
        ids,
        ["source-editor", "drawing", "snapshot-editor", "action"]
    );
    assert!(previous.active_node("action").is_some());
    assert_eq!(
        previous.editor_action_source("action"),
        previous.source.as_ref()
    );
    let requirements = previous.required_capabilities();
    assert_eq!(requirements["ui.canvas"], semver::Version::new(1, 9, 0));
    assert_eq!(
        requirements["ui.code_editor"],
        semver::Version::new(2, 0, 0)
    );
    assert_eq!(requirements["editor.layout"], semver::Version::new(2, 0, 0));

    let mut changed = previous.clone();
    changed
        .root
        .try_visit_mut(&mut |node| {
            if node.id == "snapshot-editor"
                && let ui::Kind::Editor(editor) = &mut node.kind
            {
                editor.source = ui::EditorSource::Snapshot { text: "new".into() };
            }
            Ok(())
        })
        .unwrap();
    changed.validate().unwrap();
    assert!(changed.validate_editor_replacement(&previous).is_err());
    changed.revision = 2;
    changed.validate_editor_replacement(&previous).unwrap();
    let mut compact = changed.clone();
    let reused = ui::incremental::compact(&previous, &mut compact);
    assert!(reused.iter().any(|reference| reference.id == "action"));
    ui::incremental::restore(&previous, &mut compact, &reused).unwrap();
    assert_eq!(compact, changed);

    let patch: ui::CanvasScenePatch = serde_json::from_value(json!({
        "panel":"panel","canvas":"drawing","base_ui_revision":1,"next_ui_revision":2,
        "identity":61,"base_revision":1,"next_revision":2,"operations":[{"op":"advance"}]
    }))
    .unwrap();
    let patched = patch.apply_document(&previous).unwrap().document;
    let ui::Kind::Canvas(canvas) = &patched.active_node("drawing").unwrap().kind else {
        panic!("nested Canvas must remain addressable");
    };
    assert_eq!(canvas.scene.as_ref().unwrap().revision, 2);
    assert_eq!(
        patched.editor_action_source("action"),
        patched.source.as_ref()
    );

    // With no enclosing Editor, the replacement check must still enter the Canvas overlay.
    let before_overlay =
        ui::Document::new(previous.active_node("drawing").unwrap().clone()).revision(1);
    let after_overlay =
        ui::Document::new(changed.active_node("drawing").unwrap().clone()).revision(1);
    before_overlay.validate().unwrap();
    after_overlay.validate().unwrap();
    assert!(
        after_overlay
            .validate_editor_replacement(&before_overlay)
            .is_err()
    );
}
