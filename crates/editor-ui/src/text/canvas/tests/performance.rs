//! Native-font frame workload compares continuous column resizing with viewport translation.
use super::*;
use gpui_kit::{IntoElement as _, Styled as _, TestDispatcher, canvas};
use std::{sync::Arc, time::Instant};

/// Three viewports of retained cells match a virtual canvas's visible rows plus its scroll buffer.
struct Rows {
    /// Content stays identical across frames so the benchmark measures geometry changes only.
    cells: Arc<Vec<[SharedString; 4]>>,
    /// Alternate moving a column divider and translating rows using the same drawing workload.
    resizing: bool,
    /// Absolute motion step prevents accumulated floating-point drift.
    step: usize,
}

impl Render for Rows {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl gpui_kit::IntoElement {
        let cells = self.cells.clone();
        let delta = (self.step % 120) as f32;
        let widths = if self.resizing {
            [500. - delta, 220. + delta, 90., 170.]
        } else {
            [500., 220., 90., 170.]
        };
        let offset = if self.resizing { 0. } else { delta };
        canvas(
            |_, _, _| (),
            move |bounds, _, window, cx| {
                for (row, cells) in cells.iter().enumerate() {
                    let mut x = bounds.left();
                    for (column, content) in cells.iter().enumerate() {
                        let mut face = font("Segoe UI");
                        if column == 2 {
                            face.weight = FontWeight::BOLD;
                        }
                        CanvasText::new(
                            content.clone(),
                            face,
                            px(13.),
                            rgb(0).into(),
                            Bounds::new(
                                point(x, bounds.top() + px(row as f32 * 24. - 600. - offset)),
                                size(px(widths[column] - 8.), px(24.)),
                            ),
                            if column == 1 {
                                TextAlign::Right
                            } else {
                                TextAlign::Left
                            },
                            window,
                        )
                        .paint(window, cx);
                        x += px(widths[column]);
                    }
                }
            },
        )
        .size_full()
        .into_any_element()
    }
}

/// Report real native text preparation/paint CPU cost; this is not GPU presentation FPS.
/// Run explicitly in Release on Windows, keeping the same font/content for both motions.
#[test]
#[ignore = "native text timing baseline; run Release with --nocapture --test-threads=1"]
fn canvas_column_resize_and_scroll_frame_cost() {
    let platform = gpui_platform::current_platform(false);
    let mut cx = TestAppContext::build_with_text_system(
        TestDispatcher::new(0),
        None,
        platform.text_system(),
    );
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::typography::init(cx);
    });
    let cells = Arc::new(
        (0..75)
            .map(|row| {
                [
                    format!(
                        "feat(editor-{row}): 整合本地控件、历史与插件能力 — {}",
                        "continuous native canvas layout 中文😀 ".repeat(8)
                    )
                    .into(),
                    format!("origin/codex/very-long-branch-reference-{row:04}-中文").into(),
                    "Tmiracle".into(),
                    format!("2026-10-24 18:{:02}:00", row % 60).into(),
                ]
            })
            .collect(),
    );
    let (root, visual) = cx.add_window_view(|_, _| Rows {
        cells,
        resizing: true,
        step: 0,
    });
    visual.simulate_resize(size(px(1000.), px(600.)));
    for resizing in [true, false] {
        let mut times = Vec::new();
        for step in 0..140 {
            let elapsed = visual.update(|window, cx| {
                root.update(cx, |state, cx| {
                    state.resizing = resizing;
                    state.step = step;
                    cx.notify();
                });
                let start = Instant::now();
                window.draw(cx).clear(cx);
                start.elapsed()
            });
            if step >= 20 {
                times.push(elapsed);
            }
        }
        times.sort();
        eprintln!(
            "canvas native text resizing={resizing} visible_rows=25 retained_rows=75 p50={:?} p95={:?} max={:?}",
            times[times.len() / 2],
            times[times.len() * 95 / 100],
            times.last().unwrap()
        );
    }
}
