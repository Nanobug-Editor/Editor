//! Inner diff gutters project the original native pixels and gestures without changing documents.
use super::*;
use crate::theme::ActiveTheme as _;
use gpui_base::input::{Editor, InputState, RopeExt as _};
use gpui_kit::{
    Context, ElementInputHandler, EntityInputHandler as _, InputHandler as _,
    InteractiveElement as _, MouseButton, ParentElement as _, Render, ScrollDelta, ScrollEvent,
    Styled as _, TestAppContext, TouchPhase, VisualTestContext, Window, div, gpui, point, size,
};

/// The same native entities remain mounted when the group is paused or retired.
struct GutterFixture {
    /// Presentation membership; it never owns a second editable text buffer.
    group: AlignmentGroup,
    /// Real source identities shared by native paints, selection and the input bridge.
    editors: [Entity<EditorState>; 2],
    /// An unrelated normal editor detects changes leaking outside the comparison.
    ordinary: Entity<EditorState>,
    /// A real independent search field belongs to the native source factory's visible header.
    search_input: Entity<InputState>,
    /// One window router owns both pane gestures, including captured drags.
    router: AlignedPointerRouter,
    /// Retirement remounts each existing source in its ordinary native presentation.
    comparison_visible: bool,
}

impl Render for GutterFixture {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut comparison = div().flex().w_full().h(px(230.)).flex_shrink_0();
        for index in 0..2 {
            let selector = format!("inner-gutter-test-pane-{index}");
            let content = if self.comparison_visible {
                self.group.pane(index).unwrap()
            } else {
                Editor::new(&self.editors[index]).into_any_element()
            };
            comparison = comparison.child(
                div()
                    .id(("inner-gutter-test-pane", index))
                    .debug_selector(move || selector.clone())
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .overflow_hidden()
                    .child(content),
            );
        }
        super::super::editor::synchronize_editor_appearance(&self.ordinary, cx);
        let appearance =
            crate::theme::component_styles(cx, plugin_schema::ThemeComponent::Editor).base;
        self.router.wrap(
            div()
                .size_full()
                .flex()
                .flex_col()
                .line_height(px(20.))
                // Retired sources and the unrelated control use the same typography as PaneView.
                // A font-context change must not masquerade as lost native line-number settings.
                .font_family(cx.theme().mono_font_family.clone())
                .text_size(
                    appearance
                        .font_size_px
                        .map(px)
                        .unwrap_or(cx.theme().mono_font_size),
                )
                .child(comparison)
                .child(div().flex_1().min_h_0().child(Editor::new(&self.ordinary))),
        )
    }
}

/// Additions, a deletion, Unicode and a no-wrap long line exercise real alignment and x scrolling.
fn source_text(index: usize) -> String {
    let long = format!("long {} 中文😀\n", "abcdefghijklmnopqrstuvwxyz".repeat(18));
    if index == 0 {
        format!("alpha 中文😀\nleft only\ncommon beta\ncommon gamma\n{long}tail\n")
    } else {
        format!("alpha 中文😀\ncommon beta\ninsert one\ninsert two\ncommon gamma\n{long}tail\n")
    }
}

/// Mount native controls with enabled line numbers/folding; every theme reuses these identities.
fn mount(cx: &mut TestAppContext) -> (Entity<GutterFixture>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::typography::init(cx);
        crate::theme::apply_theme(crate::theme::builtin_theme(false), cx);
    });
    let (view, ui) = cx.add_window_view(|window, cx| {
        let router = AlignedPointerRouter::default();
        let editors = std::array::from_fn(|index| {
            let editor = cx.new(|cx| {
                EditorState::new(window, cx)
                    .line_number(true)
                    .folding(true)
                    .soft_wrap(false)
            });
            editor.update(cx, |state, cx| {
                state.set_editor_paddings(Default::default());
                state.set_value(source_text(index), window, cx);
            });
            editor
        });
        let ordinary = cx.new(|cx| {
            EditorState::new(window, cx)
                .line_number(true)
                .folding(true)
                .soft_wrap(false)
        });
        ordinary.update(cx, |state, cx| {
            state.set_editor_paddings(Default::default());
            state.set_value("ordinary editor\nuntouched\n", window, cx);
        });
        let search_input = cx.new(|cx| InputState::new(window, cx));
        let native_header_input = search_input.clone();
        let native_factory: AlignmentNativeFactory = Rc::new(move |editor, _, cx| {
            let mut body = div().flex().flex_col().size_full();
            if editor.read(cx).search_session().open {
                // This local native header is a test fixture for the public factory contract.
                // Keeping its input entity stable proves the pixel path retains real controls,
                // rather than painting a rectangle that merely resembles a search panel.
                body = body.child(
                    div()
                        .id("inner-gutter-test-search-header")
                        .debug_selector(|| "inner-gutter-test-search-header".into())
                        .h(px(32.))
                        .w_full()
                        .flex_shrink_0()
                        .bg(gpui_kit::rgb(0x385a9f))
                        .child(crate::Input::new(&native_header_input)),
                );
            }
            body.child(div().flex_1().min_h_0().child(Editor::new(editor)))
                .into_any_element()
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
                        label: format!("inner gutter source {index}"),
                        input_factory: None,
                        native_authority_factory: (index == 0).then(|| native_factory.clone()),
                    })
                    .collect(),
                &router,
                cx,
            )
            .unwrap();
        group.set_inner_gutters(true, cx);
        GutterFixture {
            group,
            editors,
            ordinary,
            search_input,
            router,
            comparison_visible: true,
        }
    });
    ui.simulate_resize(size(px(800.), px(320.)));
    ui.update(|window, cx| {
        window.activate_window();
        cx.set_reduce_motion(true);
    });
    settle(ui);
    (view, ui)
}

