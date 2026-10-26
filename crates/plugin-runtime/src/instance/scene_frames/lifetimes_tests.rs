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
