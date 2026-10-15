//! Real native rendering/input regressions for shared aligned geometry and source authority.
use super::*;
use gpui_kit::{
    Context, EntityInputHandler, Focusable as _, InputHandler as _, ParentElement as _, Point,
    Render, Styled as _, TestAppContext, TextInputConfiguration, UTF16Selection, VisualTestContext,
    Window, div, gpui, point, size,
};
use std::cell::Cell;

/// Source identity and group survive every render; test-owned text is only an expected-value fixture.
struct Fixture {
    group: AlignmentGroup,
    router: AlignedPointerRouter,
    editors: [Entity<EditorState>; 2],
    routed: Rc<Cell<usize>>,
    preedits: Rc<Cell<usize>>,
    headers: [Pixels; 2],
    top_offsets: [Pixels; 2],
}

impl Render for Fixture {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let mut root = div().size_full().flex().line_height(px(20.));
        for index in 0..2 {
            root = root.child(
                div()
                    .relative()
                    .top(self.top_offsets[index])
                    .flex()
                    .flex_col()
                    .w(px(320.))
                    .h(px(180.))
                    .child(div().h(self.headers[index]).flex_shrink_0())
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .child(self.group.pane(index).unwrap()),
                    ),
            );
        }
        self.router.wrap(root)
    }
}

/// A real alternate input delegate proves the alignment bridge does not bypass host-linked input.
struct HostInput {
    editor: Entity<EditorState>,
    routed: Rc<Cell<usize>>,
    preedits: Rc<Cell<usize>>,
}

impl EntityInputHandler for HostInput {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        self.editor.update(cx, |editor, cx| {
            editor.text_for_range(range, adjusted, window, cx)
        })
    }
    fn selected_text_range(
        &mut self,
        ignore: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        self.editor.update(cx, |editor, cx| {
            editor.selected_text_range(ignore, window, cx)
        })
    }
    fn marked_text_range(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.editor
            .update(cx, |editor, cx| editor.marked_text_range(window, cx))
    }
    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editor
            .update(cx, |editor, cx| editor.unmark_text(window, cx));
    }
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.routed.set(self.routed.get() + 1);
        self.editor.update(cx, |editor, cx| {
            editor.replace_text_in_range(range, text, window, cx)
        });
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.preedits.set(self.preedits.get() + 1);
        self.editor.update(cx, |editor, cx| {
            editor.replace_and_mark_text_in_range(range, text, selected, window, cx)
        });
    }
    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        self.editor.update(cx, |editor, cx| {
            editor.bounds_for_range(range, bounds, window, cx)
        })
    }
    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<usize> {
        self.editor.update(cx, |editor, cx| {
            editor.character_index_for_point(point, window, cx)
        })
    }
    fn accepts_text_input(&self, _: &mut Window, cx: &mut Context<Self>) -> bool {
        self.editor.read(cx).presentation().is_editable()
    }
    fn text_input_configuration(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> TextInputConfiguration {
        self.editor
            .update(cx, |editor, cx| editor.text_input_configuration(window, cx))
    }
}

fn content(insertions: bool) -> String {
    (0..32).map(|row| {
        let prefix = if insertions && [1, 8, 18].contains(&row) { "inserted\ninserted again\n" } else { "" };
        format!("{prefix}row{row:02} abcdefghijklmnopqrstuvwxyz 中文😀 more wrapped words and tail\n")
    }).collect()
}

fn byte_at(text: &str, line: usize) -> usize {
    text.find(&format!("row{line:02}")).unwrap()
}

fn mount(cx: &mut TestAppContext) -> (Entity<Fixture>, &mut VisualTestContext) {
    mount_sources(cx, [content(false), content(true)])
}

