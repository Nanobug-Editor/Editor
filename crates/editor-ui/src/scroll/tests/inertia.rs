//! High-refresh native authority, inertial release and demand-driven scheduling regressions.
use super::*;

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
