//! Retained scene operations use the same public admission boundary as independent guests.
use plugin_protocol::ui;
use serde_json::json;

/// Revision-only ticks keep the accepted immutable base; a UI peer cannot counterfeit old artwork.
#[test]
fn retained_publication_skips_idle_ticks_and_rejects_counterfeit_staging() {
    let canvas:ui::Canvas=serde_json::from_value(json!({"paint":[],"scene":{"identity":1,"revision":1,
        "roots":[{"kind":"paragraph","id":"value","layout_key":"value","text":"old","rect":{"x":0,"y":0,"w":50,"h":20}}]}})).unwrap();
    let original =
        ui::Document::new(ui::Node::new("drawing", ui::Kind::Canvas(canvas))).revision(1);
    original.validate().unwrap();
    let mut tick = original.clone();
    tick.revision = 2;
    let ui::Kind::Canvas(canvas) = &mut tick.root.kind else {
        panic!()
    };
    canvas.scene.as_mut().unwrap().revision = 2;
    let idle = ui::retained_publication("panel", Some(&original), tick.clone()).unwrap();
    assert!(
        idle.output.views.is_empty()
            && idle.output.view_patches.is_empty()
            && idle.output.scene_patches.is_empty()
    );
    assert_eq!(idle.document, original);
    let patch: ui::CanvasScenePatch = serde_json::from_value(
        json!({"panel":"panel","canvas":"drawing","base_ui_revision":1,"next_ui_revision":2,
        "identity":1,"base_revision":1,"next_revision":2,"operations":[{"op":"advance"}]}),
    )
    .unwrap();
    // Supplying a newly advanced full scene in a UI shell is not a reuse proof.
    assert!(patch.apply_staged_document(&original, &tick).is_err());
    let mut neutral = tick;
    let ui::Kind::Canvas(canvas) = &mut neutral.root.kind else {
        panic!()
    };
    *canvas = Default::default();
    assert_eq!(
        patch
            .apply_staged_document(&original, &neutral)
            .unwrap()
            .document
            .revision,
        2
    );
    neutral.source = Some(plugin_protocol::api::DocumentVersion {
        id: "doc".into(),
        path: "file.txt".into(),
        revision: 1,
    });
    assert_eq!(
        patch
            .apply_staged_document(&original, &neutral)
            .err()
            .unwrap()
            .code,
        plugin_protocol::api::ErrorCode::StaleRevision
    );
}

/// Parent-first deltas keep nested native artwork out of both the outer delta and UI shell.
#[test]
fn retained_publication_updates_nested_owners_in_public_order() {
    let inner:ui::Canvas=serde_json::from_value(json!({"paint":[],"scene":{"identity":2,"revision":1,
        "roots":[{"kind":"paragraph","id":"inner-text","layout_key":"inner","text":"old","rect":{"x":0,"y":0,"w":30,"h":20}}]}})).unwrap();
    let mut outer:ui::Canvas=serde_json::from_value(json!({"paint":[],"scene":{"identity":1,"revision":1,
        "roots":[],"targets":[{"id":"anchor","epoch":1,"geometry":{"kind":"rect","rect":{"x":0,"y":0,"w":100,"h":100}}}]}})).unwrap();
    outer.overlays.push(ui::CanvasOverlay{id:"owner".into(),focus_return:None,
        anchor:serde_json::from_value(json!({"identity":1,"scene_revision":1,"target":{"id":"anchor","epoch":1},"rect":{"x":0,"y":0,"w":100,"h":100}})).unwrap(),
        rect:plugin_protocol::Rect{x:0.,y:0.,w:100.,h:100.},
        child:Box::new(ui::Node::new("a-inner",ui::Kind::Canvas(inner)))});
    // Lexical ID ordering would incorrectly visit the inner owner first.
    let original = ui::Document::new(ui::Node::new("z-outer", ui::Kind::Canvas(outer))).revision(1);
    original.validate().unwrap();
    let mut next = original.clone();
    next.revision = 2;
    next.root
        .try_visit_mut(&mut |node| {
            if let ui::Kind::Canvas(canvas) = &mut node.kind {
                let scene = canvas.scene.as_mut().unwrap();
                scene.revision = 2;
                if let Some(ui::SceneNode::Paragraph { text, .. }) = scene.roots.first_mut() {
                    *text = "changed".into();
                }
            }
            Ok(())
        })
        .unwrap();
    let publication = ui::retained_publication("panel", Some(&original), next.clone()).unwrap();
    assert!(publication.output.views.is_empty());
    assert_eq!(
        publication
            .output
            .scene_patches
            .iter()
            .map(|p| p.canvas.as_str())
            .collect::<Vec<_>>(),
        ["z-outer", "a-inner"]
    );
    let mut accepted = original.clone();
    for patch in &publication.output.scene_patches {
        accepted = patch
            .apply_staged_document(&original, &accepted)
            .unwrap()
            .document;
    }
    assert_eq!(accepted, next);
    assert!(
        !serde_json::to_string(&publication.output.scene_patches[0])
            .unwrap()
            .contains("inner-text")
    );
}