fn mount_sources(
    cx: &mut TestAppContext,
    sources: [String; 2],
) -> (Entity<Fixture>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::ui::typography::init(cx);
        crate::ui::theme::apply_theme(crate::ui::theme::builtin_theme(false), cx);
    });
    let (view, ui) = cx.add_window_view(|window, cx| {
        let router = AlignedPointerRouter::default();
        let editors = std::array::from_fn(|index| {
            let editor = cx.new(|cx| {
                EditorState::new(window, cx)
                    .line_number(false)
                    .soft_wrap(true)
            });
            editor.update(cx, |editor, cx| {
                editor.set_editor_paddings(Default::default());
                editor.set_value(sources[index].clone(), window, cx);
                editor.set_readonly(index == 1, cx);
            });
            editor
        });
        let routed = Rc::new(Cell::new(0));
        let preedits = Rc::new(Cell::new(0));
        let bridge = cx.new(|_| HostInput {
            editor: editors[0].clone(),
            routed: routed.clone(),
            preedits: preedits.clone(),
        });
        let factory: AlignmentInputFactory = Rc::new(move |bounds, _, _| {
            Box::new(gpui_kit::ElementInputHandler::new(bounds, bridge.clone()))
        });
        let mut group = AlignmentGroup::new(cx);
        group
            .configure(
                editors
                    .iter()
                    .enumerate()
                    .map(|(index, editor)| AlignmentPane {
                        editor: editor.clone(),
                        revision: 1,
                        label: format!("source {index}"),
                        input_factory: (index == 0).then(|| factory.clone()),
                        native_authority_factory: None,
                    })
                    .collect(),
                &router,
                cx,
            )
            .unwrap();
        Fixture {
            group,
            router,
            editors,
            routed,
            preedits,
            headers: [px(0.); 2],
            top_offsets: [px(0.); 2],
        }
    });
    ui.simulate_resize(size(px(640.), px(180.)));
    ui.update(|window, cx| {
        window.activate_window();
        // Scrollbar transitions are separately tested by the local control. Snap them here so
        // the viewport/idle assertions distinguish geometry work from intentional fade animation.
        cx.set_reduce_motion(true);
    });
    ui.run_until_parked();
    settle(ui);
    (view, ui)
}

/// Native direct state paints can change layout while painting. Settle only the finite group
/// measurement rounds; these draws never replace source text or inject semantic plugin events.
fn settle(ui: &mut VisualTestContext) {
    for _ in 0..6 {
        ui.update(|window, cx| {
            window.draw(cx).clear(cx);
            window.simulate_next_frame(cx);
        });
        ui.run_until_parked();
    }
}

#[track_caller]
fn painted_caret(view: &Entity<Fixture>, window: &Window, cx: &App) {
    let fixture = view.read(cx);
    let native = fixture.editors[0].read(cx);
    let (caret, _) = native.cursor_layout().unwrap();
    let expected = fixture.group.state.read(cx).project(0, caret).unwrap();
    let scale = window.scale_factor();
    assert!(
        window
            .painted_quads()
            .iter()
            .any(
                |quad| (quad.bounds.left().0 / scale - f32::from(expected.left())).abs() < 0.5
                    && (quad.bounds.top().0 / scale - f32::from(expected.top())).abs() < 0.5
                    && (quad.bounds.size.width.0 / scale - f32::from(expected.size.width)).abs()
                        < 0.5
                    && (quad.bounds.size.height.0 / scale - f32::from(expected.size.height)).abs()
                        < 0.5
                    && quad.background.as_solid().is_some_and(|color| color.a > 0.)
                    && quad.content_mask.bounds.contains(&quad.bounds.center())
            ),
        "painted source caret {expected:?}"
    );
}

