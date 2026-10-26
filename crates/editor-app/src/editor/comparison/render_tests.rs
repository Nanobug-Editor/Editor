//! Native comparison layout, shared rail gestures and row paint retain original source ownership.
use super::*;
use editor_ui::aligned_pointer::AlignedPointerRouter;
use gpui_kit::{
    Render, ScrollDelta, ScrollWheelEvent, TestAppContext, TouchPhase, VisualTestContext, gpui,
    size,
};

/// Retain the same production line/character layers and native entities across theme/geometry changes.
struct Fixture {
    /// Shared display geometry for the original native comparison sources.
    group: AlignmentGroup,
    /// Window-owned pointer bridge used by the aligned presentation.
    router: AlignedPointerRouter,
    /// Existing native text/Undo owners reused by independent and unified presentation.
    editors: [Entity<EditorState>; 2],
    /// Full-width backgrounds owned only by this comparison.
    line_marks: [RowBackgroundCollection; 2],
    /// Character emphasis stays in native range decoration layers.
    _inline_marks: [RangeDecorationCollection; 2],
    /// Closing the comparison withdraws its own row layers while preserving the native sources.
    decorated: bool,
    /// Independent native mounting also serves readonly unified comparisons.
    aligned: bool,
}

impl Render for Fixture {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let hunks = diff::compare(&source(0), &source(1)).unwrap();
        let colors = comparison_colors(cx);
        for index in 0..2 {
            self.line_marks[index].set(
                if self.decorated {
                    comparison_marks(&hunks, colors, index == 0)
                } else {
                    Vec::new()
                },
                cx,
            );
        }
        let panes = (0..2)
            .map(|index| {
                let content = if self.aligned {
                    self.group.pane(index).unwrap()
                } else {
                    editor_ui::source_editor::Editor::new(&self.editors[index])
                        .identity("work-window-document", None)
                        .document_commands(true)
                        .tools(false)
                        .into_any_element()
                };
                div()
                    .id(("comparison-paint-pane", index))
                    .debug_selector(move || format!("comparison-paint-pane-{index}"))
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(content)
            })
            .collect::<Vec<_>>();
        self.router.wrap(div().size_full().flex().children(panes))
    }
}

/// A wrapped changed line, a short change and unchanged neighbors distinguish full-row painting.
fn source(index: usize) -> String {
    format!(
        "unchanged\n共同前缀 {} 共同后缀\nshort {}\nunchanged tail\n",
        "中文 wrapping ".repeat(15) + if index == 0 { "old" } else { "new" },
        if index == 0 { "old" } else { "new" }
    )
}

/// Mount actual aligned native code controls using the same decorations as document comparisons.
fn mount(cx: &mut TestAppContext) -> (Entity<Fixture>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        editor_ui::typography::init(cx);
        crate::theme::apply_theme(crate::theme::builtin_theme(false), cx);
    });
    let (view, ui) = cx.add_window_view(|window, cx| {
        let editors: [Entity<EditorState>; 2] = std::array::from_fn(|index| {
            cx.new(|cx| {
                let mut editor = EditorState::new(window, cx)
                    .line_number(true)
                    .soft_wrap(true);
                editor.set_value(source(index), window, cx);
                editor
            })
        });
        let hunks = diff::compare(&source(0), &source(1)).unwrap();
        let colors = comparison_colors(cx);
        let line_marks = std::array::from_fn(|index| {
            editors[index].update(cx, |_, cx| {
                RowBackgroundCollection::new(
                    &cx.entity(),
                    comparison_marks(&hunks, colors, index == 0),
                    cx,
                )
            })
        });
        let inline_marks = presentation::create_inline_marks(&editors[0], &editors[1], &hunks, cx);
        let router = AlignedPointerRouter::default();
        let mut group = AlignmentGroup::new(cx);
        group
            .configure(
                editors
                    .iter()
                    .enumerate()
                    .map(|(index, editor)| {
                        AlignmentPane::new(editor.clone(), 1, format!("comparison {index}"))
                    })
                    .collect(),
                &router,
                cx,
            )
            .unwrap();
        group.set_inner_gutters(true, cx);
        Fixture {
            group,
            router,
            editors,
            line_marks,
            _inline_marks: inline_marks,
            decorated: true,
            aligned: true,
        }
    });
    ui.simulate_resize(size(px(1000.), px(640.)));
    // Snap Base's intentional thumb fade so idle checks observe comparison geometry only.
    ui.update(|_, cx| cx.set_reduce_motion(true));
    settle(ui);
    (view, ui)
}

