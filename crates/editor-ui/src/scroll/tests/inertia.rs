//! High-refresh native authority, inertial release and demand-driven scheduling regressions.
use super::*;

/// Pixel alignment must not accumulate rounding error between separate, fully settled gestures.
#[gpui::test]
fn smooth_scroll_editor_keeps_distance_between_settled_gestures(cx: &mut TestAppContext) {
    let (host, ui) = open(cx, Kind::Editor);
    ui.simulate_scale_factor_change(1.5);
    frame(ui, 0);
    let line = ui.update(|_, cx| host.read(cx).editor.read(cx).line_height().unwrap());
    for gesture in 1..=8 {
        wheel(ui, ScrollDelta::Lines(point(0., -1.)));
        frame(ui, 1000);
        let actual = offset(&host, ui).y;
        let expected = -line * gesture as f32;
        assert!(
            f32::from((actual - expected).abs()) * 1.5 <= 0.501,
            "gesture {gesture} accumulated pixel rounding: {actual:?} versus {expected:?}"
        );
    }
}

/// The native text origin must follow wheel motion through the last frame and remain still at rest.
#[gpui::test]
fn smooth_scroll_editor_text_stays_aligned_when_settling(cx: &mut TestAppContext) {
    let (host, ui) = open(cx, Kind::Editor);
    let sample = |ui: &mut VisualTestContext| {
        ui.update(|_, cx| {
            let editor = host.read(cx).editor.read(cx);
            (
                editor.scroll_offset(),
                editor.text_bounds().unwrap().origin - editor.input_bounds().origin,
            )
        })
    };
    for (dark, scale) in [(false, 1.), (true, 1.25), (false, 1.5), (true, 2.)] {
        ui.update(|_, cx| {
            crate::theme::apply_theme(crate::theme::builtin_theme(dark), cx);
            host.read(cx).editor.clone().update(cx, |editor, cx| {
                editor.set_scroll_offset(point(px(0.), px(0.)), cx)
            });
        });
        ui.simulate_scale_factor_change(scale);
        frame(ui, 0);
        let (initial_offset, initial_origin) = sample(ui);
        wheel(ui, ScrollDelta::Lines(point(0., -3.)));
        let mut previous = initial_offset;
        for frame_index in 0..240 {
            ui.executor().advance_clock(Duration::from_nanos(6_944_444));
            frame(ui, 0);
            let (offset, origin) = sample(ui);
            assert!(
                offset.y <= previous.y,
                "wheel motion reversed at frame {frame_index}"
            );
            let error = (origin.y - initial_origin.y) - (offset.y - initial_offset.y);
            assert!(
                f32::from(error).abs() < 0.001,
                "text shifted away from its scroll position at frame {frame_index}: {offset:?}, {origin:?}, {error:?}"
            );
            let movement = f32::from(origin.y - initial_origin.y) * scale;
            assert!(
                (movement - movement.round()).abs() < 0.001,
                "all glyph rows must share one pixel translation at scale {scale}: {movement}"
            );
            previous = offset;
        }
        let settled = sample(ui);
        frame(ui, 1000);
        assert_eq!(
            sample(ui),
            settled,
            "an idle repaint must not move settled text"
        );
    }
}

/// A released wheel keeps decelerating across 120 Hz frames instead of stopping at 150 ms.
#[gpui::test]
fn smooth_scroll_inertia_moves_on_each_120hz_frame_after_release(cx: &mut TestAppContext) {
    let (host, ui) = open(cx, Kind::Nested);
    wheel(ui, ScrollDelta::Lines(point(0., -6.)));
    let mut positions = Vec::new();
    for _ in 0..48 {
        ui.executor().advance_clock(Duration::from_nanos(8_333_333));
        frame(ui, 0);
        positions.push(f32::from(-offset(&host, ui).y));
    }
    assert!(
        positions[29] > positions[23] + 0.5,
        "released wheel must retain inertia from 200 to 250 ms: {positions:?}"
    );
    assert!(
        positions[..30].windows(2).all(|pair| pair[1] > pair[0]),
        "every requested 120 Hz frame must advance the native position"
    );
    let travel = |range: std::ops::Range<usize>| positions[range.end - 1] - positions[range.start];
    assert!(travel(24..30) < travel(12..18), "release must decelerate");
}

/// Redraw bursts share one native frame callback; changing accessibility preferences stops inertia.
#[gpui::test]
fn smooth_scroll_coalesces_vsync_and_honors_changed_motion_preferences(cx: &mut TestAppContext) {
    let (host, ui) = open(cx, Kind::Nested);
    wheel(ui, ScrollDelta::Lines(point(0., -3.)));
    for _ in 0..12 {
        frame(ui, 1);
    }
    ui.update(|window, cx| {
        assert_eq!(
            window.simulate_next_frame(cx),
            1,
            "redraws before vsync must share one callback for both axes"
        );
    });
    frame(ui, 8);
    let before = offset(&host, ui);
    ui.update(|_, cx| cx.set_reduce_motion(true));
    frame(ui, 8);
    frame(ui, 1000);
    assert_eq!(
        offset(&host, ui),
        before,
        "enabling reduced motion must stop an in-flight gesture"
    );
    ui.update(|window, cx| {
        window.simulate_next_frame(cx);
    });
    frame(ui, 0);
    ui.update(|window, cx| assert_eq!(window.simulate_next_frame(cx), 0));
}