/// Reordering and reparenting retain large leaves by identity, while attributes are a separate delta.
#[test]
fn retained_publication_moves_existing_artwork_and_inserts_only_new_data() {
    let shape = |id: &str| json!({"kind":"shape","id":id,"geometry":{"kind":"rect","rect":{"x":0,"y":0,"w":10,"h":10}},"fill":{"kind":"solid","rgba":255}});
    let before: ui::Canvas = serde_json::from_value(
        json!({"paint":[],"scene":{"identity":1,"revision":1,"roots":[
        {"kind":"group","id":"parent","children":[shape("a"),shape("b"),shape("c")]},
        {"kind":"group","id":"destination","children":[]}]}}),
    )
    .unwrap();
    let mut next = before.clone();
    let scene = next.scene.as_mut().unwrap();
    scene.revision = 2;
    let ui::SceneNode::Group { children, .. } = &mut scene.roots[0] else {
        panic!()
    };
    let a = children.remove(0);
    children.reverse();
    let ui::SceneNode::Group {
        children,
        transform,
        ..
    } = &mut scene.roots[1]
    else {
        panic!()
    };
    children.push(a);
    children.push(serde_json::from_value(shape("new")).unwrap());
    transform.0[4] = 50.;
    let document =
        |canvas| ui::Document::new(ui::Node::new("drawing", ui::Kind::Canvas(canvas))).revision(1);
    let original = document(before);
    let mut desired = document(next);
    desired.revision = 2;
    let publication = ui::retained_publication("panel", Some(&original), desired.clone()).unwrap();
    assert!(publication.output.views.is_empty());
    let patch = &publication.output.scene_patches[0];
    assert!(
        patch
            .operations
            .iter()
            .any(|op| matches!(op,ui::CanvasSceneOperation::Move{id,..} if id=="a"))
    );
    assert!(
        patch
            .operations
            .iter()
            .any(|op| matches!(op,ui::CanvasSceneOperation::Group{id,..} if id=="destination"))
    );
    assert!(
        patch
            .operations
            .iter()
            .any(|op| matches!(op,ui::CanvasSceneOperation::Insert{node,..} if node.id()=="new"))
    );
    assert_eq!(patch.apply_document(&original).unwrap().document, desired);
}