/// Finish the component's bounded native geometry rounds after wraps, resizing and scrolling.
fn settle(ui: &mut VisualTestContext) {
    for _ in 0..8 {
        ui.update(|window, cx| {
            window.draw(cx).clear(cx);
            window.simulate_next_frame(cx);
        });
        ui.run_until_parked();
    }
}

/// Equal comparison columns must give identical native text the same measured wrapping width.
#[gpui::test]
fn comparison_sources_have_equal_wrapping_width(cx: &mut TestAppContext) {
    let (view, ui) = mount(cx);
    let text = format!(
        "相同内容 same words {}\n",
        "中文😀 abcdefghijklmnopqrstuvwxyz ".repeat(45)
    );
    ui.update(|window, cx| {
        let editors = view.read(cx).editors.clone();
        for editor in editors {
            editor.update(cx, |state, cx| state.set_value(text.clone(), window, cx));
        }
    });
    for dark in [false, true] {
        ui.update(|_, cx| crate::theme::apply_theme(crate::theme::builtin_theme(dark), cx));
        for scale in [1., 1.5] {
            ui.simulate_scale_factor_change(scale);
            for width in [650., 1000., 1311.] {
                ui.simulate_resize(size(px(width), px(640.)));
                settle(ui);
                let columns = ["comparison-paint-pane-0", "comparison-paint-pane-1"]
                    .map(|name| ui.debug_bounds(name).unwrap());
                // Odd physical widths may allocate one extra pixel to a column. Text wrapping
                // must still use one common width rather than inheriting that rounding error.
                assert!((columns[0].size.width - columns[1].size.width).abs() <= px(1.));
                ui.update(|_, cx| {
                    let fixture = view.read(cx);
                    let widths = fixture
                        .editors
                        .each_ref()
                        .map(|editor| editor.read(cx).input_bounds().size.width);
                    assert!(
                        (widths[0] - widths[1]).abs() < px(0.5),
                        "same text receives different native wrap widths: {widths:?}"
                    );
                    assert_same_wrapping(fixture, &text, cx);
                });
                ui.update(|window, cx| {
                    window.draw(cx).clear(cx);
                    assert_eq!(
                        window.simulate_next_frame(cx),
                        0,
                        "common width must settle without scheduling a repaint loop"
                    );
                });
            }
        }
    }
}

/// Compare every original character in `common` using Base's completed source geometry.
fn assert_same_wrapping(fixture: &Fixture, common: &str, cx: &App) {
    let editors = fixture.editors.each_ref().map(|editor| editor.read(cx));
    let origins = editors.map(|editor| editor.range_to_bounds(&(0..0)).unwrap().origin);
    let widths = [0, 1].map(|side| editors[side].input_bounds().right() - origins[side].x);
    assert!(
        (widths[0] - widths[1]).abs() < px(0.5),
        "native text widths differ after gutters: {widths:?}"
    );
    for (byte, _) in common.char_indices() {
        let y = editors.map(|editor| editor.range_to_bounds(&(byte..byte)).unwrap().top());
        assert!(
            (y[0] - origins[0].y - (y[1] - origins[1].y)).abs() < px(0.5),
            "same character wraps differently at byte {byte}: {y:?}"
        );
    }
}

