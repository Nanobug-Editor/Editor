//! Native drop consent and current display geometry retain one original receiving offer.
use super::*;

/// Actual drop consent precedes materialization; the consumer's selected priority loads one representation.
#[test]
#[ignore = "opt-in actual independent Canvas Package/WASM; see fixtures/packages/README.md"]
fn native_drop_offer_is_lazy_until_a_selected_read_after_drop() {
    use plugin_runtime::plugin_protocol::transfer::{self, Format, Origin, Phase, Position};
    let mut harness = Harness::open();
    let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let revision = harness.manager.live["canvas-flow"].views["diagram"].revision;
    let offer = harness
        .manager
        .begin_transfer(
            "canvas-flow",
            "diagram",
            transfer::Address::node("drop-zone", revision),
            plugin_runtime::HostTransferOffer::new(
                Origin::OperatingSystem,
                vec![
                    Format {
                        mime: "text/plain;charset=utf-8".into(),
                        byte_len: None,
                    },
                    Format {
                        mime: "text/html".into(),
                        byte_len: None,
                    },
                ],
                std::sync::Arc::new(CountingDrop(count.clone())),
            )
            .unwrap(),
            Position { x: 3., y: 5. },
        )
        .unwrap();
    assert_eq!(count.load(std::sync::atomic::Ordering::Acquire), 0);
    let operation = || api::Operation::Editor {
        operation: api::EditorOperation::Transfer {
            operation: transfer::Operation::Read {
                offer: offer.handle.clone(),
                preferred: vec![
                    "image/png".into(),
                    "text/html".into(),
                    "text/plain;charset=utf-8".into(),
                ],
            },
        },
        timeout_ms: 30_000,
    };
    assert_eq!(
        harness
            .call(operation())
            .err()
            .expect("hover cannot authorize a payload read")
            .code,
        api::ErrorCode::InvalidState
    );
    harness
        .manager
        .update_transfer(
            "canvas-flow",
            "diagram",
            &offer.handle,
            Phase::Move,
            Position { x: 8., y: 5. },
        )
        .unwrap();
    assert_eq!(count.load(std::sync::atomic::Ordering::Acquire), 0);
    harness
        .manager
        .update_transfer(
            "canvas-flow",
            "diagram",
            &offer.handle,
            Phase::Drop,
            Position { x: 8., y: 5. },
        )
        .unwrap();
    assert!(matches!(
        harness.call(operation()).unwrap(),
        api::Value::Accepted(_)
    ));
    let request = harness.manager.take_editor_requests().pop().unwrap().1;
    assert!(request.begin());
    let resource = request.transfer_offer().unwrap();
    let plugin_runtime::HostTransferData::Bytes(item) = resource
        .read(&["text/html".into(), "text/plain;charset=utf-8".into()])
        .unwrap()
    else {
        panic!("HTML bytes expected")
    };
    assert_eq!(item.mime, "text/html");
    assert_eq!(item.bytes, b"<table><tr><td>inert</td></tr></table>");
    assert_eq!(count.load(std::sync::atomic::Ordering::Acquire), 1);
    harness
        .call(api::Operation::CloseResource {
            handle: offer.handle,
        })
        .unwrap();
    assert!(!resource.is_live());
}

/// A moving receiver reports the latest actually painted basis while retaining its original offer.
#[test]
#[ignore = "opt-in actual independent Canvas Package/WASM; see fixtures/packages/README.md"]
fn canvas_drop_follows_the_current_shown_frame_without_rebinding_its_offer() {
    use plugin_runtime::plugin_protocol::{Rect, transfer};
    let mut harness = Harness::open();
    let present = |harness: &mut Harness| {
        let document = harness.manager.live["canvas-flow"].views["diagram"].clone();
        let node = document.active_node("drawing").unwrap();
        let ui::Kind::Canvas(canvas) = &node.kind else {
            unreachable!()
        };
        let scene = canvas.scene.as_ref().unwrap();
        let address = transfer::Address {
            node: "drawing".into(),
            ui_revision: document.revision,
            source: document.source.clone(),
            file: document.file.clone(),
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
        };
        harness
            .manager
            .event(
                "canvas-flow",
                Some("diagram".into()),
                api::Notification::Ui(ui::UiEvent {
                    revision: document.revision,
                    node: "drawing".into(),
                    action: ui::Action::ViewportInput(ui::ViewportInput {
                        content: node.viewport.as_ref().unwrap().content.unwrap(),
                        event: ui::CanvasEvent::ScenePresented {
                            identity: scene.identity,
                            scene_revision: scene.revision,
                            source: document.source.clone(),
                            file: document.file.clone(),
                            projection: node.viewport.clone(),
                            error: None,
                        },
                    }),
                }),
            )
            .unwrap();
        address
    };
    let first = present(&mut harness);
    let source = plugin_runtime::HostTransferOffer::new(
        transfer::Origin::OperatingSystem,
        vec![Format {
            mime: "text/html".into(),
            byte_len: None,
        }],
        std::sync::Arc::new(CountingDrop(std::sync::Arc::new(
            std::sync::atomic::AtomicUsize::new(0),
        ))),
    )
    .unwrap();
    let offer = harness
        .manager
        .begin_transfer(
            "canvas-flow",
            "diagram",
            first,
            source,
            transfer::Position { x: 30., y: 30. },
        )
        .unwrap();
    let current = harness.manager.live["canvas-flow"].views["diagram"].revision;
    harness
        .manager
        .event(
            "canvas-flow",
            Some("diagram".into()),
            api::Notification::Ui(ui::UiEvent {
                revision: current,
                node: "pan".into(),
                action: ui::Action::Click,
            }),
        )
        .unwrap();
    let next = present(&mut harness);
    harness
        .manager
        .update_transfer_snapshot(
            "canvas-flow",
            "diagram",
            &offer.handle,
            next.clone(),
            transfer::Phase::Drop,
            transfer::Position { x: 45., y: 35. },
        )
        .unwrap();
    let mut observed = None;
    harness.manager.live["canvas-flow"].views["diagram"]
        .root
        .visit(&mut |node| {
            if node.id == "exchange"
                && let ui::Kind::Text { text } = &node.kind
            {
                observed = Some(serde_json::from_str::<transfer::Event>(text).unwrap());
            }
        });
    let observed = observed.unwrap();
    assert_eq!(observed.address, next);
    assert_eq!(observed.offer.handle, offer.handle);
    assert_eq!(observed.position, transfer::Position { x: 45., y: 35. });
}

