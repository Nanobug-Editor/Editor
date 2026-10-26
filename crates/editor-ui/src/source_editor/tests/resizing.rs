//! Repeated real source layouts isolate width-sensitive frame cost from native window focus.

use super::*;
use gpui_kit::{EntityInputHandler as _, gpui};
use std::time::{Duration, Instant};

/// Draw the same source using native fonts; no-op test fonts omit actual shaping costs.
/// Borrow the production platform text system while retaining deterministic test window dispatch.
#[test]
#[ignore = "explicit native-font width/height CPU comparison; run Release with --nocapture"]
fn source_resize_frame_cost() {
    let platform = gpui_kit::platform::current_platform(false);
    let mut cx = TestAppContext::build_with_text_system(
        gpui_kit::TestDispatcher::new(0),
        None,
        platform.text_system(),
    );
    let (editor, visual) = source(&mut cx);
    let requested_rows = std::env::var("NANOBUG_DRAG_SOURCE_ROWS")
        .unwrap_or_else(|_| "1500".into())
        .parse::<usize>()
        .unwrap();
    assert!((1..=10_000).contains(&requested_rows));
    let text = if let Some(path) = std::env::var_os("NANOBUG_DRAG_SOURCE_PATH") {
        std::fs::read_to_string(path).unwrap()
    } else {
        (0..requested_rows).map(|row| format!("fn row_{row:04}() {{ let message = \"原生编辑器 resize 中文😀 wrapped source\"; }}\n"))
            .collect::<String>()
    };
    let rows = text.lines().count();
    visual.update(|window, cx| {
        editor.update(cx, |state, cx| {
            state.set_value(text.clone(), window, cx);
            if std::env::var_os("NANOBUG_DRAG_NO_WRAP").is_some() {
                state.set_soft_wrap(false, window, cx);
            }
        })
    });
    let mut timings = [Vec::new(), Vec::new()];
    for (axis, times) in timings.iter_mut().enumerate() {
        for step in 0..36 {
            let extent = if axis == 0 {
                size(px(800. + step as f32), px(500.))
            } else {
                size(px(800.), px(500. + step as f32))
            };
            let start = Instant::now();
            visual.simulate_resize(extent);
            visual.update(|window, cx| window.draw(cx).clear(cx));
            let elapsed = start.elapsed();
            if step >= 4 {
                times.push(elapsed);
            }
        }
    }
    for (axis, times) in timings.iter_mut().enumerate() {
        times.sort();
        eprintln!(
            "source resize rows={rows} axis={axis}: p50={:?}, p95={:?}, max={:?}",
            times[times.len() / 2],
            times[times.len() * 95 / 100],
            times.last().unwrap()
        );
    }
    visual.update(|_, cx| assert_eq!(editor.read(cx).value().as_ref(), text));
    assert!(
        timings[0][16] <= timings[1][16] * 3 + Duration::from_millis(2),
        "changing source width must not add document-wide work to each frame"
    );
}

/// Live width changes must preserve the original IME selection and Undo owner while reflowing.
#[gpui::test]
fn source_resize_preserves_composition_and_reflows_immediately(cx: &mut TestAppContext) {
    let (editor, visual) = source(cx);
    let original = (0..50)
        .map(|row| {
            format!(
                "    line {row}: {}\n",
                "中文😀 alpha beta finish ".repeat(12)
            )
        })
        .collect::<String>();
    for dark in [false, true] {
        visual.update(|window, cx| {
            crate::theme::apply_theme(crate::theme::builtin_theme(dark), cx);
            editor.update(cx, |state, cx| {
                state.set_value(original.clone(), window, cx)
            });
        });
        visual.simulate_keystrokes("ctrl-end");
        visual.run_until_parked();
        let (composing, cursor, marked) = visual.update(|window, cx| {
            editor.update(cx, |state, cx| {
                state.replace_and_mark_text_in_range(None, "拼😀", Some(3..3), window, cx);
                (
                    state.value().to_string(),
                    state.cursor(),
                    state.marked_text_range(window, cx).unwrap(),
                )
            })
        });
        let mut row_counts = Vec::new();
        for width in [220., 600., 270., 1200., 221., 220.] {
            visual.simulate_resize(size(px(width), px(500.)));
            visual.update(|window, cx| {
                window.draw(cx).clear(cx);
                editor.update(cx, |state, cx| {
                    assert_eq!(state.value().as_ref(), composing);
                    assert_eq!(state.cursor(), cursor);
                    assert_eq!(state.marked_text_range(window, cx), Some(marked.clone()));
                    row_counts.push(state.display_map().display_row_count());
                });
            });
        }
        assert!(
            row_counts[0] > row_counts[3],
            "narrow widths must wrap immediately"
        );
        assert_eq!(
            row_counts[0], row_counts[5],
            "returning to a width restores its rows"
        );
        visual.update(|window, cx| {
            editor.update(cx, |state, cx| {
                state.replace_text_in_range(None, "中文😀", window, cx);
                assert_eq!(state.marked_text_range(window, cx), None);
                assert_eq!(state.value().as_ref(), format!("{original}中文😀"));
            });
        });
        visual.simulate_keystrokes("ctrl-z");
        visual.run_until_parked();
        visual.update(|_, cx| assert_eq!(editor.read(cx).value().as_ref(), original));
    }
}
