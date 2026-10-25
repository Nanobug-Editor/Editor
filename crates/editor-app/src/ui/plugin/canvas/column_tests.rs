//! A nonresponding canvas sink proves resize frames never wait for guest acknowledgement.
use super::*;
use gpui_kit::{AppContext as _, TestAppContext, gpui};
use std::cell::RefCell;

/// Native pointer routing, actual paint rectangles and late replies share one public declaration.
#[gpui::test]
fn canvas_columns_paint_every_move_without_guest_roundtrips(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        editor_ui::typography::init(cx);
    });
    let drawing = Canvas {
        column_resize: Some(protocol::ui::ColumnResize {
            epoch: 1,
            x: 0.,
            widths: vec![200., 200.],
            minimum: vec![80., 80.],
            bindings: vec![
                protocol::ui::ColumnBinding {
                    paint: 0,
                    column: 0,
                },
                protocol::ui::ColumnBinding {
                    paint: 1,
                    column: 1,
                },
            ],
        }),
        paint: (0..2)
            .map(|index| protocol::Paint::TextBox {
                rect: protocol::Rect {
                    x: index as f32 * 200. + 4.,
                    y: 10.,
                    w: 192.,
                    h: 20.,
                },
                text: "Native 中文 column".into(),
                color: 0xffffff,
                size: 12.,
                bold: false,
                font: None,
                align: protocol::ui::Alignment::Start,
            })
            .collect(),
        ..Default::default()
    };
    let events = Rc::new(RefCell::new(Vec::new()));
    let output = events.clone();
    let saved = Rc::new(RefCell::new(None));
    let retained = saved.clone();
    let initial = drawing.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let view = cx.new(|cx| {
            CanvasView::new(
                initial,
                move |event, _, _| output.borrow_mut().push(event),
                window,
                cx,
            )
        });
        *retained.borrow_mut() = Some(view.clone());
        gpui_base::Root::new(view, window, cx)
    });
    visual.run_until_parked();
    let view = saved.borrow().as_ref().unwrap().clone();
    let start = visual.update(|window, cx| {
        window.draw(cx).clear(cx);
        view.read(cx).bounds.origin + point(px(200.), px(40.))
    });
    events.borrow_mut().clear();
    visual.simulate_mouse_down(start, MouseButton::Left, Default::default());
    for delta in 1..=12 {
        visual.simulate_mouse_move(
            start + point(px(delta as f32), px(0.)),
            MouseButton::Left,
            Default::default(),
        );
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
            assert!(
                (view.read(cx).painted_boxes[0].w - (192. + delta as f32)).abs() < 0.01,
                "paint must follow this frame's pointer without a guest reply"
            );
        });
    }
    assert!(!events.borrow().iter().any(|e| matches!(
        e,
        CanvasEvent::Pointer { .. } | CanvasEvent::ColumnsResized { .. }
    )));
    visual.simulate_mouse_up(
        start + point(px(12.), px(0.)),
        MouseButton::Left,
        Default::default(),
    );
    assert_eq!(
        events
            .borrow()
            .iter()
            .filter(|e| matches!(e, CanvasEvent::ColumnsResized { .. }))
            .count(),
        1
    );
    visual.update(|window, cx| {
        view.update(cx, |view, _| view.update_drawing(drawing.clone()));
        window.draw(cx).clear(cx);
        assert_eq!(
            view.read(cx).painted_boxes[0].w,
            204.,
            "old artwork must not roll back an unacknowledged release"
        );
    });
}

/// A new layout cannot reinterpret the old local press anchor, even when the surface size is stable.
#[gpui::test]
fn repositioned_canvas_cancels_its_native_column_gesture(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        editor_ui::typography::init(cx);
    });
    let events = Rc::new(RefCell::new(Vec::new()));
    let output = events.clone();
    let retained = Rc::new(RefCell::new(None));
    let saved = retained.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let view = cx.new(|cx| {
            PluginView::new(
                "ordinary".into(),
                relocated_document(0., 1),
                Environment::default(),
                move |event, _| output.borrow_mut().push(event),
                window,
                cx,
            )
        });
        *saved.borrow_mut() = Some(view.clone());
        gpui_base::Root::new(view, window, cx)
    });
    visual.simulate_resize(size(px(600.), px(160.)));
    visual.run_until_parked();
    let view = retained.borrow().as_ref().unwrap().clone();
    let before = visual.update(|window, cx| {
        view.read(cx)
            .native_canvas_input_snapshot("drawing", window, cx)
            .unwrap()
    });
    let start = point(
        px(before["origin_x"].as_f64().unwrap() as f32 + 200.),
        px(before["origin_y"].as_f64().unwrap() as f32 + 30.),
    );
    visual.simulate_mouse_down(start, MouseButton::Left, Default::default());
    visual.simulate_mouse_move(
        start + point(px(20.), px(0.)),
        MouseButton::Left,
        Default::default(),
    );
    visual.run_until_parked();
    events.borrow_mut().clear();
    visual.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.update_document(
                relocated_document(80., 2),
                Environment::default(),
                window,
                cx,
            );
        })
    });
    visual.run_until_parked();
    let after = visual.update(|window, cx| {
        view.read(cx)
            .native_canvas_input_snapshot("drawing", window, cx)
            .unwrap()
    });
    assert_eq!(
        after["origin_x"].as_f64().unwrap() - before["origin_x"].as_f64().unwrap(),
        80.
    );
    assert_eq!(after["width"], before["width"]);
    assert_eq!(after["height"], before["height"]);
    visual.simulate_mouse_move(
        start + point(px(20.), px(0.)),
        MouseButton::Left,
        Default::default(),
    );
    visual.simulate_mouse_up(
        start + point(px(20.), px(0.)),
        MouseButton::Left,
        Default::default(),
    );
    visual.run_until_parked();
    assert!(
        !events.borrow().iter().any(|event| matches!(
            event.action,
            protocol::ui::Action::Canvas(CanvasEvent::ColumnsResized { .. })
        )),
        "a displaced surface must cancel, not commit a reinterpreted press anchor"
    );
}

/// A fixed-size public Canvas moves only through its preceding sibling's layout width.
fn relocated_document(left: f32, revision: u64) -> Document {
    let drawing = protocol::ui::Node::new(
        "drawing",
        Kind::Canvas(Canvas {
            column_resize: Some(protocol::ui::ColumnResize {
                epoch: 1,
                x: 0.,
                widths: vec![200., 200.],
                minimum: vec![80., 80.],
                bindings: vec![],
            }),
            ..Default::default()
        }),
    )
    .width(400.)
    .height(120.);
    let document = Document::new(protocol::ui::Node::row(
        "row",
        vec![
            protocol::ui::Node::text("spacer", "")
                .width(left)
                .height(120.),
            drawing,
        ],
    ))
    .revision(revision);
    document.validate().unwrap();
    document
}
