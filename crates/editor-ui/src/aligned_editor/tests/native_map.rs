//! Native wrapped-row maps avoid hidden glyph work without losing nowrap horizontal scrolling.
use super::*;
use gpui_kit::{ScrollDelta, ScrollEvent, TouchPhase};

/// Build enough genuinely wrapped Unicode lines to exercise the large-source native-map path.
fn wrapped_source() -> String {
    (0..180)
        .map(|row| {
            format!(
                "row{row:04} 中文😀 {}\n",
                "native wrapped source ".repeat(5)
            )
        })
        .collect()
}

/// Start counting only the next interaction, excluding initial admission and queued setup paints.
fn reset_hidden_work() {
    geometry_measurement::PLANNED_ROWS.with(|rows| rows.set([0; 3]));
    geometry::MEASURED_ROWS.with(|rows| rows.set(0));
}

/// Assert that the actual mounts and native range queries did not revisit the complete source.
fn assert_no_hidden_work() {
    assert_eq!(
        geometry_measurement::PLANNED_ROWS.with(|rows| rows.get()),
        [0; 3],
        "native wrapped mapping must not schedule hidden batches or glyph leases"
    );
    assert_eq!(
        geometry::MEASURED_ROWS.with(|rows| rows.get()),
        0,
        "native wrapped mapping must not query every offscreen source row"
    );
}

/// Compare admitted rows with original painted glyph positions and the native EOF row budget.
fn assert_wrapped_native_geometry(view: &Entity<Fixture>, cx: &App) {
    let fixture = view.read(cx);
    let group = fixture.group.state.read(cx);
    assert_eq!(group.effective_status(), AlignmentStatus::Ready);
    for (index, source) in fixture.editors.iter().enumerate() {
        let native = source.read(cx);
        let map = native.display_map();
        assert!(map.wrap_row_count() > map.buffer_line_count());
        assert!(map.folded_ranges().is_empty());
        let measured = group.measurements[index].as_ref().unwrap();
        let plane = native.text_bounds().unwrap();
        let line = native.line_height().unwrap();
        let expected_height = map.display_row_count() as f32 * f32::from(line);
        assert_eq!(measured.logical_rows.len(), map.buffer_line_count());
        assert_eq!(measured.visual_rows.len(), map.display_row_count());
        for (row, admitted) in measured.logical_rows.iter().enumerate() {
            let native_rows = map.buffer_line_to_display_row_range(row).unwrap();
            assert!((admitted.start - native_rows.start as f32 * f32::from(line)).abs() < 0.5);
            assert!((admitted.end - native_rows.end as f32 * f32::from(line)).abs() < 0.5);
        }
        assert!(
            (measured.logical_rows.last().unwrap().end - expected_height).abs() < 0.5,
            "the last native logical row, including trailing EOF, must remain addressable"
        );
        for row in native.visible_row_range().unwrap() {
            let byte = group.rows.as_ref().unwrap().source_rows[index][row].start;
            let painted = native.range_to_bounds(&(byte..byte)).unwrap();
            let admitted = px(measured.logical_rows[row].start);
            assert!((painted.top() - plane.top() - admitted).abs() < px(0.5));
        }
    }
}

/// First admission must learn native wrapping from the bounded authority without hidden batches.
#[gpui::test]
fn native_map_cold_admission_avoids_hidden_source_layout(cx: &mut TestAppContext) {
    reset_hidden_work();
    let text = wrapped_source();
    let (view, ui) = mount_sources(cx, [text.clone(), text]);
    assert_no_hidden_work();
    ui.update(|_, cx| assert_wrapped_native_geometry(&view, cx));
}

/// Blink-like notifications and full-window refresh must preserve native geometry at deep scroll.
#[gpui::test]
fn native_map_revalidates_unknown_notifications_without_hidden_source_layout(
    cx: &mut TestAppContext,
) {
    let original = wrapped_source();
    let (view, ui) = mount_sources(cx, [original.clone(), original.clone()]);
    let editors = ui.update(|window, cx| {
        let fixture = view.read(cx);
        let editors = fixture.editors.clone();
        let group = fixture.group.state.clone();
        editors[0].update(cx, |editor, cx| editor.focus(window, cx));
        group.update(cx, |group, cx| group.scroll_by(px(900.), cx));
        editors
    });
    settle(ui);
    ui.update(|_, cx| assert_wrapped_native_geometry(&view, cx));
    reset_hidden_work();
    for editor in &editors {
        ui.update(|window, cx| {
            cx.notify(editor.entity_id());
            window.refresh();
            window.draw(cx).clear(cx);
        });
        settle(ui);
        assert_no_hidden_work();
        ui.update(|_, cx| {
            assert_wrapped_native_geometry(&view, cx);
            for (index, native) in editors.iter().enumerate() {
                assert_eq!(view.read(cx).editors[index], *native);
                assert_eq!(native.read(cx).value().as_ref(), original);
            }
        });
    }
}