#[gpui::test]
fn aligned_editor_preserves_gap_click_host_input_preedit_undo_and_shared_scroll(
    cx: &mut TestAppContext,
) {
    let (view, ui) = mount(cx);
    let original = content(false);
    let target = byte_at(&original, 22) + 8;
    ui.update(|_, cx| {
        let (group, editor) = {
            let fixture = view.read(cx);
            assert_eq!(fixture.group.status(cx), AlignmentStatus::Ready);
            (fixture.group.state.clone(), fixture.editors[0].clone())
        };
        group.update(cx, |state, cx| {
            let measure = state.measurements[0].as_ref().unwrap();
            let source = editor.read(cx).range_to_bounds(&(target..target)).unwrap();
            let display = state.layout.as_ref().unwrap().panes[0]
                .source_to_display(measure.source_y(source))
                .unwrap();
            state.scroll.set(px(display - 80.));
            cx.notify();
        });
    });
    settle(ui);
    let click = ui.update(|_, cx| {
        let bounds = view
            .read(cx)
            .group
            .project_range(0, target..target, cx)
            .unwrap();
        point(bounds.left() + px(0.4), bounds.center().y)
    });
    ui.simulate_click(click, Default::default());
    ui.run_until_parked();
    settle(ui);
    ui.update(|window, cx| {
        let fixture = view.read(cx);
        assert_eq!(fixture.editors[0].read(cx).cursor(), target);
        assert!(fixture.group.scroll_offset(cx) > px(500.));
        let right = content(true);
        for line in [21, 22, 23] {
            let left_at = byte_at(&original, line);
            let right_at = byte_at(&right, line);
            let group = fixture.group.state.read(cx);
            let left = group
                .project(
                    0,
                    fixture.editors[0]
                        .read(cx)
                        .range_to_bounds(&(left_at..left_at))
                        .unwrap(),
                )
                .unwrap();
            let right = group
                .project(
                    1,
                    fixture.editors[1]
                        .read(cx)
                        .range_to_bounds(&(right_at..right_at))
                        .unwrap(),
                )
                .unwrap();
            assert!((left.top() - right.top()).abs() < px(0.5));
        }
        painted_caret(&view, window, cx);
    });
    ui.simulate_input("中文😀");
    settle(ui);
    ui.update(|window, cx| {
        assert!(
            view.read(cx).routed.get() >= 3,
            "the caller's original input delegate must run"
        );
        painted_caret(&view, window, cx);
    });
    ui.simulate_keystrokes("ctrl-z");
    settle(ui);
    ui.update(|window, cx| {
        assert_eq!(view.read(cx).editors[0].read(cx).value().as_ref(), original);
        let fixture = view.read(cx);
        let editor = fixture.editors[0].clone();
        let frame = fixture.group.state.read(cx).frames[0].clone().unwrap();
        let factory = fixture.group.state.read(cx).sources[0]
            .input_factory
            .clone()
            .unwrap();
        let delegate = factory(editor.read(cx).input_bounds(), window, cx);
        let mut handler = input::AlignedInputHandler {
            delegate,
            editor,
            projection: Some(frame),
            suppress_geometry: false,
        };
        handler.replace_and_mark_text_in_range(None, "拼😀", Some(1..3), window, cx);
        assert!(handler.marked_text_range(window, cx).is_some());
        assert_eq!(view.read(cx).preedits.get(), 1);
    });
    settle(ui);
    ui.update(|window, cx| {
        let fixture = view.read(cx);
        let editor = fixture.editors[0].clone();
        let frame = fixture.group.state.read(cx).frames[0].clone().unwrap();
        let factory = fixture.group.state.read(cx).sources[0]
            .input_factory
            .clone()
            .unwrap();
        let delegate = factory(editor.read(cx).text_bounds().unwrap(), window, cx);
        let mut handler = input::AlignedInputHandler {
            delegate,
            editor,
            projection: Some(frame),
            suppress_geometry: false,
        };
        let mark = handler.marked_text_range(window, cx).unwrap();
        assert!(handler.bounds_for_range(mark, window, cx).is_some());
        painted_caret(&view, window, cx);
        handler.replace_text_in_range(None, "中文😀", window, cx);
    });
    settle(ui);
    // The preedit and committed Unicode share the native transaction; native history remains
    // authoritative regardless of how many visual gaps or source slices were painted.
    ui.simulate_keystrokes("ctrl-z");
    settle(ui);
    ui.update(|window, cx| {
        assert_eq!(view.read(cx).editors[0].read(cx).value().as_ref(), original);
        painted_caret(&view, window, cx);
    });
}

#[gpui::test]
fn aligned_editor_shared_budget_rebuilds_zero_cost_edits_and_retirement_returns_reservation(
    cx: &mut TestAppContext,
) {
    let (view, ui) = mount_sources(cx, ["alpha".into(), "alpha".into()]);
    ui.update(|window, cx| {
        let (group, editor) = {
            let fixture = view.read(cx);
            (fixture.group.clone(), fixture.editors[0].clone())
        };
        assert_eq!(
            group.state.read(cx).ledger().unwrap().get(),
            super::super::aligned_rows::MAX_LCS_CELLS
        );
        editor.update(cx, |editor, cx| {
            editor.replace_text_in_range(Some(0..1), "b", window, cx)
        });
    });
    settle(ui);
    ui.update(|_, cx| {
        let (first, router, sources) = {
            let fixture = view.read(cx);
            assert_eq!(fixture.group.status(cx), AlignmentStatus::Ready);
            (
                fixture.group.clone(),
                fixture.router.clone(),
                fixture.group.state.read(cx).sources.clone(),
            )
        };
        let ledger = Rc::new(Cell::new(1));
        let mut first = first;
        let mut sources = sources;
        for source in &mut sources {
            source.revision += 1;
        }
        first
            .configure_shared(sources.clone(), &router, ledger.clone(), cx)
            .unwrap();
        assert_eq!(ledger.get(), 0);
        let mut second = AlignmentGroup::new(cx);
        assert_eq!(
            second.configure_shared(sources.clone(), &router, ledger.clone(), cx),
            Err(AlignmentError::Budget)
        );
        let retained_old = first.clone();
        first.retire(cx);
        assert_eq!(
            ledger.get(),
            1,
            "retirement returns the reservation even with retained old frame/group clones"
        );
        second
            .configure_shared(sources, &router, ledger.clone(), cx)
            .unwrap();
        assert!(second.state.read(cx).rows.is_some());
        assert_eq!(ledger.get(), 0);
        retained_old.state.update(cx, |state, cx| state.rebuild(cx));
        assert_eq!(
            ledger.get(),
            0,
            "a retained retired view cannot rebuild or spend the returned reservation"
        );
        second.retire(cx);
        assert_eq!(ledger.get(), 1);
    });
}

