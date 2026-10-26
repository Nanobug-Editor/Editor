//! Native outgoing sources retain package ownership, actual frame proofs and nested lifetimes.
use super::*;

/// Only native drag admission exports the declared bytes; guests cannot turn that private root into a read.
#[test]
#[ignore = "opt-in actual independent Canvas Package/WASM; see fixtures/packages/README.md"]
fn native_drag_source_keeps_package_ownership_and_reports_one_terminal_result() {
    use plugin_runtime::plugin_protocol::transfer::{self, Origin};
    let mut harness = Harness::open();
    let revision = harness.manager.live["canvas-flow"].views["diagram"].revision;
    let alive = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let source = harness
        .manager
        .begin_drag(
            "canvas-flow",
            "diagram",
            transfer::Address::node("drag-source", revision),
            alive.clone(),
        )
        .unwrap();
    assert_eq!(
        source.metadata().source,
        Origin::Plugin {
            package: "canvas-flow".into()
        }
    );
    let plugin_runtime::HostTransferData::Bytes(item) = source.read(&["text/html".into()]).unwrap()
    else {
        panic!("HTML expected")
    };
    assert_eq!(item.bytes, b"<table><tr><td>Flow</td></tr></table>");
    assert_eq!(
        harness
            .call(api::Operation::Editor {
                operation: api::EditorOperation::Transfer {
                    operation: transfer::Operation::Read {
                        offer: source.metadata().handle.clone(),
                        preferred: vec!["text/html".into()]
                    }
                },
                timeout_ms: 30_000
            })
            .unwrap_err()
            .code,
        api::ErrorCode::InvalidHandle
    );
    alive.store(false, std::sync::atomic::Ordering::Release);
    assert!(!source.is_live());
    harness
        .manager
        .finish_drag(
            "canvas-flow",
            "diagram",
            &source.metadata().handle,
            transfer::DragOutcome::Cancelled,
        )
        .unwrap();
    let mut result = None;
    harness.manager.live["canvas-flow"].views["diagram"]
        .root
        .visit(&mut |node| {
            if node.id == "drag-result"
                && let ui::Kind::Text { text } = &node.kind
            {
                result = Some(text.clone());
            }
        });
    assert_eq!(result.as_deref(), Some("cancelled"));
    assert!(
        harness
            .manager
            .finish_drag(
                "canvas-flow",
                "diagram",
                &source.metadata().handle,
                transfer::DragOutcome::Dropped
            )
            .is_err()
    );
}

/// An OS drop can precede the receiver's selected read; completion must not tear down its delayed producer.
#[test]
#[ignore = "opt-in actual independent Canvas Package/WASM; see fixtures/packages/README.md"]
fn completed_os_copy_keeps_only_its_bounded_delayed_read_lease() {
    use plugin_runtime::plugin_protocol::transfer::{self, DragOutcome};
    let mut harness = Harness::open();
    let revision = harness.manager.live["canvas-flow"].views["diagram"].revision;
    let source = harness
        .manager
        .begin_drag(
            "canvas-flow",
            "diagram",
            transfer::Address::node("drag-source", revision),
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
        )
        .unwrap();
    let handle = source.metadata().handle;
    harness
        .manager
        .finish_drag("canvas-flow", "diagram", &handle, DragOutcome::Dropped)
        .unwrap();
    assert!(
        source.is_live(),
        "successful OS completion must allow a receiver's delayed selected read"
    );
    let plugin_runtime::HostTransferData::Bytes(item) =
        source.read(&["text/plain;charset=utf-8".into()]).unwrap()
    else {
        panic!("text expected")
    };
    assert_eq!(item.bytes, b"Flow\tnode");
    assert!(
        harness
            .manager
            .finish_drag("canvas-flow", "diagram", &handle, DragOutcome::Dropped)
            .is_err()
    );
    harness
        .call(api::Operation::CloseResource { handle })
        .unwrap();
    assert!(!source.is_live());
}

