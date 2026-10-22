//! A generic plugin canvas must paint its rail inside its own viewport and retain scroll authority.
use editor_ui::theme::ActiveTheme as _;
use gpui_kit::{
    Bounds, Context, Entity, IntoElement, MouseButton, ParentElement as _, Pixels, Point, Render,
    TestAppContext, VisualTestContext, Window, div, gpui, point, prelude::*, px, size,
};
use plugin_schema::ThemeComponent;
use std::rc::Rc;

/// Apply the same production palette and scrollbar projection used by the host.
fn init(cx: &mut TestAppContext, dark: bool) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        editor_ui::typography::init(cx);
        editor_ui::theme::apply_theme(editor_ui::theme::builtin_theme(dark), cx);
        editor_ui::init(cx);
    });
}

/// Complete one native frame after an ordinary input event.
fn frame(ui: &mut VisualTestContext, millis: u64) {
    ui.executor()
        .advance_clock(std::time::Duration::from_millis(millis));
    ui.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
}

/// Read the actual scrollbar quad inside its canvas; no rail coordinates are invented.
fn thumb(
    ui: &mut VisualTestContext,
    viewport: Bounds<Pixels>,
    _: bool,
) -> Option<(f32, f32, Point<Pixels>)> {
    ui.update(|window, cx| {
        let scale = window.scale_factor();
        let palette = editor_ui::theme::component_styles(cx, ThemeComponent::Scrollbar);
        let color = palette
            .base
            .background
            .unwrap_or(cx.theme().primary)
            .opacity(0.55);
        window.painted_quads().into_iter().find_map(|quad| {
            if quad.background.as_solid() != Some(color) {
                return None;
            }
            let bounds = Bounds::new(
                point(
                    px(quad.bounds.left().0 / scale),
                    px(quad.bounds.top().0 / scale),
                ),
                size(
                    px(quad.bounds.size.width.0 / scale),
                    px(quad.bounds.size.height.0 / scale),
                ),
            );
            let width = bounds.size.width / px(1.);
            (width > 0.
                && width <= 8.
                && bounds.size.height >= px(20.)
                && viewport.contains(&bounds.center()))
            .then_some((width, color.a, bounds.center()))
        })
    })
}

/// Draw after the framework has applied startup palette work.
fn configure(ui: &mut VisualTestContext) {
    frame(ui, 0);
}

