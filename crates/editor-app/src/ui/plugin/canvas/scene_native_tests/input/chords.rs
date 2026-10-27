//! Native pressed-button availability cannot be confused with the button that started capture.
use super::*;

/// Two held buttons complete independently outside the canvas; navigation buttons retain identity.
pub(in crate::ui::plugin::canvas::scene_native_tests) fn run(
    snapshot: &serde_json::Value,
    window: &mut Window,
    cx: &mut App,
) {
    let at = |x, y| {
        point(
            px(snapshot["origin_x"].as_f64().unwrap() as f32 + x),
            px(snapshot["origin_y"].as_f64().unwrap() as f32 + y),
        )
    };
    let down = |button, window: &mut Window, cx: &mut App| {
        window.dispatch_event(
            PlatformInput::MouseDown(MouseDownEvent {
                position: at(30., 30.),
                button,
                modifiers: Default::default(),
                click_count: 1,
                first_mouse: false,
            }),
            cx,
        );
        window.draw(cx).clear(cx);
    };
    let up = |button, window: &mut Window, cx: &mut App| {
        window.dispatch_event(
            PlatformInput::MouseUp(MouseUpEvent {
                position: at(280., 160.),
                button,
                modifiers: Default::default(),
                click_count: 1,
            }),
            cx,
        );
        window.draw(cx).clear(cx);
    };
    down(MouseButton::Left, window, cx);
    down(MouseButton::Right, window, cx);
    for button in [MouseButton::Right, MouseButton::Left] {
        window.dispatch_event(
            PlatformInput::MouseMove(MouseMoveEvent {
                position: at(280., 160.),
                pressed_button: Some(button),
                modifiers: Default::default(),
            }),
            cx,
        );
        up(button, window, cx);
    }
    for direction in [
        gpui_kit::NavigationDirection::Back,
        gpui_kit::NavigationDirection::Forward,
    ] {
        let button = MouseButton::Navigate(direction);
        down(button, window, cx);
        up(button, window, cx);
    }
}
/// Native button changes are exact; missing full native masks remain explicitly unavailable.
pub(in crate::ui::plugin::canvas::scene_native_tests) fn verify(
    events: &[ui::UiEvent],
) -> anyhow::Result<()> {
    use protocol::ui::{CanvasInputKind, CanvasPointerPhase as Phase};
    let pointers = events
        .iter()
        .filter_map(|event| match &event.action {
            ui::Action::Canvas(CanvasEvent::Input { input }) => match &input.kind {
                CanvasInputKind::Pointer(pointer) => Some(pointer),
                _ => None,
            },
            _ => None,
        })
        .collect::<Vec<_>>();
    let buttons = |phase| {
        pointers
            .iter()
            .filter(|pointer| pointer.phase == phase)
            .map(|pointer| pointer.button)
            .collect::<Vec<_>>()
    };
    anyhow::ensure!(
        buttons(Phase::Down) == vec![Some(0), Some(2), Some(3), Some(4)]
            && buttons(Phase::Up) == vec![Some(2), Some(0), Some(3), Some(4)],
        "native button identities/capture: {pointers:?}"
    );
    anyhow::ensure!(
        pointers
            .iter()
            .filter(|pointer| pointer.phase == Phase::Move
                && pointer.x == 280.
                && pointer.y == 160.)
            .count()
            == 2
            && !pointers
                .iter()
                .any(|pointer| pointer.phase == Phase::Cancel)
            && pointers.iter().all(|pointer| pointer.buttons.is_none()),
        "native chord capture cannot infer a lost initiating button from another reported button: {pointers:?}"
    );
    Ok(())
}
