//! Sparse diff hunks must keep native wheel work bounded across thousands of hidden source rows.
use super::*;
use gpui_kit::{ScrollDelta, ScrollEvent, TouchPhase};
use std::{sync::Arc, time::Instant};

/// A disclosure blocks native text clicks while wheel input still reaches the shared viewport.
#[gpui::test]
fn folded_disclosure_preserves_shared_wheel_input(cx: &mut TestAppContext) {
    let original = (0..1280)
        .map(|row| format!("dependency_{row:04}\n"))
        .collect::<String>();
    let changed = original.replace("dependency_0500", "changed_at_0500");
    let (view, ui) = mount_sources(cx, [original.clone(), changed.clone()]);
    ui.update(|_, cx| {
        let group = view.read(cx).group.clone();
        let svg: Arc<[u8]> =
            Arc::from(include_bytes!("../../../assets/icons/explorer-expand-all.svg").as_slice());
        group.set_folding(UnchangedFolds::new(3, [svg.clone(), svg]), cx);
        group.fold_unchanged(true, cx);
    });
    settle(ui);
    for delta in [
        ScrollDelta::Pixels(point(px(0.), px(-8.))),
        ScrollDelta::Lines(point(0., -1.)),
    ] {
        let (position, before, folds) = ui.update(|_, cx| {
            let fixture = view.read(cx);
            let group = fixture.group.state.read(cx);
            let viewport = group.frames[0].as_ref().unwrap().viewport;
            let y = group
                .layout
                .as_ref()
                .unwrap()
                .folds
                .iter()
                .find(|(_, y)| {
                    px(y.start) >= group.scroll.get()
                        && px(y.end) <= group.scroll.get() + viewport.size.height
                })
                .unwrap()
                .1
                .clone();
            (
                point(
                    viewport.center().x,
                    viewport.top() + px((y.start + y.end) / 2.) - group.scroll.get(),
                ),
                group.scroll.get(),
                group.folding.as_ref().unwrap().collapsed(),
            )
        });
        ui.simulate_mouse_move(position, None, Default::default());
        ui.simulate_event(ScrollEvent {
            source: gpui_kit::ScrollSource::Pointer,
            position,
            delta,
            touch_phase: TouchPhase::Moved,
            modifiers: Default::default(),
        });
        for _ in 0..10 {
            ui.executor()
                .advance_clock(std::time::Duration::from_millis(16));
            settle(ui);
        }
        ui.update(|_, cx| {
            let fixture = view.read(cx);
            let group = fixture.group.state.read(cx);
            assert!(
                group.scroll.get() > before,
                "a disclosure must pass wheel input to the shared comparison viewport"
            );
            assert_eq!(group.folding.as_ref().unwrap().collapsed(), folds);
            for (index, editor) in fixture.editors.iter().enumerate() {
                assert_eq!(
                    editor.read(cx).value().as_ref(),
                    [&original, &changed][index]
                );
                assert_eq!(editor.read(cx).selected_range(), 0..0);
            }
        });
    }
}