/// Holding guest replies must not hold a retained scrolling row at its previous screen position.
#[gpui::test]
fn scrollbar_canvas_paints_motion_before_guest_reply(cx: &mut TestAppContext) {
    use gpui_kit::{ScrollDelta, ScrollWheelEvent, TouchPhase};
    use plugin_runtime::plugin_protocol::{
        Paint, Rect,
        ui::{Canvas, CanvasEvent, PointerPhase, ScrollBuffer, ScrollRange},
    };
    init(cx, false);
    let events = Rc::new(std::cell::RefCell::new(Vec::new()));
    let sink = events.clone();
    let slot = Rc::new(std::cell::RefCell::new(None));
    let capture = slot.clone();
    let (_, ui) = cx.add_window_view(move |window, cx| {
        let canvas = cx.new(|cx| {
            crate::ui::plugin::CanvasView::new(
                Canvas {
                    scroll: Some(ScrollRange {
                        content: 2000.,
                        offset: 0.,
                    }),
                    scroll_epoch: Some(42),
                    scroll_buffer: Some(ScrollBuffer {
                        start: 0.,
                        end: 600.,
                    }),
                    paint: vec![Paint::Fill {
                        rect: Rect {
                            x: 20.,
                            y: 80.,
                            w: 120.,
                            h: 10.,
                        },
                        color: 0xff6600,
                        extend_to_bottom: false,
                    }],
                    ..Default::default()
                },
                move |event, _, _| {
                    sink.borrow_mut().push(event);
                }, // Defer guest acknowledgement.
                window,
                cx,
            )
        });
        capture.replace(Some(canvas.clone()));
        let host = cx.new(|_| CanvasHost { canvas });
        gpui_base::Root::new(host, window, cx)
    });
    frame(ui, 0);
    for expected in [118., 101., 84.] {
        ui.simulate_event(ScrollWheelEvent {
            position: point(px(80.), px(100.)),
            delta: ScrollDelta::Pixels(point(px(0.), px(-17.))),
            modifiers: Default::default(),
            touch_phase: TouchPhase::Moved,
        });
        frame(ui, 7);
        let top = ui
            .update(|window, _| {
                window
                    .painted_quads()
                    .iter()
                    .find(|quad| quad.background.as_solid() == Some(gpui_kit::rgb(0xff6600).into()))
                    .map(|quad| quad.bounds.top().0 / window.scale_factor())
            })
            .expect("the visible row must remain painted");
        assert!(
            (top - expected).abs() < 0.5,
            "row stayed at {top}, native frame requires {expected}"
        );
    }
    let canvas = slot.borrow_mut().take().unwrap();
    ui.update(|_, cx| {
        canvas.update(cx, |canvas, cx| {
            let mut drawing = canvas.drawing.clone();
            drawing.scroll.as_mut().unwrap().offset = 17.;
            if let Paint::Fill { rect, .. } = &mut drawing.paint[0] {
                rect.y -= 17.;
            }
            canvas.update_drawing(drawing);
            cx.notify();
        })
    });
    frame(ui, 7);
    ui.simulate_mouse_down(
        point(px(40.), px(90.)),
        MouseButton::Left,
        Default::default(),
    );
    assert!(events.borrow().iter().any(|event| matches!(event,
        CanvasEvent::Pointer { phase: PointerPhase::Down, y, .. } if (*y + 17. - 86.).abs() < 0.5
    )), "pointer must target the visible content after a delayed acknowledgement");
}

/// A canvas after a header must paint its rail inside its own viewport and emit public drag input.
#[gpui::test]
fn scrollbar_canvas_overlay_is_inside_viewport_and_drag_uses_content_epoch(
    cx: &mut TestAppContext,
) {
    use crate::ui::plugin::CanvasView;
    use plugin_runtime::plugin_protocol::ui::{Canvas, CanvasEvent, ScrollChange, ScrollRange};
    for dark in [false, true] {
        init(cx, dark);
        let events = Rc::new(std::cell::RefCell::new(Vec::new()));
        let sink = events.clone();
        let (_, ui) = cx.add_window_view(move |window, cx| {
            let canvas = cx.new(|cx| {
                CanvasView::new(
                    Canvas {
                        scroll: Some(ScrollRange {
                            content: 1200.,
                            offset: 0.,
                        }),
                        scroll_epoch: Some(42),
                        ..Default::default()
                    },
                    move |event, _, _| sink.borrow_mut().push(event),
                    window,
                    cx,
                )
            });
            CanvasHost { canvas }
        });
        configure(ui);
        let viewport = Bounds::new(point(px(0.), px(55.)), size(px(300.), px(200.)));
        let actual = thumb(ui, viewport, false)
            .expect("the trailing canvas overlay must be inside the viewport");
        assert_eq!(actual.0, 6.);
        ui.simulate_mouse_move(actual.2, None, Default::default());
        ui.simulate_mouse_down(actual.2, MouseButton::Left, Default::default());
        let destination = actual.2 + point(px(0.), px(60.));
        ui.simulate_mouse_move(destination, Some(MouseButton::Left), Default::default());
        frame(ui, 0);
        ui.simulate_mouse_up(destination, MouseButton::Left, Default::default());
        frame(ui, 0);
        assert!(events.borrow().iter().any(|event| matches!(event,
            CanvasEvent::ScrollInput { epoch: 42, change: ScrollChange::To { offset } } if *offset > 0. && *offset <= 1000.
        )), "dragging the native rail must preserve the public content epoch");
    }
}

