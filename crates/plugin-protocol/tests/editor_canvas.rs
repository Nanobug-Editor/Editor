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
        "paint":[],
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

/// Literal Editor rails carry UI-revision authority even when the nested target remains unchanged.
#[test]
fn snapshot_editor_canvas_actions_reject_prior_ui_input() {
    let canvas = ui::Canvas {
        scene: Some(
            serde_json::from_value(json!({
                "identity":41,"revision":1,"roots":[],"targets":[{
                    "id":"surface","epoch":1,
                    "geometry":{"kind":"rect","rect":{"x":0,"y":0,"w":64,"h":48}}
                }]
            }))
            .unwrap(),
        ),
        ..Default::default()
    };
    let event = ui::UiEvent {
        revision: 1,
        node: "target-canvas".into(),
        action: ui::Action::Canvas(ui::CanvasEvent::Input {
            input: ui::CanvasInput {
                identity: 41,
                presented_revision: 1,
                target: "surface".into(),
                epoch: 1,
                sequence: 1,
                source: None,
                file: None,
                projection: None,
                kind: ui::CanvasInputKind::Wheel(ui::CanvasWheelInput {
                    x: 4.,
                    y: 4.,
                    delta_x: 1.,
                    delta_y: 0.,
                    unit: ui::CanvasWheelUnit::Lines,
                    modifiers: Default::default(),
                }),
            },
        }),
    };
    let ordinary = ui::Document::new(ui::Node::new(
        "target-canvas",
        ui::Kind::Canvas(canvas.clone()),
    ))
    .revision(2);
    ordinary.validate().unwrap();
    ordinary
        .validate_event(&event)
        .expect("ordinary retained targets keep their paint-only exception");
    for right in [false, true] {
        for nested in [false, true] {
            let child = ui::Node::new("target-canvas", ui::Kind::Canvas(canvas.clone()));
            let child =
                if nested {
                    let mut outer = canvas.clone();
                    outer.scene.as_mut().unwrap().identity = 51;
                    outer.overlays.push(serde_json::from_value(json!({
                    "id":"owner","anchor":{"identity":51,"scene_revision":1,
                        "target":{"id":"surface","epoch":1},"rect":{"x":0,"y":0,"w":64,"h":48}},
                    "rect":{"x":0,"y":0,"w":64,"h":48},"child":child
                })).unwrap());
                    ui::Node::new("outer", ui::Kind::Canvas(outer))
                } else {
                    child
                };
            let action = ui::EditorAction {
                offset: 0,
                content: Box::new(child),
            };
            let mut editor = ui::Editor::snapshot("old");
            editor.line_actions = Some(if right {
                ui::EditorActions {
                    left: vec![],
                    right: vec![action],
                }
            } else {
                ui::EditorActions {
                    left: vec![action],
                    right: vec![],
                }
            });
            let original =
                ui::Document::new(ui::Node::new("snapshot", ui::Kind::Editor(editor))).revision(1);
            original.validate().unwrap();
            assert!(
                original.editor_action_source("target-canvas").is_none(),
                "literal content cannot acquire document authority"
            );
            original.validate_event(&event).unwrap();
            let mut changed = original.clone();
            changed.revision = 2;
            let ui::Kind::Editor(editor) = &mut changed.root.kind else {
                panic!()
            };
            editor.source = ui::EditorSource::Snapshot { text: "new".into() };
            changed.validate().unwrap();
            let failure = changed
                .validate_event(&event)
                .expect_err("snapshot action must not adopt old shown-target input");
            assert_eq!(
                failure.code,
                api::ErrorCode::StaleRevision,
                "right={right}, nested={nested}"
            );
        }
    }
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