/// One source crossing 999 lines cannot steal another character of common-line wrap width.
#[gpui::test]
fn comparison_equal_wrapping_survives_different_line_number_digit_counts(cx: &mut TestAppContext) {
    let (view, ui) = mount(cx);
    let common = format!(
        "共享长段落 {}\n",
        "中文😀 abcdefghijklmnopqrstuvwxyz ".repeat(45)
    );
    let texts = [
        common.clone(),
        common.clone()
            + &(0..1000)
                .map(|row| format!("added {row}\n"))
                .collect::<String>(),
    ];
    ui.update(|window, cx| {
        let editors = view.read(cx).editors.clone();
        for (editor, text) in editors.into_iter().zip(texts) {
            editor.update(cx, |state, cx| state.set_value(text, window, cx));
        }
    });
    settle(ui);
    ui.update(|_, cx| assert_same_wrapping(view.read(cx), &common, cx));
}

/// A synchronized comparison exposes its one vertical gesture surface at the far right only.
#[gpui::test]
fn comparison_has_one_shared_vertical_scrollbar(cx: &mut TestAppContext) {
    let (view, ui) = mount(cx);
    let text = (0..100)
        .map(|row| format!("共同内容 {row}\n"))
        .collect::<String>();
    ui.update(|window, cx| {
        for editor in &view.read(cx).editors.clone() {
            editor.update(cx, |state, cx| state.set_value(text.clone(), window, cx));
        }
    });
    ui.simulate_resize(size(px(1000.), px(220.)));
    settle(ui);
    assert!(
        ui.debug_bounds("aligned-vertical-scrollbar-0").is_none(),
        "the center seam must not contain another vertical scrollbar"
    );
    assert!(
        ui.debug_bounds("aligned-vertical-scrollbar-1").is_some(),
        "the rightmost shared scrollbar must remain available"
    );
    let selections = ui.update(|_, cx| {
        view.read(cx)
            .editors
            .each_ref()
            .map(|e| e.read(cx).selected_range())
    });
    let rail = ui.debug_bounds("aligned-vertical-scrollbar-1").unwrap();
    // The configured scrolling-mode thumb is intentionally hidden before the first scroll.
    // Reveal it through a real wheel gesture, then grab its painted pixels rather than guessing
    // that an invisible track already accepts mouse-down at its theoretical origin.
    ui.simulate_event(ScrollWheelEvent {
        position: point(rail.left() - px(40.), rail.center().y),
        delta: ScrollDelta::Pixels(point(px(0.), px(-36.))),
        touch_phase: TouchPhase::Moved,
        modifiers: Default::default(),
    });
    settle(ui);
    let before = ui.update(|_, cx| view.read(cx).group.scroll_offset(cx));
    assert!(before > px(0.), "wheel must reveal the shared scrollbar");
    // The original Base thumb receives physical capture; the aligned editor cannot intercept it
    // as a source-selection drag. Both peers must follow the same pixel viewport afterwards.
    let start = ui.update(|window, _| {
        let scale = window.scale_factor();
        window
            .painted_quads()
            .iter()
            .find_map(|quad| {
                let bounds = Bounds::new(
                    point(
                        px(quad.bounds.origin.x.0 / scale),
                        px(quad.bounds.origin.y.0 / scale),
                    ),
                    size(
                        px(quad.bounds.size.width.0 / scale),
                        px(quad.bounds.size.height.0 / scale),
                    ),
                );
                (rail.contains(&bounds.center())
                    && bounds.size.width >= px(4.)
                    && bounds.size.width <= px(12.)
                    && bounds.size.height >= px(16.)
                    && bounds.size.height < rail.size.height - px(10.)
                    && quad.background.as_solid().is_some_and(|color| color.a > 0.))
                .then_some(bounds.center())
            })
            .expect("a real shared scrollbar thumb must be painted after wheel scrolling")
    });
    let end = point(start.x, (start.y + px(60.)).min(rail.bottom() - px(10.)));
    ui.simulate_mouse_down(start, MouseButton::Left, Default::default());
    ui.simulate_event(gpui_kit::MouseMoveEvent {
        position: end,
        pressed_button: Some(MouseButton::Left),
        modifiers: Default::default(),
    });
    ui.simulate_mouse_up(end, MouseButton::Left, Default::default());
    settle(ui);
    ui.update(|_, cx| {
        let fixture = view.read(cx);
        assert!(
            fixture.group.scroll_offset(cx) > before,
            "physical right-rail drag must scroll"
        );
        let anchors = text
            .match_indices("共同内容")
            .find_map(|(byte, _)| {
                Some([
                    fixture.group.project_range(0, byte..byte, cx)?,
                    fixture.group.project_range(1, byte..byte, cx)?,
                ])
            })
            .expect("the common scrolled row must be visible in both sources");
        assert!((anchors[0].top() - anchors[1].top()).abs() < px(0.5));
        for (side, editor) in fixture.editors.iter().enumerate() {
            assert_eq!(editor.read(cx).value().as_ref(), text);
            assert_eq!(editor.read(cx).selected_range(), selections[side]);
        }
    });
}