/// The same trailing-child composition as Git and terminal canvases, with a measurable header.
struct CanvasHost {
    /// Original generic canvas entity owns its bounds and admitted event sink.
    canvas: Entity<crate::ui::plugin::CanvasView>,
}

impl Render for CanvasHost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(300.))
            .h(px(255.))
            .flex()
            .flex_col()
            .child(div().h(px(55.)).flex_shrink_0())
            .child(
                div()
                    .relative()
                    .h(px(200.))
                    .flex_shrink_0()
                    .child(self.canvas.clone()),
            )
    }
}

/// Common kinetic motion reaches a generic guest canvas through its public range contract.
/// Precise input, reserved domain wheels and replacement epochs retain their original behavior.
#[gpui::test]
fn scrollbar_canvas_common_smooth_motion_precise_input_and_epoch_retirement(
    cx: &mut TestAppContext,
) {
    use gpui_kit::{ScrollDelta, ScrollWheelEvent, TouchPhase};
    use plugin_runtime::plugin_protocol::ui::{Canvas, CanvasEvent, ScrollChange, ScrollRange};
    init(cx, false);
    let events = Rc::new(std::cell::RefCell::new(Vec::new()));
    let sink = events.clone();
    let slot = Rc::new(std::cell::RefCell::new(None));
    let capture = slot.clone();
    let (_, ui) = cx.add_window_view(move |window, cx| {
        let canvas = cx.new(|cx| {
            crate::ui::plugin::CanvasView::new(
                Canvas {
                    scroll: Some(ScrollRange {
                        content: 20_000.,
                        offset: 0.,
                    }),
                    scroll_epoch: Some(42),
                    ..Default::default()
                },
                move |event, _, _| sink.borrow_mut().push(event),
                window,
                cx,
            )
        });
        capture.replace(Some(canvas.clone()));
        let host = cx.new(|_| CanvasHost { canvas });
        gpui_base::Root::new(host, window, cx)
    });
    let canvas = slot.borrow_mut().take().unwrap();
    frame(ui, 0);
    events.borrow_mut().clear();
    let wheel = |delta, modifiers| ScrollWheelEvent {
        position: point(px(80.), px(100.)),
        delta,
        modifiers,
        touch_phase: TouchPhase::Moved,
    };
    ui.simulate_event(wheel(
        ScrollDelta::Lines(point(0., -3.)),
        Default::default(),
    ));
    frame(ui, 8);
    frame(ui, 8);
    let offset = || {
        events
            .borrow()
            .iter()
            .rev()
            .find_map(|event| match event {
                CanvasEvent::ScrollInput {
                    change: ScrollChange::To { offset },
                    ..
                } => Some(px(-*offset)),
                _ => None,
            })
            .expect("native motion must emit an actual public range request")
    };
    let first = offset();
    assert!(first < px(0.));
    for _ in 0..8 {
        frame(ui, 8);
    }
    let later = offset();
    assert!(
        later < first,
        "one physical wheel must retain an inertial tail"
    );
    assert!(
        events.borrow().iter().all(|event| !matches!(
            event,
            CanvasEvent::ScrollInput {
                change: ScrollChange::Delta { .. },
                ..
            }
        )),
        "the common router must replace the original discrete wheel exactly once"
    );
    assert!(
        events
            .borrow()
            .iter()
            .filter(|event| matches!(
                event,
                CanvasEvent::ScrollInput {
                    epoch: 42,
                    change: ScrollChange::To { .. }
                }
            ))
            .count()
            > 2
    );
    ui.simulate_event(wheel(
        ScrollDelta::Pixels(point(px(0.), px(-17.))),
        Default::default(),
    ));
    frame(ui, 0);
    let precise = offset();
    assert!((precise - later + px(17.)).abs() < px(0.5));
    frame(ui, 100);
    assert_eq!(offset(), precise);
    events.borrow_mut().clear();
    ui.simulate_event(wheel(
        ScrollDelta::Lines(point(0., -1.)),
        gpui_kit::Modifiers {
            shift: true,
            ..Default::default()
        },
    ));
    assert!(
        events
            .borrow()
            .iter()
            .any(|event| matches!(event, CanvasEvent::Wheel { shift: true, .. }))
    );
    ui.simulate_event(wheel(
        ScrollDelta::Lines(point(0., -3.)),
        Default::default(),
    ));
    frame(ui, 8);
    ui.update(|_, cx| {
        canvas.update(cx, |canvas, cx| {
            canvas.update_drawing(Canvas {
                scroll: Some(ScrollRange {
                    content: 20_000.,
                    offset: 0.,
                }),
                scroll_epoch: Some(43),
                ..Default::default()
            });
            cx.notify();
        })
    });
    frame(ui, 0);
    events.borrow_mut().clear();
    for _ in 0..8 {
        frame(ui, 16);
    }
    assert!(
        !events
            .borrow()
            .iter()
            .any(|event| matches!(event, CanvasEvent::ScrollInput { .. })),
        "retired inertia must not address replacement content"
    );
}

