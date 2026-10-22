//! Native fold disclosures, synchronized projection and input keep their original source owners.
use super::*;
use gpui_base::input::InputEvent;
use gpui_kit::{
    Context, ParentElement as _, Render, Styled as _, TestAppContext, VisualTestContext, Window,
    div, gpui, size,
};

/// Both pane counts use the same native controls and one shared unchanged-region model.
struct Fixture {
    /// Geometry membership, never a text or selection owner.
    group: AlignmentGroup,
    /// Original source entities remain mounted through collapsing and native-search pauses.
    editors: Vec<Entity<EditorState>>,
    /// Root-level router must leave real disclosure button gestures untranslated.
    router: AlignedPointerRouter,
    /// Retain original expectations separately from product state.
    originals: Vec<String>,
    /// Counts only native text-change events, which presentation actions must never generate.
    changes: Rc<Cell<usize>>,
    /// Event counters stay alive as long as their fixture sources.
    _subscriptions: Vec<gpui_kit::Subscription>,
}

impl Render for Fixture {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let mut panes = div().size_full().flex();
        for index in 0..self.editors.len() {
            panes = panes.child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(self.group.pane(index).unwrap()),
            );
        }
        self.router.wrap(panes)
    }
}

/// Chinese/emoji, additions, three-way changes and unequal wrapping exercise the real geometry.
fn source(pane: usize) -> String {
    (0..70)
        .map(|line| {
            if (pane, line) == (1, 5) {
                "changed result 中文😀\nadded result\n".into()
            } else if (pane, line) == (2, 58) {
                "third parent change\n".into()
            } else {
                format!(
                    "common {line}: {}\n",
                    "中文😀 ".repeat(if line == 30 { 18 } else { 1 })
                )
            }
        })
        .collect()
}

/// Mount the original native entities, package-style SVG bytes and actual source notifications.
fn mount(cx: &mut TestAppContext, count: usize) -> (Entity<Fixture>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::typography::init(cx);
        crate::theme::apply_theme(crate::theme::builtin_theme(false), cx);
    });
    let (view, ui) = cx.add_window_view(|window, cx| {
        let originals = (0..count).map(source).collect::<Vec<_>>();
        let editors = originals
            .iter()
            .map(|text| {
                cx.new(|cx| {
                    let mut editor = EditorState::new(window, cx)
                        .line_number(true)
                        .soft_wrap(true);
                    editor.set_value(text.clone(), window, cx);
                    editor
                })
            })
            .collect::<Vec<_>>();
        let changes = Rc::new(Cell::new(0));
        let subscriptions = editors
            .iter()
            .map(|editor| {
                let changes = changes.clone();
                cx.subscribe(editor, move |_, _, event, _| {
                    if matches!(event, InputEvent::Change) {
                        changes.set(changes.get() + 1);
                    }
                })
            })
            .collect();
        let router = AlignedPointerRouter::default();
        let mut group = AlignmentGroup::new(cx);
        group
            .configure(
                editors
                    .iter()
                    .enumerate()
                    .map(|(index, editor)| AlignmentPane {
                        editor: editor.clone(),
                        revision: 1,
                        label: format!("fold source {index}"),
                        input_factory: None,
                        native_authority_factory: None,
                    })
                    .collect(),
                &router,
                cx,
            )
            .unwrap();
        let svg: std::sync::Arc<[u8]> = std::sync::Arc::from(
            include_bytes!("../../assets/icons/explorer-expand-all.svg").as_slice(),
        );
        group.set_folding(UnchangedFolds::new(3, [svg.clone(), svg]), cx);
        group.set_inner_gutters(true, cx);
        Fixture {
            group,
            editors,
            router,
            originals,
            changes,
            _subscriptions: subscriptions,
        }
    });
    ui.simulate_resize(size(px(1100.), px(600.)));
    settle(ui);
    (view, ui)
}

/// Complete bounded native measurements after a presentation/scale change.
fn settle(ui: &mut VisualTestContext) {
    for _ in 0..8 {
        ui.update(|window, cx| {
            window.draw(cx).clear(cx);
            window.simulate_next_frame(cx);
        });
        ui.run_until_parked();
    }
}

