//! The complete source frame preserves native edits and its local completion popup inside alignment.
use super::*;
use editor_ui::{aligned_editor::AlignmentPane, aligned_pointer};
use gpui_base::Root;
use gpui_kit::TestAppContext;
use gpui_kit::gpui;

/// A native test surface with one source frame, one snapshot and the same production group.
struct AlignedSourceHost {
    app: Entity<EditorApp>,
    group: Option<AlignmentGroup>,
    notifications: Rc<Cell<usize>>,
    menu_anchor: Rc<Cell<Option<Point<Pixels>>>>,
    _source_changes: Subscription,
}

impl Render for AlignedSourceHost {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(group) = self.group.as_ref() else {
            return div().into_any_element();
        };
        let right = self.app.update(cx, |app, cx| {
            app.render_native_editor_aligned(group, 1, window, cx)
        });
        let anchor = self.menu_anchor.clone();
        // Observe the production local-menu boundary after Base right-click selection and
        // projected anchor resolution, without replacing the source input or its state.
        let menu = projected_context_menu(
            self.app.read(cx).editor.clone(),
            Rc::new(|menu, _, _| menu),
            group.bounds_projector(1),
            Rc::new(move |_, point, _, _| anchor.set(Some(point))),
        );
        div()
            .relative()
            .flex()
            .size_full()
            // SourceFrame is a flex child, as it is in the real split-comparison column. A block
            // wrapper gives its percentage-height aligned input no definite available height.
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .w_0()
                    .min_h_0()
                    .child(group.pane(0).unwrap()),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .w_0()
                    .min_h_0()
                    .child(right),
            )
            .child(menu)
            .into_any_element()
    }
}

/// Completion uses projected physical caret geometry, and acceptance/Undo mutate only the source.
#[gpui::test]
#[ignore = "native aligned SourceFrame regression; run explicitly with --ignored --test-threads=1"]
fn aligned_source_frame_keeps_completion_anchor_and_native_history(cx: &mut TestAppContext) {
    verify_aligned_source_frame(cx, false);
}

/// Large original source factories must keep the same session while root validation retains glyphs.
#[gpui::test]
#[ignore = "native large SourceFrame cache/input regression; run explicitly with --ignored --test-threads=1"]
fn large_aligned_source_frame_keeps_native_input_completion_and_history(cx: &mut TestAppContext) {
    verify_aligned_source_frame(cx, true);
}

