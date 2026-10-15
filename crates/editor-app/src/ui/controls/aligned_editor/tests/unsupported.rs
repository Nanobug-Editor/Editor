//! Unsupported physical layout keeps original native input and independent scrolling usable.
use super::*;

#[gpui::test]
fn aligned_editor_disjoint_native_viewports_use_independent_native_editing(
    cx: &mut TestAppContext,
) {
    let (view, ui) = mount(cx);
    // Both panes have a real positive native viewport, inside this one native window. Their
    // vertical intervals deliberately do not overlap; this is not a collapsed/invisible control.
    ui.simulate_resize(size(px(640.), px(420.)));
    view.update(ui, |fixture, cx| {
        fixture.top_offsets[1] = px(220.);
        cx.notify();
    });
    settle(ui);
    let click = ui.update(|_, cx| {
        let fixture = view.read(cx);
        let left = fixture.group.viewport(0, cx).unwrap();
        let right = fixture.group.viewport(1, cx).unwrap();
        assert!(left.size.height > px(100.) && right.size.height > px(100.));
        assert!(left.bottom() < right.top());
        assert_eq!(
            fixture.group.status(cx),
            AlignmentStatus::Unavailable(AlignmentError::Geometry),
            "zero common height must not report Ready while hiding every source slice",
        );
        for index in 0..2 {
            let native = fixture.editors[index]
                .read(cx)
                .range_to_bounds(&(0..0))
                .unwrap();
            assert_eq!(fixture.group.project_range(index, 0..0, cx), Some(native));
        }
        let caret = fixture.group.project_range(0, 0..0, cx).unwrap();
        point(caret.left() + px(0.4), caret.center().y)
    });
    ui.simulate_click(click, Default::default());
    ui.run_until_parked();
    settle(ui);
    let before = view.read_with(ui, |fixture, _| fixture.routed.get());
    ui.simulate_input("中文😀");
    settle(ui);
    ui.update(|_, cx| {
        let fixture = view.read(cx);
        assert!(
            fixture.routed.get() > before,
            "fallback must preserve the original host delegate"
        );
        assert!(fixture.editors[0].read(cx).value().contains("中文😀"));
    });
    ui.simulate_keystrokes("ctrl-z");
    settle(ui);
    let (position, peer_scroll) = ui.update(|_, cx| {
        let fixture = view.read(cx);
        assert_eq!(fixture.editors[0].read(cx).value().as_ref(), content(false));
        assert_eq!(fixture.editors[1].read(cx).value().as_ref(), content(true));
        let viewport = fixture.group.viewport(0, cx).unwrap();
        let native = fixture.editors[0].read(cx);
        // This is a long, wrapped document in a real clipped native viewport. Keep this explicit
        // so a failed wheel cannot be dismissed as a short document with no scrollable overflow.
        let status = fixture.group.status(cx);
        let epoch = fixture.group.state.read(cx).epoch;
        assert!(
            native.input_bounds().size.height > px(100.),
            "native={:?}, viewport={viewport:?}, status={status:?}, epoch={epoch}",
            native.input_bounds(),
        );
        assert!(
            native.input_bounds().size.height <= viewport.size.height + px(0.5),
            "native={:?}, viewport={viewport:?}, status={status:?}, epoch={epoch}",
            native.input_bounds(),
        );
        assert!(native.visible_row_range().unwrap().end < 32);
        assert!(
            native.input_bounds().contains(&viewport.center()),
            "native={:?}, viewport={viewport:?}, status={status:?}, epoch={epoch}",
            native.input_bounds(),
        );
        (
            viewport.center(),
            fixture.editors[1].read(cx).scroll_offset(),
        )
    });
    let dispatched_scroll = ui.update(|window, cx| {
        // Dispatch the actual platform event and distinguish native handling from a later paint
        // accidentally resetting its offset. No direct scroll mutation substitutes for the wheel.
        window.dispatch_event(
            gpui_kit::PlatformInput::ScrollWheel(gpui_kit::ScrollWheelEvent {
                position,
                delta: gpui_kit::ScrollDelta::Pixels(point(px(0.), px(-70.))),
                ..Default::default()
            }),
            cx,
        );
        view.read(cx).editors[0].read(cx).scroll_offset()
    });
    settle(ui);
    ui.update(|_, cx| {
        let fixture = view.read(cx);
        let native = fixture.editors[0].read(cx);
        assert!(
            native.scroll_offset().y < px(0.),
            "fallback wheel must move the native source: immediate={dispatched_scroll:?}, final={:?}, input={:?}, visible={:?}, status={:?}",
            native.scroll_offset(), native.input_bounds(), native.visible_row_range(), fixture.group.status(cx),
        );
        assert_eq!(
            fixture.editors[1].read(cx).scroll_offset(),
            peer_scroll,
            "independent fallback must not scroll another native source"
        );
        assert_eq!(fixture.group.scroll_offset(cx), px(0.));
    });
    for _ in 0..3 {
        ui.update(|window, cx| {
            window.draw(cx).clear(cx);
            assert_eq!(
                window.simulate_next_frame(cx),
                0,
                "unsupported geometry must settle after the native fallback paint"
            );
        });
        ui.run_until_parked();
    }
    // Returning to a valid physical arrangement recovers without editing text, replacing the
    // native sources, toggling focus or requiring a fresh document revision from the owner.
    view.update(ui, |fixture, cx| {
        fixture.top_offsets[1] = px(0.);
        cx.notify();
    });
    settle(ui);
    let click = ui.update(|_, cx| {
        let fixture = view.read(cx);
        assert_eq!(fixture.group.status(cx), AlignmentStatus::Ready);
        assert_eq!(fixture.editors[0].read(cx).value().as_ref(), content(false));
        assert_eq!(fixture.editors[1].read(cx).value().as_ref(), content(true));
        let byte = byte_at(&content(false), 0) + 8;
        let bounds = fixture.group.project_range(0, byte..byte, cx).unwrap();
        point(bounds.left() + px(0.4), bounds.center().y)
    });
    ui.simulate_click(click, Default::default());
    ui.run_until_parked();
    settle(ui);
    ui.update(|window, cx| {
        assert_eq!(view.read(cx).editors[0].read(cx).cursor(), 8);
        painted_caret(&view, window, cx);
    });
    ui.update(|window, cx| {
        window.draw(cx).clear(cx);
        assert_eq!(
            window.simulate_next_frame(cx),
            0,
            "a recovered group must stop remeasuring"
        );
    });
}
