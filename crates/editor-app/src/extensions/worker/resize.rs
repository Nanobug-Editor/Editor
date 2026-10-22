//! Combine queued canvas observations without losing deltas or crossing input/lifecycle boundaries.
use super::*;

/// Retain the latest resize/absolute offset and sum same-direction scroll deltas before WASM.
/// Opposite directions remain ordered because clamping at an endpoint is not commutative.
/// The first nonmatching input/lifecycle message stays queued for the next actor turn.
pub(super) fn coalesce(
    mut work: Work,
    rx: &impl super::queued::WorkInbox,
    next_received: &mut Option<Work>,
) -> Work {
    if resize_target(&work).is_none() && scroll_target(&work).is_none() {
        return work;
    }
    // Bound channel draining even when a producer continuously emits geometry changes.
    for _ in 0..1024 {
        let Some(next) = next_received.take().or_else(|| rx.try_work().ok()) else {
            break;
        };
        if resize_target(&work).is_some() && resize_target(&work) == resize_target(&next) {
            work = next;
        } else if merge_scroll(&mut work, &next) {
        } else {
            *next_received = Some(next);
            break;
        }
    }
    work
}

/// Stream identity includes incarnation, panel, node and guest-owned content epoch.
fn scroll_target(work: &Work) -> Option<(&str, u64, &Option<String>, &str, u64)> {
    match work {
        Work::Event(plugin, incarnation, panel, api::Notification::Ui(event)) => {
            match &event.action {
                ui::Action::Canvas(ui::CanvasEvent::ScrollInput { epoch, .. }) => {
                    Some((plugin, *incarnation, panel, &event.node, *epoch))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// A combined delta remains within the public finite-coordinate quota; otherwise keep both inputs.
fn merge_scroll(work: &mut Work, next: &Work) -> bool {
    if scroll_target(work).is_none() || scroll_target(work) != scroll_target(next) {
        return false;
    }
    let Work::Event(_, _, _, api::Notification::Ui(current)) = work else {
        return false;
    };
    let Work::Event(_, _, _, api::Notification::Ui(next)) = next else {
        return false;
    };
    let ui::Action::Canvas(ui::CanvasEvent::ScrollInput { change, .. }) = &mut current.action
    else {
        return false;
    };
    let ui::Action::Canvas(ui::CanvasEvent::ScrollInput {
        change: incoming, ..
    }) = &next.action
    else {
        return false;
    };
    match (change, incoming) {
        (ui::ScrollChange::Delta { delta_y }, ui::ScrollChange::Delta { delta_y: next })
            if delta_y.is_finite()
                && next.is_finite()
                && delta_y.signum() == next.signum()
                && (*delta_y + *next).abs() <= 1_000_000. =>
        {
            *delta_y += *next
        }
        (ui::ScrollChange::To { offset }, ui::ScrollChange::To { offset: next }) => *offset = *next,
        _ => return false,
    }
    current.revision = current.revision.max(next.revision);
    true
}

/// Only absolute Resize events can replace each other; deltas and release gestures are never dropped.
fn resize_target(work: &Work) -> Option<(&str, u64, &Option<String>, u64, &str)> {
    match work {
        Work::Event(plugin, epoch, panel, api::Notification::Ui(event))
            if matches!(
                event.action,
                ui::Action::Canvas(ui::CanvasEvent::Resize { .. })
                    | ui::Action::ViewportInput(ui::ViewportInput {
                        event: ui::CanvasEvent::Resize { .. },
                        ..
                    })
            ) =>
        {
            Some((plugin, *epoch, panel, event.revision, &event.node))
        }
        _ => None,
    }
}