#[gpui::test]
fn inner_gutters_preserve_visible_native_factory_headers_and_real_search_input(
    cx: &mut TestAppContext,
) {
    let (view, ui) = mount(cx);
    for dark in [false, true] {
        ui.update(|window, cx| {
            crate::theme::apply_theme(crate::theme::builtin_theme(dark), cx);
            let (editor, search) = {
                let fixture = view.read(cx);
                (fixture.editors[0].clone(), fixture.search_input.clone())
            };
            search.update(cx, |state, cx| state.set_value("", window, cx));
            editor.update(cx, |state, cx| state.open_search(false, cx));
        });
        settle(ui);
        let header = ui
            .debug_bounds("inner-gutter-test-search-header")
            .expect("the source factory's header must remain mounted");
        let sample = point(header.left() + px(4.), header.bottom() - px(1.));
        ui.update(|window, cx| {
            let scale = window.scale_factor();
            let expected: gpui_kit::Hsla = gpui_kit::rgb(0x385a9f).into();
            assert!(
                window.painted_quads().iter().any(|quad| {
                    let bounds = Bounds::new(
                        point(
                            px(quad.bounds.left().0 / scale),
                            px(quad.bounds.top().0 / scale),
                        ),
                        size(
                            px(quad.bounds.size.width.0 / scale),
                            px(quad.bounds.size.height.0 / scale),
                        ),
                    );
                    bounds.contains(&sample)
                        && quad.background.as_solid() == Some(expected)
                        && quad.content_mask.bounds.contains(&point(
                            gpui_kit::ScaledPixels(f32::from(sample.x) * scale),
                            gpui_kit::ScaledPixels(f32::from(sample.y) * scale),
                        ))
                }),
                "a hidden authority header cannot substitute for visible native header pixels",
            );
            assert!(view.read(cx).editors[0].read(cx).input_bounds().top() >= header.bottom());
        });
        let moved = gutter(&view, ui);
        assert!(moved.top() >= header.bottom());
        ui.simulate_click(
            point(header.left() + px(50.), header.top() + px(14.)),
            Default::default(),
        );
        ui.simulate_input("needle");
        settle(ui);
        ui.update(|_, cx| {
            let fixture = view.read(cx);
            assert_eq!(fixture.search_input.read(cx).value().as_ref(), "needle");
            assert_eq!(fixture.editors[0].read(cx).value().as_ref(), source_text(0));
            let editor = fixture.editors[0].clone();
            editor.update(cx, |state, cx| state.close_search(cx));
        });
        settle(ui);
    }
}

/// Finish bounded native geometry rounds without replacing source text or synthesizing UI actions.
fn settle(ui: &mut VisualTestContext) {
    for _ in 0..8 {
        ui.update(|window, cx| {
            window.draw(cx).clear(cx);
            window.simulate_next_frame(cx);
        });
        ui.run_until_parked();
    }
}

