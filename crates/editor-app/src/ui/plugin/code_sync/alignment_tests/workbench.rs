//! Fast native layout regressions for the public three-pane conflict-workbench scene.
//! These tests exercise routing and geometry, not Git mutations or the WASM worker lifecycle.
use super::*;
use gpui_kit::{
    Context, IntoElement, ParentElement, Render, Styled, Subscription, Window, WindowBounds,
    canvas, div,
};

const RESULT: &str = "header\n<<<<<<< HEAD\nlocal block\n=======\nincoming block\n>>>>>>> main\nkeep 1\nkeep 2\nleft only\nkeep 3\nright only\nend\n";
const LEFT: &str = "header\nlocal block\nkeep 1\nkeep 2\nleft only\nkeep 3\nbase right\nend\n";
const RIGHT: &str = "header\nincoming block\nkeep 1\nkeep 2\nbase left\nkeep 3\nright only\nend\n";

/// Mirror the production sizing declarations, including both rails and the wrapping footer.
/// The file-region body is a static label because tree transport is unrelated to this seam.
fn document(version: &DocumentVersion) -> Document {
    let peer = |id, title_id, title, text: &str| {
        ui::Node::column(
            format!("{id}-column"),
            vec![
                ui::Node::text(title_id, title),
                ui::Node::new(
                    id,
                    Kind::Editor(ui::Editor {
                        // Full-file changes also exercise background paints and their finite
                        // geometry follow-ups in the real independent-window idle regression.
                        changes: vec![plugin_runtime::plugin_protocol::api::TextRange {
                            start: 0,
                            end: text.len(),
                        }],
                        sync_group: Some("comparison".into()),
                        ..ui::Editor::snapshot(text)
                    }),
                )
                .grow(),
            ],
        )
        .grow()
    };
    let edge = |side: &str, arrow: &str| {
        vec![ui::EditorAction {
            offset: 7,
            content: Box::new(ui::Node::row(
                format!("edge-{side}-0"),
                vec![
                    ui::Node::button(format!("merge-block-0-{side}"), arrow),
                    ui::Node::button(format!("merge-block-0-ignore-{side}"), "×"),
                ],
            )),
        }]
    };
    let panes = ui::Node::row(
        "compare-columns",
        vec![
            peer(
                "left",
                "pane-title-0",
                "Stage 2 · Read only · refs/heads/feature · 0123456789012345678901234567890123456789",
                LEFT,
            ),
            ui::Node::column(
                "result-column",
                vec![
                    ui::Node::text("result-title", "Result · Editable"),
                    ui::Node::new(
                        "source",
                        Kind::Editor(plugin_runtime::plugin_protocol::ui::Editor { line_actions: Some(plugin_runtime::plugin_protocol::ui::EditorActions { left: edge("left", "→"), right: edge("right", "←") }), sync_group: Some("comparison".into()), ..plugin_runtime::plugin_protocol::ui::Editor::document(version.clone()) }),
                    )
                    .grow(),
                ],
            )
            .grow(),
            peer(
                "right",
                "pane-title-2",
                "Stage 3 · Read only · refs/heads/main · 9876543210987654321098765432109876543210",
                RIGHT,
            ),
        ],
    )
    .resizable()
    .grow();
    let footer = ui::Node::row(
        "merge-actions",
        vec![
            ui::Node::button("merge-nonconflicting", "Apply nonconflicting changes").disabled(true),
            ui::Node::button("merge-show-base", "View base"),
            ui::Node::button("merge-from-base", "Restart from base (Undo available)"),
            ui::Node::button("conflict-stage-2", "Use complete stage 2…"),
            ui::Node::button("conflict-stage-3", "Use complete stage 3…"),
            ui::Node::button("resolved", "Save and resolve").disabled(true),
            ui::Node::button("continue", "Continue").disabled(true),
            ui::Node::button("skip", "Skip…").disabled(true),
            ui::Node::button("abort", "Abort…"),
        ],
    )
    .wrap();
    let mut document = Document::new(
        ui::Node::column(
            "window-root",
            vec![
                ui::Node::text("commit-identity", "Resolve conflicts"),
                ui::Node::row(
                    "window-body",
                    vec![
                        ui::Node::column(
                            "conflicts",
                            vec![ui::Node::text("conflict-file", "merge.txt")],
                        )
                        .width(220.)
                        .grow(),
                        ui::Node::column(
                            "details-right",
                            vec![
                                ui::Node::text("details-path", "merge.txt"),
                                ui::Node::text("no-parent-controls", ""),
                                panes,
                            ],
                        )
                        .grow(),
                    ],
                )
                .resizable()
                .grow(),
                footer,
                ui::Node::text("window-status", ""),
            ],
        )
        .grow()
        .padding(8.),
    )
    .revision(1)
    .code_alignment(true);
    document.source = Some(version.clone());
    document.editor_layout = true;
    document
}

