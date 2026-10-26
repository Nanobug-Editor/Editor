//! Native clipboard metadata, ordered selection and instance isolation through an actual guest.
use super::*;

/// Metadata selection is lazy and ordered; foreign/closed roots cannot borrow native clipboard authority.
#[test]
#[ignore = "opt-in actual independent Canvas Package/WASM; see fixtures/packages/README.md"]
fn clipboard_offers_are_lazy_prioritized_and_instance_scoped() {
    let mut first = Harness::open();
    let mut second = Harness::open();
    let call = first.clipboard(ClipboardOperation::Enumerate);
    assert!(call.begin());
    let offer = call.clipboard_offer().unwrap().clone();
    let descriptor = offer
        .publish(
            19,
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
        )
        .unwrap();
    // Native enumeration has supplied no bytes and no reader. Only its metadata is needed here.
    call.finish(Ok(api::EditorValue::ClipboardFormats(
        plugin_runtime::plugin_protocol::transfer::ClipboardValue::Offer(descriptor.clone()),
    )));
    first.manager.poll();
    let read = first.clipboard(ClipboardOperation::Read {
        offer: descriptor.handle.clone(),
        preferred: vec![
            "image/png".into(),
            "text/html".into(),
            "text/plain;charset=utf-8".into(),
        ],
    });
    assert_eq!(
        read.clipboard_offer()
            .unwrap()
            .select(&["text/html".into(), "text/plain;charset=utf-8".into()])
            .unwrap(),
        (19, "text/html".into())
    );
    let foreign = second
        .call(api::Operation::Editor {
            operation: api::EditorOperation::Clipboard {
                operation: ClipboardOperation::Read {
                    offer: descriptor.handle.clone(),
                    preferred: vec!["text/html".into()],
                },
            },
            timeout_ms: 30_000,
        })
        .err()
        .expect("foreign offer must be rejected");
    assert_eq!(foreign.code, api::ErrorCode::InvalidHandle);
    first
        .call(api::Operation::CloseResource {
            handle: descriptor.handle.clone(),
        })
        .unwrap();
    assert!(!offer.is_live());
    assert!(
        read.clipboard_offer()
            .unwrap()
            .select(&["text/html".into()])
            .is_err(),
        "a queued reader cannot revive an explicitly closed offer"
    );
    let closed = first
        .call(api::Operation::Editor {
            operation: api::EditorOperation::Clipboard {
                operation: ClipboardOperation::Read {
                    offer: descriptor.handle,
                    preferred: vec!["text/html".into()],
                },
            },
            timeout_ms: 30_000,
        })
        .err()
        .expect("released offer must be rejected");
    assert_eq!(closed.code, api::ErrorCode::InvalidHandle);
}

/// Failed native enumeration never delivers a handle, so it must release its reserved offer before retry.
#[test]
#[ignore = "opt-in actual independent Canvas Package/WASM; see fixtures/packages/README.md"]
fn failed_clipboard_enumeration_releases_each_undelivered_offer() {
    let mut harness = Harness::open();
    for _ in 0..20 {
        let request = harness.clipboard(ClipboardOperation::Enumerate);
        assert!(request.begin());
        let lease = request.clipboard_offer().unwrap().clone();
        request.finish(Err(api::Failure::new(
            api::ErrorCode::OperationFailed,
            "native clipboard temporarily busy",
        )));
        harness.manager.poll();
        assert!(
            !lease.is_live(),
            "an undelivered failed enumeration cannot retain authority or quota"
        );
        assert!(
            lease
                .publish(
                    1,
                    vec![Format {
                        mime: "text/html".into(),
                        byte_len: None
                    }]
                )
                .is_err(),
            "a late native publisher cannot resurrect failed enumeration"
        );
    }
    // Both in-flight and completed-but-undelivered requests own the same hidden offer.
    for attempt in 0..20 {
        let request = harness.clipboard(ClipboardOperation::Enumerate);
        assert!(request.begin());
        let lease = request.clipboard_offer().unwrap().clone();
        if attempt % 2 == 1 {
            let descriptor = lease
                .publish(
                    1,
                    vec![Format {
                        mime: "text/html".into(),
                        byte_len: None,
                    }],
                )
                .unwrap();
            request.finish(Ok(api::EditorValue::ClipboardFormats(
                plugin_runtime::plugin_protocol::transfer::ClipboardValue::Offer(descriptor),
            )));
        }
        harness
            .call(api::Operation::CloseResource {
                handle: request.handle().clone(),
            })
            .unwrap();
        assert!(
            !lease.is_live(),
            "closing an enumeration request releases its undelivered offer"
        );
        assert!(
            lease
                .publish(
                    1,
                    vec![Format {
                        mime: "text/html".into(),
                        byte_len: None,
                    }],
                )
                .is_err(),
            "a late publisher cannot resurrect a closed enumeration"
        );
    }
    let request = harness.clipboard(ClipboardOperation::Enumerate);
    assert!(request.begin());
    let lease = request.clipboard_offer().unwrap().clone();
    let descriptor = lease
        .publish(
            2,
            vec![Format {
                mime: "text/html".into(),
                byte_len: None,
            }],
        )
        .unwrap();
    request.finish(Ok(api::EditorValue::ClipboardFormats(
        plugin_runtime::plugin_protocol::transfer::ClipboardValue::Offer(descriptor.clone()),
    )));
    harness.manager.poll();
    let read = harness.clipboard(ClipboardOperation::Read {
        offer: descriptor.handle.clone(),
        preferred: vec!["text/html".into()],
    });
    assert!(read.begin());
    read.finish(Err(api::Failure::new(
        api::ErrorCode::OperationFailed,
        "native selected read temporarily failed",
    )));
    harness.manager.poll();
    assert!(
        lease.is_live(),
        "failed Read cannot withdraw its caller's existing delivered offer"
    );
    assert_eq!(
        lease.select(&["text/html".into()]).unwrap(),
        (2, "text/html".into())
    );
    harness
        .call(api::Operation::CloseResource {
            handle: descriptor.handle,
        })
        .unwrap();
}
