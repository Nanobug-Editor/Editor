//! Shared native receipt navigation retains real cards, wheel events and pointer activation.
//! Raw debug rectangles are not clickable masks, and original session postimages remain the oracle.
use super::*;
use gpui_base::test_support::ElementSnapshot;
use gpui_kit::{Bounds, ElementId, MouseButton, ScrollDelta, ScrollEvent, TouchPhase};

/// Reveal the current real button by wheel, then prove native pointer-down hit it before releasing.
/// Earlier Undone cards remain retained; removing them or calling an internal Undo would hide this UI boundary.
/// `phase` preserves each caller's diagnostic label. Panics if no current button is usable within 30s.
pub(in crate::extensions::worker) fn undo(
    fixture: &Fixture,
    visual: &mut VisualTestContext,
    phase: &'static str,
) {
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut previous = String::new();
    let (target, position) = loop {
        wait(&fixture.app, &fixture.actor, visual, phase, 1, |_, _| true);
        let observed = visual.update(|window, _| {
            current(window).map(|target| {
                // The production receipt overlay is anchored 36px above the footer and capped at
                // 220px. This point navigates its existing anonymous scroll region, without adding
                // a host test API. This region is derived geometry; Base's last-painted visible
                // fact observes actual clipping. content_mask() is paint-only and is not read here.
                let viewport = window.viewport_size();
                let region = Bounds::new(
                    point(target.bounds().left(), viewport.height - px(256.)),
                    size(target.bounds().size.width, px(220.)),
                );
                (target, region, viewport)
            })
        });
        if let Some((target, region, viewport)) = observed {
            let bounds = target.bounds();
            let fully_in_receipt =
                bounds.top() >= region.top() && bounds.bottom() <= region.bottom();
            let clickable = target.visible()
                && target.disabled() != Some(true)
                && target.focused().is_some()
                && fully_in_receipt;
            let state = format!(
                "bounds={bounds:?}, painted_visible={}, disabled={:?}, derived_receipt_region={region:?}, viewport={viewport:?}, fully_in_receipt={fully_in_receipt}, clickable={clickable}",
                target.visible(),
                target.disabled()
            );
            if state != previous {
                eprintln!("{phase}: Undo target: {state}");
                previous = state;
            }
            if clickable {
                break (target, bounds.center());
            }
            if bounds.top() < region.top() || bounds.bottom() > region.bottom() {
                let delta = if bounds.center().y < region.center().y {
                    96.
                } else {
                    -96.
                };
                let position = region.center();
                visual.simulate_mouse_move(position, None, Default::default());
                visual.simulate_event(ScrollEvent {
                    // Wheel and trackpad fixtures retain pointer-device semantics.
                    source: gpui_kit::ScrollSource::Pointer,
                    position,
                    delta: ScrollDelta::Pixels(point(px(0.), px(delta))),
                    touch_phase: TouchPhase::Moved,
                    modifiers: Default::default(),
                });
            }
        }
        assert!(
            Instant::now() < deadline,
            "{phase}: current receipt never became clickable: {previous}"
        );
    };
    visual.simulate_mouse_move(position, None, Default::default());
    visual.simulate_mouse_down(position, MouseButton::Left, Default::default());
    visual.update(|window, cx| {
        // Base snapshots report the last paint, rather than live focus. Force this diagnostic
        // frame to repaint cached views after pointer-down before checking the same button.
        window.refresh();
        window.draw(cx).clear(cx);
        let pressed = current(window).expect("pointer-down must retain the current receipt button");
        assert_eq!(pressed.path(), target.path());
        if pressed.focused() != Some(true) {
            let overlapping = gpui_base::test_support::snapshots(window)
                .into_iter()
                .filter(|node| node.visible() && node.bounds().contains(&position))
                .take(12)
                .collect::<Vec<_>>();
            eprintln!(
                "{phase}: pointer-down diagnostics: position={position:?}; actual_mouse={:?}; focused={:?}; overlapping={overlapping:?}",
                window.mouse_position(),
                window.focused(cx)
            );
        }
        assert_eq!(
            pressed.focused(),
            Some(true),
            "native pointer-down missed the actual receipt hit region"
        );
        eprintln!("{phase}: Undo native pointer-down hit: {:?}", pressed.path());
    });
    visual.simulate_mouse_up(position, MouseButton::Left, Default::default());
    visual.run_until_parked();
}

/// All earlier successful rows are Undone, so precisely one current recoverable button is expected.
fn current(window: &gpui_kit::Window) -> Option<ElementSnapshot> {
    let targets = gpui_base::test_support::snapshots(window)
        .into_iter()
        .filter(|node| {
            matches!(node.path().last(), Some(ElementId::Name(id)) if id.as_str().starts_with("workspace-batch-undo-"))
        })
        .collect::<Vec<_>>();
    assert!(
        targets.len() <= 1,
        "ambiguous current Undo targets: {targets:?}"
    );
    targets.into_iter().next()
}