/// Include both native source layout and public scene bounds so an offscreen rail differs from a
/// visible button whose click was intercepted. No private native editor state is inspected.
fn diagnostics(mounted: &Mounted, ui: &mut VisualTestContext) -> String {
    let wrappers = [
        "plugin-ui-window-root",
        "plugin-ui-window-body",
        "plugin-ui-conflicts",
        "plugin-ui-details-right",
        "plugin-ui-compare-columns",
        "plugin-ui-source",
        "plugin-ui-merge-actions",
        "plugin-ui-merge-from-base",
        "plugin-ui-merge-block-0-left",
        "plugin-ui-merge-block-0-right",
    ]
    .map(|id| (id, ui.debug_bounds(id)));
    ui.update(|window, cx| {
        let source = mounted.source.read(cx);
        let alignment = mounted.view.read(cx).code_sync.groups["comparison"]
            .alignment
            .as_ref()
            .unwrap();
        let panes = [0, 1, 2].map(|index| alignment.viewport(index, cx));
        let buttons: Vec<_> = gpui_base::test_support::snapshots(window)
            .iter()
            .filter(|snapshot| snapshot.role() == Some(gpui_kit::Role::Button))
            .map(|snapshot| {
                format!(
                    "{:?}: bounds={:?}, visible={}, focus={:?}",
                    snapshot.path(),
                    snapshot.bounds(),
                    snapshot.visible(),
                    snapshot.focused(),
                )
            })
            .collect();
        format!(
            "window={:?}; wrappers={wrappers:?}; status={:?}; panes={panes:?}; native_input={:?}; native_visible_rows={:?}; native_edge={:?}; projected_edge={:?}; buttons={buttons:?}",
            window.viewport_size(),
            alignment.status(cx),
            source.input_bounds(),
            source.visible_row_range(),
            source.range_to_bounds(&(7..7)),
            alignment.project_range(1, 7..7, cx),
        )
    })
}