/// Read the real clipped gutter surface; guessed coordinates could pass with no visible gutter.
fn gutter(view: &Entity<GutterFixture>, ui: &mut VisualTestContext) -> Bounds<Pixels> {
    let selector = ui.update(|_, cx| {
        format!(
            "aligned-inner-gutter-{:?}",
            view.read(cx).editors[0].entity_id()
        )
    });
    // GPUI's debug lookup requires a static selector; this bounded test-only allocation has no
    // production lifetime and remains valid for the test window's queued frame callbacks.
    let gutter = ui
        .debug_bounds(Box::leak(selector.into_boxed_str()))
        .expect("inner gutter must mount");
    let pane = ui.debug_bounds("inner-gutter-test-pane-0").unwrap();
    assert!(gutter.size.width > px(20.) && gutter.size.width < px(120.));
    assert!(gutter.right() <= pane.right() + px(0.5));
    assert!(
        gutter.right() >= pane.right() - px(16.),
        "the left source gutter belongs at its inner edge: {gutter:?} in {pane:?}",
    );
    ui.update(|window, _| {
        let scale = window.scale_factor();
        let sample = gutter.center();
        assert!(
            window.painted_quads().iter().any(|quad| {
                let bounds = Bounds::new(
                    point(
                        px(quad.bounds.left().0 / scale),
                        px(quad.bounds.top().0 / scale),
                    ),
                    size(
                        px(quad.bounds.size.width.0 / scale),
                        px(quad.bounds.size.height.0 / scale),
                    ),
                );
                let native_sample = point(
                    gpui_kit::ScaledPixels(f32::from(sample.x) * scale),
                    gpui_kit::ScaledPixels(f32::from(sample.y) * scale),
                );
                bounds.size.width <= gutter.size.width + px(1.)
                    && bounds.contains(&sample)
                    && quad.content_mask.bounds.contains(&native_sample)
                    && quad.background.as_solid().is_some_and(|color| color.a > 0.)
            }),
            "the moved gutter must paint visible native background pixels",
        );
    });
    gutter
}

/// A visible logical token is located through the component's public source-to-screen seam.
fn token_bounds(
    view: &Entity<GutterFixture>,
    ui: &mut VisualTestContext,
    index: usize,
    token: &str,
) -> Bounds<Pixels> {
    let byte = ui.update(|_, cx| {
        let fixture = view.read(cx);
        fixture.editors[index].read(cx).value().find(token).unwrap()
    });
    byte_bounds(view, ui, index, byte)
}

/// Keep line-start caret visibility strict while exposing native and projected failure geometry.
fn byte_bounds(
    view: &Entity<GutterFixture>,
    ui: &mut VisualTestContext,
    index: usize,
    byte: usize,
) -> Bounds<Pixels> {
    ui.update(|_, cx| {
        let fixture = view.read(cx);
        let source = fixture.editors[index].read(cx);
        let native = source.range_to_bounds(&(byte..byte));
        let projected = native.and_then(|bounds| fixture.group.project_bounds(index, bounds, cx));
        fixture
            .group
            .project_range(index, byte..byte, cx)
            .unwrap_or_else(|| {
                panic!(
                    "visible source caret missing: pane={index}, byte={byte}, status={:?}, \
                 native={native:?}, pane_viewport={:?}, project_bounds={projected:?}, \
                 input_bounds={:?}, text_bounds={:?}, scroll={:?}",
                    fixture.group.status(cx),
                    fixture.group.viewport(index, cx),
                    source.input_bounds(),
                    source.text_bounds(),
                    source.scroll_offset(),
                )
            })
    })
}

/// Public native glyph geometry includes the real line-number and folding width, without x scroll.
fn native_gutter_width(editor: &EditorState) -> Pixels {
    editor.range_to_bounds(&(0..0)).unwrap().left() - editor.text_bounds().unwrap().left()
}

