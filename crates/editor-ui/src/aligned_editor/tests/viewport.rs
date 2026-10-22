//! Deep native viewports must preserve exact location, host input, Unicode preedit and Undo.
use super::*;

/// Rebuild the same host input chain with the actual last painted vertical/horizontal projection.
fn handler(
    view: &Entity<Fixture>,
    window: &mut Window,
    cx: &mut App,
) -> input::AlignedInputHandler {
    let fixture = view.read(cx);
    let group = fixture.group.state.read(cx);
    let editor = fixture.editors[0].clone();
    let projection = group.frames[0].clone();
    let horizontal = group.gutters[0];
    let factory = group.sources[0].input_factory.clone().unwrap();
    let delegate = factory(editor.read(cx).text_bounds().unwrap(), window, cx);
    input::AlignedInputHandler {
        editor,
        delegate,
        projection,
        horizontal,
        suppress_geometry: false,
    }
}

/// A screen-off byte is measured natively before projection; its original editor handles input.
#[gpui::test]
fn deep_viewport_preserves_location_host_input_ime_hit_testing_and_undo(cx: &mut TestAppContext) {
    verify_deep_input(cx, false);
}

/// A collapsed presentation must retain the same deep native input and host delegate behavior.
#[gpui::test]
fn collapsed_viewport_preserves_location_host_input_ime_hit_testing_and_undo(
    cx: &mut TestAppContext,
) {
    verify_deep_input(cx, true);
}

/// A physical viewport can contain distant source rows on either side of one collapsed interval.
#[gpui::test]
fn collapsed_viewport_clicks_both_sides_of_a_distant_source_interval(cx: &mut TestAppContext) {
    let text = (0..300)
        .map(|row| format!("row{row:04} 中文😀\n"))
        .collect::<String>();
    let changed = text
        .replace("row0080", "diff0080")
        .replace("row0180", "diff0180");
    let (view, ui) = mount_sources(cx, [text.clone(), changed]);
    ui.update(|_, cx| {
        let group = view.read(cx).group.clone();
        let svg: std::sync::Arc<[u8]> = std::sync::Arc::from(
            include_bytes!("../../../assets/icons/explorer-expand-all.svg").as_slice(),
        );
        group.set_folding(UnchangedFolds::new(3, [svg.clone(), svg]), cx);
        group.fold_unchanged(true, cx);
        group
            .state
            .update(cx, |state, cx| state.scroll_by(px(110.), cx));
    });
    settle(ui);
    let targets = ui.update(|_, cx| {
        let fixture = view.read(cx);
        let group = fixture.group.state.read(cx);
        let viewport = group.frames[0].as_ref().unwrap().viewport;
        let visible: Vec<_> = (0..300)
            .filter_map(|row| {
                let byte = text.find(&format!("row{row:04}")).unwrap() + 4;
                let bounds = fixture.group.project_range(0, byte..byte, cx)?;
                viewport.contains(&bounds.center()).then_some((
                    row,
                    byte,
                    point(bounds.left() + px(0.4), bounds.center().y),
                ))
            })
            .collect();
        let pair = visible
            .windows(2)
            .find(|pair| pair[1].0 > pair[0].0 + 20)
            .expect("the native fixture must straddle a distant collapsed interval");
        [pair[1], pair[0]]
    });
    for (_, byte, position) in targets {
        ui.simulate_click(position, Default::default());
        settle(ui);
        ui.update(|_, cx| assert_eq!(view.read(cx).editors[0].read(cx).cursor(), byte));
    }
    ui.simulate_mouse_down(
        targets[0].2,
        gpui_kit::MouseButton::Left,
        Default::default(),
    );
    ui.simulate_mouse_move(
        targets[1].2,
        Some(gpui_kit::MouseButton::Left),
        Default::default(),
    );
    ui.simulate_mouse_up(
        targets[1].2,
        gpui_kit::MouseButton::Left,
        Default::default(),
    );
    settle(ui);
    ui.update(|window, cx| {
        let editor = view.read(cx).editors[0].read(cx);
        assert_eq!(
            editor.selected_range(),
            targets[1].1..targets[0].1,
            "native dragging includes the original hidden source bytes"
        );
        painted_caret(&view, window, cx);
    });
}

