//! Group membership is derived from the active tree; hidden branches cannot become a scroll driver.
use super::*;
use gpui_base::Root;
use gpui_kit::{Focusable as _, TestAppContext, VisualTestContext};
use gpui_kit::{gpui, point, px, size};
use std::cell::RefCell;

/// Readonly plugin configuration restricts the original document only while its source is mounted.
#[gpui::test]
fn configurable_editor_readonly_scene_withdrawal_preserves_native_undo(cx: &mut TestAppContext) {
    let (view, source, visual) = mount(cx);
    let original = visual.update(|_, cx| source.read(cx).text().to_string());
    visual.update(|window, cx| {
        view.update(cx, |view, cx| {
            let mut document = view.document.clone();
            let Kind::Row { children } = &mut document.root.kind else {
                unreachable!()
            };
            let Kind::Editor(editor) = &mut children[1].kind else {
                unreachable!()
            };
            editor.read_only = true;
            document.revision += 1;
            view.update_document(document, view.environment.clone(), window, cx);
            view.sync_editor_access(cx);
        })
    });
    settle(visual);
    visual.update(|window, cx| source.update(cx, |state, cx| state.focus(window, cx)));
    visual.simulate_input("blocked");
    settle(visual);
    visual.update(|_, cx| assert_eq!(source.read(cx).text().to_string(), original));
    visual.update(|_, cx| view.update(cx, |view, cx| view.set_scene_current(false, cx)));
    settle(visual);
    visual.update(|_, cx| assert!(source.read(cx).is_editable()));
    visual.simulate_input("allowed");
    settle(visual);
    visual.update(|_, cx| assert_ne!(source.read(cx).text().to_string(), original));
    visual.simulate_keystrokes("ctrl-z");
    settle(visual);
    visual.update(|_, cx| assert_eq!(source.read(cx).text().to_string(), original));
}