/// OLE may revoke the data object before its queued Leave reaches the actor; cleanup still seals once.
#[test]
#[ignore = "opt-in actual independent Canvas Package/WASM; see fixtures/packages/README.md"]
fn native_leave_seals_the_original_offer_after_os_data_revocation() {
    use plugin_runtime::plugin_protocol::transfer;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };
    let mut harness = Harness::open();
    let alive = Arc::new(AtomicBool::new(true));
    let count = Arc::new(AtomicUsize::new(0));
    let source = plugin_runtime::HostTransferOffer::new(
        transfer::Origin::OperatingSystem,
        vec![Format {
            mime: "text/html".into(),
            byte_len: None,
        }],
        Arc::new(CountingDrop(count.clone())),
    )
    .unwrap()
    .with_native_lifetime(alive.clone());
    let revision = harness.manager.live["canvas-flow"].views["diagram"].revision;
    let offer = harness
        .manager
        .begin_transfer(
            "canvas-flow",
            "diagram",
            transfer::Address::node("drop-zone", revision),
            source,
            transfer::Position { x: 1., y: 2. },
        )
        .unwrap();
    alive.store(false, Ordering::Release);
    harness
        .manager
        .update_transfer(
            "canvas-flow",
            "diagram",
            &offer.handle,
            transfer::Phase::Leave,
            transfer::Position { x: 3., y: 4. },
        )
        .expect("native data retirement must not swallow its terminal Leave");
    let mut observed = None;
    harness.manager.live["canvas-flow"].views["diagram"]
        .root
        .visit(&mut |node| {
            if node.id == "exchange"
                && let ui::Kind::Text { text } = &node.kind
            {
                observed = Some(serde_json::from_str::<transfer::Event>(text).unwrap());
            }
        });
    assert_eq!(observed.unwrap().phase, transfer::Phase::Leave);
    assert_eq!(count.load(Ordering::Acquire), 0);
    assert!(
        harness
            .manager
            .update_transfer(
                "canvas-flow",
                "diagram",
                &offer.handle,
                transfer::Phase::Leave,
                transfer::Position { x: 3., y: 4. }
            )
            .is_err()
    );
}

/// An unchanged ordinary receiver keeps its shown native model while unrelated UI output is accepted.
#[test]
#[ignore = "opt-in actual independent Canvas Package/WASM; see fixtures/packages/README.md"]
fn native_drop_snapshot_cannot_borrow_an_unshown_replacement_declaration() {
    use plugin_runtime::plugin_protocol::transfer;
    use std::sync::{Arc, atomic::AtomicUsize};
    let mut harness = Harness::open();
    let shown = harness.manager.live["canvas-flow"].views["diagram"].clone();
    let expected = shown
        .active_node("drop-zone")
        .unwrap()
        .drop_target
        .clone()
        .unwrap();
    let _ = harness.clipboard(ClipboardOperation::Enumerate);
    assert!(harness.manager.live["canvas-flow"].views["diagram"].revision > shown.revision);
    let source = || {
        plugin_runtime::HostTransferOffer::new(
            transfer::Origin::OperatingSystem,
            vec![Format {
                mime: "text/html".into(),
                byte_len: None,
            }],
            Arc::new(CountingDrop(Arc::new(AtomicUsize::new(0)))),
        )
        .unwrap()
    };
    let address = transfer::Address::node("drop-zone", shown.revision);
    let mut forged = expected.clone();
    forged.preferred = vec!["text/plain;charset=utf-8".into()];
    assert!(
        harness
            .manager
            .begin_transfer_snapshot(
                "canvas-flow",
                "diagram",
                address.clone(),
                forged,
                source(),
                transfer::Position { x: 1., y: 2. }
            )
            .is_err()
    );
    let offer = harness
        .manager
        .begin_transfer_snapshot(
            "canvas-flow",
            "diagram",
            address,
            expected,
            source(),
            transfer::Position { x: 1., y: 2. },
        )
        .unwrap();
    harness
        .manager
        .update_transfer(
            "canvas-flow",
            "diagram",
            &offer.handle,
            transfer::Phase::Leave,
            transfer::Position { x: 2., y: 3. },
        )
        .unwrap();
}