/// Ordinary declarative containers and overflowing tab strips reach the shared UI component.
#[gpui::test]
fn scrollbar_plugin_containers_and_tabs_share_native_kinetic_motion(cx: &mut TestAppContext) {
    use super::*;
    use gpui_kit::{ScrollDelta, ScrollWheelEvent};
    use plugin_runtime::plugin_protocol::ui::{Node, Tab};
    for horizontal in [false, true] {
        init(cx, false);
        let slot = Rc::new(std::cell::RefCell::new(None));
        let capture = slot.clone();
        let (_, ui) = cx.add_window_view(move |window, cx| {
            let node = if horizontal {
                Node::new(
                    "tabs",
                    Kind::Tabs {
                        selected: "tab-0".into(),
                        tabs: (0..24)
                            .map(|index| {
                                Tab::new(
                                    format!("tab-{index}"),
                                    format!("Long tab {index} 中文"),
                                    Node::text(format!("content-{index}"), "content"),
                                )
                            })
                            .collect(),
                    },
                )
            } else {
                Node::scroll(
                    "list",
                    Node::new(
                        "items",
                        Kind::List {
                            items: (0..80).map(|index| format!("row {index} 中文")).collect(),
                        },
                    ),
                )
            }
            .width(300.)
            .height(180.);
            let view = cx.new(|cx| {
                PluginView::new(
                    "scroll-provider".into(),
                    Document::new(node),
                    Environment::default(),
                    |_, _| {},
                    window,
                    cx,
                )
            });
            capture.replace(Some(view.clone()));
            gpui_base::Root::new(view, window, cx)
        });
        let view = slot.borrow_mut().take().unwrap();
        frame(ui, 0);
        let coordinate = |ui: &mut VisualTestContext| {
            if horizontal {
                ui.debug_bounds("plugin-tab-0")
                    .expect("native tab geometry must remain visible to the test")
                    .left()
            } else {
                ui.update(|_, cx| view.read(cx).scrolls["list"].offset().y)
            }
        };
        let initial = coordinate(ui);
        ui.simulate_event(ScrollWheelEvent {
            position: point(px(40.), px(20.)),
            delta: ScrollDelta::Lines(point(0., -3.)),
            ..Default::default()
        });
        frame(ui, 8);
        frame(ui, 8);
        let first = coordinate(ui);
        for _ in 0..8 {
            frame(ui, 8);
        }
        assert!(
            first < initial && coordinate(ui) < first,
            "one wheel must move the original container and continue after release: horizontal={horizontal}, initial={initial:?}, first={first:?}, last={:?}",
            coordinate(ui)
        );
    }
}