/// Switching to nowrap must recover real horizontal extent before wrapped mapping can resume.
#[gpui::test]
fn native_map_nowrap_transition_preserves_horizontal_scroll_and_reenters_wrapped_path(
    cx: &mut TestAppContext,
) {
    let original = wrapped_source();
    let (view, ui) = mount_sources(cx, [original.clone(), original]);
    ui.update(|_, cx| assert_wrapped_native_geometry(&view, cx));
    reset_hidden_work();
    ui.update(|window, cx| {
        for editor in view.read(cx).editors.clone() {
            editor.update(cx, |editor, cx| editor.set_soft_wrap(false, window, cx));
        }
    });
    settle(ui);
    assert!(geometry::MEASURED_ROWS.with(|rows| rows.get()) > 0);
    assert!(geometry_measurement::PLANNED_ROWS.with(|rows| rows.get()[0]) > 0);
    let position = ui.update(|_, cx| {
        let fixture = view.read(cx);
        let group = fixture.group.state.read(cx);
        let native = fixture.editors[0].read(cx);
        assert_eq!(
            native.display_map().wrap_row_count(),
            native.display_map().buffer_line_count()
        );
        let measured = group.measurements[0].as_ref().unwrap();
        let first = &group.rows.as_ref().unwrap().source_rows[0][0];
        let end = first.end - 1;
        let glyphs = native.range_to_bounds(&(first.start..end)).unwrap();
        let extent = glyphs.right() - native.text_bounds().unwrap().left();
        assert!(extent > measured.viewport.size.width + px(100.));
        assert!(measured.source_width >= extent - px(0.5));
        measured.viewport.center()
    });
    // Exercise the native horizontal input path, not a direct mutation of our cached metrics.
    ui.simulate_event(ScrollEvent {
        // Wheel and trackpad fixtures retain pointer-device semantics.
        source: gpui_kit::ScrollSource::Pointer,
        position,
        delta: ScrollDelta::Pixels(point(px(-75.), px(0.))),
        touch_phase: TouchPhase::Moved,
        modifiers: Default::default(),
    });
    settle(ui);
    ui.update(|window, cx| {
        let editors = view.read(cx).editors.clone();
        assert!(editors[0].read(cx).scroll_offset().x < px(-50.));
        assert_eq!(editors[1].read(cx).scroll_offset().x, px(0.));
        for editor in editors {
            assert_eq!(editor.read(cx).selected_range(), 0..0);
            editor.update(cx, |editor, cx| editor.set_soft_wrap(true, window, cx));
        }
    });
    settle(ui);
    reset_hidden_work();
    ui.update(|window, cx| {
        for editor in view.read(cx).editors.clone() {
            cx.notify(editor.entity_id());
        }
        window.refresh();
        window.draw(cx).clear(cx);
    });
    settle(ui);
    assert_no_hidden_work();
    ui.update(|_, cx| {
        assert_wrapped_native_geometry(&view, cx);
        let fixture = view.read(cx);
        let group = fixture.group.state.read(cx);
        for (index, editor) in fixture.editors.iter().enumerate() {
            assert_eq!(editor.read(cx).scroll_offset().x, px(0.));
            let measured = group.measurements[index].as_ref().unwrap();
            assert_eq!(measured.source_width, measured.viewport.size.width);
        }
    });
}

/// Masking changes native glyph presentation even while the existing row map still wraps.
#[gpui::test]
fn native_map_rejects_masked_native_presentation(cx: &mut TestAppContext) {
    let text = wrapped_source();
    let (view, ui) = mount_sources(cx, [text.clone(), text.clone()]);
    ui.update(|window, cx| {
        let fixture = view.read(cx);
        let editor = fixture.editors[0].clone();
        let rows = fixture.group.state.read(cx).rows.clone().unwrap();
        assert!(geometry_display_map::supported(
            editor.read(cx),
            &rows.source_rows[0]
        ));
        // This is a public native presentation change, with no source edit or host revision.
        editor.update(cx, |editor, cx| editor.toggle_masked(window, cx));
        let native = editor.read(cx);
        assert!(native.presentation().is_masked());
        assert!(native.display_map().wrap_row_count() > native.display_map().buffer_line_count());
        assert!(!geometry_display_map::supported(
            native,
            &rows.source_rows[0]
        ));
        assert_eq!(native.value().as_ref(), text);
        editor.update(cx, |editor, cx| editor.toggle_masked(window, cx));
    });
    settle(ui);
    ui.update(|_, cx| assert_wrapped_native_geometry(&view, cx));
}
