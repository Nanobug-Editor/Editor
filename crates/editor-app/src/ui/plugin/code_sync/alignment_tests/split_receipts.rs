//! Source-bound split gestures retain the owning native window for ordinary authority checks.
use super::*;

/// Native splitter release inside a real source Editor rail must reach the public event sink.
#[gpui::test]
fn source_editor_rail_split_delivers_its_release(cx: &mut TestAppContext) {
    let (mounted, ui) = mount_with_document(
        cx,
        |version| {
            let controls = ui::Node::column(
                "rail-panes",
                vec![
                    ui::Node::button("left-control", "Left").grow(),
                    ui::Node::button("right-control", "Right").grow(),
                ],
            )
            .width(70.)
            .height(300.)
            .split_proportions(vec![]);
            let mut editor = ui::Editor::document(version.clone());
            editor.line_actions = Some(ui::EditorActions {
                left: vec![ui::EditorAction {
                    offset: 0,
                    content: Box::new(controls),
                }],
                right: vec![],
            });
            let mut document =
                Document::new(ui::Node::new("source", Kind::Editor(editor)).height(480.))
                    .revision(1);
            document.source = Some(version.clone());
            document.editor_layout = true;
            document.validate().unwrap();
            document
        },
        "first line\nsecond line\n".into(),
        false,
        false,
        size(px(1000.), px(560.)),
    );
    let original = ui.update(|_, cx| mounted.source.read(cx).text().to_string());
    mounted.events.borrow_mut().clear();
    drag_rail(ui, |_| {
        assert!(
            mounted
                .events
                .borrow()
                .iter()
                .all(|event| event.node != "rail-panes"
                    || !matches!(event.action, Action::SplitResized(_))),
            "motion alone must not persist a layout"
        );
    });
    assert_eq!(
        mounted
            .events
            .borrow()
            .iter()
            .filter(|event| event.node == "rail-panes"
                && matches!(event.action, Action::SplitResized(_)))
            .count(),
        1,
        "a current source rail must deliver exactly one final split gesture"
    );
    {
        let events = mounted.events.borrow();
        let event = events
            .iter()
            .find(|event| {
                event.node == "rail-panes" && matches!(event.action, Action::SplitResized(_))
            })
            .unwrap();
        assert_eq!(event.revision, mounted.document.revision);
        let Action::SplitResized(weights) = &event.action else {
            unreachable!()
        };
        assert!(weights[0] > 0.5 && (weights.iter().sum::<f32>() - 1.).abs() < 0.001);
    }
    mounted.events.borrow_mut().clear();
    mounted.current_revision.set(mounted.version.revision + 1);
    drag_rail(ui, |_| {});
    assert!(
        mounted
            .events
            .borrow()
            .iter()
            .all(|event| event.node != "rail-panes"
                || !matches!(event.action, Action::SplitResized(_))),
        "stale source rails cannot persist a layout"
    );
    ui.update(|_, cx| assert_eq!(mounted.source.read(cx).text().to_string(), original));

    mounted.current_revision.set(mounted.version.revision);
    mounted.events.borrow_mut().clear();
    drag_rail(ui, |ui| {
        ui.update(|window, cx| {
            mounted.source.update(cx, |source, cx| {
                source.focus(window, cx);
                source.replace_and_mark_text_in_range(None, "拼", Some(0..1), window, cx);
                assert!(source.marked_text_range(window, cx).is_some());
            })
        });
    });
    assert!(
        mounted
            .events
            .borrow()
            .iter()
            .all(|event| event.node != "rail-panes"
                || !matches!(event.action, Action::SplitResized(_))),
        "preedit must suppress a source-bound release"
    );
    ui.update(|window, cx| {
        mounted.source.update(cx, |source, cx| {
            assert!(source.marked_text_range(window, cx).is_some());
        });
    });
}

/// Dispatch one physical divider gesture, allowing source authority to change before release.
fn drag_rail(ui: &mut VisualTestContext, before_release: impl FnOnce(&mut VisualTestContext)) {
    let start = ui
        .debug_bounds("plugin-split-divider-rail-panes")
        .expect("real native rail splitter")
        .center();
    let end = point(start.x, start.y + px(24.));
    ui.simulate_mouse_down(start, MouseButton::Left, Default::default());
    ui.simulate_mouse_move(
        point(start.x, start.y + px(8.)),
        MouseButton::Left,
        Default::default(),
    );
    ui.simulate_mouse_move(end, MouseButton::Left, Default::default());
    settle(ui);
    before_release(ui);
    ui.simulate_mouse_up(end, MouseButton::Left, Default::default());
    settle(ui);
}