#[gpui::test]
fn comparison_independent_rows_color_a_change_extending_beyond_the_viewport(
    cx: &mut TestAppContext,
) {
    let (view, ui) = mount(cx);
    view.update(ui, |fixture, cx| {
        fixture.group.retire(cx);
        fixture.aligned = false;
        cx.notify();
    });
    ui.simulate_resize(size(px(1000.), px(65.)));
    settle(ui);
    ui.update(|window, cx| {
        let fixture = view.read(cx);
        let appearance =
            crate::theme::component_styles(cx, plugin_schema::ThemeComponent::Editor).base;
        let expected = appearance
            .background
            .unwrap_or(cx.theme().background)
            .blend(comparison_colors(cx)[2]);
        for editor in &fixture.editors {
            let native = editor.read(cx);
            let input = native.input_bounds();
            let sample = point(
                input.right() - px(2.),
                input.top() + native.line_height().unwrap() * 1.5,
            );
            let scale = window.scale_factor();
            let sample = point(
                gpui_kit::ScaledPixels(f32::from(sample.x) * scale),
                gpui_kit::ScaledPixels(f32::from(sample.y) * scale),
            );
            assert!(
                window.painted_quads().iter().any(|quad| {
                    quad.bounds.contains(&sample)
                        && quad.content_mask.bounds.contains(&sample)
                        && quad.background.as_solid() == Some(expected)
                }),
                "a visible part of a long changed block must retain its full row background"
            );
        }
    });
}

/// Full row colors must reach real painted pixels, including a tail and the inward-facing gutter.
fn assert_row_color(view: &Entity<Fixture>, ui: &mut VisualTestContext, index: usize, token: &str) {
    let pane = ui
        .debug_bounds(if index == 0 {
            "comparison-paint-pane-0"
        } else {
            "comparison-paint-pane-1"
        })
        .unwrap();
    ui.update(|window, cx| {
        let fixture = view.read(cx);
        let offset = fixture.editors[index].read(cx).value().find(token).unwrap();
        let caret = fixture.group.project_range(index, offset..offset, cx).unwrap();
        let viewport = fixture.group.viewport(index, cx).unwrap();
        let appearance = crate::theme::component_styles(cx, plugin_schema::ThemeComponent::Editor).base;
        let expected = appearance.background.unwrap_or(cx.theme().background).blend(comparison_colors(cx)[2]);
        let scale = window.scale_factor();
        for x in [viewport.left() + px(2.), viewport.right() - px(2.)] {
            let sample = point(x, caret.center().y);
            assert!(pane.contains(&sample));
            assert!(window.painted_quads().iter().any(|quad| {
                let scaled = point(gpui_kit::ScaledPixels(f32::from(sample.x) * scale), gpui_kit::ScaledPixels(f32::from(sample.y) * scale));
                quad.bounds.contains(&scaled) && quad.content_mask.bounds.contains(&scaled)
                    && quad.background.as_solid() == Some(expected)
            }), "changed row tail/gutter is unpainted: pane={index}, token={token}, sample={sample:?}");
        }
    });
}

