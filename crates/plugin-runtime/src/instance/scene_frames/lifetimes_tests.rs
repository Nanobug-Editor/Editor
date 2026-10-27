//! Pure accepted-history boundary tests use ordinary public documents and real frame state.
use super::Frames;
use plugin_protocol::{Rect, api, ui};
use std::{collections::BTreeMap, sync::Arc};

/// A transparent interactive surface is valid independently of optional decorative artwork.
fn canvas(identity: u64) -> ui::Canvas {
    let target = serde_json::from_value(serde_json::json!({
        "id":"surface","epoch":1,"geometry":{"kind":"rect","rect":{"x":0,"y":0,"w":64,"h":48}}
    }))
    .unwrap();
    let scene = ui::CanvasScene {
        identity,
        revision: 1,
        roots: vec![],
        targets: vec![target],
        semantics: vec![],
    };
    scene.validate().unwrap();
    ui::Canvas {
        scene: Some(scene),
        ..Default::default()
    }
}

/// Keep scene identity/drawing stable while outer UI revisions and foreground ownership change.
fn document() -> ui::Document {
    ui::Document::new(ui::Node::new("drawing", ui::Kind::Canvas(canvas(71)))).revision(1)
}

/// Construct native observation fields solely from the exact public accepted snapshot.
fn receipt(document: &ui::Document, node: &str, error: Option<String>) -> ui::UiEvent {
    let declaration = document.active_node(node).unwrap();
    let ui::Kind::Canvas(canvas) = &declaration.kind else {
        panic!("expected ordinary Canvas")
    };
    let scene = canvas.scene.as_ref().unwrap();
    ui::UiEvent {
        revision: document.revision,
        node: node.into(),
        action: ui::Action::Canvas(ui::CanvasEvent::ScenePresented {
            identity: scene.identity,
            scene_revision: scene.revision,
            source: document.source.clone(),
            file: document.file.clone(),
            projection: declaration.viewport.clone(),
            error,
        }),
    }
}

/// Exercise the production remember→publish→reconcile ordering without a Manager mock or WASM.
fn publish(
    frames: &mut Frames,
    current: &mut BTreeMap<String, Arc<ui::Document>>,
    next: ui::Document,
) {
    frames.remember(
        &[api::View {
            panel: "panel".into(),
            document: next.clone(),
        }],
        current,
    );
    current.insert("panel".into(), Arc::new(next));
    frames.reconcile(current);
}

/// Establish a live old snapshot first, then prove B irreversibly revokes A even after A returns.
fn irreversible_aba(original: ui::Document, change: impl FnOnce(&mut ui::Document)) {
    let shown = Arc::new(original.clone());
    let event = receipt(&shown, "drawing", None);
    shown.validate_event(&event).unwrap();
    let mut current = BTreeMap::from([("panel".into(), shown.clone())]);
    let mut frames = Frames::default();
    let mut unchanged = original.clone();
    unchanged.revision = 2;
    publish(&mut frames, &mut current, unchanged);
    assert!(Arc::ptr_eq(
        &shown,
        &frames
            .accepted_document("panel", &event, &current["panel"])
            .expect("A must initially be eligible")
    ));
    let mut replacement = original.clone();
    replacement.revision = 3;
    change(&mut replacement);
    publish(&mut frames, &mut current, replacement);
    assert!(
        frames
            .accepted_document("panel", &event, &current["panel"])
            .is_none(),
        "B must revoke A"
    );
    let mut restored = original;
    restored.revision = 4;
    publish(&mut frames, &mut current, restored);
    assert!(
        frames
            .accepted_document("panel", &event, &current["panel"])
            .is_none(),
        "returning to A cannot restore its pruned qualification"
    );
}

/// Text-source A→B→A cannot revive a receipt carrying an earlier source lifetime.
#[test]
fn accepted_history_source_aba_never_revives_old_receipt() {
    let mut original = document();
    original.source = Some(api::DocumentVersion {
        id: "source-a".into(),
        path: "notes.txt".into(),
        revision: 1,
    });
    irreversible_aba(original, |next| {
        next.source.as_mut().unwrap().id = "source-b".into()
    });
}

/// Binary file ownership is independent of text metadata and has the same irreversible retirement.
#[test]
fn accepted_history_file_aba_never_revives_old_receipt() {
    let mut original = document();
    let file = api::FileVersion {
        id: "file-a".into(),
        path: "image.bin".into(),
        revision: 1,
    };
    file.validate().unwrap();
    original.file = Some(file);
    irreversible_aba(original, |next| {
        next.file.as_mut().unwrap().id = "file-b".into()
    });
}

/// Closing a foreground dialog cannot revive an old underlying receipt with unchanged artwork.
#[test]
fn accepted_history_modal_aba_never_revives_old_receipt() {
    irreversible_aba(document(), |next| {
        next.dialog = Some(ui::Dialog::new(
            "modal",
            "Modal",
            ui::Node::new(
                "modal-content",
                ui::Kind::Text {
                    text: "Foreground".into(),
                },
            ),
        ))
    });
}