/// A removed paragraph retires its world-space input, queries, semantic descendants and native child.
#[test]
fn retained_removal_retires_paragraph_dependents_and_covers_removed_overlay() {
    let mut before:ui::Canvas=serde_json::from_value(json!({"paint":[],"scene":{"identity":1,"revision":1,
        "roots":[{"kind":"group","id":"parent","children":[{"kind":"paragraph","id":"value","layout_key":"layout","text":"value","rect":{"x":0,"y":0,"w":50,"h":20}}]}],
        "targets":[{"id":"input","epoch":1,"focusable":true,"geometry":{"kind":"rect","rect":{"x":300,"y":300,"w":50,"h":20}},"text_input":{"paragraph":"value","layout_key":"layout","text_revision":1,"selection":{"start":0,"end":0}}}],
        "semantics":[{"id":"parent-semantic","target":"input","role":"text_input","name":{"en":"Value","zh_cn":"值"}},
            {"id":"child-semantic","parent":"parent-semantic","role":"group","name":{"en":"Child","zh_cn":"子项"},"rect":{"x":0,"y":0,"w":10,"h":10}}]},
        "text_queries":[{"id":"query","paragraph":"value","layout_key":"layout","points":[],"positions":[{"byte":0}],"selections":[]}] })).unwrap();
    before.overlays.push(ui::CanvasOverlay{id:"editor".into(),focus_return:None,
        anchor:serde_json::from_value(json!({"identity":1,"scene_revision":1,"target":{"id":"input","epoch":1},"rect":{"x":300,"y":300,"w":50,"h":20}})).unwrap(),
        rect:plugin_protocol::Rect{x:0.,y:30.,w:100.,h:40.},child:Box::new(ui::Node::input("native",Default::default()))});
    before.validate().unwrap();
    let moved:ui::CanvasScenePatch=serde_json::from_value(json!({"panel":"panel","canvas":"drawing","base_ui_revision":1,"next_ui_revision":2,
        "identity":1,"base_revision":1,"next_revision":2,"operations":[{"op":"group","id":"parent","transform":[1,0,0,1,50,0],"clip":null,"opacity":1}]})).unwrap();
    let mut grouped = before.clone();
    grouped.scene.as_mut().unwrap().targets[0].space = Some("parent".into());
    grouped.overlays[0].anchor.space = Some("parent".into());
    grouped.validate().unwrap();
    let moved_damage = moved.apply(&grouped).unwrap().damage.unwrap();
    assert!(
        moved_damage.x + moved_damage.w >= 450.,
        "moving a group must cover unchanged native children outside its artwork"
    );
    let patch:ui::CanvasScenePatch=serde_json::from_value(json!({"panel":"panel","canvas":"drawing","base_ui_revision":1,"next_ui_revision":2,
        "identity":1,"base_revision":1,"next_revision":2,"operations":[{"op":"remove","id":"parent"}]})).unwrap();
    let accepted = patch.apply(&before).unwrap();
    let scene = accepted.canvas.scene.unwrap();
    assert!(scene.targets.is_empty() && scene.semantics.is_empty());
    assert!(accepted.canvas.text_queries.is_empty() && accepted.canvas.overlays.is_empty());
    let damage = accepted.damage.unwrap();
    assert!(damage.x + damage.w >= 400. && damage.y + damage.h >= 370.);
    assert_eq!(before.overlays.len(), 1);
}

/// New unknown references and byte expansion errors reject a private candidate without changing base.
#[test]
fn retained_patch_rejects_unknown_references_and_expanded_budget() {
    let mut segments = vec![ui::ScenePathSegment::Move {
        x: 12345.678,
        y: 12345.678,
    }];
    segments.extend((0..35_000).map(|_| ui::ScenePathSegment::Line {
        x: 12345.678,
        y: 12345.678,
    }));
    let large = ui::SceneNode::Shape {
        id: "large".into(),
        geometry: ui::SceneGeometry::Path {
            segments,
            rule: Default::default(),
        },
        fill: Some(ui::SceneBrush::Solid { rgba: 255 }),
        stroke: None,
    };
    let before = ui::Canvas {
        scene: Some(ui::CanvasScene {
            identity: 1,
            revision: 1,
            roots: vec![large.clone()],
            targets: vec![],
            semantics: vec![],
        }),
        ..Default::default()
    };
    before.validate().unwrap();
    let patch = |operations| {
        serde_json::from_value::<ui::CanvasScenePatch>(
            json!({"panel":"panel","canvas":"drawing","base_ui_revision":1,"next_ui_revision":2,
        "identity":1,"base_revision":1,"next_revision":2,"operations":operations}),
        )
        .unwrap()
    };
    let unknown = patch(
        json!([{"op":"upsert_target","index":0,"target":{"id":"input","epoch":1,"space":"unknown","geometry":{"kind":"rect","rect":{"x":0,"y":0,"w":10,"h":10}}}}]),
    );
    assert_eq!(
        unknown.apply(&before).err().unwrap().code,
        plugin_protocol::api::ErrorCode::InvalidRequest
    );
    let mut inserted = large;
    let ui::SceneNode::Shape { id, .. } = &mut inserted else {
        panic!()
    };
    *id = "large-2".into();
    let expansion = patch(json!([{"op":"insert","parent":null,"index":1,"node":inserted}]));
    assert!(serde_json::to_vec(&expansion).unwrap().len() < ui::MAX_SCENE_PATCH_BYTES);
    assert_eq!(
        expansion.apply(&before).err().unwrap().code,
        plugin_protocol::api::ErrorCode::LimitExceeded
    );
    let mut encoded = expansion.clone();
    encoded.operations.extend(expansion.operations.clone());
    assert!(serde_json::to_vec(&encoded).unwrap().len() > ui::MAX_SCENE_PATCH_BYTES);
    assert_eq!(
        encoded.apply(&before).err().unwrap().code,
        plugin_protocol::api::ErrorCode::LimitExceeded
    );
    assert_eq!(before.scene.unwrap().roots.len(), 1);
}

