//! Receipt-bearing geometry retains adjacent coalescing and exact scene boundaries.
use super::*;

fn resize(width: f32, revision: u64) -> (Work, futures::channel::oneshot::Receiver<bool>) {
    let (reply, receipt) = futures::channel::oneshot::channel();
    (
        Work::CanvasResize {
            plugin: "ordinary-provider".into(),
            epoch: 1,
            panel: "panel".into(),
            event: ui::UiEvent {
                revision,
                node: "canvas".into(),
                action: ui::Action::Canvas(ui::CanvasEvent::Resize {
                    width,
                    height: 300.,
                    grid: None,
                }),
            },
            reply,
        },
        receipt,
    )
}

/// Superseded sizes drop their receipts instead of asking native layout to replay intermediates.
#[test]
fn canvas_resize_receipts_coalesce_latest_size_without_crossing_scene() {
    let (sender, receiver) = std::sync::mpsc::channel();
    let (first, mut old) = resize(100., 1);
    let (latest, mut current) = resize(200., 1);
    let (next_scene, _next) = resize(300., 2);
    sender.send(latest).unwrap();
    sender.send(next_scene).unwrap();
    let mut next = None;
    let work = coalesce(first, &receiver, &mut next);
    assert!(
        old.try_recv().is_err(),
        "superseded sample must not request a retry"
    );
    assert!(current.try_recv().unwrap().is_none());
    let Work::CanvasResize { event, reply, .. } = work else {
        panic!("resize lost");
    };
    assert!(matches!(
        event.action,
        ui::Action::Canvas(ui::CanvasEvent::Resize { width: 200., .. })
    ));
    assert!(matches!(
        next,
        Some(Work::CanvasResize {
            event: ui::UiEvent { revision: 2, .. },
            ..
        })
    ));
    reply.send(false).unwrap();
    assert_eq!(current.try_recv().unwrap(), Some(false));
}