/// Hidden long lines retain the native horizontal limit without repainting every source glyph.
#[gpui::test]
fn folded_nowrap_uses_native_horizontal_extent_without_hidden_glyph_batches(
    cx: &mut TestAppContext,
) {
    let original = (0..1280)
        .map(|row| {
            if row == 900 {
                format!("hidden_long_line {}\n", "中文abcdef".repeat(140))
            } else {
                format!("dependency_{row:04} = \"1.0\"\n")
            }
        })
        .collect::<String>();
    let changed = original.replace("dependency_0500", "changed_at_0500");
    let (view, ui) = mount_sources(cx, [original, changed]);
    ui.update(|window, cx| {
        let fixture = view.read(cx);
        let group = fixture.group.clone();
        let editors = fixture.editors.clone();
        for editor in &editors {
            editor.update(cx, |native, cx| native.set_soft_wrap(false, window, cx));
        }
        let svg: Arc<[u8]> =
            Arc::from(include_bytes!("../../../assets/icons/explorer-expand-all.svg").as_slice());
        group.set_folding(UnchangedFolds::new(3, [svg.clone(), svg]), cx);
        group.fold_unchanged(true, cx);
        editors[0].update(cx, |native, cx| native.focus(window, cx));
    });
    settle(ui);
    geometry::MEASURED_ROWS.with(|rows| rows.set(0));
    geometry_measurement::PLANNED_ROWS.with(|rows| rows.set([0; 3]));
    ui.update(|_, cx| {
        let editor = view.read(cx).editors[0].clone();
        editor.update(cx, |native, cx| {
            native.set_scroll_offset(point(px(-1_000_000.), native.scroll_offset().y), cx)
        });
    });
    settle(ui);
    assert_eq!(
        geometry::MEASURED_ROWS.with(|rows| rows.get()),
        0,
        "folded native extent must not measure every hidden glyph"
    );
    assert_eq!(
        geometry_measurement::PLANNED_ROWS.with(|rows| rows.get()),
        [0; 3]
    );
    ui.update(|_, cx| {
        let fixture = view.read(cx);
        let native = fixture.editors[0].read(cx);
        let group = fixture.group.state.read(cx);
        let measured = group.measurements[0].as_ref().unwrap();
        assert!(
            native.scroll_offset().x < px(-1000.),
            "the hidden long line must remain horizontally scrollable"
        );
        assert!(
            (measured.source_width - measured.viewport.size.width + native.scroll_offset().x).abs()
                < px(1.)
        );
        assert_eq!(native.cursor(), 0);
        assert!(native.visible_row_range().unwrap().len() <= 64);
    });
    ui.update(|window, cx| {
        for editor in view.read(cx).editors.clone() {
            editor.update(cx, |native, cx| native.set_soft_wrap(true, window, cx));
        }
    });
    settle(ui);
    ui.update(|_, cx| {
        let fixture = view.read(cx);
        let group = fixture.group.state.read(cx);
        for (index, editor) in fixture.editors.iter().enumerate() {
            assert_eq!(editor.read(cx).scroll_offset().x, px(0.));
            let measured = group.measurements[index].as_ref().unwrap();
            assert_eq!(measured.source_width, measured.viewport.size.width);
        }
    });
}

/// A native blink-like notification must validate once, without band redraws restarting admission.
#[gpui::test]
fn folded_band_repaints_do_not_repeat_private_native_validation(cx: &mut TestAppContext) {
    let original = (0..1280)
        .map(|row| format!("dependency_{row:04} = \"1.0\"\n"))
        .collect::<String>();
    let changed = [500, 720, 940, 1160]
        .into_iter()
        .fold(original.clone(), |text, row| {
            text.replace(
                &format!("dependency_{row:04}"),
                &format!("changed_at_{row:04}"),
            )
        });
    let (view, ui) = mount_sources(cx, [original, changed]);
    let _backgrounds = ui.update(|window, cx| {
        let fixture = view.read(cx);
        let group = fixture.group.clone();
        let editors = fixture.editors.clone();
        let backgrounds = editors
            .iter()
            .map(|source| {
                source.update(cx, |native, cx| native.set_soft_wrap(false, window, cx));
                let mut start = 0;
                let marks = source
                    .read(cx)
                    .value()
                    .split_inclusive('\n')
                    .enumerate()
                    .filter_map(|(row, line)| {
                        let range = start..start + line.len();
                        start = range.end;
                        (row % 7 == 0).then(|| gpui_base::input::RangeDecoration::new(range))
                    })
                    .collect();
                crate::row_backgrounds::RowBackgroundCollection::new(source, marks, cx)
            })
            .collect::<Vec<_>>();
        let svg: Arc<[u8]> =
            Arc::from(include_bytes!("../../../assets/icons/explorer-expand-all.svg").as_slice());
        group.set_folding(UnchangedFolds::new(3, [svg.clone(), svg]), cx);
        group.fold_unchanged(true, cx);
        editors[0].update(cx, |native, cx| native.focus(window, cx));
        group
            .state
            .update(cx, |state, cx| state.scroll_by(px(380.), cx));
        backgrounds
    });
    settle(ui);
    let repaints = Rc::new(Cell::new(0));
    let observed = repaints.clone();
    let _repaints = ui.update(|_, cx| {
        let pane = view.read(cx).group.panes[0].clone();
        cx.observe(&pane, move |_, _| observed.set(observed.get() + 1))
    });
    for _ in 0..12 {
        geometry::MEASURED_ROWS.with(|rows| rows.set(0));
        geometry_display_map::VALIDATED_ROWS.with(|rows| rows.set(0));
        ui.update(|_, cx| {
            let fixture = view.read(cx);
            let group = fixture.group.state.clone();
            let editor = fixture.editors[0].clone();
            group.update(cx, |state, cx| state.scroll_by(px(12.), cx));
            cx.notify(editor.entity_id());
        });
        settle(ui);
        assert_eq!(
            geometry::MEASURED_ROWS.with(|rows| rows.get())
                + geometry_display_map::VALIDATED_ROWS.with(|rows| rows.get()),
            1281,
            "one unknown native change must validate every row once; missing band presentation must not revoke it again"
        );
        repaints.set(0);
        settle(ui);
        assert_eq!(
            repaints.get(),
            0,
            "band admission must settle without an ongoing presentation repaint loop"
        );
    }
}

