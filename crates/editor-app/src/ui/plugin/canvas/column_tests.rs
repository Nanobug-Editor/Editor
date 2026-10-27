//! A nonresponding canvas sink proves resize frames never wait for guest acknowledgement.
use super::*;
use gpui_kit::{AppContext as _, TestAppContext, gpui};
use std::cell::RefCell;

/// The native viewport may resize every frame while the worker is still drawing an older width.
#[gpui::test]
fn canvas_columns_follow_viewport_before_guest_resize_reply(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        editor_ui::typography::init(cx);
    });
    let events = Rc::new(RefCell::new(Vec::new()));
    let output = events.clone();
    let saved = Rc::new(RefCell::new(None));
    let retained = saved.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let view = cx.new(|cx| {
            CanvasView::new(
                viewport_columns(600., None),
                move |event, _, _| output.borrow_mut().push(event),
                window,
                cx,
            )
        });
        *retained.borrow_mut() = Some(view.clone());
        gpui_base::Root::new(view, window, cx)
    });
    let view = saved.borrow().as_ref().unwrap().clone();
    visual.simulate_resize(size(px(600.), px(160.)));
    visual.run_until_parked();
    events.borrow_mut().clear();
    for width in [560., 640., 590., 620.] {
        visual.simulate_resize(size(px(width), px(160.)));
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
            let view = view.read(cx);
            let last = view.painted_boxes.last().unwrap();
            assert_eq!(
                last.x + last.w,
                width - 12.,
                "the last column must keep its right inset before any worker resize reply"
            );
        });
    }
    assert!(!events.borrow().iter().any(|event| matches!(
        event,
        CanvasEvent::ColumnsResized { .. } | CanvasEvent::Pointer { .. }
    )));
    for stale_width in [560., 640., 600.] {
        visual.update(|window, cx| {
            view.update(cx, |view, _| {
                view.update_drawing(viewport_columns(stale_width, None))
            });
            window.draw(cx).clear(cx);
            let last = *view.read(cx).painted_boxes.last().unwrap();
            assert_eq!(
                last.x + last.w,
                608.,
                "late resized artwork cannot move the right anchor"
            );
        });
    }
    let start = visual.update(|_, cx| view.read(cx).bounds.origin + point(px(420.), px(40.)));
    visual.simulate_mouse_down(start, MouseButton::Left, Default::default());
    visual.simulate_mouse_move(
        start + point(px(20.), px(0.)),
        MouseButton::Left,
        Default::default(),
    );
    visual.update(|window, cx| {
        view.update(cx, |view, _| {
            view.update_drawing(viewport_columns(560., None))
        });
        window.draw(cx).clear(cx);
        assert!(
            view.read(cx).columns.dragging(),
            "a delayed reply is not an external native resize"
        );
        assert_eq!(view.read(cx).painted_boxes[2].x, 444.);
        assert_eq!(
            view.read(cx)
                .column_rect(
                    4,
                    protocol::Rect {
                        x: 0.,
                        y: 0.,
                        w: 560.,
                        h: 28.
                    }
                )
                .w,
            620.
        );
    });
    visual.simulate_mouse_up(
        start + point(px(20.), px(0.)),
        MouseButton::Left,
        Default::default(),
    );
    let releases = events
        .borrow()
        .iter()
        .filter_map(|event| match event {
            CanvasEvent::ColumnsResized { widths, .. } => Some(widths.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(releases, [vec![320., 120., 60., 112.]]);
    visual.simulate_resize(size(px(660.), px(160.)));
    visual.update(|window, cx| {
        view.update(cx, |view, _| {
            view.update_drawing(viewport_columns(620., Some([120., 60., 112.])))
        });
        window.draw(cx).clear(cx);
        assert_eq!(view.read(cx).painted_boxes[2].x, 484.);
        let last = *view.read(cx).painted_boxes.last().unwrap();
        assert_eq!(last.x + last.w, 648.);
    });
    // Reference drawings retain the preferred tail even below its total minimum. Compare all
    // metadata edges, not just the right anchor: alternate compression policies can hide there.
    for actual in [340., 260., 660., 300., 660.] {
        visual.simulate_resize(size(px(actual), px(160.)));
        let before = visual.update(|window, cx| {
            window.draw(cx).clear(cx);
            view.read(cx).painted_boxes.clone()
        });
        for reply in [600., 340., 260., 660.] {
            visual.update(|window, cx| {
                view.update(cx, |view, _| {
                    view.update_drawing(viewport_columns(reply, Some([120., 60., 112.])))
                });
                window.draw(cx).clear(cx);
                for (before, after) in before.iter().zip(&view.read(cx).painted_boxes) {
                    assert!(
                        (before.x - after.x).abs() < 0.002,
                        "reply {reply} at {actual}"
                    );
                    assert!(
                        (before.w - after.w).abs() < 0.002,
                        "reply {reply} at {actual}"
                    );
                }
            });
        }
    }
}

/// Four ordinary table columns reserve eight outer pixels and four inner pixels per side.
fn viewport_columns(width: f32, trailing: Option<[f32; 3]>) -> Canvas {
    let tail = trailing.unwrap_or([100., 80., 112.]);
    let width = width.max(104. + tail.iter().sum::<f32>() + 8.);
    let widths = vec![
        width - 8. - tail.iter().sum::<f32>(),
        tail[0],
        tail[1],
        tail[2],
    ];
    let mut x = 0.;
    let mut paint: Vec<_> = widths
        .iter()
        .map(|width| {
            let rect = protocol::Rect {
                x: x + 4.,
                y: 10.,
                w: *width - 8.,
                h: 20.,
            };
            x += *width;
            protocol::Paint::TextBox {
                rect,
                text: "Table 中文".into(),
                color: 0xffffff,
                size: 12.,
                bold: false,
                font: None,
                align: protocol::ui::Alignment::Start,
            }
        })
        .collect();
    paint.push(protocol::Paint::Fill {
        rect: protocol::Rect {
            x: 0.,
            y: 40.,
            w: width,
            h: 28.,
        },
        color: 0x123456,
        extend_to_bottom: false,
    });
    Canvas {
        column_resize: Some(protocol::ui::ColumnResize {
            epoch: 1,
            x: 0.,
            widths,
            minimum: vec![104., 40., 24., 100.],
            bindings: (0..4)
                .map(|index| protocol::ui::ColumnBinding {
                    paint: index,
                    column: index,
                })
                .collect(),
            viewport: Some(protocol::ui::ColumnViewport {
                width,
                flexible: 0,
                span_paints: vec![4],
            }),
        }),
        paint,
        ..Default::default()
    }
}

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
            viewport: None,
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
                viewport: None,
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