/// Exercise actual clicks, native Unicode preedit, caret geometry, commit and Undo in both modes.
fn verify_deep_input(cx: &mut TestAppContext, collapsed: bool) {
    let original = (0..300)
        .map(|row| format!("row{row:04} 中文😀 原生定位 wrapped content\n"))
        .collect::<String>();
    let changed = if collapsed {
        original
            .split_inclusive('\n')
            .enumerate()
            .map(|(row, line)| {
                if (245..285).contains(&row) {
                    line.replace("原生定位", "比较修改")
                } else {
                    line.to_owned()
                }
            })
            .collect()
    } else {
        original.replace("row0150", "inserted comparison row\nrow0150")
    };
    let target = original.find("row0260").unwrap() + 8;
    let (view, ui) = mount_sources(cx, [original.clone(), changed]);
    ui.update(|_, cx| {
        let group = view.read(cx).group.clone();
        group.set_inner_gutters(true, cx);
        if collapsed {
            let svg: std::sync::Arc<[u8]> = std::sync::Arc::from(
                include_bytes!("../../../assets/icons/explorer-expand-all.svg").as_slice(),
            );
            group.set_folding(UnchangedFolds::new(3, [svg.clone(), svg]), cx);
            group.fold_unchanged(true, cx);
        }
        assert!(group.reveal_offset_at(0, target, 0.5, cx));
    });
    settle(ui);
    let click = ui.update(|_, cx| {
        let fixture = view.read(cx);
        let editor = fixture.editors[0].read(cx);
        assert_eq!(
            editor.selected_range(),
            0..0,
            "location must preserve native selection"
        );
        let visible = editor.visible_row_range().unwrap();
        assert!(
            visible.start > 200 && visible.len() <= 32,
            "deep native viewport: {visible:?}"
        );
        let bounds = fixture.group.project_range(0, target..target, cx).unwrap();
        let group = fixture.group.state.read(cx);
        let viewport = group.frames[0].as_ref().unwrap().viewport;
        assert!(
            (bounds.top() - viewport.top() - (viewport.size.height - bounds.size.height) * 0.5)
                .abs()
                < px(0.5)
        );
        assert!(
            fixture.group.project_range(0, 0..0, cx).is_none(),
            "offscreen bytes must not alias the first visible native row"
        );
        point(bounds.left() + px(0.4), bounds.center().y)
    });
    ui.simulate_click(click, Default::default());
    settle(ui);
    ui.update(|window, cx| {
        assert_eq!(view.read(cx).editors[0].read(cx).cursor(), target);
        handler(&view, window, cx).replace_and_mark_text_in_range(
            None,
            "拼😀",
            Some(1..3),
            window,
            cx,
        );
        assert_eq!(view.read(cx).preedits.get(), 1);
    });
    settle(ui);
    ui.update(|window, cx| {
        let mut input = handler(&view, window, cx);
        let mark = input.marked_text_range(window, cx).unwrap();
        let bounds = input.bounds_for_range(mark.clone(), window, cx).unwrap();
        let at = input
            .character_index_for_point(bounds.center(), window, cx)
            .unwrap();
        assert!(
            (mark.start..=mark.end).contains(&at),
            "native Unicode hit test: {at}, mark={mark:?}"
        );
        painted_caret(&view, window, cx);
        input.replace_text_in_range(None, "中文😀", window, cx);
    });
    settle(ui);
    ui.update(|_, cx| {
        let mut expected = original.clone();
        expected.insert_str(target, "中文😀");
        assert_eq!(view.read(cx).editors[0].read(cx).value().as_ref(), expected);
        assert!(
            view.read(cx).routed.get() > 0,
            "source-owned input chain must run"
        );
    });
    ui.simulate_keystrokes("ctrl-z");
    settle(ui);
    ui.update(|window, cx| {
        assert_eq!(view.read(cx).editors[0].read(cx).value().as_ref(), original);
        painted_caret(&view, window, cx);
        assert!(view.read(cx).group.clone().reveal_offset(0, 0, cx));
    });
    settle(ui);
    ui.update(|_, cx| {
        assert!(view.read(cx).group.project_range(0, 0..0, cx).is_some());
        assert_eq!(
            view.read(cx).editors[0].read(cx).selected_range(),
            target..target
        );
    });
}