/// One editable value and a normal status control travel without the unchanged scene dataset.
#[test]
fn retained_publication_combines_local_scene_and_ordinary_ui_changes() {
    let mut roots = vec![
        json!({"kind":"paragraph","id":"value","layout_key":"value-1","text":"old",
        "rect":{"x":0,"y":0,"w":80,"h":30}}),
    ];
    roots.extend((0..500).map(|index|json!({"kind":"shape","id":format!("event-{index}"),
        "geometry":{"kind":"rect","rect":{"x":index,"y":50,"w":1,"h":1}},"fill":{"kind":"solid","rgba":863633407}})));
    let canvas:ui::Canvas=serde_json::from_value(json!({"paint":[],"scene":{"identity":81,"revision":1,"roots":roots,
        "targets":[{"id":"cell","epoch":1,"focusable":true,"geometry":{"kind":"rect","rect":{"x":0,"y":0,"w":80,"h":30}},
            "text_input":{"paragraph":"value","layout_key":"value-1","text_revision":1,"selection":{"start":0,"end":0}}}],
        "semantics":[{"id":"semantic","target":"cell","role":"text_input","name":{"en":"Value","zh_cn":"值"},"value":"old","actions":["focus","set_value"]}]}})).unwrap();
    let original = ui::Document::new(ui::Node::column(
        "root",
        vec![
            ui::Node::text("status", "old"),
            ui::Node::new("drawing", ui::Kind::Canvas(canvas)),
        ],
    ))
    .revision(1);
    let mut wire = serde_json::to_value(&original).unwrap();
    wire["revision"] = 2.into();
    wire["root"]["kind"]["children"][0]["kind"]["text"] = "changed".into();
    let scene = &mut wire["root"]["kind"]["children"][1]["kind"]["scene"];
    scene["revision"] = 2.into();
    scene["roots"][0]["text"] = "new".into();
    scene["roots"][0]["layout_key"] = "value-2".into();
    scene["targets"][0]["text_input"]["layout_key"] = "value-2".into();
    scene["targets"][0]["text_input"]["text_revision"] = 2.into();
    scene["semantics"][0]["value"] = "new".into();
    let next: ui::Document = serde_json::from_value(wire).unwrap();
    next.validate().unwrap();
    let publication = ui::retained_publication("panel", Some(&original), next.clone()).unwrap();
    assert!(publication.output.views.is_empty());
    assert_eq!(publication.output.scene_patches.len(), 1);
    assert_eq!(publication.output.view_patches.len(), 1);
    assert!(
        serde_json::to_vec(&publication.output).unwrap().len()
            < serde_json::to_vec(&next).unwrap().len() / 5
    );
    let view = &publication.output.view_patches[0];
    let mut staged = view.document.clone();
    ui::incremental::restore(&original, &mut staged, &view.reused).unwrap();
    let accepted = publication.output.scene_patches[0]
        .apply_staged_document(&original, &staged)
        .unwrap();
    assert_eq!(accepted.document, next);
    assert_eq!(publication.document, next);
    assert_eq!(original.revision, 1);
}