/// The outer Canvas remains live so an old document stays retained after its inner owner retires.
#[test]
fn accepted_history_overlay_lineage_aba_prunes_only_original_child_qualification() {
    let mut outer = canvas(91);
    outer.overlays.push(ui::CanvasOverlay {
        focus_return: None,
        id: "owner-a".into(),
        anchor: ui::CanvasAnchor {
            identity: 91,
            scene_revision: 1,
            target: ui::CanvasTargetRef {
                id: "surface".into(),
                epoch: 1,
            },
            space: None,
            rect: Rect {
                x: 0.,
                y: 0.,
                w: 64.,
                h: 48.,
            },
        },
        rect: Rect {
            x: 0.,
            y: 0.,
            w: 64.,
            h: 48.,
        },
        child: Box::new(document().root),
    });
    let original = ui::Document::new(ui::Node::new("outer", ui::Kind::Canvas(outer))).revision(1);
    let shown = Arc::new(original.clone());
    let inner = receipt(&shown, "drawing", None);
    let enclosing = receipt(&shown, "outer", None);
    shown.validate_event(&inner).unwrap();
    shown.validate_event(&enclosing).unwrap();
    let mut current = BTreeMap::from([("panel".into(), shown.clone())]);
    let mut frames = Frames::default();
    let mut next = original.clone();
    next.revision = 2;
    publish(&mut frames, &mut current, next);
    assert!(
        frames
            .accepted_document("panel", &inner, &current["panel"])
            .is_some()
    );
    let mut changed = original.clone();
    changed.revision = 3;
    let ui::Kind::Canvas(parent) = &mut changed.root.kind else {
        panic!()
    };
    parent.overlays[0].id = "owner-b".into();
    publish(&mut frames, &mut current, changed);
    assert!(
        frames
            .accepted_document("panel", &inner, &current["panel"])
            .is_none()
    );
    let mut restored = original;
    restored.revision = 4;
    publish(&mut frames, &mut current, restored);
    assert!(
        frames
            .accepted_document("panel", &inner, &current["panel"])
            .is_none()
    );
    assert!(
        Arc::ptr_eq(
            &shown,
            &frames
                .accepted_document("panel", &enclosing, &current["panel"])
                .expect("live outer sibling must retain the same historical document")
        ),
        "the child must be pruned individually, not accidentally rejected because its whole document vanished"
    );
}