#[gpui::test]
fn inner_gutters_keep_native_rows_and_stay_fixed_during_horizontal_scrolling(
    cx: &mut TestAppContext,
) {
    let (view, ui) = mount(cx);
    for dark in [false, true] {
        ui.update(|_, cx| {
            crate::theme::apply_theme(crate::theme::builtin_theme(dark), cx);
        });
        settle(ui);
        let before = gutter(&view, ui);
        let left = token_bounds(&view, ui, 0, "common gamma");
        let right = token_bounds(&view, ui, 1, "common gamma");
        assert!((left.top() - right.top()).abs() < px(0.5));
        let left_pane = ui.debug_bounds("inner-gutter-test-pane-0").unwrap();
        let right_pane = ui.debug_bounds("inner-gutter-test-pane-1").unwrap();
        assert!((left.left() - left_pane.left() - px(12.)).abs() < px(0.5));
        assert!(right.left() > right_pane.left() + px(20.));
        let selected = ui.update(|_, cx| view.read(cx).editors[0].read(cx).selected_range());
        ui.simulate_event(ScrollEvent {
            // Wheel and trackpad fixtures retain pointer-device semantics.
            source: gpui_kit::ScrollSource::Pointer,
            position: point(left_pane.left() + px(100.), left.center().y),
            delta: ScrollDelta::Pixels(point(px(-48.), px(0.))),
            touch_phase: TouchPhase::Moved,
            modifiers: Default::default(),
        });
        settle(ui);
        assert_eq!(
            gutter(&view, ui),
            before,
            "x scrolling cannot move the gutter"
        );
        ui.update(|_, cx| {
            let fixture = view.read(cx);
            assert_eq!(fixture.group.status(cx), AlignmentStatus::Ready);
            assert!(fixture.editors[0].read(cx).scroll_offset().x < px(0.));
            assert_eq!(fixture.editors[0].read(cx).selected_range(), selected);
            for (index, editor) in fixture.editors.iter().enumerate() {
                assert_eq!(editor.read(cx).value().as_ref(), source_text(index));
            }
            assert_eq!(fixture.editors[0].read(cx).text().lines_len(), 7);
            assert_eq!(fixture.editors[1].read(cx).text().lines_len(), 8);
            // Restore only the native scroll position before checking the other palette.
            let editor = fixture.editors[0].clone();
            editor.update(cx, |state, cx| {
                state.set_scroll_offset(point(px(0.), px(0.)), cx)
            });
        });
        settle(ui);
    }
    // A real narrow pane needs its complete gutter even when it exceeds half the viewport.
    ui.simulate_resize(size(px(180.), px(320.)));
    ui.simulate_scale_factor_change(1.5);
    settle(ui);
    let inner = gutter(&view, ui);
    let pane = ui.debug_bounds("inner-gutter-test-pane-0").unwrap();
    assert!(inner.size.width > pane.size.width / 2.);
    let first = token_bounds(&view, ui, 0, "common gamma");
    assert!(
        (first.left() - pane.left() - px(12.)).abs() < px(0.5),
        "the full original gutter must move to the inner edge"
    );
}

#[gpui::test]
fn inner_gutter_gestures_body_input_undo_and_ime_keep_the_original_source(cx: &mut TestAppContext) {
    let (view, ui) = mount(cx);
    for dark in [false, true] {
        ui.update(|_, cx| {
            crate::theme::apply_theme(crate::theme::builtin_theme(dark), cx);
        });
        settle(ui);
        let bounds = gutter(&view, ui);
        let beta = token_bounds(&view, ui, 0, "common beta");
        let gamma = token_bounds(&view, ui, 0, "common gamma");
        let start = point(bounds.left() + px(4.), beta.center().y);
        let end = point(bounds.left() + px(4.), gamma.center().y);
        ui.simulate_click(start, Default::default());
        settle(ui);
        ui.update(|_, cx| {
            let native = view.read(cx).editors[0].read(cx);
            let byte = source_text(0).find("common beta").unwrap();
            // Base 0.7's single gutter click places a caret at line start; it does not select
            // the whole line. Preserve that real behavior rather than implementing it in a test.
            assert_eq!(native.selected_range(), byte..byte);
        });
        ui.simulate_mouse_down(start, MouseButton::Left, Default::default());
        ui.simulate_mouse_move(end, MouseButton::Left, Default::default());
        ui.simulate_mouse_up(end, MouseButton::Left, Default::default());
        settle(ui);
        ui.update(|_, cx| {
            let native = view.read(cx).editors[0].read(cx);
            let text = source_text(0);
            let begin = text.find("common beta").unwrap();
            let finish = text.find("common gamma").unwrap();
            assert_eq!(native.selected_range(), begin..finish);
            assert_eq!(native.value().as_ref(), text);
        });
        let target = source_text(0).find("common beta").unwrap() + 7;
        let caret = byte_bounds(&view, ui, 0, target);
        ui.simulate_click(
            point(caret.left() + px(0.25), caret.center().y),
            Default::default(),
        );
        settle(ui);
        ui.update(|_, cx| assert_eq!(view.read(cx).editors[0].read(cx).cursor(), target));
        ui.simulate_input("中文😀");
        settle(ui);
        ui.update(|_, cx| {
            let actual = view.read(cx).editors[0].read(cx).value().to_string();
            let mut expected = source_text(0);
            expected.insert_str(target, "中文😀");
            assert_eq!(actual, expected);
        });
        ui.simulate_keystrokes("ctrl-z");
        settle(ui);
        assert_ime_projection(&view, ui);
        ui.simulate_keystrokes("ctrl-z");
        settle(ui);
        ui.update(|_, cx| {
            assert_eq!(
                view.read(cx).editors[0].read(cx).value().as_ref(),
                source_text(0)
            );
            assert_eq!(
                view.read(cx).editors[1].read(cx).value().as_ref(),
                source_text(1)
            );
        });
    }
}