/// Compare cached source rows to the current native glyph plane only where Base laid them out.
#[track_caller]
fn assert_visible_native_rows(view: &Entity<Fixture>, cx: &App) {
    let fixture = view.read(cx);
    let group = fixture.group.state.read(cx);
    assert_eq!(group.effective_status(), AlignmentStatus::Ready);
    for (index, source) in fixture.editors.iter().enumerate() {
        let source = source.read(cx);
        let measured = group.measurements[index].as_ref().unwrap();
        let origin = source.text_bounds().unwrap().top();
        let visible = source.visible_row_range().unwrap();
        assert!(visible.len() <= 32);
        for row in visible {
            let byte = group.rows.as_ref().unwrap().source_rows[index][row].start;
            let actual = source.range_to_bounds(&(byte..byte)).unwrap();
            assert!((actual.top() - origin - px(measured.logical_rows[row].start)).abs() < px(0.5));
        }
    }
}

/// Bounded paints refresh native wrapping without hidden source layout and retain deferred x.
#[gpui::test]
fn bounded_viewport_remeasures_wrap_padding_font_dpi_and_preserves_native_requests(
    cx: &mut TestAppContext,
) {
    let text = (0..160)
        .map(|row| {
            format!(
                "row{row:04} 中文😀 {}\n",
                "native wrapped source ".repeat(3)
            )
        })
        .collect::<String>();
    let target = text.find("row0100").unwrap();
    let (view, ui) = mount_sources(cx, [text.clone(), text]);
    ui.update(|_, cx| assert!(view.read(cx).group.clone().reveal_offset(0, target, cx)));
    settle(ui);
    geometry::MEASURED_ROWS.with(|count| count.set(0));
    ui.simulate_resize(size(px(480.), px(180.)));
    settle(ui);
    assert_eq!(
        geometry::MEASURED_ROWS.with(|count| count.get()),
        0,
        "wrapped resize must read native row mapping instead of hidden whole-source glyphs"
    );
    ui.update(|_, cx| assert_visible_native_rows(&view, cx));
    ui.update(|_, cx| {
        crate::typography::set_font_size(cx, 22.);
        crate::theme::apply_theme(crate::theme::builtin_theme(true), cx);
        let source = view.read(cx).editors[0].clone();
        source.update(cx, |editor, _| {
            editor.set_editor_paddings(gpui_kit::Edges {
                top: px(9.),
                left: px(11.),
                right: px(13.),
                bottom: px(0.),
            })
        });
        cx.notify(view.entity_id());
    });
    settle(ui);
    ui.simulate_scale_factor_change(1.5);
    settle(ui);
    ui.update(|window, cx| {
        assert_visible_native_rows(&view, cx);
        let sources = view.read(cx).editors.clone();
        for source in sources {
            source.update(cx, |editor, cx| {
                editor.set_soft_wrap(false, window, cx);
                editor.set_scroll_offset(point(px(-40.), editor.scroll_offset().y), cx);
            });
        }
    });
    settle(ui);
    ui.update(|_, cx| {
        assert_visible_native_rows(&view, cx);
        for editor in &view.read(cx).editors {
            assert_eq!(editor.read(cx).scroll_offset().x, px(-40.));
        }
    });
}

/// Bounded native inputs must not add their embedded rails beside the shared comparison rail.
#[gpui::test]
fn bounded_comparison_paints_only_the_shared_vertical_scrollbar(cx: &mut TestAppContext) {
    let text = (0..160)
        .map(|row| format!("line {row}\n"))
        .collect::<String>();
    let (view, ui) = mount_sources(cx, [text.clone(), text]);
    ui.update(|_, cx| view.read(cx).group.clone().set_inner_gutters(true, cx));
    settle(ui);
    ui.update(|window, _| {
        let scale = window.scale_factor();
        let thumbs = window
            .painted_quads()
            .iter()
            .filter(|quad| {
                (quad.bounds.size.width.0 / scale - 6.).abs() < 0.25
                    && quad.bounds.size.height.0 / scale >= 20.
                    && quad.background.as_solid().is_some_and(|color| color.a > 0.)
                    && quad.content_mask.bounds.contains(&quad.bounds.center())
            })
            .count();
        assert_eq!(
            thumbs, 1,
            "embedded native rails must yield to the shared comparison rail"
        );
    });
}