/// Use the Base control's own hit bounds, then require a legal public event at the current scene
/// revision. The fixture deliberately performs no guest mutation, isolating routing from Git.
fn click_control(mounted: &Mounted, ui: &mut VisualTestContext, id: &str, selector: &'static str) {
    let geometry = diagnostics(mounted, ui);
    eprintln!("[native-alignment-workbench] {id}: {geometry}");
    let hit = ui
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("missing actual button {id}: {geometry}"));
    let bounds = ui.update(|window, _| {
        let snapshots = gpui_base::test_support::snapshots(window);
        let button = snapshots
            .iter()
            .find(|snapshot| {
                snapshot.role() == Some(gpui_kit::Role::Button) && snapshot.bounds() == hit
            })
            .unwrap_or_else(|| panic!("missing actual button {id}: {geometry}"));
        let bounds = button.bounds();
        let viewport = Bounds::new(point(px(0.), px(0.)), window.viewport_size());
        assert!(button.visible(), "hidden {id}: {geometry}");
        assert!(button.focused().is_some(), "disabled {id}: {geometry}");
        assert!(
            bounds.size.width > px(0.)
                && bounds.size.height > px(0.)
                && viewport.contains(&bounds.origin)
                && viewport.contains(&bounds.bottom_right()),
            "button must be fully inside the actual work window: {geometry}",
        );
        bounds
    });
    let selection = ui.update(|_, cx| mounted.source.read(cx).selected_range());
    mounted.events.borrow_mut().clear();
    ui.simulate_click(bounds.center(), Default::default());
    settle(ui);
    assert_eq!(
        *mounted.events.borrow(),
        vec![UiEvent {
            revision: mounted.document.revision,
            node: id.into(),
            action: Action::Click,
        }],
        "a painted enabled control must route exactly one current public click: {geometry}",
    );
    ui.update(|_, cx| {
        assert_eq!(mounted.source.read(cx).value().as_ref(), RESULT);
        assert_eq!(mounted.source.read(cx).selected_range(), selection);
    });
}

#[gpui::test]
#[ignore = "native resizable workbench footer routing; run explicitly with --ignored --test-threads=1"]
fn native_code_alignment_resizable_workbench_footer_routes_click(cx: &mut TestAppContext) {
    let (mounted, ui) = mount_with_document(
        cx,
        document,
        RESULT.into(),
        true,
        true,
        size(px(1280.), px(800.)),
    );
    click_control(
        &mounted,
        ui,
        "merge-from-base",
        "plugin-button-merge-from-base",
    );
}

#[gpui::test]
#[ignore = "native resizable workbench rail routing; run explicitly with --ignored --test-threads=1"]
fn native_code_alignment_resizable_workbench_rails_route_clicks(cx: &mut TestAppContext) {
    let (mounted, ui) = mount_with_document(
        cx,
        document,
        RESULT.into(),
        true,
        true,
        size(px(1280.), px(800.)),
    );
    // Do not reveal the edge first: opening the production scene must leave its first conflict
    // actionable without an artificial scroll that would conceal a flexible-layout regression.
    click_control(
        &mounted,
        ui,
        "merge-block-0-left",
        "plugin-button-merge-block-0-left",
    );
    click_control(
        &mounted,
        ui,
        "merge-block-0-right",
        "plugin-button-merge-block-0-right",
    );
}

/// The main window reads the borrowed session for metadata without mounting another native input.
/// A bounded observation count turns a synchronous cross-window notification loop into a failure.
struct SessionReader {
    source: Entity<EditorState>,
    notifications: Rc<Cell<usize>>,
    _subscription: Subscription,
}

impl Render for SessionReader {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let _cursor = self.source.read(cx).cursor();
        div().size_full()
    }
}

/// Count actual paint traversals, including cached descendants, rather than only Root renders.
struct WorkbenchFrames {
    view: Entity<PluginView>,
    paints: Rc<Cell<usize>>,
}

impl Render for WorkbenchFrames {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let paints = self.paints.clone();
        div()
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .child(self.view.clone())
            .child(
                canvas(
                    |_, _, _| (),
                    move |_, _, _, _| {
                        let count = paints.get() + 1;
                        paints.set(count);
                        assert!(
                            count <= 256,
                            "shared-session work window never idled: {count} paints"
                        );
                    },
                )
                .absolute()
                .size_full(),
            )
    }
}