/// Editor 2's left/right rails require exact UI proof, including nested foreground canvases.
#[test]
fn accepted_history_editor_rails_never_retain_prior_ui_proofs() {
    for right_rail in [false, true] {
        let mut outer = canvas(91);
        outer.overlays.push(
            serde_json::from_value(serde_json::json!({
                "id": "owner-a",
                "anchor": {
                    "identity": 91, "scene_revision": 1,
                    "target": {"id": "surface", "epoch": 1},
                    "rect": {"x": 0, "y": 0, "w": 64, "h": 48}
                },
                "rect": {"x": 0, "y": 0, "w": 64, "h": 48},
                "child": document().root
            }))
            .unwrap(),
        );
        let action = ui::EditorAction {
            offset: 0,
            content: Box::new(ui::Node::new("outer", ui::Kind::Canvas(outer))),
        };
        let mut editor = ui::Editor::snapshot("Readonly context");
        editor.line_actions = Some(if right_rail {
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
            ui::Document::new(ui::Node::new("editor", ui::Kind::Editor(editor))).revision(1);
        original.validate().unwrap();
        assert_eq!(
            crate::ui_owners::lineage(&original, "drawing")
                .unwrap()
                .len(),
            1
        );
        let event = receipt(&original, "drawing", None);
        let mut current = BTreeMap::from([("panel".into(), Arc::new(original.clone()))]);
        let mut frames = Frames::default();
        publish(&mut frames, &mut current, original.revision(2));
        assert!(
            frames
                .accepted_document("panel", &event, &current["panel"])
                .is_none(),
            "a snapshot rail cannot acquire the ordinary Canvas late-presentation exception"
        );
    }
}

/// A late native cancellation seals only its old rail; a new rail keeps its independent focus.
#[test]
fn editor_action_old_cleanup_cannot_clear_replacement_focus() {
    let mut drawing = document().root;
    let ui::Kind::Canvas(canvas) = &mut drawing.kind else {
        panic!()
    };
    canvas.scene.as_mut().unwrap().targets[0].focusable = true;
    let mut editor = ui::Editor::snapshot("before");
    editor.line_actions = Some(ui::EditorActions {
        left: vec![ui::EditorAction {
            offset: 0,
            content: Box::new(drawing),
        }],
        right: vec![],
    });
    let original =
        Arc::new(ui::Document::new(ui::Node::new("editor", ui::Kind::Editor(editor))).revision(1));
    original.validate().unwrap();
    assert!(original.editor_action_source("drawing").is_none());
    let mut frames = Frames::default();
    let mut current = BTreeMap::from([("panel".into(), original.clone())]);
    show(&mut frames, &original);
    let (event, input) = native_focus(&original, 1, true);
    frames.validate("panel", &event, &input).unwrap();
    frames.commit("panel", &event, &input);
    let mut replacement = original.as_ref().clone().revision(2);
    let ui::Kind::Editor(editor) = &mut replacement.root.kind else {
        panic!()
    };
    editor.source = ui::EditorSource::Snapshot {
        text: "after".into(),
    };
    replacement.validate().unwrap();
    publish(&mut frames, &mut current, replacement);
    let replacement = current["panel"].clone();
    show(&mut frames, &replacement);
    let (unowned, input) = native_focus(&replacement, 2, false);
    assert!(
        frames.validate("panel", &unowned, &input).is_err(),
        "new UI must not inherit the previous rail's active cleanup lease"
    );
    let (fresh, input) = native_focus(&replacement, 2, true);
    frames.validate("panel", &fresh, &input).unwrap();
    frames.commit("panel", &fresh, &input);
    let (old_cleanup, input) = native_focus(&original, 3, false);
    frames.validate("panel", &old_cleanup, &input).unwrap();
    assert!(
        !frames.same_ancestry("panel", &old_cleanup, &input, Some(&replacement)),
        "old UI cleanup cannot reach the replacement's guest focus/draft"
    );
    frames.commit("panel", &old_cleanup, &input);
    let (new_cleanup, input) = native_focus(&replacement, 4, false);
    frames
        .validate("panel", &new_cleanup, &input)
        .expect("old cleanup must leave new focus owned");
    frames.commit("panel", &new_cleanup, &input);
    let (repeated, input) = native_focus(&replacement, 5, false);
    assert!(frames.validate("panel", &repeated, &input).is_err());
}

/// A native proof is built only from the actual public document selected for this display.
fn show(frames: &mut Frames, document: &ui::Document) {
    let event = receipt(document, "drawing", None);
    document.validate_event(&event).unwrap();
    let ui::Action::Canvas(action) = &event.action else {
        panic!()
    };
    let ui::Kind::Canvas(canvas) = &document.active_node("drawing").unwrap().kind else {
        panic!()
    };
    frames
        .present(
            "panel",
            &event,
            action,
            canvas.scene.as_ref().unwrap(),
            document,
        )
        .unwrap();
}

/// Focus observations echo exact source/projection fields and use one monotonically ordered stream.
fn native_focus(
    document: &ui::Document,
    sequence: u64,
    focused: bool,
) -> (ui::UiEvent, ui::CanvasInput) {
    let node = document.active_node("drawing").unwrap();
    let ui::Kind::Canvas(canvas) = &node.kind else {
        panic!()
    };
    let scene = canvas.scene.as_ref().unwrap();
    let input = ui::CanvasInput {
        identity: scene.identity,
        presented_revision: scene.revision,
        target: "surface".into(),
        epoch: 1,
        sequence,
        source: document.source.clone(),
        file: document.file.clone(),
        projection: node.viewport.clone(),
        kind: ui::CanvasInputKind::Focus { focused },
    };
    let event = ui::UiEvent {
        revision: document.revision,
        node: "drawing".into(),
        action: ui::Action::Canvas(ui::CanvasEvent::Input {
            input: input.clone(),
        }),
    };
    document.validate_event(&event).unwrap();
    (event, input)
}

/// Late failure cannot select a historical document or revoke a newer successful shown proof.
#[test]
fn accepted_history_old_failure_leaves_new_shown_frame_usable() {
    let original = Arc::new(document());
    let mut current = BTreeMap::from([("panel".into(), original.clone())]);
    let mut frames = Frames::default();
    let mut next = (*original).clone();
    next.revision = 2;
    publish(&mut frames, &mut current, next);
    let latest = &current["panel"];
    let presented = receipt(latest, "drawing", None);
    latest.validate_event(&presented).unwrap();
    let ui::Action::Canvas(action) = &presented.action else {
        panic!()
    };
    let node = latest.active_node("drawing").unwrap();
    let ui::Kind::Canvas(canvas) = &node.kind else {
        panic!()
    };
    frames
        .present(
            "panel",
            &presented,
            action,
            canvas.scene.as_ref().unwrap(),
            latest,
        )
        .unwrap();
    let failure = receipt(&original, "drawing", Some("old preparation failed".into()));
    assert!(
        frames
            .accepted_document("panel", &failure, latest)
            .is_none(),
        "old failure must reject before present can retire any frame"
    );
    assert!(
        frames.current("panel", node, latest),
        "newer successful proof must remain current"
    );
    let input = ui::CanvasInput {
        identity: 71,
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
    };
    let event = ui::UiEvent {
        revision: 2,
        node: "drawing".into(),
        action: ui::Action::Canvas(ui::CanvasEvent::Input {
            input: input.clone(),
        }),
    };
    frames
        .validate("panel", &event, &input)
        .expect("original current target remains input-eligible");
}
