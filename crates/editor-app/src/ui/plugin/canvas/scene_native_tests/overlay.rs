//! Existing native controls, real pointer focus and IME survive scene-only foreground movement.
use super::*;

/// Actual pointer routing obeys the same inherited rectangular mask as native pixels.
pub(super) fn group_clip_phase(
    view: &Entity<PluginView>,
    phase: u8,
    window: &mut Window,
    cx: &mut App,
) -> anyhow::Result<()> {
    window.draw(cx).clear(cx);
    let input = view.read(cx).inputs["rename"].state.clone();
    if phase == 1 {
        anyhow::ensure!(
            !input.read(cx).focus_handle(cx).is_focused(window),
            "clipped-away native input must not receive a pointer press"
        );
    }
    if phase == 2 {
        anyhow::ensure!(
            input.read(cx).focus_handle(cx).is_focused(window),
            "visible native input must remain focusable inside the group clip"
        );
        return Ok(());
    }
    let snapshot = view
        .read(cx)
        .native_canvas_input_snapshot("drawing", window, cx)
        .unwrap();
    let position = point(
        px(snapshot["origin_x"].as_f64().unwrap() as f32 + if phase == 0 { 145. } else { 70. }),
        px(snapshot["origin_y"].as_f64().unwrap() as f32 + 75.),
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
    Ok(())
}
use gpui_kit::{EntityInputHandler, Focusable as _, PlatformInput, Role};

/// Retain the actual ordinary input endpoint to detect unwanted recreation during drawing changes.
#[derive(Default)]
pub(super) struct Evidence {
    input: Option<Entity<gpui_base::input::InputState>>,
    pub(super) return_focus: bool,
    pub(super) projected: bool,
    pub(super) tab: bool,
    pub(super) hidden: bool,
    /// Include real hover dispatch before editing to detect foreground pointer leakage.
    pub(super) hover: bool,
    /// A captured Canvas gesture must own additional buttons over a foreground native control.
    pub(super) capture: bool,
    /// A fully cropped foreground control must leave the native sequential focus order.
    pub(super) clipped: bool,
    /// Common native buttons must use the same uniform projection as the adjacent input.
    pub(super) scaled_button: bool,
    /// Existing multiline fields must share the adjacent input's projected content inset.
    pub(super) scaled_textarea: bool,
}

/// Drive one native step, allowing the event loop to deliver the preceding ordinary form event.
pub(super) fn phase(
    view: &Entity<PluginView>,
    evidence: &mut Evidence,
    phase: u8,
    window: &mut Window,
    cx: &mut App,
) -> anyhow::Result<()> {
    window.draw(cx).clear(cx);
    if evidence.scaled_textarea && phase == 2 {
        let snapshots = gpui_base::test_support::snapshots(window);
        let textarea = snapshots
            .iter()
            .find(|node| {
                node.visible()
                    && node.role() == Some(Role::TextInput)
                    && node.label() == Some("多行 / Multiline")
            })
            .ok_or_else(|| anyhow::anyhow!("native multiline foreground must be visible"))?;
        let input = snapshots
            .iter()
            .find(|node| {
                node.visible()
                    && node.role() == Some(Role::TextInput)
                    && node.label() != Some("多行 / Multiline")
            })
            .ok_or_else(|| anyhow::anyhow!("adjacent native input must be visible"))?;
        anyhow::ensure!(
            ((textarea.bounds().origin.x - input.bounds().origin.x) / px(1.)).abs() < 1.,
            "multiline native inset must follow the same projected scale as ordinary input: {:?} vs {:?}",
            textarea.bounds(),
            input.bounds()
        );
    }
    if evidence.scaled_button && phase == 2 {
        let button = gpui_base::test_support::snapshots(window)
            .into_iter()
            .find(|node| {
                node.visible()
                    && node.role() == Some(Role::Button)
                    && node.label() == Some("应用 / Apply")
            })
            .ok_or_else(|| {
                anyhow::anyhow!("existing native button must be painted in the foreground subtree")
            })?;
        anyhow::ensure!(
            (button.bounds().size.height / px(1.) - 42.).abs() < 1.,
            "native button metrics must follow the same 1.5 scale as the input: {:?}",
            button.bounds()
        );
    }
    if evidence.clipped && phase == 4 {
        anyhow::ensure!(
            !gpui_base::test_support::snapshots(window)
                .iter()
                .any(|node| node.role() == Some(Role::TextInput) && node.visible()),
            "off-viewport native field must not remain visible"
        );
        window.dispatch_event(
            PlatformInput::KeyDown(KeyDownEvent {
                keystroke: gpui_kit::Keystroke {
                    key: "tab".into(),
                    key_char: None,
                    modifiers: Default::default(),
                },
                is_held: false,
                prefer_character_input: false,
            }),
            cx,
        );
        anyhow::ensure!(
            !evidence
                .input
                .as_ref()
                .unwrap()
                .read(cx)
                .focus_handle(cx)
                .is_focused(window),
            "Tab must not enter a native control fully clipped by the Canvas viewport"
        );
        return Ok(());
    }
    if phase == 4 && evidence.return_focus {
        anyhow::ensure!(
            !gpui_base::test_support::snapshots(window)
                .iter()
                .any(|node| node.role() == Some(Role::TextInput) && node.visible()),
            "withdrawn native control must leave the actual layout"
        );
        window.dispatch_event(
            PlatformInput::KeyDown(KeyDownEvent {
                keystroke: gpui_kit::Keystroke {
                    key: "right".into(),
                    key_char: None,
                    modifiers: Default::default(),
                },
                is_held: false,
                prefer_character_input: false,
            }),
            cx,
        );
        return Ok(());
    }
    let field = gpui_base::test_support::snapshots(window)
        .into_iter()
        .find(|node| {
            node.role() == Some(Role::TextInput)
                && node.visible()
                && node.label() != Some("多行 / Multiline")
        })
        .ok_or_else(|| anyhow::anyhow!("scene-anchored native input must actually be painted"))?;
    let origin = view
        .read(cx)
        .native_canvas_input_snapshot("drawing", window, cx)
        .unwrap();
    if evidence.capture && phase >= 3 {
        capture_phase(field.bounds(), &origin, phase, window, cx);
        return Ok(());
    }
    if evidence.clipped && phase == 3 {
        super::super::targets::native_text_tests::redraw(
            view,
            |document| {
                if let Kind::Canvas(canvas) = &mut document.root.kind {
                    canvas.overlays[0].rect.x = 500.;
                }
            },
            window,
            cx,
        )?;
        let position = point(
            px(origin["origin_x"].as_f64().unwrap() as f32 + 70.),
            px(origin["origin_y"].as_f64().unwrap() as f32 + 30.),
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
        return Ok(());
    }
    // Base exposes the editable content box, inset by the existing local Input's 8 px padding.
    let expected_x = origin["origin_x"].as_f64().unwrap() as f32
        + if evidence.projected {
            if phase >= 2 { 57. } else { 27. }
        } else if phase >= 2 {
            68.
        } else {
            28.
        };
    anyhow::ensure!(
        (field.bounds().origin.x / px(1.) - expected_x).abs() < 1.1,
        "foreground native input must follow its group: {:?}",
        field.bounds()
    );
    if evidence.projected {
        anyhow::ensure!(
            (field.bounds().size.width / px(1.) - 186.).abs() < 1.1
                && (field.bounds().size.height / px(1.) - 39.).abs() < 1.1,
            "native control geometry must scale with 2x group and 0.75x viewport, independently of OS DPI: {:?}",
            field.bounds()
        );
    }
    if evidence.tab && phase >= 3 {
        let input = evidence.input.as_ref().unwrap();
        if phase == 4 {
            anyhow::ensure!(
                !input.read(cx).focus_handle(cx).is_focused(window),
                "Shift-Tab must leave the native control for the preceding scene target"
            );
        }
        window.dispatch_event(
            PlatformInput::KeyDown(KeyDownEvent {
                keystroke: gpui_kit::Keystroke {
                    key: "tab".into(),
                    key_char: None,
                    modifiers: gpui_kit::Modifiers {
                        shift: phase == 3,
                        ..Default::default()
                    },
                },
                is_held: false,
                prefer_character_input: false,
            }),
            cx,
        );
        if phase == 4 {
            anyhow::ensure!(
                input.read(cx).focus_handle(cx).is_focused(window),
                "Tab from a scene target must enter the ordinary native foreground control"
            );
        }
        return Ok(());
    }
    if evidence.hidden && phase >= 3 {
        if phase == 3 {
            super::super::targets::native_text_tests::redraw(
                view,
                |document| {
                    if let Kind::Canvas(canvas) = &mut document.root.kind {
                        if let Kind::Input(input) = &mut canvas.overlays[0].child.kind {
                            input.value = "中文".into();
                            input.value_revision = 1;
                        }
                    }
                },
                window,
                cx,
            )?;
            evidence.input.as_ref().unwrap().update(cx, |input, cx| {
                input.replace_and_mark_text_in_range(None, "旧", Some(1..1), window, cx)
            });
            view.update(cx, |view, cx| {
                view.set_scene_current(false, cx);
                view.set_scene_current(true, cx);
            });
        } else {
            let old = evidence.input.as_ref().unwrap();
            let current = view.read(cx).inputs["rename"].state.clone();
            anyhow::ensure!(
                old.entity_id() != current.entity_id(),
                "hidden foreground must not revive its previous native endpoint"
            );
            current.update(cx, |input, cx| {
                assert!(
                    input.marked_text_range(window, cx).is_none(),
                    "hidden native preedit must be cancelled"
                );
                assert_eq!(
                    input.value(),
                    "中文",
                    "committed guest value must survive hiding"
                );
            });
            old.update(cx, |input, cx| {
                input.replace_text_in_range(None, "late-old-endpoint", window, cx)
            });
        }
        return Ok(());
    }
    if phase == 0 {
        let position = field.bounds().center();
        if evidence.hover {
            window.dispatch_event(
                PlatformInput::MouseMove(MouseMoveEvent {
                    position,
                    pressed_button: None,
                    modifiers: Default::default(),
                }),
                cx,
            );
        }
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
        let input = view.read(cx).inputs["rename"].state.clone();
        anyhow::ensure!(
            input.read(cx).focus_handle(cx).is_focused(window),
            "real click must focus existing native input"
        );
        input.update(cx, |input, cx| {
            input.replace_and_mark_text_in_range(None, "中", Some(1..1), window, cx)
        });
        evidence.input = Some(input);
    } else if phase == 1 {
        super::super::targets::native_text_tests::redraw(
            view,
            |document| {
                if let Kind::Canvas(canvas) = &mut document.root.kind {
                    if let ui::SceneNode::Group { transform, .. } =
                        &mut canvas.scene.as_mut().unwrap().roots[0]
                    {
                        transform.0[4] = 60.;
                    }
                }
            },
            window,
            cx,
        )?;
    } else if phase == 2 {
        let input = evidence.input.as_ref().unwrap();
        anyhow::ensure!(
            view.read(cx).inputs["rename"].state.entity_id() == input.entity_id(),
            "drawing updates must retain native identity"
        );
        input.update(cx, |input, cx| {
            assert!(
                input.marked_text_range(window, cx).is_some(),
                "drawing changes must preserve Chinese preedit"
            );
            input.replace_text_in_range(None, "中文", window, cx);
        });
    } else if phase == 3 {
        if evidence.return_focus {
            super::super::targets::native_text_tests::redraw(
                view,
                |document| {
                    if let Kind::Canvas(canvas) = &mut document.root.kind {
                        canvas.overlays.clear();
                    }
                },
                window,
                cx,
            )?;
        } else {
            evidence.input.as_ref().unwrap().update(cx, |input, cx| {
                input.replace_and_mark_text_in_range(None, "旧", Some(1..1), window, cx)
            });
        }
    } else if phase == 4 {
        super::super::targets::native_text_tests::redraw(
            view,
            |document| {
                if let Kind::Canvas(canvas) = &mut document.root.kind {
                    canvas.scene.as_mut().unwrap().targets[0].epoch = 3;
                    canvas.overlays[0].anchor.target.epoch = 3;
                    if let Kind::Input(input) = &mut canvas.overlays[0].child.kind {
                        input.value = "replacement".into();
                    }
                }
            },
            window,
            cx,
        )?;
    } else {
        let old = evidence.input.as_ref().unwrap();
        let replacement = view.read(cx).inputs["rename"].state.clone();
        anyhow::ensure!(
            replacement.entity_id() != old.entity_id(),
            "a replacement target epoch must not inherit the old native endpoint"
        );
        anyhow::ensure!(
            replacement.read(cx).value() == "replacement",
            "new target must start from its own declared value"
        );
        old.update(cx, |input, cx| {
            input.replace_text_in_range(None, "late-old-endpoint", window, cx)
        });
    }
    Ok(())
}

/// Verify the ordinary committed Change and both actual display receipts after the drawing update.
pub(super) fn verify(events: &[UiEvent], fixture: &serde_json::Value) -> anyhow::Result<()> {
    let flag = |name: &str| fixture[name].as_bool().unwrap_or(false);
    let (retired, returned, hidden, captured, clipped) = (
        flag("overlay_epoch_script"),
        flag("overlay_return_script"),
        flag("overlay_hide_script"),
        flag("overlay_capture_script"),
        flag("overlay_clip_script"),
    );
    anyhow::ensure!(
        events
            .iter()
            .filter(|event| event.node == "rename"
                && matches!(&event.action,
        Action::Change(text) if text=="中文"))
            .count()
            == 1,
        "native foreground must deliver exactly one committed ordinary Change: {events:?}"
    );
    let revisions = events
        .iter()
        .filter_map(|event| match &event.action {
            Action::Canvas(CanvasEvent::ScenePresented {
                identity: 61,
                scene_revision,
                error: None,
                ..
            })
            | Action::ViewportInput(ui::ViewportInput {
                event:
                    CanvasEvent::ScenePresented {
                        identity: 61,
                        scene_revision,
                        error: None,
                        ..
                    },
                ..
            }) => Some(*scene_revision),
            _ => None,
        })
        .collect::<Vec<_>>();
    anyhow::ensure!(
        revisions
            == if hidden || retired || returned || clipped {
                vec![1, 2, 3]
            } else {
                vec![1, 2]
            },
        "foreground movement must accompany actual scene paints: {revisions:?}"
    );
    if hidden || retired {
        anyhow::ensure!(
            !events
                .iter()
                .any(|event| matches!(&event.action,Action::Change(text) if text.contains('旧'))),
            "withdrawn preedit cannot be committed into the replacement native control: {events:?}"
        );
    }
    if returned {
        anyhow::ensure!(events.iter().any(|event|matches!(&event.action,
            Action::Canvas(CanvasEvent::Input{input:ui::CanvasInput{target,epoch:2,kind:ui::CanvasInputKind::Key{key,..},..}})
            if target=="node"&&key=="right")),"removing a focused overlay must return native keyboard routing to the declared live target: {events:?}");
    }
    anyhow::ensure!(!events.iter().any(|event|matches!(&event.action,Action::Change(text) if text.contains("late-old-endpoint"))),
        "retired native endpoints cannot emit into the replacement target");
    if captured {
        let pointers = events
            .iter()
            .filter_map(|event| match &event.action {
                Action::Canvas(CanvasEvent::Input {
                    input:
                        ui::CanvasInput {
                            target,
                            epoch: 2,
                            kind: ui::CanvasInputKind::Pointer(pointer),
                            ..
                        },
                }) if target == "node" => Some(pointer),
                _ => None,
            })
            .collect::<Vec<_>>();
        let downs = pointers
            .iter()
            .filter(|pointer| pointer.phase == ui::CanvasPointerPhase::Down)
            .map(|pointer| pointer.button)
            .collect::<Vec<_>>();
        let ups = pointers
            .iter()
            .filter(|pointer| pointer.phase == ui::CanvasPointerPhase::Up)
            .map(|pointer| pointer.button)
            .collect::<Vec<_>>();
        anyhow::ensure!(
            downs == [Some(0), Some(2)]
                && ups == [Some(2), Some(0)]
                && !pointers
                    .iter()
                    .any(|pointer| pointer.phase == ui::CanvasPointerPhase::Cancel),
            "original Canvas capture must retain both buttons through native foreground occlusion: {pointers:?}"
        );
        return Ok(());
    }
    if clipped {
        return Ok(());
    }
    anyhow::ensure!(
        !events.iter().any(|event| matches!(
            &event.action,
            Action::Canvas(CanvasEvent::Input {
                input: ui::CanvasInput {
                    kind: ui::CanvasInputKind::Pointer { .. },
                    ..
                }
            })
        )),
        "foreground controls must occlude artwork input"
    );
    Ok(())
}

/// Start a capture outside the field, then chord and release over its actual native rectangle.
fn capture_phase(
    field: Bounds<Pixels>,
    origin: &serde_json::Value,
    phase: u8,
    window: &mut Window,
    cx: &mut App,
) {
    let outside = point(
        px(origin["origin_x"].as_f64().unwrap() as f32 + 70.),
        px(origin["origin_y"].as_f64().unwrap() as f32 + 30.),
    );
    if phase == 3 {
        window.dispatch_event(
            PlatformInput::MouseDown(MouseDownEvent {
                position: outside,
                button: MouseButton::Left,
                modifiers: Default::default(),
                click_count: 1,
                first_mouse: false,
            }),
            cx,
        );
    } else if phase == 4 {
        window.dispatch_event(
            PlatformInput::MouseMove(MouseMoveEvent {
                position: field.center(),
                pressed_button: Some(MouseButton::Left),
                modifiers: Default::default(),
            }),
            cx,
        );
        window.dispatch_event(
            PlatformInput::MouseDown(MouseDownEvent {
                position: field.center(),
                button: MouseButton::Right,
                modifiers: Default::default(),
                click_count: 1,
                first_mouse: false,
            }),
            cx,
        );
    } else {
        for button in [MouseButton::Right, MouseButton::Left] {
            window.dispatch_event(
                PlatformInput::MouseUp(MouseUpEvent {
                    position: field.center(),
                    button,
                    modifiers: Default::default(),
                    click_count: 1,
                }),
                cx,
            );
        }
    }
}
