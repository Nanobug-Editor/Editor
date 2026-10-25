//! Native dispatch and observable public input envelopes validate target routing without a host test API.
use super::*;
use gpui_kit::{KeyUpEvent, Keystroke, Modifiers, PlatformInput};
pub(super) mod chords;
pub(super) mod deactivation;
pub(super) mod draft;
pub(super) mod editing;
pub(super) mod hits;
pub(super) mod wheel;

/// Click the first visible cell, repeat a modified native key, then navigate with Tab.
pub(super) fn keys(
    snapshot: &serde_json::Value,
    window: &mut Window,
    cx: &mut App,
) -> anyhow::Result<()> {
    let position = point(
        px(snapshot["origin_x"].as_f64().unwrap() as f32 + 30.),
        px(snapshot["origin_y"].as_f64().unwrap() as f32 + 30.),
    );
    window.dispatch_event(
        PlatformInput::MouseDown(MouseDownEvent {
            position,
            button: MouseButton::Left,
            modifiers: Default::default(),
            click_count: 1,
            first_mouse: false,
        }),
        cx,
    );
    window.dispatch_event(
        PlatformInput::MouseUp(MouseUpEvent {
            position,
            button: MouseButton::Left,
            modifiers: Default::default(),
            click_count: 1,
        }),
        cx,
    );
    let all = Modifiers {
        control: true,
        alt: true,
        shift: true,
        platform: true,
        function: true,
    };
    for (key, modifiers, repeated) in [
        ("right", all, true),
        ("tab", Modifiers::default(), false),
        ("down", Modifiers::default(), false),
    ] {
        let stroke = Keystroke {
            key: key.into(),
            modifiers,
            key_char: None,
        };
        window.dispatch_event(
            PlatformInput::KeyDown(KeyDownEvent {
                keystroke: stroke.clone(),
                is_held: false,
                prefer_character_input: false,
            }),
            cx,
        );
        if repeated {
            window.dispatch_event(
                PlatformInput::KeyDown(KeyDownEvent {
                    keystroke: stroke.clone(),
                    is_held: true,
                    prefer_character_input: false,
                }),
                cx,
            );
        }
        window.draw(cx).clear(cx);
        window.dispatch_event(PlatformInput::KeyUp(KeyUpEvent { keystroke: stroke }), cx);
    }
    Ok(())
}
/// Verify target identity, native down/up/repeat/modifiers, Tab order and monotonic delivery.
pub(super) fn verify_keys(events: &[ui::UiEvent]) -> anyhow::Result<()> {
    use protocol::ui::{CanvasInputKind, CanvasKeyPhase};
    let inputs = events
        .iter()
        .filter_map(|event| match &event.action {
            ui::Action::Canvas(CanvasEvent::Input { input })
                if matches!(input.kind, protocol::ui::CanvasInputKind::Key { .. }) =>
            {
                Some(input)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    anyhow::ensure!(
        inputs.len() == 7,
        "native stable target keys missing or duplicated: {inputs:?}"
    );
    anyhow::ensure!(
        inputs
            .iter()
            .all(|input| input.identity == 47 && input.presented_revision == 1 && input.epoch == 1)
            && inputs
                .windows(2)
                .all(|pair| pair[0].sequence < pair[1].sequence),
        "native scene/generation/order mismatch: {inputs:?}"
    );
    for (index, input) in inputs.iter().enumerate() {
        let CanvasInputKind::Key {
            phase,
            key,
            code,
            repeat,
            modifiers,
        } = &input.kind
        else {
            unreachable!("filtered native key input")
        };
        anyhow::ensure!(
            input.target == if index < 4 { "cell-a" } else { "cell-b" },
            "Tab target routing mismatch: {inputs:?}"
        );
        anyhow::ensure!(
            key == if index < 3 {
                "right"
            } else if index < 5 {
                "tab"
            } else {
                "down"
            } && *phase
                == if [2, 4, 6].contains(&index) {
                    CanvasKeyPhase::Up
                } else {
                    CanvasKeyPhase::Down
                }
                && *repeat == (index == 1)
                && code.is_none(),
            "native key transition or physical-code availability mismatch: {input:?}"
        );
        if index < 3 {
            anyhow::ensure!(
                modifiers.ctrl
                    && modifiers.alt
                    && modifiers.shift
                    && modifiers.meta
                    && modifiers.function,
                "native modifiers must remain independent: {input:?}"
            );
        }
    }
    anyhow::ensure!(
        !events
            .iter()
            .any(|event| matches!(&event.action, ui::Action::Canvas(CanvasEvent::Key { .. }))),
        "target keys must not duplicate legacy Canvas key delivery"
    );
    Ok(())
}

/// Drive an exact path hit, an outside release and a second gesture cancelled by native blur.
pub(super) fn pointer(
    snapshot: &serde_json::Value,
    phase: u8,
    window: &mut Window,
    cx: &mut App,
) -> anyhow::Result<()> {
    let at = |x, y| {
        point(
            px(snapshot["origin_x"].as_f64().unwrap() as f32 + x),
            px(snapshot["origin_y"].as_f64().unwrap() as f32 + y),
        )
    };
    let moved = |x, y, pressed_button, window: &mut Window, cx: &mut App| {
        window.dispatch_event(
            PlatformInput::MouseMove(MouseMoveEvent {
                position: at(x, y),
                pressed_button,
                modifiers: Default::default(),
            }),
            cx,
        );
    };
    let down = |window: &mut Window, cx: &mut App| {
        window.dispatch_event(
            PlatformInput::MouseDown(MouseDownEvent {
                position: at(30., 30.),
                button: MouseButton::Left,
                click_count: 1,
                modifiers: Default::default(),
                first_mouse: false,
            }),
            cx,
        );
    };
    let up = |window: &mut Window, cx: &mut App| {
        window.dispatch_event(
            PlatformInput::MouseUp(MouseUpEvent {
                position: at(280., 160.),
                button: MouseButton::Left,
                click_count: 1,
                modifiers: Default::default(),
            }),
            cx,
        );
    };
    match phase {
        0 => {
            moved(110., 90., None, window, cx);
            moved(30., 30., None, window, cx);
            window.dispatch_event(
                PlatformInput::ScrollWheel(ScrollWheelEvent {
                    position: at(30., 30.),
                    delta: gpui_kit::ScrollDelta::Pixels(point(px(0.5), px(-2.25))),
                    modifiers: Modifiers {
                        shift: true,
                        ..Default::default()
                    },
                    ..Default::default()
                }),
                cx,
            );
            down(window, cx);
            window.draw(cx).clear(cx);
            moved(280., 160., Some(MouseButton::Left), window, cx);
            up(window, cx);
            window.draw(cx).clear(cx);
            moved(30., 30., None, window, cx);
            down(window, cx);
            window.draw(cx).clear(cx);
        }
        1 => window.blur(cx),
        2 => {
            moved(280., 160., Some(MouseButton::Left), window, cx);
            up(window, cx);
        }
        _ => unreachable!("bounded native pointer acceptance phases"),
    }
    Ok(())
}
/// Observe exact hit/capture lifetime, native availability, precision and public event ordering.
pub(super) fn verify_pointer(
    events: &[ui::UiEvent],
    reason: protocol::ui::CanvasCancelReason,
) -> anyhow::Result<()> {
    use protocol::ui::{
        CanvasInputKind, CanvasPointerDevice, CanvasPointerPhase as Phase, CanvasWheelUnit,
    };
    let inputs = events
        .iter()
        .filter_map(|event| match &event.action {
            ui::Action::Canvas(CanvasEvent::Input { input }) => Some(input),
            _ => None,
        })
        .collect::<Vec<_>>();
    anyhow::ensure!(
        inputs.iter().all(|input| input.identity == 48
            && input.presented_revision == 1
            && input.target == "drag"
            && input.epoch == 1)
            && inputs
                .windows(2)
                .all(|pair| pair[0].sequence < pair[1].sequence),
        "native pointer addressing/order mismatch: {inputs:?}"
    );
    let pointers = inputs
        .iter()
        .filter_map(|input| {
            if let CanvasInputKind::Pointer(pointer) = &input.kind {
                Some(pointer)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let count = |phase| {
        pointers
            .iter()
            .filter(|pointer| pointer.phase == phase)
            .count()
    };
    anyhow::ensure!(
        count(Phase::Down) == 2
            && count(Phase::Up) == 1
            && count(Phase::Cancel) == 1
            && count(Phase::Enter) >= 2
            && count(Phase::Leave) >= 1,
        "native pointer lifecycle mismatch: {inputs:?}"
    );
    anyhow::ensure!(
        pointers.iter().all(|pointer| pointer.pointer == 1
            && pointer.device == CanvasPointerDevice::Mouse
            && pointer.pressure.is_none()
            && pointer.buttons.is_none())
            && !pointers
                .iter()
                .any(|pointer| pointer.x == 110. && pointer.y == 90.),
        "native availability or exact triangle hit mismatch: {pointers:?}"
    );
    let outside = pointers
        .iter()
        .filter(|pointer| pointer.phase == Phase::Move && pointer.x == 280. && pointer.y == 160.)
        .count();
    anyhow::ensure!(
        outside == 1
            && pointers
                .last()
                .is_some_and(|pointer| pointer.phase == Phase::Cancel
                    && pointer.cancel_reason == Some(reason)),
        "capture must finish outside once and stop after blur: {pointers:?}"
    );
    let wheels = inputs
        .iter()
        .filter_map(|input| {
            if let CanvasInputKind::Wheel(wheel) = &input.kind {
                Some(wheel)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    anyhow::ensure!(
        wheels.len() == 1
            && wheels[0].unit == CanvasWheelUnit::Pixels
            && wheels[0].delta_x == 0.5
            && wheels[0].delta_y == -2.25
            && wheels[0].modifiers.shift,
        "native fractional wheel was rounded or duplicated: {wheels:?}"
    );
    anyhow::ensure!(
        !events.iter().any(|event| matches!(
            &event.action,
            ui::Action::Canvas(CanvasEvent::Pointer { .. } | CanvasEvent::Wheel { .. })
        )),
        "target input must not duplicate legacy Canvas pointer/wheel delivery"
    );
    Ok(())
}