#[gpui::test]
#[ignore = "native two-window shared-session frame convergence; run explicitly with --ignored --test-threads=1"]
fn native_code_alignment_independent_workbench_stops_painting_with_a_main_session_reader(
    cx: &mut TestAppContext,
) {
    let (mounted, main) = mount_with_document(
        cx,
        document,
        RESULT.into(),
        true,
        true,
        size(px(1280.), px(800.)),
    );
    let notifications = Rc::new(Cell::new(0));
    let paints = Rc::new(Cell::new(0));
    let source = mounted.source.clone();
    let observed = notifications.clone();
    // Transfer the existing view to the independent window. The main Root retains only a native
    // session reader, matching EditorApp metadata while its source is borrowed by WorkWindow.
    main.update(|window, cx| {
        mounted.view.update(cx, |view, cx| view.retire_code_panes(cx));
        window.replace_root(cx, |window, cx| {
            let reader = cx.new(|cx| SessionReader {
                _subscription: cx.observe(&source, |reader: &mut SessionReader, _, cx| {
                    let count = reader.notifications.get() + 1;
                    reader.notifications.set(count);
                    assert!(
                        count <= 256,
                        "native geometry repeatedly notified the other window: {count} notifications"
                    );
                    cx.notify();
                }),
                source: source.clone(),
                notifications: observed.clone(),
            });
            Root::new(reader, window, cx)
        });
        // Record the metadata dependency before the other window paints its first native slice.
        window.draw(cx).clear(cx);
    });
    let view = mounted.view.clone();
    let paint_count = paints.clone();
    let handle = main.update(|_, cx| {
        let mut options = editor_ui::window::TitleBar::window_options();
        options.window_background = crate::theme::window_background(cx);
        options.window_bounds = Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(1280.), px(800.)),
            cx,
        )));
        options.window_min_size = Some(size(px(900.), px(520.)));
        cx.open_window(options, |window, cx| {
            view.update(cx, |view, cx| {
                view.update_document(
                    mounted.document.clone().revision(2),
                    Environment::default(),
                    window,
                    cx,
                );
            });
            let frames = cx.new(|_| WorkbenchFrames {
                view,
                paints: paint_count,
            });
            cx.new(|cx| Root::new(frames, window, cx))
        })
        .unwrap()
    });
    let mut work = VisualTestContext::from_window(handle.into(), &main.cx);
    settle(&mut work);
    assert!(
        paints.get() > 0,
        "the independent native window must actually paint"
    );
    let stable_paints = paints.get();
    // Do not manually draw here: only pending native callbacks or notifications can repaint.
    // The paint/notification guards above also bound run_until_parked if regression never parks.
    for _ in 0..8 {
        work.run_until_parked();
        main.run_until_parked();
    }
    assert_eq!(
        paints.get(),
        stable_paints,
        "idle shared session keeps requesting paint"
    );
    assert!(notifications.get() < 256);
    work.update(|_, cx| assert_eq!(group(&mounted, cx).status(cx), AlignmentStatus::Ready));

    // Suppressing geometry echoes must not suppress real input or the original session's Undo.
    let pointer = source_point(&mounted, &mut work, 0);
    work.simulate_click(pointer, Default::default());
    let before_input = notifications.get();
    work.simulate_input("中文😀");
    settle(&mut work);
    work.update(|_, cx| {
        assert_eq!(
            mounted.source.read(cx).value().as_ref(),
            format!("中文😀{RESULT}")
        );
    });
    assert!(
        notifications.get() > before_input,
        "real source input must still notify the main reader"
    );
    work.simulate_keystrokes("ctrl-z");
    settle(&mut work);
    work.update(|_, cx| assert_eq!(mounted.source.read(cx).value().as_ref(), RESULT));
    work.update(|window, cx| {
        mounted.source.update(cx, |source, cx| {
            source.replace_and_mark_text_in_range(None, "拼😀", Some(1..3), window, cx);
            assert!(source.marked_text_range(window, cx).is_some());
        });
    });
    settle(&mut work);
    work.update(|window, cx| {
        mounted.source.update(cx, |source, cx| {
            source.replace_text_in_range(None, "中文😀", window, cx);
        });
    });
    work.simulate_keystrokes("ctrl-z");
    settle(&mut work);
    work.update(|_, cx| assert_eq!(mounted.source.read(cx).value().as_ref(), RESULT));
}
