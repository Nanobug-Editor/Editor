//! Real wheel frames must reuse source geometry instead of rescanning an unchanged long diff.
use super::*;
use gpui_kit::{ScrollDelta, ScrollEvent, TouchPhase};
use std::time::Instant;

/// Native wrapped source, a middle insertion and distant scroll positions share one real viewport.
#[gpui::test]
#[ignore = "explicit native long-diff performance regression; prints wheel-frame timings"]
fn long_diff_scroll_reuses_geometry_and_bounds_warm_frame_work(cx: &mut TestAppContext) {
    // Explicit diagnostic overrides tighten the same real path; CI/default measurements keep
    // the representative long-file fixture and never assert environment-sensitive elapsed time.
    let rows = std::env::var("NANOBUG_DIFF_BENCH_ROWS")
        .ok()
        .map(|value| value.parse::<usize>().unwrap())
        .unwrap_or(1800);
    let wheels = std::env::var("NANOBUG_DIFF_BENCH_WHEELS")
        .ok()
        .map(|value| value.parse::<usize>().unwrap())
        .unwrap_or(8);
    assert!((80..=1800).contains(&rows) && (1..=8).contains(&wheels));
    exercise_scroll(cx, rows, wheels, false);
}

/// Dense changed bands use the same full-row paint path as an actual colored comparison.
#[gpui::test]
#[ignore = "explicit native long-diff performance regression with full-row backgrounds"]
fn long_marked_diff_scroll_bounds_native_band_work(cx: &mut TestAppContext) {
    let rows = std::env::var("NANOBUG_DIFF_BENCH_ROWS")
        .ok()
        .map(|value| value.parse::<usize>().unwrap())
        .unwrap_or(1800);
    assert!((80..=1800).contains(&rows));
    exercise_scroll(cx, rows, 2, true);
}

/// Keep the same real performance regression in ordinary UI tests with a bounded, quick input.
#[gpui::test]
fn warm_diff_scroll_retains_row_geometry_and_alignment(cx: &mut TestAppContext) {
    exercise_scroll(cx, 120, 2, false);
}

/// Preserve the dense-band regression in the ordinary UI gate as well as the explicit long run.
#[gpui::test]
fn warm_changed_bands_share_native_row_geometry(cx: &mut TestAppContext) {
    exercise_scroll(cx, 120, 2, true);
}

/// Inertial movement supplies fractional translations, which must not invalidate wrapped rows.
#[gpui::test]
fn fractional_diff_scroll_retains_row_geometry(cx: &mut TestAppContext) {
    exercise_scroll_delta(cx, 120, 2, false, px(-48.375));
}

/// Attach real full-row background owners for every seventh changed source line.
fn attach_bands(
    view: &Entity<Fixture>,
    ui: &mut VisualTestContext,
) -> Vec<super::super::super::row_backgrounds::RowBackgroundCollection> {
    ui.update(|_, cx| {
        let fixture = view.read(cx);
        let editors = fixture.editors.clone();
        let group = fixture.group.clone();
        group.set_inner_gutters(true, cx);
        editors
            .iter()
            .enumerate()
            .map(|(index, editor)| {
                let text = editor.read(cx).value();
                let mut start = 0;
                let marks = text
                    .split_inclusive('\n')
                    .filter_map(|line| {
                        let range = start..start + line.len();
                        start = range.end;
                        let row = line.get(7..11)?.parse::<usize>().ok()?;
                        (row % 7 == 0).then(|| {
                            gpui_base::input::RangeDecoration::new(range).with_color(
                                gpui_kit::rgb(if index == 0 { 0xf9aaaa } else { 0x99dd99 }).into(),
                            )
                        })
                    })
                    .collect();
                super::super::super::row_backgrounds::RowBackgroundCollection::new(
                    editor, marks, cx,
                )
            })
            .collect()
    })
}

/// Measure only warmed wheel work; document admission and native wrapping finish before counters.
fn exercise_scroll(cx: &mut TestAppContext, rows: usize, wheels: usize, marked: bool) {
    exercise_scroll_delta(cx, rows, wheels, marked, px(-48.));
}

