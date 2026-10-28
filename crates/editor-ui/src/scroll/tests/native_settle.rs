//! Real font rasterization catches terminal pixel shifts that logical text bounds cannot expose.
use super::*;

/// Every row must share one pixel translation throughout the tail, in both themes and directions.
#[test]
#[ignore = "opens an isolated real Windows/D3D window; run explicitly with --test-threads=1"]
fn smooth_scroll_native_text_has_no_terminal_pixel_shift() {
    let samples = Rc::new(RefCell::new(Vec::new()));
    let output = samples.clone();
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            crate::typography::init(cx);
            crate::theme::apply_theme(crate::theme::builtin_theme(false), cx);
            crate::init(cx);
            let slot = Rc::new(RefCell::new(None));
            let capture = slot.clone();
            let window: gpui_kit::AnyWindowHandle = cx
                .open_window(
                    gpui_kit::WindowOptions {
                        window_bounds: Some(gpui_kit::WindowBounds::Windowed(
                            gpui_kit::Bounds::new(
                                point(px(100.), px(100.)),
                                size(px(600.), px(400.)),
                            ),
                        )),
                        ..Default::default()
                    },
                    |window, cx| {
                        let editor = cx.new(|cx| {
                            EditorState::new(window, cx).default_value(
                                (0..500)
                                    .map(|i| {
                                        format!(
                                            "row {i:03} alpha 中文 abcdefghijklmnopqrstuvwxyz\n"
                                        )
                                    })
                                    .collect::<String>(),
                            )
                        });
                        let tree = cx.new(|cx| gpui_base::TreeState::new(cx));
                        // Colored spans and fallback Chinese glyphs share the same native baseline.
                        editor.update(cx, |editor, cx| {
                            let marks = editor
                                .value()
                                .match_indices("alpha")
                                .map(|(start, word)| {
                                    gpui_base::input::TextDecoration::new(
                                        start..start + word.len(),
                                        gpui_kit::HighlightStyle {
                                            color: Some(gpui_kit::rgb(0x338866).into()),
                                            ..Default::default()
                                        },
                                    )
                                })
                                .collect();
                            editor.create_decorations_collection(marks, cx);
                        });
                        let host = cx.new(|_| Harness {
                            outer: ScrollHandle::new(),
                            inner: ScrollHandle::new(),
                            editor,
                            tree,
                            group: None,
                            kind: Kind::Editor,
                            show: true,
                            painted_rows: Rc::new(Cell::new(0)),
                        });
                        capture.replace(Some(host.clone()));
                        let cached = cx.new(|_| CachedHarness(host));
                        window.activate_window();
                        cx.new(|cx| gpui_base::Root::new(cached, window, cx))
                    },
                )
                .unwrap()
                .into();
            let host = slot.borrow_mut().take().unwrap();
            cx.spawn(async move |cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(200))
                    .await;
                for scenario in 0..8 {
                    window
                        .update(cx, |_, _, cx| {
                            crate::theme::apply_theme(
                                crate::theme::builtin_theme(scenario >= 4),
                                cx,
                            );
                            host.read(cx).editor.clone().update(cx, |editor, cx| {
                                editor.set_scroll_offset(
                                    point(px(0.), px(-3000. - scenario as f32 * 0.125)),
                                    cx,
                                );
                            });
                            host.update(cx, |_, cx| cx.notify());
                        })
                        .unwrap();
                    cx.background_executor()
                        .timer(Duration::from_millis(100))
                        .await;
                    window
                        .update(cx, |_, window, cx| {
                            window.dispatch_event(
                                gpui_kit::PlatformInput::Scroll(ScrollEvent {
                                    source: gpui_kit::ScrollSource::Pointer,
                                    position: point(px(100.), px(100.)),
                                    delta: ScrollDelta::Lines(point(
                                        0.,
                                        if scenario < 4 { -3. } else { 3. },
                                    )),
                                    modifiers: Default::default(),
                                    touch_phase: gpui_kit::TouchPhase::Moved,
                                }),
                                cx,
                            );
                        })
                        .unwrap();
                    for index in 0..70 {
                        cx.background_executor()
                            .timer(Duration::from_millis(8))
                            .await;
                        window
                            .update(cx, |_, window, cx| {
                                let scale = window.scale_factor();
                                let offset = host.read(cx).editor.read(cx).scroll_offset().y;
                                let image = window.render_to_image().unwrap();
                                // Text-only crop excludes cursor, scrollbar and viewport clipping edges.
                                let mut crop = Vec::new();
                                for y in (70. * scale) as u32..(180. * scale) as u32 {
                                    for x in (100. * scale) as u32..(440. * scale) as u32 {
                                        crop.push(image.get_pixel(x, y).0);
                                    }
                                }
                                if let Some(path) = std::env::var_os("NANOBUG_SETTLE_CAPTURE")
                                    && matches!(scenario, 0 | 4)
                                    && matches!(index, 0 | 13 | 69)
                                {
                                    image
                                        .save(
                                            std::path::Path::new(&path).join(format!(
                                                "case-{scenario}-frame-{index:03}.png"
                                            )),
                                        )
                                        .unwrap();
                                }
                                output.borrow_mut().push((
                                    scenario,
                                    f32::from(offset) * scale,
                                    (440. * scale) as usize - (100. * scale) as usize,
                                    crop,
                                ));
                            })
                            .unwrap();
                    }
                }
                window
                    .update(cx, |_, window, cx| {
                        window.blur(cx);
                        window.refresh();
                    })
                    .unwrap();
                cx.background_executor()
                    .timer(Duration::from_millis(50))
                    .await;
                window
                    .update(cx, |_, window, _| window.remove_window())
                    .unwrap();
                drop(host);
                cx.background_executor()
                    .timer(Duration::from_millis(50))
                    .await;
                cx.update(|cx| cx.quit());
            })
            .detach();
        });
    let samples = samples.borrow();
    let mut changed = 0;
    for scenario in 0..8 {
        let (_, settled, width, pixels) = samples
            .iter()
            .rev()
            .find(|(case, _, _, _)| *case == scenario)
            .unwrap();
        let tail = samples
            .iter()
            .filter(|(case, offset, _, _)| *case == scenario && (offset - settled).abs() < 1.5)
            .collect::<Vec<_>>();
        assert!(
            pixels.windows(2).any(|pair| pair[0] != pair[1]),
            "the crop must contain real text"
        );
        assert!(
            tail.iter().any(|(_, offset, _, _)| offset != settled),
            "capture at least one moving tail frame"
        );
        assert!(
            tail.len() > 4,
            "capture the final deceleration and idle frames"
        );
        let shifts = tail
            .iter()
            .filter(|(_, _, _, sample)| !same_text_rows(pixels, sample, *width))
            .count();
        eprintln!(
            "terminal text raster: case={scenario}, tail_frames={}, changed_frames={shifts}, target={settled}",
            tail.len()
        );
        changed += shifts;
    }
    assert_eq!(
        changed, 0,
        "final travel must translate text rows together without changing their spacing"
    );
}

/// Permit whole-pixel translation but reject adjacent text rows snapping on different frames.
fn same_text_rows(reference: &[[u8; 4]], sample: &[[u8; 4]], width: usize) -> bool {
    let height = reference.len() / width;
    (-2_isize..=2).any(|shift| {
        (3..height - 3).all(|y| {
            let other = y.wrapping_add_signed(shift);
            reference[y * width..(y + 1) * width] == sample[other * width..(other + 1) * width]
        })
    })
}