/// Native source evidence describes the last shown frame while a newer drawing may still be queued.
#[test]
#[ignore = "opt-in actual independent Canvas Package/WASM; see fixtures/packages/README.md"]
fn canvas_drag_snapshot_uses_shown_geometry_without_borrowing_new_source_data() {
    use plugin_runtime::plugin_protocol::transfer;
    let mut harness = Harness::open();
    let shown = harness.manager.live["canvas-flow"].views["diagram"].clone();
    let node = shown.active_node("drawing").unwrap();
    let ui::Kind::Canvas(canvas) = &node.kind else {
        unreachable!()
    };
    let scene = canvas.scene.as_ref().unwrap();
    let target = scene
        .targets
        .iter()
        .find(|target| target.id == "start")
        .unwrap();
    let expected = target
        .drag_source
        .clone()
        .expect("actual flow declares native source formats");
    let address = transfer::Address {
        node: "drawing".into(),
        ui_revision: shown.revision,
        source: shown.source.clone(),
        file: shown.file.clone(),
        canvas: Some(ui::CanvasAnchor {
            identity: scene.identity,
            scene_revision: scene.revision,
            target: ui::CanvasTargetRef {
                id: target.id.clone(),
                epoch: target.epoch,
            },
            space: None,
            rect: plugin_runtime::plugin_protocol::Rect {
                x: 20.,
                y: 20.,
                w: 100.,
                h: 60.,
            },
        }),
    };
    harness
        .manager
        .event(
            "canvas-flow",
            Some("diagram".into()),
            api::Notification::Ui(ui::UiEvent {
                revision: shown.revision,
                node: "drawing".into(),
                action: ui::Action::ViewportInput(ui::ViewportInput {
                    content: node.viewport.as_ref().unwrap().content.unwrap(),
                    event: ui::CanvasEvent::ScenePresented {
                        identity: scene.identity,
                        scene_revision: scene.revision,
                        source: shown.source.clone(),
                        file: shown.file.clone(),
                        projection: node.viewport.clone(),
                        error: None,
                    },
                }),
            }),
        )
        .unwrap();
    harness
        .manager
        .event(
            "canvas-flow",
            Some("diagram".into()),
            api::Notification::Ui(ui::UiEvent {
                revision: shown.revision,
                node: "pan".into(),
                action: ui::Action::Click,
            }),
        )
        .unwrap();
    assert!(harness.manager.live["canvas-flow"].views["diagram"].revision > shown.revision);
    let source = harness
        .manager
        .begin_drag_snapshot(
            "canvas-flow",
            "diagram",
            address.clone(),
            expected.clone(),
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
        )
        .unwrap();
    harness
        .manager
        .finish_drag(
            "canvas-flow",
            "diagram",
            &source.metadata().handle,
            transfer::DragOutcome::Cancelled,
        )
        .unwrap();
    let mut forged = expected;
    forged.items[0].bytes = b"unseen replacement".to_vec();
    assert_eq!(
        harness
            .manager
            .begin_drag_snapshot(
                "canvas-flow",
                "diagram",
                address,
                forged,
                std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true))
            )
            .err()
            .unwrap()
            .code,
        api::ErrorCode::StaleRevision,
        "a native frame cannot authorize different source representations"
    );
}

