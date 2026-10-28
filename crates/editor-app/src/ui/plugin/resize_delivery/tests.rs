//! Delayed actor receipts resample one live native canvas and cannot replay ordinary controls.
use super::*;
use gpui_kit::{TestAppContext, gpui};
use std::{cell::RefCell, collections::VecDeque};

type Mailbox = Rc<RefCell<VecDeque<(UiEvent, oneshot::Sender<bool>)>>>;

/// A same-size publication can overtake Resize in the actor queue. Only that rejected sample
/// is retried, using the new scene; successful delivery and subsequent paints remain silent.
#[gpui::test]
fn canvas_resize_receipt_resamples_after_scene_handover(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        editor_ui::typography::init(cx);
        crate::ui::theme::apply_theme(crate::ui::theme::builtin_theme(false), cx);
    });
    for projected in [false, true] {
        let mailbox: Mailbox = Default::default();
        let output = mailbox.clone();
        let slot = Rc::new(RefCell::new(None));
        let saved = slot.clone();
        let mut node = ui::Node::new("drawing", Kind::Canvas(ui::Canvas::default())).grow();
        if projected {
            node.viewport = Some(ui::VisualViewport {
                content: Some(ui::ContentSize {
                    width: 100.,
                    height: 50.,
                }),
                transform: None,
            });
        }
        let document = Document::new(node).revision(1);
        let (_, visual) = cx.add_window_view(move |window, cx| {
            let view = cx.new(|cx| {
                PluginView::new(
                    "ordinary-provider".into(),
                    document,
                    Environment::default(),
                    |_, _| {},
                    window,
                    cx,
                )
                .with_resize_receipts(Some(Rc::new(move |event, _| {
                    let (reply, receipt) = oneshot::channel();
                    output.borrow_mut().push_back((event, reply));
                    receipt
                })))
            });
            *saved.borrow_mut() = Some(view.clone());
            gpui_base::Root::new(view, window, cx)
        });
        visual.run_until_parked();
        let (old, rejected) = mailbox
            .borrow_mut()
            .pop_front()
            .expect("initial native size");
        assert_eq!(old.revision, 1);
        assert!(is_canvas_resize(&old.action));
        let view = slot.borrow_mut().take().unwrap();
        visual.update(|window, cx| {
            view.update(cx, |view, cx| {
                let mut document = view.document.clone();
                document.revision = 2;
                view.update_document(document, Environment::default(), window, cx);
            })
        });
        visual.run_until_parked();
        assert!(
            mailbox.borrow().is_empty(),
            "ordinary publication repeated an accepted size"
        );
        rejected.send(true).unwrap();
        visual.run_until_parked();
        let (current, accepted) = mailbox
            .borrow_mut()
            .pop_front()
            .expect("stale size must be resampled");
        assert_eq!(current.revision, 2);
        assert_eq!(current.action, old.action);
        accepted.send(false).unwrap();
        visual.run_until_parked();
        visual.update(|_, cx| view.update(cx, |_, cx| cx.notify()));
        visual.run_until_parked();
        assert!(
            mailbox.borrow().is_empty(),
            "accepted size must stop retrying"
        );
    }
}

/// A retry channel must never accept clicks, edits, navigation, scrolls or pointer gestures.
#[test]
fn canvas_resize_delivery_excludes_business_input() {
    assert!(!is_canvas_resize(&Action::Click));
    assert!(!is_canvas_resize(&Action::Change("draft".into())));
    assert!(!is_canvas_resize(&Action::Canvas(ui::CanvasEvent::Focus {
        focused: true
    })));
    assert!(!is_canvas_resize(&Action::Canvas(
        ui::CanvasEvent::Scroll { offset: 10. }
    )));
}