/// Source order is arbitrary; only the selected tab and enabled parents contribute pane identities.
#[test]
fn native_code_sync_collects_only_enabled_active_panes() {
    let literal = |id: &str| {
        ui::Node::new(
            id,
            Kind::Editor(plugin_runtime::plugin_protocol::ui::Editor {
                changes: Vec::new(),
                sync_group: Some("comparison".into()),
                ..plugin_runtime::plugin_protocol::ui::Editor::snapshot("sample")
            }),
        )
    };
    let mut disabled = literal("disabled");
    disabled.disabled = true;
    let root = ui::Node::column(
        "root",
        vec![
            ui::Node::new(
                "modes",
                Kind::Tabs {
                    tabs: vec![
                        ui::Tab {
                            id: "hidden".into(),
                            label: "Hidden".into(),
                            content: literal("hidden-code"),
                        },
                        ui::Tab {
                            id: "shown".into(),
                            label: "Shown".into(),
                            content: literal("shown-code"),
                        },
                    ],
                    selected: "shown".into(),
                },
            ),
            disabled,
            literal("second"),
        ],
    );
    let mut nodes = Vec::new();
    active_nodes(&root, &mut nodes);
    let codes = nodes
        .iter()
        .filter(|node| {
            matches!(
                &node.kind,
                Kind::Editor(ui::Editor {
                    source: ui::EditorSource::Snapshot { .. },
                    ..
                })
            )
        })
        .map(|node| node.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(codes, vec!["shown-code", "second"]);
}

/// Exercise real wheel dispatch and all source prepaints, including a native source between two
/// readonly panes. Their inserted prefixes must align by unchanged bytes rather than line counts.
#[gpui::test]
fn native_code_sync_wheel_maps_three_panes_and_scene_withdrawal_cancels(cx: &mut TestAppContext) {
    let (view, source, cx) = mount(cx);
    let snapshots = cx.update(|_, cx| {
        let view = view.read(cx);
        [
            view.snapshots["left"].editor.clone(),
            source.clone(),
            view.snapshots["right"].editor.clone(),
        ]
    });
    let original = cx.update(|window, cx| {
        source.update(cx, |editor, cx| editor.set_selected_range(1..3, cx));
        source.focus_handle(cx).focus(window, cx);
        snapshots
            .each_ref()
            .map(|editor| editor.read(cx).text().to_string())
    });
    // A program setter before any gesture has no synchronization authority.
    cx.update(|_, cx| {
        snapshots[0].update(cx, |editor, cx| {
            editor.set_scroll_offset(point(px(0.), px(-280.)), cx)
        });
    });
    settle(cx);
    cx.update(|_, cx| assert_eq!(source.read(cx).scroll_offset().y, px(0.)));

    wheel(cx, "plugin-code-left", -360.);
    assert_aligned(&snapshots, cx);
    // The former follower becomes the driver on its own genuine native wheel input.
    wheel(cx, "plugin-code-right", 120.);
    assert_aligned(&snapshots, cx);
    wheel(cx, "plugin-ui-source", -70.);
    assert_aligned(&snapshots, cx);
    let previous = cx.update(|window, cx| {
        for (editor, text) in snapshots.iter().zip(&original) {
            assert_eq!(editor.read(cx).text().to_string(), *text);
        }
        assert_eq!(source.read(cx).selected_range(), 1..3);
        assert!(source.focus_handle(cx).is_focused(window));
        snapshots
            .each_ref()
            .map(|editor| editor.read(cx).scroll_offset())
    });
    cx.update(|_, cx| {
        view.update(cx, |view, cx| view.set_scene_current(false, cx));
        snapshots[0].update(cx, |editor, cx| {
            editor.set_scroll_offset(point(px(0.), px(-40.)), cx)
        });
    });
    settle(cx);
    cx.update(|_, cx| {
        assert_eq!(snapshots[1].read(cx).scroll_offset(), previous[1]);
        assert_eq!(snapshots[2].read(cx).scroll_offset(), previous[2]);
    });
}

/// Window opening can paint synchronously while its parent is still mutably borrowed. Legacy
/// synchronization must defer its real owner read, without weakening the exact-version gate.
#[gpui::test]
fn native_code_sync_owner_borrow_during_paint_preserves_exact_authority(cx: &mut TestAppContext) {
    let (view, source, visual) = mount(cx);
    let checks = Rc::new(Cell::new(0));
    let (owner, editors) = visual.update(|_, cx| {
        let version = view.read(cx).document.source.clone().unwrap();
        let owner = cx.new(|_| version);
        let authority = owner.clone();
        let observed = checks.clone();
        view.update(cx, |view, cx| {
            view.action_source_current = Some(Rc::new(move |version, cx| {
                observed.set(observed.get() + 1);
                authority.read(cx) == version
            }));
            cx.notify();
        });
        source.update(cx, |source, cx| source.set_selected_range(1..3, cx));
        let view = view.read(cx);
        (
            owner,
            [
                view.snapshots["left"].editor.clone(),
                source.clone(),
                view.snapshots["right"].editor.clone(),
            ],
        )
    });
    let original = visual.update(|_, cx| {
        editors
            .each_ref()
            .map(|editor| editor.read(cx).text().to_string())
    });
    let before_borrow = checks.get();
    visual.update(|window, cx| {
        owner.update(cx, |_, cx| {
            // Refresh defeats view reuse: the authority-bearing canvas really paints while the
            // same entity that its callback reads is borrowed by the window's caller.
            window.refresh();
            window.draw(cx).clear(cx);
        });
    });
    settle(visual);
    assert!(
        checks.get() > before_borrow,
        "deferred painting must still consult the actual source owner"
    );
    let before = visual.update(|_, cx| source.read(cx).scroll_offset());
    wheel(visual, "plugin-code-left", -360.);
    visual.update(|_, cx| assert_ne!(source.read(cx).scroll_offset(), before));
    assert_aligned(&editors, visual);

    // An independently advanced owner must revoke the old scene's native follow behavior. A
    // permissive metadata substitute would pass the borrow check but fail this real-wheel check.
    visual.update(|_, cx| owner.update(cx, |version, _| version.revision += 1));
    settle(visual);
    let followers = visual.update(|_, cx| {
        [
            source.read(cx).scroll_offset(),
            editors[2].read(cx).scroll_offset(),
        ]
    });
    let left_before = visual.update(|_, cx| editors[0].read(cx).scroll_offset());
    wheel(visual, "plugin-code-left", 120.);
    visual.update(|_, cx| {
        assert_ne!(editors[0].read(cx).scroll_offset(), left_before);
        assert_eq!(source.read(cx).scroll_offset(), followers[0]);
        assert_eq!(editors[2].read(cx).scroll_offset(), followers[1]);
        assert_eq!(source.read(cx).selected_range(), 1..3);
        for (editor, text) in editors.iter().zip(original) {
            assert_eq!(editor.read(cx).text().to_string(), text);
        }
    });
}

/// Install the same generic source-borrowing seam as a work window, with one native entity only.
fn mount(
    cx: &mut TestAppContext,
) -> (
    Entity<PluginView>,
    Entity<EditorState>,
    &mut VisualTestContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        editor_ui::typography::init(cx);
        crate::ui::theme::apply_theme(crate::ui::theme::builtin_theme(false), cx);
        cx.set_reduce_motion(true);
    });
    let text: String = (0..160).map(|row| format!("row {row} 中文\r\n")).collect();
    let source_text = format!("native inserted\r\n{}", text);
    let right_text = format!("right inserted 1\r\nright inserted 2\r\n{}", text);
    let version = DocumentVersion {
        id: "source:workbench".into(),
        path: "workbench.txt".into(),
        revision: 7,
    };
    let code = |id: &str, text| {
        ui::Node::new(
            id,
            Kind::Editor(ui::Editor {
                changes: Vec::new(),
                sync_group: Some("comparison".into()),
                ..ui::Editor::snapshot(text)
            }),
        )
        .width(220.)
        .height(180.)
    };
    let mut document = Document::new(
        ui::Node::row(
            "panes",
            vec![
                code("left", text),
                ui::Node::new(
                    "source",
                    Kind::Editor(plugin_runtime::plugin_protocol::ui::Editor {
                        line_actions: Some(plugin_runtime::plugin_protocol::ui::EditorActions {
                            left: Vec::new(),
                            right: Vec::new(),
                        }),
                        sync_group: Some("comparison".into()),
                        ..plugin_runtime::plugin_protocol::ui::Editor::document(version.clone())
                    }),
                )
                .width(220.)
                .height(180.),
                code("right", right_text),
            ],
        )
        .height(180.),
    );
    document.source = Some(version.clone());
    document.editor_layout = true;
    let capture = Rc::new(RefCell::new(None));
    let slot = capture.clone();
    let (_, cx) = cx.add_window_view(move |window, cx| {
        let source = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("text")
                .line_number(true)
                .soft_wrap(false)
        });
        source.update(cx, |editor, cx| editor.set_value(source_text, window, cx));
        let view = cx.new(|cx| {
            let mut view = PluginView::new(
                "test".into(),
                document,
                Environment::default(),
                |_, _| {},
                window,
                cx,
            );
            view.set_code_sync_source(Some((version.clone(), source.clone())), cx);
            let target = source.clone();
            let expected = version.clone();
            view.native_editor = Some(Rc::new(move |version, _, _, _, cx| {
                (version == &expected).then(|| {
                    editor_ui::source_editor::Editor::new(&target)
                        .identity("work-window-document", None)
                        .document_commands(true)
                        .tools(false)
                        .into_any_element()
                })
            }));
            view.action_source_current = Some(Rc::new(move |source, _| source == &version));
            view
        });
        *slot.borrow_mut() = Some((view.clone(), source));
        Root::new(view, window, cx)
    });
    cx.simulate_resize(size(px(700.), px(220.)));
    settle(cx);
    let (view, source) = capture.borrow_mut().take().unwrap();
    (view, source, cx)
}