/// Physical disclosure clicks expand every peer, while text, selection and native events stay put.
#[gpui::test]
fn unchanged_folds_native_disclosures_preserve_two_and_three_source_sessions(
    cx: &mut TestAppContext,
) {
    for count in [2, 3] {
        let (view, ui) = mount(cx, count);
        for dark in [false, true] {
            ui.simulate_scale_factor_change(if dark { 1.5 } else { 1. });
            ui.update(|_, cx| {
                crate::theme::apply_theme(crate::theme::builtin_theme(dark), cx);
                let group = view.read(cx).group.clone();
                group.fold_unchanged(true, cx);
            });
            settle(ui);
            for index in 0..count {
                let selector: &'static str =
                    Box::leak(format!("aligned-vertical-scrollbar-{index}").into_boxed_str());
                assert_eq!(
                    ui.debug_bounds(selector).is_some(),
                    index + 1 == count,
                    "only the final comparison source exposes a vertical rail"
                );
            }
            ui.update(|_, cx| {
                let fixture = view.read(cx);
                let widths = fixture
                    .editors
                    .iter()
                    .map(|editor| editor.read(cx).input_bounds().size.width)
                    .collect::<Vec<_>>();
                assert!(
                    widths
                        .iter()
                        .all(|width| (*width - widths[0]).abs() < px(1.)),
                    "two/three comparison sources must retain equal width: {widths:?}"
                );
            });
            let (range, hidden, selections, events) = ui.update(|_, cx| {
                let fixture = view.read(cx);
                let group = fixture.group.state.read(cx);
                assert_eq!(group.effective_status(), AlignmentStatus::Ready);
                let range = group.layout.as_ref().unwrap().folds[0].0.clone();
                let rows = group.rows.as_ref().unwrap();
                let hidden = (0..count)
                    .map(|pane| {
                        rows.source_rows[pane][rows.slots[pane][range.start].unwrap()].start
                    })
                    .collect::<Vec<_>>();
                for (pane, byte) in hidden.iter().enumerate() {
                    assert!(
                        fixture
                            .group
                            .project_range(pane, *byte..*byte, cx)
                            .is_none()
                    );
                }
                (
                    range,
                    hidden,
                    fixture
                        .editors
                        .iter()
                        .map(|editor| editor.read(cx).selected_range())
                        .collect::<Vec<_>>(),
                    fixture.changes.get(),
                )
            });
            let selector: &'static str =
                Box::leak(format!("diff-unchanged-1-{}", range.start).into_boxed_str());
            let position = ui
                .debug_bounds(selector)
                .expect("a real native disclosure must be visible")
                .center();
            ui.simulate_click(position, Default::default());
            settle(ui);
            ui.update(|_, cx| {
                let fixture = view.read(cx);
                assert_eq!(
                    fixture.changes.get(),
                    events,
                    "folding cannot emit a document change"
                );
                for (pane, editor) in fixture.editors.iter().enumerate() {
                    assert_eq!(editor.read(cx).value().as_ref(), fixture.originals[pane]);
                    assert_eq!(
                        editor.read(cx).selected_range(),
                        selections[pane],
                        "a disclosure cannot select hidden native text"
                    );
                    let native = editor
                        .read(cx)
                        .range_to_bounds(&(hidden[pane]..hidden[pane]))
                        .unwrap();
                    assert!(fixture.group.project_bounds(pane, native, cx).is_some(), "disclosure must expand its original interval: count={count}, pane={pane}, dark={dark}, remaining={:?}", fixture.group.state.read(cx).layout.as_ref().map(|layout| &layout.folds));
                }
                let group = fixture.group.clone();
                group.fold_unchanged(true, cx);
            });
            settle(ui);
            ui.update(|_, cx| {
                let group = view.read(cx).group.clone();
                assert!(group.reveal_offset(1, hidden[1], cx));
            });
            settle(ui);
            ui.update(|_, cx| {
                let fixture = view.read(cx);
                assert!(
                    fixture
                        .group
                        .project_range(1, hidden[1]..hidden[1], cx)
                        .is_some(),
                    "source navigation must unfold its destination"
                );
                let group = fixture.group.clone();
                group.fold_unchanged(false, cx);
            });
            settle(ui);
            ui.update(|_, cx| {
                let fixture = view.read(cx);
                assert!(
                    fixture
                        .group
                        .state
                        .read(cx)
                        .layout
                        .as_ref()
                        .unwrap()
                        .folds
                        .is_empty()
                );
                assert_eq!(fixture.changes.get(), events);
            });
            // Real native caret movement must not leave focus/IME anchored to hidden pixels.
            let editor = ui.update(|_, cx| view.read(cx).editors[0].clone());
            ui.update(|_, cx| editor.update(cx, |state, cx| state.set_selected_range(0..0, cx)));
            settle(ui);
            ui.update(|_, cx| {
                let group = view.read(cx).group.clone();
                group.fold_unchanged(true, cx);
            });
            settle(ui);
            ui.update(|_, cx| {
                editor.update(cx, |state, cx| {
                    state.set_selected_range(hidden[0]..hidden[0], cx)
                })
            });
            settle(ui);
            ui.update(|_, cx| {
                let fixture = view.read(cx);
                for (pane, byte) in hidden.iter().enumerate() {
                    let native = fixture.editors[pane]
                        .read(cx)
                        .range_to_bounds(&(*byte..*byte))
                        .unwrap();
                    assert!(
                        fixture.group.project_bounds(pane, native, cx).is_some(),
                        "a native cursor move reveals every peer"
                    );
                }
                assert_eq!(fixture.changes.get(), events);
            });
        }
    }
}