#[gpui::test]
fn aligned_editor_budget_fallback_preserves_host_input_and_native_overlays_pause_the_group(
    cx: &mut TestAppContext,
) {
    let (view, ui) = mount_sources(cx, ["alpha\nbeta\n".into(), "alpha\nbeta\n".into()]);
    ui.update(|_, cx| {
        let group = view.read(cx).group.clone();
        group.set_enabled(false, cx);
        assert_eq!(group.status(cx), AlignmentStatus::Disabled);
        group.set_enabled(true, cx);
    });
    settle(ui);
    ui.update(|_, cx| {
        let editor = view.read(cx).editors[0].clone();
        editor.update(cx, |editor, cx| editor.open_search(false, cx));
    });
    settle(ui);
    ui.update(|_, cx| {
        let (group, editor) = {
            let fixture = view.read(cx);
            (fixture.group.clone(), fixture.editors[0].clone())
        };
        assert_eq!(group.status(cx), AlignmentStatus::NativeOverlays);
        let native = editor.read(cx).range_to_bounds(&(0..0)).unwrap();
        assert_eq!(group.project_bounds(0, native, cx), Some(native));
        editor.update(cx, |editor, cx| editor.close_search(cx));
    });
    settle(ui);
    ui.update(|_, cx| {
        let editor = view.read(cx).editors[0].clone();
        editor.update(cx, |editor, cx| {
            editor.present_code_actions(
                vec![gpui_base::input::CodeActionItem {
                    provider_id: "aligned-test".into(),
                    action: lsp_types::CodeAction {
                        title: "native action".into(),
                        ..Default::default()
                    },
                }],
                cx,
            );
        });
    });
    settle(ui);
    ui.update(|_, cx| {
        let (group, editor) = {
            let fixture = view.read(cx);
            (fixture.group.clone(), fixture.editors[0].clone())
        };
        assert_eq!(group.status(cx), AlignmentStatus::NativeOverlays);
        editor.update(cx, |editor, cx| editor.dismiss_code_action_overlay(cx));
    });
    settle(ui);
    ui.update(|window, cx| {
        let (mut group, router, editor, mut sources) = {
            let fixture = view.read(cx);
            (
                fixture.group.clone(),
                fixture.router.clone(),
                fixture.editors[0].clone(),
                fixture.group.state.read(cx).sources.clone(),
            )
        };
        assert_eq!(group.status(cx), AlignmentStatus::Ready);
        editor.update(cx, |editor, cx| {
            editor.replace_text_in_range(Some(0..1), "changed", window, cx);
            editor.focus(window, cx);
        });
        for source in &mut sources {
            source.revision += 1;
        }
        assert_eq!(
            group.configure_shared(sources, &router, Rc::new(Cell::new(0)), cx),
            Err(AlignmentError::Budget)
        );
    });
    settle(ui);
    let before = view.read_with(ui, |fixture, _| fixture.routed.get());
    ui.simulate_input("中");
    settle(ui);
    ui.update(|_, cx| {
        let fixture = view.read(cx);
        assert_eq!(
            fixture.group.status(cx),
            AlignmentStatus::Unavailable(AlignmentError::Budget)
        );
        assert!(
            fixture.routed.get() > before,
            "even refused alignment must keep the caller's host input pipeline"
        );
        assert!(fixture.editors[0].read(cx).value().contains('中'));
    });
}

mod ime;
mod origins;
mod scrollbars;
mod unsupported;

#[gpui::test]
fn aligned_editor_page_through_oversized_gap_keeps_real_cursor_and_settles_idle(
    cx: &mut TestAppContext,
) {
    let right = format!(
        "start\n{}end\n",
        (0..80)
            .map(|row| format!("insert{row}\n"))
            .collect::<String>()
    );
    let (view, ui) = mount_sources(cx, ["start\nend\n".into(), right]);
    let click = ui.update(|_, cx| {
        let bounds = view.read(cx).group.project_range(0, 0..0, cx).unwrap();
        point(bounds.left() + px(0.4), bounds.center().y)
    });
    ui.simulate_click(click, Default::default());
    ui.run_until_parked();
    settle(ui);
    ui.simulate_keystrokes("pagedown");
    ui.run_until_parked();
    settle(ui);
    ui.update(|_, cx| {
        let fixture = view.read(cx);
        assert_eq!(
            fixture.editors[0].read(cx).cursor(),
            0,
            "a blank page must not manufacture a source position"
        );
        assert!(
            fixture.group.scroll_offset(cx) >= px(160.),
            "PageDown must advance even while its target is a gap"
        );
    });
    for _ in 0..4 {
        ui.update(|window, cx| {
            window.draw(cx).clear(cx);
            assert_eq!(
                window.simulate_next_frame(cx),
                0,
                "settled alignment must not schedule itself forever"
            );
        });
        ui.run_until_parked();
    }
}