#[gpui::test]
fn comparison_rows_fill_tails_and_inner_gutters(cx: &mut TestAppContext) {
    let (view, ui) = mount(cx);
    for dark in [false, true] {
        ui.update(|_, cx| crate::theme::apply_theme(crate::theme::builtin_theme(dark), cx));
        settle(ui);
        for scale in [1., 1.5] {
            ui.simulate_scale_factor_change(scale);
            settle(ui);
            for index in 0..2 {
                assert_row_color(&view, ui, index, "共同前缀");
                assert_row_color(&view, ui, index, "short");
            }
        }
    }
}

#[gpui::test]
fn comparison_rows_preserve_native_input_default_scroll_and_layer_cleanup(cx: &mut TestAppContext) {
    let (view, ui) = mount(cx);
    // The row backdrop must not consume native input or add a second undo history.
    let target = source(1).find("short").unwrap() + "short".len();
    let caret = ui.update(|_, cx| {
        view.read(cx)
            .group
            .project_range(1, target..target, cx)
            .unwrap()
    });
    ui.simulate_click(
        point(caret.left() + px(0.25), caret.center().y),
        Default::default(),
    );
    ui.simulate_input("中文😀");
    settle(ui);
    ui.update(|_, cx| {
        let mut expected = source(1);
        expected.insert_str(target, "中文😀");
        assert_eq!(view.read(cx).editors[1].read(cx).value().as_ref(), expected);
    });
    ui.simulate_keystrokes("ctrl-z");
    settle(ui);
    ui.update(|_, cx| {
        assert_eq!(
            view.read(cx).editors[1].read(cx).value().as_ref(),
            source(1)
        )
    });

    // No toggle is needed: the group's default real wheel path moves both native projections.
    ui.simulate_resize(size(px(1000.), px(100.)));
    settle(ui);
    let (position, before) = ui.update(|_, cx| {
        let fixture = view.read(cx);
        let rectangle = fixture.group.project_range(0, 0..0, cx).unwrap();
        (
            fixture.group.viewport(0, cx).unwrap().center(),
            rectangle.top(),
        )
    });
    ui.simulate_event(ScrollWheelEvent {
        position,
        delta: ScrollDelta::Pixels(point(px(0.), px(-36.))),
        touch_phase: TouchPhase::Moved,
        modifiers: Default::default(),
    });
    settle(ui);
    ui.update(|_, cx| {
        let fixture = view.read(cx);
        let mut tops = Vec::new();
        for index in 0..2 {
            let native = fixture.editors[index]
                .read(cx)
                .range_to_bounds(&(0..0))
                .unwrap();
            tops.push(
                fixture
                    .group
                    .project_bounds(index, native, cx)
                    .unwrap()
                    .top(),
            );
            assert_eq!(
                fixture.editors[index].read(cx).value().as_ref(),
                source(index)
            );
        }
        assert!(tops[0] < before - px(20.));
        assert!((tops[0] - tops[1]).abs() < px(0.5));
    });

    ui.simulate_resize(size(px(1000.), px(640.)));
    view.update(ui, |fixture, cx| {
        fixture.decorated = false;
        cx.notify();
    });
    settle(ui);
    ui.update(|window, cx| {
        let expected = cx.theme().background.blend(comparison_colors(cx)[2]);
        assert!(
            !window
                .painted_quads()
                .iter()
                .any(|quad| quad.background.as_solid() == Some(expected)),
            "closed owner must not leave a full-row background on its native document"
        );
    });
}

#[gpui::test]
fn comparison_left_text_keeps_outer_inset(cx: &mut TestAppContext) {
    let (view, ui) = mount(cx);
    let pane = ui.debug_bounds("comparison-paint-pane-0").unwrap();
    ui.update(|_, cx| {
        let fixture = view.read(cx);
        let glyph = fixture.group.project_range(0, 0..0, cx).unwrap();
        assert!(
            glyph.left() - pane.left() >= px(10.),
            "left comparison glyph is flush with the outer border: {glyph:?}, pane={pane:?}"
        );
    });
}