/// Reusing a child scene ID never carries focus, frame proofs or transfer data across an outer target epoch.
#[test]
#[ignore = "opt-in actual independent Canvas Package/WASM; see fixtures/packages/README.md"]
fn nested_canvas_replacement_retires_original_native_authority() {
    use plugin_runtime::plugin_protocol::{Rect, transfer};
    use std::sync::{Arc, atomic::AtomicBool};
    let mut harness = Harness::open();
    let click = |harness: &mut Harness, node: &str| {
        let revision = harness.manager.live["canvas-flow"].views["diagram"].revision;
        harness
            .manager
            .event(
                "canvas-flow",
                Some("diagram".into()),
                api::Notification::Ui(ui::UiEvent {
                    revision,
                    node: node.into(),
                    action: ui::Action::Click,
                }),
            )
            .unwrap();
    };
    click(&mut harness, "nested");
    let present = |harness: &mut Harness| {
        let document = harness.manager.live["canvas-flow"].views["diagram"].clone();
        let ui::Kind::Canvas(canvas) = &document.active_node("nested-drawing").unwrap().kind else {
            unreachable!()
        };
        let scene = canvas.scene.as_ref().unwrap();
        harness
            .manager
            .event(
                "canvas-flow",
                Some("diagram".into()),
                api::Notification::Ui(ui::UiEvent {
                    revision: document.revision,
                    node: "nested-drawing".into(),
                    action: ui::Action::Canvas(ui::CanvasEvent::ScenePresented {
                        identity: scene.identity,
                        scene_revision: scene.revision,
                        source: None,
                        file: None,
                        projection: None,
                        error: None,
                    }),
                }),
            )
            .unwrap();
        let expected = scene.targets[0].drag_source.clone().unwrap();
        (document, expected)
    };
    let (old, expected) = present(&mut harness);
    let input = |document: &ui::Document, sequence, kind| {
        let ui::Kind::Canvas(canvas) = &document.active_node("nested-drawing").unwrap().kind else {
            unreachable!()
        };
        let scene = canvas.scene.as_ref().unwrap();
        ui::UiEvent {
            revision: document.revision,
            node: "nested-drawing".into(),
            action: ui::Action::Canvas(ui::CanvasEvent::Input {
                input: ui::CanvasInput {
                    identity: scene.identity,
                    presented_revision: scene.revision,
                    target: "start".into(),
                    epoch: 1,
                    sequence,
                    source: None,
                    file: None,
                    projection: None,
                    kind,
                },
            }),
        }
    };
    harness
        .manager
        .event(
            "canvas-flow",
            Some("diagram".into()),
            api::Notification::Ui(input(&old, 1, ui::CanvasInputKind::Focus { focused: true })),
        )
        .unwrap();
    let ui::Kind::Canvas(canvas) = &old.active_node("nested-drawing").unwrap().kind else {
        unreachable!()
    };
    let scene = canvas.scene.as_ref().unwrap();
    let source = harness
        .manager
        .begin_drag_snapshot(
            "canvas-flow",
            "diagram",
            transfer::Address {
                node: "nested-drawing".into(),
                ui_revision: old.revision,
                source: None,
                file: None,
                canvas: Some(ui::CanvasAnchor {
                    identity: scene.identity,
                    scene_revision: scene.revision,
                    target: ui::CanvasTargetRef {
                        id: "start".into(),
                        epoch: 1,
                    },
                    space: None,
                    rect: Rect {
                        x: 20.,
                        y: 20.,
                        w: 100.,
                        h: 60.,
                    },
                }),
            },
            expected,
            Arc::new(AtomicBool::new(true)),
        )
        .unwrap();
    click(&mut harness, "fresh-target");
    assert!(
        !source.is_live(),
        "outer target replacement must withdraw the nested source lease before another paint"
    );
    assert!(
        harness
            .manager
            .event(
                "canvas-flow",
                Some("diagram".into()),
                api::Notification::Ui(input(&old, 2, ui::CanvasInputKind::Focus { focused: true }))
            )
            .is_err()
    );
    let (next, _) = present(&mut harness);
    harness
        .manager
        .event(
            "canvas-flow",
            Some("diagram".into()),
            api::Notification::Ui(input(
                &next,
                3,
                ui::CanvasInputKind::Focus { focused: true },
            )),
        )
        .unwrap();
    harness
        .manager
        .event(
            "canvas-flow",
            Some("diagram".into()),
            api::Notification::Ui(input(
                &old,
                4,
                ui::CanvasInputKind::Focus { focused: false },
            )),
        )
        .unwrap();
    harness
        .manager
        .event(
            "canvas-flow",
            Some("diagram".into()),
            api::Notification::Ui(input(
                &next,
                5,
                ui::CanvasInputKind::Selection {
                    text_revision: 1,
                    base_sequence: 0,
                    range: ui::CanvasTextRange { start: 0, end: 0 },
                },
            )),
        )
        .expect("old cleanup cannot clear the newly presented nested target's focus");
}
