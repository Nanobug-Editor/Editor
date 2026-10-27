//! Native paint/hit/control checks share the exact two-axis scroll and visual zoom basis.
use super::*;
use gpui_kit::{Focusable as _, PlatformInput, Role};

/// Known absolute content (205,305) appears at native logical (220,230) after scroll and zoom.
pub(super) fn run(
    view: &Entity<PluginView>,
    window: &mut Window,
    cx: &mut App,
) -> anyhow::Result<()> {
    window.draw(cx).clear(cx);
    let snapshot = view
        .read(cx)
        .native_canvas_input_snapshot("drawing", window, cx)
        .unwrap();
    let origin = point(
        px(snapshot["origin_x"].as_f64().unwrap() as f32),
        px(snapshot["origin_y"].as_f64().unwrap() as f32),
    );
    for point in [
        point(px(220.), px(230.)),
        point(px(340.), px(230.)),
        point(px(340.), px(330.)),
    ] {
        click(origin + point, window, cx);
    }
    let input = view
        .read(cx)
        .inputs
        .get("scroll-field")
        .expect("ordinary native field")
        .state
        .clone();
    anyhow::ensure!(
        !input.read(cx).focus_handle(cx).is_focused(window),
        "unpainted overlay area cannot receive native focus"
    );
    let field = gpui_base::test_support::snapshots(window)
        .into_iter()
        .find(|node| node.role() == Some(Role::TextInput) && node.visible())
        .ok_or_else(|| anyhow::anyhow!("scroll-anchored input must actually be painted"))?;
    // At 2x zoom the ordinary input retains its 16px content inset; x=220 is border/padding.
    let visible = origin + point(px(250.), px(330.));
    anyhow::ensure!(
        field.bounds().contains(&visible),
        "known visible point must lie in the native editable content: {:?}, point {visible:?}",
        field.bounds()
    );
    click(visible, window, cx);
    anyhow::ensure!(
        input.read(cx).focus_handle(cx).is_focused(window),
        "painted overlay area retains the ordinary native input"
    );
    Ok(())
}

/// Only the visible target receives the original absolute content coordinates.
pub(super) fn verify(events: &[ui::UiEvent]) -> anyhow::Result<()> {
    let pointers = events
        .iter()
        .filter_map(|event| match &event.action {
            ui::Action::Canvas(CanvasEvent::Input { input }) => match &input.kind {
                ui::CanvasInputKind::Pointer(pointer) => Some((input, pointer)),
                _ => None,
            },
            ui::Action::ViewportInput(ui::ViewportInput {
                event: CanvasEvent::Input { input },
                ..
            }) => match &input.kind {
                ui::CanvasInputKind::Pointer(pointer) => Some((input, pointer)),
                _ => None,
            },
            _ => None,
        })
        .collect::<Vec<_>>();
    anyhow::ensure!(
        !pointers.is_empty(),
        "native target input missing after two-axis projection"
    );
    anyhow::ensure!(
        pointers.iter().all(|(input, _)| input.target == "visible"),
        "a retained buffer gap cannot expose a target: {pointers:?}"
    );
    let (_, pointer) = pointers
        .iter()
        .find(|(_, pointer)| pointer.phase == ui::CanvasPointerPhase::Down)
        .expect("visible native press");
    anyhow::ensure!(
        (pointer.x - 205.).abs() < 0.01 && (pointer.y - 305.).abs() < 0.01,
        "scroll then zoom inverse must preserve absolute content (205,305): {pointer:?}"
    );
    Ok(())
}

/// Real GPUI input goes through the existing native dispatch, with no guest-event injection.
fn click(position: Point<Pixels>, window: &mut Window, cx: &mut App) {
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
}
