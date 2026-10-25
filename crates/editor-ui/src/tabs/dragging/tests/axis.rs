//! The public strip preserves native pointer, reveal and cancellation behavior on both axes.

use super::*;
use gpui_kit::gpui;

/// Change only the layout policy while preserving the same owner, state and native scroll handle.
fn vertical(view: &Entity<Harness>, ui: &mut VisualTestContext, height: f32, comparison: bool) {
    ui.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.axis = Axis::Vertical;
            view.height = height;
            view.comparison = comparison;
            cx.notify();
        });
    });
    frame(ui, 0);
}

/// Vertical rows keep horizontal titles, a full-height gap and real animated neighbor movement.
#[gpui::test]
fn vertical_rows_only_commit_on_release(cx: &mut TestAppContext) {
    let (view, ui) = harness(cx, 220.);
    vertical(&view, ui, 120., false);
    let source = ui.debug_bounds("tab-1").unwrap();
    let neighbor = ui.debug_bounds("tab-2").unwrap();
    assert_eq!(source.size, size(px(220.), px(crate::PANEL_HEADER_HEIGHT)));
    assert_eq!(neighbor.top(), source.bottom());
    begin(ui, 1);
    let target = point(source.center().x, px(124.));
    ui.simulate_mouse_move(target, MouseButton::Left, Default::default());
    // The production spring reads the real clock; 80 ms observes movement rather than settling.
    frame(ui, 0);
    std::thread::sleep(Duration::from_millis(80));
    frame(ui, 0);
    let floating = ui.debug_bounds("file-tab-drag-preview").unwrap();
    let slot = ui.debug_bounds("file-tab-drag-slot").unwrap();
    assert_eq!(floating.size, source.size);
    assert_eq!(slot.size.height, source.size.height);
    assert_eq!(slot.top(), px(76.));
    assert_eq!(slot.left(), source.left() + px(3.));
    assert!(ui.debug_bounds("tab-2").unwrap().top() < neighbor.top());
    ui.update(|_, cx| {
        assert!(view.read(cx).moves.is_empty());
        assert_eq!(view.read(cx).tabs[0].0, 1);
    });
    ui.simulate_mouse_up(target, MouseButton::Left, Default::default());
    frame(ui, 0);
    ui.update(|_, cx| {
        let view = view.read(cx);
        assert_eq!(
            view.tabs.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
            vec![2, 3, 1]
        );
        assert_eq!(
            view.moves,
            vec![FileTabMove {
                source: 1,
                before: None
            }]
        );
        assert_eq!(view.clicks, 0);
    });
    assert_eq!(ui.debug_bounds("tab-2").unwrap().top(), source.top());
    assert!(ui.debug_bounds("file-tab-drag-slot").is_none());
}

/// A nonreorderable comparison remains a direct reveal target after all vertical document rows.
#[gpui::test]
fn vertical_reveal_and_edges_use_the_shared_scroll_handle(cx: &mut TestAppContext) {
    let (view, ui) = harness(cx, 220.);
    vertical(&view, ui, 56., true);
    ui.update(|_, cx| view.read(cx).scroll.scroll_to_item(3));
    frame(ui, 0);
    let viewport = ui.debug_bounds("file-tab-strip").unwrap();
    let comparison = ui.debug_bounds("comparison-tab-3").unwrap();
    assert_eq!(
        comparison.size,
        size(px(220.), px(crate::PANEL_HEADER_HEIGHT))
    );
    assert!(comparison.bottom() <= viewport.bottom());
    assert!(comparison.top() >= viewport.top());
    ui.update(|_, cx| view.read(cx).scroll.set_offset(point(px(0.), px(0.))));
    frame(ui, 0);
    let start = begin(ui, 1);
    let edge = point(start.x, viewport.bottom() - px(3.));
    ui.simulate_mouse_move(edge, MouseButton::Left, Default::default());
    frame(ui, 48);
    let offset = ui.update(|_, cx| view.read(cx).scroll.offset());
    assert!(offset.y < px(0.));
    assert_eq!(offset.x, px(0.));
    let outside = point(viewport.right() + px(10.), edge.y);
    ui.simulate_mouse_move(outside, MouseButton::Left, Default::default());
    frame(ui, 0);
    assert!(ui.debug_bounds("file-tab-drag-slot").is_none());
    ui.simulate_mouse_up(outside, MouseButton::Left, Default::default());
    frame(ui, 0);
    ui.update(|_, cx| assert!(view.read(cx).moves.is_empty()));
}

/// A layout switch cancels dragging and applies reveal after GPUI updates the handle's metrics.
#[gpui::test]
fn axis_change_cancels_drag_and_reveals_after_layout(cx: &mut TestAppContext) {
    let (view, ui) = harness(cx, 220.);
    ui.update(|_, cx| view.read(cx).scroll.scroll_to_item(2));
    frame(ui, 0);
    assert!(ui.update(|_, cx| view.read(cx).scroll.offset().x) < px(0.));
    let start = begin(ui, 3);
    let owner = view.downgrade();
    ui.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.axis = Axis::Vertical;
            view.height = 56.;
            cx.notify();
        });
        // Mirror the host's layout-switch boundary. Native Div consumes scroll_to_item
        // before replacing its old overflow/bounds; a request queued before the first new
        // frame is lost against the old axis. Recompute the active ID after that frame.
        window.on_next_frame(move |window, cx| {
            let _ = owner.update(cx, |view, _| {
                if view.axis == Axis::Vertical {
                    if let Some(index) = view.tabs.iter().position(|(id, _)| *id == 3) {
                        view.scroll.scroll_to_item(index);
                    }
                }
            });
            window.refresh();
        });
    });
    frame(ui, 0);
    assert!(!ui.update(|_, cx| cx.has_active_drag()));
    assert!(ui.debug_bounds("file-tab-drag-preview").is_none());
    // VisualTestContext has no platform frame loop; deliver the real after-frame callback
    // explicitly, then paint the resulting reveal without changing the visibility assertion.
    ui.update(|window, cx| assert!(window.simulate_next_frame(cx) > 0));
    frame(ui, 0);
    let viewport = ui.debug_bounds("file-tab-strip").unwrap();
    assert!(ui.debug_bounds("tab-3").unwrap().bottom() <= viewport.bottom());
    let offset = ui.update(|_, cx| view.read(cx).scroll.offset());
    assert_eq!(offset.x, px(0.));
    assert!(offset.y < px(0.));
    ui.simulate_mouse_up(start, MouseButton::Left, Default::default());
    frame(ui, 0);
    let title = ui.debug_bounds("tab-3").unwrap().center();
    ui.simulate_click(title, Default::default());
    frame(ui, 0);
    let close = ui.debug_bounds("close-3").unwrap().center();
    ui.simulate_click(close, Default::default());
    ui.update(|_, cx| {
        assert!(view.read(cx).moves.is_empty());
        assert_eq!(view.read(cx).clicks, 1);
        assert_eq!(view.read(cx).closes, 1);
    });
}