/// Replacing one drawing item preserves the input and computes damage beyond a smaller guest hint.
#[test]
fn retained_scene_patch_replaces_one_item_atomically_and_covers_actual_damage() {
    let original: ui::Canvas = serde_json::from_value(json!({"paint":[],"scene":{
        "identity":81,"revision":1,"roots":[{"kind":"shape","id":"event",
        "geometry":{"kind":"rect","rect":{"x":0,"y":0,"w":40,"h":20}},
        "fill":{"kind":"solid","rgba":863633407}},
        {"kind":"shape","id":"unchanged","geometry":{"kind":"rect", "rect":{"x":1000,"y":1000,"w":200,"h":300}},
        "fill":{"kind":"solid","rgba":863633407}}]}}))
    .unwrap();
    let patch: ui::CanvasScenePatch = serde_json::from_value(json!({
        "panel":"timeline","canvas":"drawing","base_ui_revision":1,"next_ui_revision":2,
        "identity":81,"base_revision":1,"next_revision":2,
        "operations":[{"op":"replace","node":{"kind":"shape","id":"event",
        "geometry":{"kind":"rect","rect":{"x":30,"y":10,"w":40,"h":20}},
        "fill":{"kind":"solid","rgba":863633407}}}],
        "damage":{"x":35,"y":15,"w":1,"h":1}}))
    .unwrap();
    let accepted = patch.apply(&original).unwrap();
    assert_eq!(original.scene.as_ref().unwrap().revision, 1);
    assert_eq!(accepted.canvas.scene.as_ref().unwrap().revision, 2);
    assert_eq!(
        accepted.damage,
        Some(plugin_protocol::Rect {
            x: 0.,
            y: 0.,
            w: 70.,
            h: 30.
        })
    );
}

/// Independent guests publish deltas without serializing a full View, with both bases explicit.
#[test]
fn retained_patch_output_restores_only_the_exact_document_base() {
    let canvas: ui::Canvas = serde_json::from_value(json!({"paint":[],"scene":{"identity":81,
        "revision":1,"roots":[{"kind":"shape","id":"event","geometry":{"kind":"rect",
        "rect":{"x":0,"y":0,"w":40,"h":20}}}]}}))
    .unwrap();
    let original =
        ui::Document::new(ui::Node::new("drawing", ui::Kind::Canvas(canvas))).revision(1);
    let patch: ui::CanvasScenePatch =
        serde_json::from_value(json!({"panel":"timeline","canvas":"drawing",
        "base_ui_revision":1,"next_ui_revision":2,"identity":81,"base_revision":1,"next_revision":2,
        "operations":[{"op":"remove","id":"event"}]}))
        .unwrap();
    let output = plugin_protocol::api::Output {
        scene_patches: vec![patch],
        ..Default::default()
    };
    let decoded: plugin_protocol::api::Output =
        serde_json::from_value(serde_json::to_value(output).unwrap()).unwrap();
    assert!(decoded.views.is_empty());
    let accepted = decoded.scene_patches[0].apply_document(&original).unwrap();
    assert_eq!(accepted.document.revision, 2);
    assert_eq!(original.revision, 1);
    assert_eq!(
        decoded.scene_patches[0]
            .apply_document(&accepted.document)
            .err()
            .unwrap()
            .code,
        plugin_protocol::api::ErrorCode::StaleRevision
    );
}

/// Removing a parent retires dependent input instead of leaving a dangling coordinate owner.
#[test]
fn retained_scene_removal_retires_the_deleted_groups_targets() {
    let original: ui::Canvas = serde_json::from_value(json!({"paint":[],"scene":{
        "identity":81,"revision":1,"roots":[{"kind":"group","id":"bucket",
        "transform":[1,0,0,1,20,10],"children":[{"kind":"shape","id":"event",
        "geometry":{"kind":"rect","rect":{"x":0,"y":0,"w":40,"h":20}},
        "fill":{"kind":"solid","rgba":863633407}}]}],"targets":[{"id":"cell","epoch":1,
        "space":"bucket","geometry":{"kind":"rect","rect":{"x":0,"y":0,"w":40,"h":20}}}]}}))
    .unwrap();
    original.validate().unwrap();
    let patch: ui::CanvasScenePatch = serde_json::from_value(json!({
        "panel":"timeline","canvas":"drawing","base_ui_revision":1,"next_ui_revision":2,
        "identity":81,"base_revision":1,"next_revision":2,
        "operations":[{"op":"remove","id":"bucket"}]}))
    .unwrap();
    let accepted = patch.apply(&original).unwrap();
    assert_eq!(original.scene.as_ref().unwrap().targets.len(), 1);
    assert!(accepted.canvas.scene.as_ref().unwrap().roots.is_empty());
    assert!(accepted.canvas.scene.as_ref().unwrap().targets.is_empty());
    assert_eq!(
        accepted.damage,
        Some(plugin_protocol::Rect {
            x: 20.,
            y: 10.,
            w: 40.,
            h: 20.
        })
    );
}