/// Exercise the same native wheel path with `delta` in logical pixels, including subpixel motion.
fn exercise_scroll_delta(
    cx: &mut TestAppContext,
    rows: usize,
    wheels: usize,
    marked: bool,
    delta: Pixels,
) {
    let original = (0..rows)
        .map(|row| {
            format!("fn row_{row:04}() {{ let message = \"原生滚动 中文😀 wrapped source\"; }}\n")
        })
        .collect::<String>();
    let needle = format!("fn row_{:04}()", rows / 2);
    let changed = original
        .split_inclusive('\n')
        .enumerate()
        .map(|(row, line)| {
            if marked && row % 7 == 0 {
                line.replace("let message", "let revised")
            } else {
                line.to_owned()
            }
        })
        .collect::<String>()
        .replace(&needle, &format!("// inserted change\n{needle}"));
    let started = Instant::now();
    let (view, ui) = mount_sources(cx, [original.clone(), changed.clone()]);
    let _layers = marked.then(|| attach_bands(&view, ui));
    if marked {
        settle(ui);
    }
    let (point, selections) = ui.update(|_, cx| {
        let fixture = view.read(cx);
        let group = fixture.group.state.read(cx);
        assert_eq!(group.effective_status(), AlignmentStatus::Ready);
        (
            group.viewports[0].unwrap().center(),
            fixture
                .editors
                .each_ref()
                .map(|editor| editor.read(cx).selected_range()),
        )
    });
    let setup = started.elapsed();
    geometry::MEASURED_ROWS.with(|count| count.set(0));
    geometry::MEASURE_NANOS.with(|time| time.set(0));
    geometry::LAYOUT_BUILDS.with(|count| count.set(0));
    super::super::super::row_backgrounds::NATIVE_ROW_READS.with(|count| count.set(0));
    let mut frames = Vec::new();
    for _ in 0..wheels {
        let started = Instant::now();
        let before = ui.update(|_, cx| view.read(cx).group.scroll_offset(cx));
        ui.simulate_event(ScrollEvent {
            // Wheel and trackpad fixtures retain pointer-device semantics.
            source: gpui_kit::ScrollSource::Pointer,
            position: point,
            delta: ScrollDelta::Pixels(gpui_kit::point(px(0.), delta)),
            touch_phase: TouchPhase::Moved,
            modifiers: Default::default(),
        });
        // Deliver the finite projection/authority follow-up, as a real platform frame loop does.
        for _ in 0..2 {
            ui.update(|window, cx| {
                window.draw(cx).clear(cx);
                window.simulate_next_frame(cx);
            });
            ui.run_until_parked();
        }
        frames.push(started.elapsed());
        ui.update(|_, cx| {
            assert!(
                view.read(cx).group.scroll_offset(cx) > before,
                "wheel must reach real shared scrolling"
            );
        });
    }
    let scanned = geometry::MEASURED_ROWS.with(|count| count.get());
    let geometry_nanos = geometry::MEASURE_NANOS.with(|time| time.get());
    let layouts = geometry::LAYOUT_BUILDS.with(|count| count.get());
    let band_reads =
        super::super::super::row_backgrounds::NATIVE_ROW_READS.with(|count| count.get());
    eprintln!(
        "long diff: rows={rows}, wheels={wheels}, marked={marked}, setup={setup:?}, warm_wheel_frames={frames:?}, logical_row_probes={scanned}, geometry_ms={}, alignment_builds={layouts}, native_band_reads={band_reads}",
        geometry_nanos as f64 / 1_000_000.
    );
    ui.update(|_, cx| {
        let fixture = view.read(cx);
        assert_eq!(fixture.group.status(cx), AlignmentStatus::Ready);
        for (index, expected) in [original, changed].into_iter().enumerate() {
            assert_eq!(fixture.editors[index].read(cx).value().as_ref(), expected);
            assert_eq!(
                fixture.editors[index].read(cx).selected_range(),
                selections[index]
            );
            let visible = fixture.editors[index].read(cx).visible_row_range().unwrap();
            assert!(
                visible.len() <= 32,
                "scrolling laid out {} logical source rows for a small native viewport",
                visible.len()
            );
        }
    });
    // The window shows fewer than twenty wrapped rows per pane. A warm wheel may inspect a
    // viewport-sized band; traversing the whole source again is the observed costly behavior.
    assert!(
        scanned <= wheels * 2 * 32,
        "stable scrolling scanned {scanned} source rows instead of viewport-bounded work"
    );
    assert_eq!(
        layouts, 0,
        "translating source pixels must not rebuild row alignment"
    );
    assert!(
        band_reads <= wheels * 2 * 32,
        "warm changed bands performed {band_reads} native coordinate reads"
    );
}