/// Reproduce a lockfile comparison with distant changes visible beside several disclosures.
#[gpui::test]
fn folded_diff_scroll_bounds_native_layout_across_distant_hunks(cx: &mut TestAppContext) {
    let original = (0..4200)
        .map(|row| format!("dependency_{row:04} = \"1.0\"\n"))
        .collect::<String>();
    let changed = original
        .replace("dependency_1750", "changed_at_1750")
        .replace("dependency_1795", "changed_at_1795")
        .replace("dependency_3670", "changed_at_3670");
    let (view, ui) = mount_sources(cx, [original.clone(), changed.clone()]);
    ui.update(|_, cx| {
        let group = view.read(cx).group.clone();
        let svg: Arc<[u8]> =
            Arc::from(include_bytes!("../../../assets/icons/explorer-expand-all.svg").as_slice());
        group.set_folding(UnchangedFolds::new(3, [svg.clone(), svg]), cx);
        group.fold_unchanged(true, cx);
    });
    settle(ui);
    let wheel_position = ui.update(|_, cx| {
        let fixture = view.read(cx);
        let group = fixture.group.state.read(cx);
        assert_eq!(group.effective_status(), AlignmentStatus::Ready);
        assert!(group.layout.as_ref().unwrap().folds.len() >= 3);
        group.viewports[0].unwrap().center()
    });
    let started = Instant::now();
    let mut largest = 0;
    for _ in 0..3 {
        let before = ui.update(|_, cx| view.read(cx).group.scroll_offset(cx));
        ui.simulate_event(ScrollEvent {
            source: gpui_kit::ScrollSource::Pointer,
            position: wheel_position,
            delta: ScrollDelta::Pixels(point(px(0.), px(-24.))),
            touch_phase: TouchPhase::Moved,
            modifiers: Default::default(),
        });
        settle(ui);
        ui.update(|_, cx| {
            let fixture = view.read(cx);
            assert!(fixture.group.scroll_offset(cx) > before);
            for (index, editor) in fixture.editors.iter().enumerate() {
                let native = editor.read(cx);
                largest = largest.max(native.visible_row_range().unwrap().len());
                assert_eq!(native.value().as_ref(), [&original, &changed][index]);
            }
        });
    }
    eprintln!(
        "folded sparse diff: warm_wheels={:?}, native_rows_max={largest}",
        started.elapsed()
    );
    assert!(
        largest <= 64,
        "folded scrolling laid out {largest} source rows for a viewport with fewer than ten visible rows"
    );
    ui.update(|_, cx| {
        let group = view.read(cx).group.clone();
        group
            .state
            .update(cx, |state, cx| state.scroll_by(px(120.), cx));
    });
    settle(ui);
    let targets = ui.update(|_, cx| {
        let fixture = view.read(cx);
        let viewport = fixture.group.state.read(cx).frames[0]
            .as_ref()
            .unwrap()
            .viewport;
        let visible = (1747..1799)
            .filter_map(|row| {
                let byte = original.find(&format!("dependency_{row:04}")).unwrap() + 4;
                let bounds = fixture.group.project_range(0, byte..byte, cx)?;
                viewport.contains(&bounds.center()).then_some((
                    row,
                    byte,
                    point(bounds.left() + px(0.4), bounds.center().y),
                ))
            })
            .collect::<Vec<_>>();
        let pair = visible
            .windows(2)
            .find(|pair| pair[1].0 > pair[0].0 + 20)
            .expect("visible source carets must remain queryable on both sides of a distant fold");
        [pair[1], pair[0]]
    });
    for (_, byte, position) in targets {
        ui.simulate_click(position, Default::default());
        settle(ui);
        ui.update(|window, cx| {
            let editor = view.read(cx).editors[0].read(cx);
            assert_eq!(editor.cursor(), byte);
            assert!(editor.visible_row_range().unwrap().len() <= 64);
            painted_caret(&view, window, cx);
        });
    }
}
