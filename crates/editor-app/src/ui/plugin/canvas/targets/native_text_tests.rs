//! Native acceptance drives the public EntityInputHandler seam and observes text/geometry/events only.
use super::*;
use editor_ui::text::input::CanvasInputState;
use protocol::ui::{CanvasTextRange, ContentSize, ContentTransform, SceneNode, VisualViewport};

/// Keep a real old native endpoint alive to exercise platform callback revocation after target replacement.
#[derive(Default)]
pub(in crate::ui::plugin::canvas) struct Evidence {
    input: Option<Entity<CanvasInputState>>,
    caret: Option<Bounds<Pixels>>,
}
/// Locate the ordinary native component used by the published target, without reading its private draft.
pub(in crate::ui::plugin::canvas) fn endpoint(
    view: &Entity<PluginView>,
    cx: &App,
) -> Entity<CanvasInputState> {
    let canvas = view.read(cx).canvases["drawing"].read(cx);
    canvas
        .targets
        .sessions
        .values()
        .next()
        .expect("focused native text endpoint")
        .state
        .clone()
}
/// Select the actually focused endpoint when a real package publishes multiple editable targets.
pub(in crate::ui::plugin::canvas) fn focused_endpoint(
    view: &Entity<PluginView>,
    window: &Window,
    cx: &App,
) -> Entity<CanvasInputState> {
    view.read(cx).canvases["drawing"]
        .read(cx)
        .targets
        .sessions
        .values()
        .find(|input| input.state.read(cx).active(window))
        .expect("actual focused native text endpoint")
        .state
        .clone()
}
/// Publish through the ordinary native UI/resource adapters, then complete an actual native draw.
pub(in crate::ui::plugin::canvas) fn redraw(
    view: &Entity<PluginView>,
    edit: impl FnOnce(&mut Document),
    window: &mut Window,
    cx: &mut App,
) -> anyhow::Result<()> {
    let (mut document, environment) = {
        let view = view.read(cx);
        (view.document.clone(), view.environment.clone())
    };
    document.revision += 1;
    if let Kind::Canvas(canvas) = &mut document.root.kind {
        canvas.scene.as_mut().unwrap().revision += 1;
    }
    edit(&mut document);
    document.validate().map_err(anyhow::Error::msg)?;
    let mut renderer = super::super::super::images::VectorRenderer::default();
    renderer.set_environment(environment.clone());
    let images = renderer.prepare_resources(
        &BTreeMap::from([(
            "ordinary/diagram".into(),
            std::sync::Arc::new(document.clone()),
        )]),
        &BTreeMap::new(),
    );
    view.update(cx, |view, cx| {
        view.update_document(document, environment, window, cx);
        view.update_images("ordinary/diagram", &images, window, cx);
    });
    window.draw(cx).clear(cx);
    Ok(())
}
/// Read via the real native text API, including its normalized UTF-16 range.
fn assert_text(
    input: &Entity<CanvasInputState>,
    expected: &str,
    window: &mut Window,
    cx: &mut App,
) {
    input.update(cx, |input, cx| {
        let length = expected.encode_utf16().count();
        let mut adjusted = None;
        assert_eq!(
            input
                .text_for_range(0..length, &mut adjusted, window, cx)
                .as_deref(),
            Some(expected)
        );
        assert_eq!(adjusted, Some(0..length));
    });
}
/// Retain preedit through redraw/zoom, preserve later commits after old acknowledgements, then revoke epoch.
pub(in crate::ui::plugin::canvas) fn phase(
    view: &Entity<PluginView>,
    events: &[ui::UiEvent],
    evidence: &mut Evidence,
    phase: u8,
    window: &mut Window,
    cx: &mut App,
) -> anyhow::Result<()> {
    match phase {
        0 | 8 => {
            let snapshot = view
                .read(cx)
                .native_canvas_input_snapshot("drawing", window, cx)
                .unwrap();
            let coordinate = if phase == 0 { 30. } else { 60. };
            let position = point(
                px(snapshot["origin_x"].as_f64().unwrap() as f32 + coordinate),
                px(snapshot["origin_y"].as_f64().unwrap() as f32 + coordinate),
            );
            window.dispatch_event(
                gpui_kit::PlatformInput::MouseDown(MouseDownEvent {
                    position,
                    button: MouseButton::Left,
                    modifiers: Default::default(),
                    click_count: 1,
                    first_mouse: false,
                }),
                cx,
            );
            window.dispatch_event(
                gpui_kit::PlatformInput::MouseUp(MouseUpEvent {
                    position,
                    button: MouseButton::Left,
                    modifiers: Default::default(),
                    click_count: 1,
                }),
                cx,
            );
            window.draw(cx).clear(cx);
            if phase == 8 {
                let old = evidence.input.as_ref().unwrap();
                old.update(cx, |input, cx| {
                    input.replace_text_in_range(None, "late-after-new-focus", window, cx)
                });
                assert_text(&endpoint(view, cx), "e\u{301} 中文", window, cx);
            }
        }
        1 => {
            let input = endpoint(view, cx);
            evidence.caret = input.update(cx, |input, cx| {
                input.replace_and_mark_text_in_range(None, "zhong", Some(5..5), window, cx);
                assert_eq!(input.marked_text_range(window, cx), Some(3..8));
                assert_eq!(
                    input.selected_text_range(false, window, cx).unwrap().range,
                    8..8
                );
                input.bounds_for_range(8..8, Bounds::default(), window, cx)
            });
            anyhow::ensure!(
                evidence.caret.is_some(),
                "native preedit must have a glyph-derived candidate rectangle"
            );
            assert_text(&input, "e\u{301} zhong", window, cx);
            evidence.input = Some(input);
            window.draw(cx).clear(cx);
        }
        2 => {
            redraw(view, |_| {}, window, cx)?;
            let input = evidence.input.as_ref().unwrap();
            input.update(cx, |input, cx| {
                assert_eq!(input.marked_text_range(window, cx), Some(3..8))
            });
            assert_text(input, "e\u{301} zhong", window, cx);
        }
        3 => {
            redraw(
                view,
                |document| {
                    document.root.viewport = Some(VisualViewport {
                        content: Some(ContentSize {
                            width: 320.,
                            height: 240.,
                        }),
                        transform: Some(ContentTransform {
                            scale: 1.25,
                            x: 20.,
                            y: 10.,
                            anchor_x: 0.,
                            anchor_y: 0.,
                        }),
                    })
                },
                window,
                cx,
            )?;
            let input = evidence.input.as_ref().unwrap();
            let caret = input.update(cx, |input, cx| {
                assert_eq!(input.marked_text_range(window, cx), Some(3..8));
                input
                    .bounds_for_range(8..8, Bounds::default(), window, cx)
                    .unwrap()
            });
            let before = evidence.caret.unwrap();
            let snapshot = view
                .read(cx)
                .native_canvas_input_snapshot("drawing", window, cx)
                .unwrap();
            let origin = point(
                px(snapshot["origin_x"].as_f64().unwrap() as f32),
                px(snapshot["origin_y"].as_f64().unwrap() as f32),
            );
            let expected = origin + (before.origin - origin) * 1.25 + point(px(20.), px(10.));
            anyhow::ensure!(
                (caret.origin.x - expected.x).abs() < px(0.75)
                    && (caret.origin.y - expected.y).abs() < px(0.75)
                    && (caret.size.height - px(33.75)).abs() < px(0.01),
                "native candidate must follow logical zoom/pan once: before={before:?}, after={caret:?}"
            );
        }
        4 => {
            let input = evidence.input.as_ref().unwrap();
            input.update(cx, |input, cx| {
                input.replace_text_in_range(None, "中文", window, cx);
                input.replace_text_in_range(None, "a", window, cx);
                input.replace_text_in_range(None, "b", window, cx);
                assert_eq!(input.marked_text_range(window, cx), None);
                assert_eq!(
                    input.selected_text_range(false, window, cx).unwrap().range,
                    7..7
                );
            });
            assert_text(input, "e\u{301} 中文ab", window, cx);
            window.draw(cx).clear(cx);
        }
        5 | 6 => {
            let acknowledged = events
                .iter()
                .find_map(|event| match &event.action {
                    Action::Canvas(CanvasEvent::Input {
                        input:
                            protocol::ui::CanvasInput {
                                sequence,
                                kind: CanvasInputKind::Text(edit),
                                ..
                            },
                    }) if edit.text == "中文" => Some(*sequence),
                    _ => None,
                })
                .expect("native Chinese commit sequence");
            redraw(
                view,
                |document| {
                    let Kind::Canvas(canvas) = &mut document.root.kind else {
                        unreachable!()
                    };
                    let scene = canvas.scene.as_mut().unwrap();
                    if let SceneNode::Paragraph {
                        text, layout_key, ..
                    } = &mut scene.roots[0]
                    {
                        *text = "e\u{301} 中文".into();
                        *layout_key = "ime-2".into();
                    }
                    let binding = scene.targets[0].text_input.as_mut().unwrap();
                    binding.layout_key = "ime-2".into();
                    binding.text_revision = 2;
                    binding.acknowledged_sequence = acknowledged;
                    binding.selection = CanvasTextRange { start: 10, end: 10 };
                },
                window,
                cx,
            )?;
            let input = evidence.input.as_ref().unwrap();
            assert_text(input, "e\u{301} 中文ab", window, cx);
            if phase == 6 {
                input.update(cx, |input, cx| {
                    input.replace_and_mark_text_in_range(None, "ni", Some(2..2), window, cx);
                    assert_eq!(input.marked_text_range(window, cx), Some(7..9));
                });
                window.draw(cx).clear(cx);
            }
        }
        7 => {
            redraw(
                view,
                |document| {
                    let Kind::Canvas(canvas) = &mut document.root.kind else {
                        unreachable!()
                    };
                    let target = &mut canvas.scene.as_mut().unwrap().targets[0];
                    target.epoch = 2;
                    target.text_input.as_mut().unwrap().acknowledged_sequence = 0;
                },
                window,
                cx,
            )?;
            evidence.input.as_ref().unwrap().update(cx, |input, cx| {
                assert_eq!(input.marked_text_range(window, cx), None);
                assert!(input.selected_text_range(false, window, cx).is_none());
                input.replace_text_in_range(None, "late", window, cx);
            });
        }
        _ => unreachable!("bounded native text acceptance phases"),
    }
    Ok(())
}
/// Observe the public original-byte commit chain and native presentation outcomes.
pub(in crate::ui::plugin::canvas) fn verify(events: &[ui::UiEvent]) -> anyhow::Result<()> {
    let edits = events
        .iter()
        .filter_map(|event| match &event.action {
            Action::Canvas(CanvasEvent::Input {
                input:
                    protocol::ui::CanvasInput {
                        sequence,
                        epoch,
                        kind: CanvasInputKind::Text(edit),
                        ..
                    },
            }) => Some((*sequence, *epoch, edit)),
            _ => None,
        })
        .collect::<Vec<_>>();
    anyhow::ensure!(
        edits.len() == 3 && edits.iter().all(|(_, epoch, _)| *epoch == 1),
        "native text commits were lost, duplicated or redirected after epoch replacement: {edits:?}"
    );
    anyhow::ensure!(
        edits[0].2.text == "中文"
            && edits[0].2.range == CanvasTextRange { start: 4, end: 15 }
            && edits[0].2.base_sequence == 0
            && edits[0].2.text_revision == 1
            && edits[1].2.text == "a"
            && edits[1].2.range == CanvasTextRange { start: 10, end: 10 }
            && edits[1].2.base_sequence == edits[0].0
            && edits[2].2.text == "b"
            && edits[2].2.range == CanvasTextRange { start: 11, end: 11 }
            && edits[2].2.base_sequence == edits[1].0,
        "native UTF-16 replacement must preserve the exact UTF-8 predecessor chain: {edits:?}"
    );
    anyhow::ensure!(
        !events.iter().any(|event| matches!(
            &event.action,
            Action::Canvas(CanvasEvent::Text { .. })
                | Action::Canvas(CanvasEvent::Input {
                    input: protocol::ui::CanvasInput {
                        kind: CanvasInputKind::TextError { .. },
                        ..
                    }
                })
        )),
        "native target text must neither duplicate legacy commits nor hide session failures: {events:?}"
    );
    let cancelled=events.iter().filter(|event|matches!(&event.action,Action::Canvas(CanvasEvent::Input{input})
        if input.epoch==1&&matches!(&input.kind,CanvasInputKind::Composition(value)
            if value.phase==protocol::ui::CanvasCompositionPhase::Cancel&&value.text_revision==2&&value.range==CanvasTextRange{start:12,end:12}))).count();
    anyhow::ensure!(
        cancelled == 1,
        "target epoch replacement must deliver one original-scoped preedit cancellation: {events:?}"
    );
    let receipts = events
        .iter()
        .filter_map(|event| match &event.action {
            Action::Canvas(CanvasEvent::ScenePresented {
                scene_revision,
                error,
                ..
            }) => Some((*scene_revision, error)),
            Action::ViewportInput(protocol::ui::ViewportInput {
                event:
                    CanvasEvent::ScenePresented {
                        scene_revision,
                        error,
                        ..
                    },
                ..
            }) => Some((*scene_revision, error)),
            _ => None,
        })
        .collect::<Vec<_>>();
    anyhow::ensure!(
        receipts.len() >= 6 && receipts.iter().all(|(_, error)| error.is_none()),
        "native IME redraws must reach successful native frames: {receipts:?}"
    );
    Ok(())
}