/// Draws permit deferred native clamps and finite geometry searches to complete before observation.
fn settle(cx: &mut VisualTestContext) {
    for _ in 0..8 {
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.run_until_parked();
    }
}

/// Dispatch the platform event through the readonly native frame instead of claiming a test driver.
fn wheel(cx: &mut VisualTestContext, selector: &'static str, delta: f32) {
    let bounds = cx.debug_bounds(selector).unwrap();
    cx.simulate_event(gpui_kit::ScrollWheelEvent {
        position: bounds.center(),
        delta: gpui_kit::ScrollDelta::Pixels(point(px(0.), px(delta))),
        modifiers: Default::default(),
        touch_phase: gpui_kit::TouchPhase::Moved,
    });
    settle(cx);
}

/// Native geometry must show the same unchanged row and fractional phase in every pane.
fn assert_aligned(editors: &[Entity<EditorState>; 3], cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        let positions = editors.each_ref().map(|editor| {
            let editor = editor.read(cx);
            let anchor =
                native_viewport::sample(editor, window, cx).expect("painted native anchor");
            let text = editor.text().to_string();
            let row = text[anchor.offset..]
                .lines()
                .next()
                .unwrap_or("")
                .to_string();
            (row, anchor.line_fraction)
        });
        assert!(positions[0].0.starts_with("row "));
        for position in &positions[1..] {
            assert_eq!(position.0, positions[0].0);
            assert!((position.1 - positions[0].1).abs() < 0.03);
        }
    });
}