/// Use the production Root plugins and atomic SourceFrame for both small and batched sources.
fn verify_aligned_source_frame(cx: &mut TestAppContext, large: bool) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        editor_ui::init(cx);
        typography::init(cx);
        theme::apply_theme(theme::builtin_theme(false), cx);
        cx.set_reduce_motion(true);
    });
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("source.txt");
    let tail = if large {
        (0..300)
            .map(|row| format!("source {row} 中文😀 native input\n"))
            .collect::<String>()
    } else {
        String::new()
    };
    let source = format!("first\nsecond\n{tail}");
    let snapshot_text = format!("first\ninsert one\ninsert two\nsecond\n{tail}");
    std::fs::write(&path, &source).unwrap();
    let workspace = editor_core::Workspace::open(directory.path()).unwrap();
    let (root, visual) = cx.add_window_view(move |window, cx| {
        let app = cx.new(|cx| EditorApp::new(workspace, Some(path), window, cx));
        let editor = app.read(cx).editor.clone();
        let host = cx.new(|cx| AlignedSourceHost {
            _source_changes: cx.observe(&editor, |host: &mut AlignedSourceHost, _, cx| {
                host.notifications.set(host.notifications.get() + 1);
                cx.notify();
            }),
            app,
            group: None,
            notifications: Rc::default(),
            menu_anchor: Rc::default(),
        });
        Root::new(host, window, cx)
    });
    let (app, group, snapshot) = visual.update(|window, cx| {
        let host = root
            .read(cx)
            .view()
            .clone()
            .downcast::<AlignedSourceHost>()
            .unwrap();
        let app = host.read(cx).app.clone();
        let editor = app.read(cx).editor.clone();
        let snapshot = cx.new(|cx| EditorState::new(window, cx).line_number(false));
        snapshot.update(cx, |state, cx| {
            state.set_value(snapshot_text.clone(), window, cx);
            state.set_readonly(true, cx);
        });
        let router = aligned_pointer::for_window(window, cx).unwrap();
        let mut group = AlignmentGroup::new(cx);
        group
            .configure(
                vec![
                    AlignmentPane::new(snapshot.clone(), 0, "Snapshot".into()),
                    AlignmentPane::new(editor.clone(), 0, "Source".into()),
                ],
                &router,
                cx,
            )
            .unwrap();
        host.update(cx, |host, cx| {
            host.group = Some(group.clone());
            cx.notify();
        });
        editor.update(cx, |state, cx| {
            state.set_selected_range(6..6, cx);
            state.focus(window, cx);
        });
        (app, group, snapshot)
    });
    visual.simulate_resize(size(px(1100.), px(600.)));
    for _ in 0..6 {
        visual.update(|window, cx| {
            window.refresh();
            window.draw(cx).clear(cx);
        });
        visual.run_until_parked();
    }
    let idle_notifications = visual.update(|_, cx| {
        root.read(cx)
            .view()
            .clone()
            .downcast::<AlignedSourceHost>()
            .unwrap()
            .read(cx)
            .notifications
            .clone()
    });
    let settled = idle_notifications.get();
    // Draining without input or forced refresh must not re-publish presentation indefinitely.
    // A self-sustaining frame queue would keep run_until_parked from reaching this assertion.
    for _ in 0..4 {
        visual.run_until_parked();
    }
    assert_eq!(
        idle_notifications.get(),
        settled,
        "aligned SourceFrame must be quiet after completed paint"
    );
    let caret = visual.update(|_, cx| {
        let viewport = group.viewport(1, cx).expect("source viewport painted");
        assert!(
            viewport.size.height > px(100.),
            "source frame must receive the split column height: {viewport:?}"
        );
        let editor = app.read(cx).editor.clone();
        let caret = group
            .project_range(1, 6..6, cx)
            .unwrap_or_else(|| panic!(
                "aligned caret unavailable: status={:?}, viewport={viewport:?}, native={:?}, origin={:?}",
                group.status(cx), editor.read(cx).range_to_bounds(&(6..6)), editor.read(cx).input_bounds()
            ));
        assert!(caret.top() - viewport.top() > editor.read(cx).line_height().unwrap() * 2.);
        caret
    });
    let position = point(caret.left() + px(0.5), caret.center().y);
    visual.simulate_mouse_down(position, MouseButton::Right, Modifiers::default());
    visual.simulate_mouse_up(position, MouseButton::Right, Modifiers::default());
    visual.run_until_parked();
    visual.update(|_, cx| {
        let host = root
            .read(cx)
            .view()
            .clone()
            .downcast::<AlignedSourceHost>()
            .unwrap();
        let anchor = host
            .read(cx)
            .menu_anchor
            .get()
            .expect("native menu callback");
        assert!((anchor.y - position.y).abs() < px(1.));
        assert!(group.viewport(1, cx).unwrap().contains(&anchor));
        assert_eq!(app.read(cx).editor.read(cx).cursor(), 6);
        app.read(cx).editor.clone().update(cx, |state, cx| {
            state.set_selected_range(0..0, cx);
        });
    });
    visual.simulate_mouse_down(position, MouseButton::Middle, Modifiers::default());
    visual.simulate_mouse_up(position, MouseButton::Middle, Modifiers::default());
    visual.run_until_parked();
    visual.update(|_, cx| {
        let app = app.read(cx);
        assert_eq!(
            app.editor.read(cx).cursor(),
            6,
            "query uses the clicked source caret"
        );
        let notice = app.definition_notice.expect("missing provider notice");
        assert_eq!(
            notice.position, position,
            "notice remains at the physical click"
        );
    });
    // Platform text input traverses the actual SourceFrame delegate before the session's Undo.
    visual.simulate_input("中文😀");
    visual.update(|_, cx| {
        assert_eq!(
            app.read(cx).editor.read(cx).value().as_ref(),
            format!("first\n中文😀second\n{tail}")
        );
    });
    visual.simulate_keystrokes("ctrl-z");
    visual.update(|_, cx| assert_eq!(app.read(cx).editor.read(cx).value().as_ref(), source));
    visual.update(|_, cx| {
        app.read(cx).editor.clone().update(cx, |state, cx| {
            state.present_completion_items(
                6,
                "",
                vec![lsp_types::CompletionItem {
                    label: "inserted".into(),
                    ..Default::default()
                }],
                cx,
            )
        });
    });
    visual.run_until_parked();
    visual.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
    let popup = visual
        .debug_bounds("editor-completion-card")
        .expect("local source completion card");
    assert!(
        popup.top() >= caret.bottom(),
        "popup must follow the visible projected caret: {popup:?} vs {caret:?}"
    );
    visual.simulate_keystrokes("enter");
    visual.update(|_, cx| {
        assert_eq!(
            app.read(cx).editor.read(cx).value().as_ref(),
            format!("first\ninsertedsecond\n{tail}")
        )
    });
    visual.simulate_keystrokes("ctrl-z");
    visual.update(|_, cx| {
        assert_eq!(app.read(cx).editor.read(cx).value().as_ref(), source);
        assert_eq!(snapshot.read(cx).value().as_ref(), snapshot_text);
    });
}