/// Exercise the same installed adapter type against completed native paints and real preedit text.
fn assert_ime_projection(view: &Entity<GutterFixture>, ui: &mut VisualTestContext) {
    ui.update(|window, cx| {
        let editor = view.read(cx).editors[0].clone();
        editor.update(cx, |state, cx| {
            state.replace_and_mark_text_in_range(None, "拼😀", Some(1..3), window, cx);
        });
    });
    settle(ui);
    ui.update(|window, cx| {
        let (group, editor) = {
            let fixture = view.read(cx);
            (fixture.group.clone(), fixture.editors[0].clone())
        };
        let projection = group.state.read(cx).frames[0].clone().unwrap();
        let bounds = editor.read(cx).text_bounds().unwrap();
        let mut handler = input::AlignedInputHandler {
            delegate: Box::new(ElementInputHandler::new(bounds, editor.clone())),
            editor: editor.clone(),
            projection: Some(projection),
            horizontal: group.state.read(cx).gutters[0],
            suppress_geometry: false,
        };
        let marked = handler.marked_text_range(window, cx).unwrap();
        let actual = handler
            .bounds_for_range(marked.clone(), window, cx)
            .unwrap();
        let mut native = ElementInputHandler::new(bounds, editor);
        let source = native.bounds_for_range(marked, window, cx).unwrap();
        let expected = group.project_bounds(0, source, cx).unwrap();
        assert_eq!(actual, expected);
        assert!(
            actual.left() < px(320.),
            "IME anchor cannot remain in the inner gutter"
        );
        assert!(group.viewport(0, cx).unwrap().contains(&actual.center()));
        handler.replace_text_in_range(None, "中文😀", window, cx);
    });
    settle(ui);
}

#[gpui::test]
fn inner_gutters_survive_native_pause_search_and_retirement_without_changing_normal_editors(
    cx: &mut TestAppContext,
) {
    let (view, ui) = mount(cx);
    for dark in [false, true] {
        ui.update(|_, cx| {
            crate::theme::apply_theme(crate::theme::builtin_theme(dark), cx);
        });
        settle(ui);
        let ordinary_width =
            ui.update(|_, cx| native_gutter_width(view.read(cx).ordinary.read(cx)));
        ui.update(|_, cx| {
            let group = view.read(cx).group.clone();
            group.set_enabled(false, cx);
        });
        settle(ui);
        gutter(&view, ui);
        ui.update(|_, cx| {
            let fixture = view.read(cx);
            assert_eq!(fixture.group.status(cx), AlignmentStatus::Disabled);
            let normal = native_gutter_width(fixture.ordinary.read(cx));
            assert_eq!(
                normal, ordinary_width,
                "pausing a comparison cannot change its peer"
            );
            assert_eq!(
                native_gutter_width(fixture.editors[0].read(cx)),
                normal,
                "paused source preserves native numbers/folding in the same font context"
            );
            let group = fixture.group.clone();
            let editor = fixture.editors[0].clone();
            group.set_enabled(true, cx);
            editor.update(cx, |state, cx| state.open_search(false, cx));
        });
        settle(ui);
        gutter(&view, ui);
        ui.update(|_, cx| {
            let fixture = view.read(cx);
            assert_eq!(fixture.group.status(cx), AlignmentStatus::NativeOverlays);
            assert!(fixture.editors[0].read(cx).search_session().open);
            let editor = fixture.editors[0].clone();
            editor.update(cx, |state, cx| state.close_search(cx));
        });
        settle(ui);
        gutter(&view, ui);
    }
    view.update(ui, |fixture, cx| {
        fixture.group.retire(cx);
        fixture.comparison_visible = false;
        cx.notify();
    });
    settle(ui);
    ui.update(|_, cx| {
        let fixture = view.read(cx);
        let normal = native_gutter_width(fixture.ordinary.read(cx));
        assert_eq!(
            native_gutter_width(fixture.editors[0].read(cx)),
            normal,
            "retired source and ordinary editor share current theme/typography"
        );
        // Subtracting absolute glyph coordinates in the second pane adds float rounding only.
        assert!((native_gutter_width(fixture.editors[1].read(cx)) - normal).abs() < px(0.01));
        for (index, editor) in fixture.editors.iter().enumerate() {
            assert_eq!(editor.read(cx).value().as_ref(), source_text(index));
            assert!(
                editor.read(cx).range_to_bounds(&(0..0)).unwrap().left()
                    > editor.read(cx).input_bounds().left() + px(20.)
            );
        }
        assert_eq!(
            fixture.ordinary.read(cx).value().as_ref(),
            "ordinary editor\nuntouched\n"
        );
    });
}
