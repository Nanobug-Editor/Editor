//! Real native wheels retain their motion across paint-only PluginView document publications.
use super::*;
use crate::ui::plugin::{Environment, PluginView};
use gpui_kit::{AppContext as _, ScrollDelta, ScrollWheelEvent, TestAppContext, VisualTestContext};
use plugin_runtime::plugin_protocol::ui::{Canvas, CanvasScene, Document, Kind, Node, SceneExtent};
use std::time::Duration;

/// Hiding revokes the old native endpoint; later artwork revisions must not revoke new motion.
#[gpui_kit::gpui::test]
fn two_axis_paint_updates_after_visibility_restore_preserve_native_motion(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        editor_ui::typography::init(cx);
        editor_ui::theme::apply_theme(editor_ui::theme::builtin_theme(false), cx);
        editor_ui::init(cx);
    });
    let slot = Rc::new(RefCell::new(None));
    let capture = slot.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let view = cx.new(|cx| {
            PluginView::new(
                "retained-motion".into(),
                document(0),
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
    visual.simulate_resize(size(px(400.), px(300.)));
    frame(visual, 0);
    visual.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.set_scene_current(false, cx);
            view.set_scene_current(true, cx);
        });
    });
    frame(visual, 0);
    visual.simulate_event(ScrollWheelEvent {
        position: point(px(50.), px(50.)),
        delta: ScrollDelta::Lines(point(-3., -3.)),
        modifiers: Default::default(),
        touch_phase: gpui_kit::TouchPhase::Moved,
    });
    frame(visual, 0);
    let mut previous = point(px(0.), px(0.));
    for revision in 1..=8 {
        visual.update(|window, cx| {
            view.update(cx, |view, cx| {
                view.update_document(document(revision), Environment::default(), window, cx);
            });
        });
        frame(visual, 40);
        let offset = visual.update(|_, cx| {
            let canvas = view.read(cx).canvases["drawing"].read(cx);
            assert_eq!(
                canvas.scroll_2d.shown().x,
                -canvas.scroll_2d.offset().x / px(1.)
            );
            assert_eq!(
                canvas.scroll_2d.shown().y,
                -canvas.scroll_2d.offset().y / px(1.)
            );
            canvas.scroll_2d.offset()
        });
        assert!(
            offset.x < px(0.) && offset.y < px(0.),
            "paint revision {revision} lost native wheel motion: {offset:?}"
        );
        assert!(
            offset.x <= previous.x && offset.y <= previous.y,
            "paint revision {revision} rewound native motion"
        );
        previous = offset;
    }
}

/// A stable content epoch and complete buffer isolate painting changes from native movement.
fn document(revision: u64) -> Document {
    Document::new(
        Node::new(
            "drawing",
            Kind::Canvas(Canvas {
                scene: Some(CanvasScene {
                    identity: 41,
                    revision: revision + 1,
                    roots: Vec::new(),
                    targets: Vec::new(),
                    semantics: Vec::new(),
                }),
                scroll_2d: Some(SceneScroll2D {
                    content: SceneExtent {
                        width: 4000.,
                        height: 5000.,
                    },
                    offset: SceneOffset::default(),
                    buffer_rect: plugin_runtime::plugin_protocol::Rect {
                        x: 0.,
                        y: 0.,
                        w: 4000.,
                        h: 5000.,
                    },
                    epoch: 7,
                }),
                ..Default::default()
            }),
        )
        .height(300.),
    )
    .revision(revision)
}

/// Advance the same native clock and draw one finite frame, as the local scroll controls do.
fn frame(visual: &mut VisualTestContext, millis: u64) {
    visual
        .executor()
        .advance_clock(Duration::from_millis(millis));
    visual.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
}