/// Compare cached rows to the same original native editor's current completed wrap geometry.
fn assert_native_rows(view: &Entity<Fixture>, cx: &App) {
    let fixture = view.read(cx);
    let group = fixture.group.state.read(cx);
    assert_eq!(group.effective_status(), AlignmentStatus::Ready);
    let rows = group.rows.as_ref().unwrap();
    for (index, source) in fixture.editors.iter().enumerate() {
        let source = source.read(cx);
        let origin = source.range_to_bounds(&(0..0)).unwrap();
        let measured = group.measurements[index].as_ref().unwrap();
        for (row, range) in rows.source_rows[index].iter().enumerate() {
            let actual = source.range_to_bounds(&(range.start..range.start)).unwrap();
            assert!(
                (f32::from(actual.top() - origin.top()) - measured.logical_rows[row].start).abs()
                    < 0.5
            );
        }
    }
}

/// Cache invalidation uses actual native geometry and notification boundaries, not content alone.
#[gpui::test]
fn geometry_cache_remeasures_wrap_font_padding_and_scale_changes(cx: &mut TestAppContext) {
    let (view, ui) = mount(cx);
    let (old_height, epoch) = ui.update(|_, cx| {
        let group = view.read(cx).group.state.read(cx);
        (group.layout.as_ref().unwrap().display_height, group.epoch)
    });
    ui.update(|window, cx| {
        for editor in &view.read(cx).editors.clone() {
            editor.update(cx, |state, cx| state.set_soft_wrap(false, window, cx));
        }
    });
    settle(ui);
    let unwrapped = ui.update(|_, cx| {
        assert_native_rows(&view, cx);
        let group = view.read(cx).group.state.read(cx);
        assert_eq!(
            group.epoch, epoch,
            "geometry changes must retain source correspondence"
        );
        let height = group.layout.as_ref().unwrap().display_height;
        assert!(
            height < old_height,
            "wrap changes must revoke old row heights"
        );
        height
    });
    ui.update(|_, cx| {
        crate::typography::set_font_size(cx, 22.);
        crate::theme::apply_theme(crate::theme::builtin_theme(true), cx);
        cx.notify(view.entity_id());
    });
    settle(ui);
    ui.update(|_, cx| {
        assert_native_rows(&view, cx);
        let group = view.read(cx).group.state.read(cx);
        assert!(group.layout.as_ref().unwrap().display_height > unwrapped);
    });
    view.update(ui, |fixture, cx| {
        // This setter intentionally does not notify EditorState. A parent repaint must still
        // detect a changed native inset rather than reusing old wrap/input coordinates.
        fixture.editors[0].update(cx, |editor, _| {
            editor.set_editor_paddings(gpui_kit::Edges {
                top: px(9.),
                left: px(11.),
                right: px(13.),
                bottom: px(0.),
            });
        });
        cx.notify();
    });
    settle(ui);
    ui.update(|_, cx| {
        assert_native_rows(&view, cx);
        assert_eq!(
            view.read(cx).group.state.read(cx).measurements[0]
                .as_ref()
                .unwrap()
                .native_inset_y,
            px(9.)
        );
    });
    geometry::MEASURED_ROWS.with(|count| count.set(0));
    ui.simulate_scale_factor_change(1.5);
    settle(ui);
    assert!(
        geometry::MEASURED_ROWS.with(|count| count.get()) > 0,
        "DPI change must revoke the geometry cache"
    );
    ui.update(|_, cx| assert_native_rows(&view, cx));
}
