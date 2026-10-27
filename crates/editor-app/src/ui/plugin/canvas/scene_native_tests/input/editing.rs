//! Native editing keys must produce independent whole-grapheme commits without host domain undo.
use super::*;
use gpui_kit::EntityInputHandler;

/// Dispatch actual key transitions and observe only the installed native text endpoint.
pub(in crate::ui::plugin::canvas::scene_native_tests) fn run(
    view: &Entity<PluginView>,
    window: &mut Window,
    cx: &mut App,
) -> anyhow::Result<()> {
    let snapshot = view
        .read(cx)
        .native_canvas_input_snapshot("drawing", window, cx)
        .unwrap();
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
    window.draw(cx).clear(cx);
    let input = super::super::super::targets::native_text_tests::endpoint(view, cx);
    for (key, control, expected) in [
        ("backspace", false, "e\u{301} "),
        ("left", false, "e\u{301} "),
        ("backspace", false, " "),
        ("a", true, " "),
        ("enter", false, "\n"),
        ("z", true, "\n"),
    ] {
        let stroke = Keystroke {
            key: key.into(),
            key_char: None,
            modifiers: Modifiers {
                control,
                ..Default::default()
            },
        };
        window.dispatch_event(
            PlatformInput::KeyDown(KeyDownEvent {
                keystroke: stroke.clone(),
                is_held: false,
                prefer_character_input: false,
            }),
            cx,
        );
        window.dispatch_event(PlatformInput::KeyUp(KeyUpEvent { keystroke: stroke }), cx);
        window.draw(cx).clear(cx);
        let text = input.update(cx, |input, cx| {
            input.text_for_range(0..65536, &mut None, window, cx)
        });
        anyhow::ensure!(
            text.as_deref() == Some(expected),
            "native {key}: expected {expected:?}, observed {text:?}"
        );
    }
    Ok(())
}

/// Raw keys and independent text commits keep their native order and predecessor chain.
pub(in crate::ui::plugin::canvas::scene_native_tests) fn verify(
    events: &[ui::UiEvent],
) -> anyhow::Result<()> {
    let inputs = events
        .iter()
        .filter_map(|event| match &event.action {
            ui::Action::Canvas(CanvasEvent::Input { input }) => Some(input),
            _ => None,
        })
        .collect::<Vec<_>>();
    let edits = inputs
        .iter()
        .filter_map(|input| match &input.kind {
            protocol::ui::CanvasInputKind::Text(edit) => Some((input.sequence, edit)),
            _ => None,
        })
        .collect::<Vec<_>>();
    anyhow::ensure!(edits.len() == 3, "native editing commits: {edits:?}");
    for (index, ((_, edit), (start, end, text))) in edits
        .iter()
        .zip([(4, 15, ""), (0, 3, ""), (0, 1, "\n")])
        .enumerate()
    {
        anyhow::ensure!(
            edit.range.start == start
                && edit.range.end == end
                && edit.text == text
                && edit.base_sequence == if index == 0 { 0 } else { edits[index - 1].0 },
            "native edit chain: {edits:?}"
        );
    }
    let keys = inputs
        .iter()
        .filter(|input| matches!(input.kind, protocol::ui::CanvasInputKind::Key { .. }))
        .count();
    anyhow::ensure!(
        keys == 12,
        "each native down/up must remain observable: {inputs:?}"
    );
    anyhow::ensure!(
        inputs
            .windows(2)
            .all(|pair| pair[0].sequence < pair[1].sequence),
        "native editing order"
    );
    Ok(())
}