/// A nowrap notify-only repaint retains complete bounded validation and native horizontal extent.
#[gpui::test]
fn private_native_notifications_remeasure_all_rows_without_a_whole_source_lookup(
    cx: &mut TestAppContext,
) {
    let text = (0..360)
        .map(|row| format!("row{row:04} 中文 wrapped native source\n"))
        .collect::<String>();
    let (view, ui) = mount_sources(cx, [text.clone(), text]);
    ui.update(|window, cx| {
        for editor in view.read(cx).editors.clone() {
            editor.update(cx, |editor, cx| editor.set_soft_wrap(false, window, cx));
        }
        let editor = view.read(cx).editors[0].clone();
        editor.update(cx, |editor, cx| editor.focus(window, cx));
    });
    settle(ui);
    geometry::MEASURED_ROWS.with(|count| count.set(0));
    geometry::MAX_LOOKUP_ROWS.with(|count| count.set(0));
    ui.update(|_, cx| cx.notify(view.read(cx).editors[0].entity_id()));
    settle(ui);
    assert_eq!(
        geometry::MEASURED_ROWS.with(|count| count.get()),
        361,
        "private notifications must revalidate every source row exactly once"
    );
    assert!(
        geometry::MAX_LOOKUP_ROWS.with(|count| count.get()) <= 140,
        "native range lookups must use small windows rather than all source rows: {}",
        geometry::MAX_LOOKUP_ROWS.with(|count| count.get())
    );
    ui.update(|_, cx| assert_visible_native_rows(&view, cx));
}

/// A nowrap redraw warms a bounded readonly snapshot; private notifications still validate all
/// rows because native display mapping does not expose its complete horizontal content extent.
#[gpui::test]
fn global_refresh_bounds_snapshot_glyph_work_and_preserves_full_native_validation(
    cx: &mut TestAppContext,
) {
    let text = (0..360)
        .map(|row| format!("row{row:04} 中文😀 native source\n"))
        .collect::<String>();
    let (view, ui) = mount_sources(cx, [text.clone(), text]);
    ui.update(|window, cx| {
        for editor in view.read(cx).editors.clone() {
            editor.update(cx, |editor, cx| editor.set_soft_wrap(false, window, cx));
        }
    });
    settle(ui);
    geometry_measurement::PLANNED_ROWS.with(|rows| rows.set([0; 3]));
    ui.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
    let planned = geometry_measurement::PLANNED_ROWS.with(|rows| rows.get());
    assert_eq!(
        planned[0], 361,
        "the editable source must retain its complete native glyph cache"
    );
    assert!(
        planned[1] > 0 && planned[1] < 180,
        "a forced refresh must bound background snapshot glyph work: {planned:?}"
    );
    geometry::MEASURED_ROWS.with(|rows| rows.set(0));
    ui.update(|_, cx| cx.notify(view.read(cx).editors[1].entity_id()));
    settle(ui);
    assert_eq!(
        geometry::MEASURED_ROWS.with(|rows| rows.get()),
        361,
        "readonly snapshots still require complete native validation after private changes"
    );
    ui.update(|_, cx| assert_visible_native_rows(&view, cx));
}

/// A retained glyph lease must not leave hidden native key actions or platform text input alive.
#[gpui::test]
fn retiring_focused_comparison_withdraws_glyphs_actions_and_input(cx: &mut TestAppContext) {
    let text = (0..160)
        .map(|row| format!("row{row:04} 中文😀\n"))
        .collect::<String>();
    let (view, ui) = mount_sources(cx, [text.clone(), text.clone()]);
    ui.update(|window, cx| {
        for editor in view.read(cx).editors.clone() {
            editor.update(cx, |editor, cx| editor.set_soft_wrap(false, window, cx));
        }
        let editor = view.read(cx).editors[0].clone();
        editor.update(cx, |editor, cx| editor.focus(window, cx));
    });
    settle(ui);
    ui.update(|window, cx| {
        assert_eq!(
            geometry_glyphs::live_count(window, cx),
            2,
            "exercise actually retained native glyph views"
        );
        view.read(cx).group.clone().retire(cx);
    });
    settle(ui);
    ui.simulate_keystrokes("ctrl-a backspace");
    ui.simulate_input("不应写入");
    settle(ui);
    ui.update(|window, cx| {
        assert_eq!(view.read(cx).editors[0].read(cx).value().as_ref(), text);
        assert_eq!(geometry_glyphs::live_count(window, cx), 0);
    });
}
