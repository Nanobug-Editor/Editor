//! Bounded real Windows/D3D acceptance, isolated from the user's editor windows and profile.
use super::*;

/// Open an isolated native root, measure actual vsync paints and observe the released inertial tail.
/// It does not use TestPlatform: native offsets are committed by real input layout/paint.
#[test]
#[ignore = "opens an isolated real Windows/D3D window; run explicitly with --test-threads=1"]
fn smooth_scroll_real_windows_paints_intermediate_native_positions() {
    let kind = match std::env::var("NANOBUG_SCROLL_SURFACE").as_deref() {
        Ok("tree") => Kind::Tree,
        Ok("tabs") => Kind::Tabs,
        Ok("nested") => Kind::Nested,
        Ok("comparison") => Kind::Comparison,
        _ => Kind::Editor,
    };
    let position = if matches!(kind, Kind::Tabs) {
        point(px(80.), px(16.))
    } else {
        point(px(80.), px(80.))
    };
    let samples = Rc::new(RefCell::new(Vec::new()));
    let output = samples.clone();
    let frames = Rc::new(RefCell::new(Vec::with_capacity(512)));
    let frame_output = frames.clone();
    let moving_frames = frames.clone();
    // Explicit matrix controls exercise deep/wrapped/colored two- and three-source native views.
    let panes = std::env::var("NANOBUG_DIFF_NATIVE_PANES")
        .ok()
        .map(|v| v.parse::<usize>().unwrap())
        .unwrap_or(2);
    let start_row = std::env::var("NANOBUG_DIFF_NATIVE_ROW")
        .ok()
        .map(|v| v.parse::<usize>().unwrap())
        .unwrap_or(0);
    let marked = std::env::var_os("NANOBUG_DIFF_NATIVE_MARKED").is_some();
    // Dense native text runs expose quadratic shaping costs hidden by whole-row backgrounds.
    let decorated = std::env::var_os("NANOBUG_DIFF_NATIVE_DECORATED").is_some();
    let wrapped = std::env::var_os("NANOBUG_DIFF_NATIVE_WRAPPED").is_some();
    let focused = std::env::var_os("NANOBUG_DIFF_NATIVE_FOCUSED").is_some();
    let different = std::env::var_os("NANOBUG_DIFF_NATIVE_DIFFERENT").is_some();
    // Collapsed unchanged intervals must exercise the same original native input authority.
    let collapsed = std::env::var_os("NANOBUG_DIFF_NATIVE_COLLAPSED").is_some();
    // Production comparisons retain readonly snapshots beside their original editable source.
    let readonly_peers = std::env::var_os("NANOBUG_DIFF_NATIVE_READONLY_PEERS").is_some();
    // Cross several disclosures instead of measuring only inside the first expanded diff hunk.
    let wheel_lines = if collapsed { -6. } else { -2. };
    assert!((2..=3).contains(&panes) && start_row < 1700);
    if matches!(kind, Kind::Comparison) {
        eprintln!(
            "native comparison: panes={panes}, focused={focused}, wrapped={wrapped}, marked={marked}, decorated={decorated}, different={different}, collapsed={collapsed}, readonly_peers={readonly_peers}, source_row={start_row}"
        );
    }
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            crate::typography::init(cx);
            crate::theme::apply_theme(crate::theme::builtin_theme(false), cx);
            crate::init(cx);
            let text = (0..1800)
                .map(|i| {
                    if wrapped {
                        format!(
                            "第 {i} 行 alpha 中文😀 {}\n",
                            "native wrapped source ".repeat(2)
                        )
                    } else {
                        format!("第 {i} 行 alpha 中文\n")
                    }
                })
                .collect::<String>();
            let source_text = text.clone();
            let start_byte = text
                .split_inclusive('\n')
                .take(start_row)
                .map(str::len)
                .sum::<usize>();
            let background_leases = Rc::new(RefCell::new(Vec::new()));
            let paint_leases = background_leases.clone();
            let slot = Rc::new(RefCell::new(None));
            let capture = slot.clone();
            let window = cx
                .open_window(
                    gpui_kit::WindowOptions {
                        window_bounds: Some(gpui_kit::WindowBounds::Windowed(
                            gpui_kit::Bounds::new(
                                point(px(100.), px(100.)),
                                size(
                                    px(if matches!(kind, Kind::Comparison) {
                                        300. * panes as f32
                                    } else {
                                        600.
                                    }),
                                    px(400.),
                                ),
                            ),
                        )),
                        ..Default::default()
                    },
                    |window, cx| {
                        let editor = cx.new(|cx| EditorState::new(window, cx).default_value(text));
                        let tree = cx.new(|cx| gpui_base::TreeState::new(cx));
                        if matches!(kind, Kind::Tree) {
                            tree.update(cx, |tree, cx| {
                                tree.set_items(
                                    (0..10_000)
                                        .map(|i| {
                                            gpui_base::TreeItem::new(
                                                format!("item-{i}"),
                                                format!("row {i}"),
                                            )
                                        })
                                        .collect::<Vec<_>>(),
                                    cx,
                                )
                            });
                        }
                        let group = if matches!(kind, Kind::Comparison) {
                            let sources = (0..panes)
                                .map(|index| {
                                    if index == 0 {
                                        editor.clone()
                                    } else {
                                        let value = if different {
                                            comparison_peer(
                                                &source_text,
                                                index,
                                                start_row,
                                                collapsed,
                                            )
                                        } else {
                                            source_text.clone()
                                        };
                                        cx.new(|cx| {
                                            let mut source =
                                                EditorState::new(window, cx).default_value(value);
                                            source.set_readonly(readonly_peers, cx);
                                            source
                                        })
                                    }
                                })
                                .collect::<Vec<_>>();
                            if marked {
                                for source in &sources {
                                    let mut start = 0;
                                    let value = source.read(cx).value();
                                    let marks = value
                                        .split_inclusive('\n')
                                        .enumerate()
                                        .filter_map(|(row, line)| {
                                            let range = start..start + line.len();
                                            start = range.end;
                                            (row % 7 == 0).then(|| {
                                                gpui_base::input::RangeDecoration::new(range)
                                                    .with_color(gpui_kit::rgb(0x99bb99).into())
                                            })
                                        })
                                        .collect();
                                    paint_leases.borrow_mut().push(
                                        crate::row_backgrounds::RowBackgroundCollection::new(
                                            source, marks, cx,
                                        ),
                                    );
                                }
                            }
                            let mut group = crate::aligned_editor::AlignmentGroup::new(cx);
                            if decorated {
                                for source in &sources {
                                    let value = source.read(cx).value();
                                    let marks = value
                                        .match_indices("native")
                                        .map(|(start, word)| {
                                            gpui_base::input::TextDecoration::new(
                                                start..start + word.len(),
                                                gpui_kit::HighlightStyle {
                                                    font_weight: Some(gpui_kit::FontWeight::BOLD),
                                                    color: Some(gpui_kit::rgb(0xaaccff).into()),
                                                    ..Default::default()
                                                },
                                            )
                                        })
                                        .collect();
                                    // Base collections remain owned by the original source state.
                                    source.update(cx, |source, cx| {
                                        source.create_decorations_collection(marks, cx);
                                    });
                                }
                            }
                            group
                                .configure(
                                    sources
                                        .into_iter()
                                        .enumerate()
                                        .map(|(i, editor)| crate::aligned_editor::AlignmentPane {
                                            editor,
                                            revision: 1,
                                            label: format!("source {i}"),
                                            input_factory: None,
                                            native_authority_factory: None,
                                        })
                                        .collect(),
                                    &crate::aligned_pointer::AlignedPointerRouter::default(),
                                    cx,
                                )
                                .unwrap();
                            group.set_inner_gutters(true, cx);
                            if collapsed {
                                let svg: std::sync::Arc<[u8]> = std::sync::Arc::from(
                                    include_bytes!("../../../assets/icons/explorer-expand-all.svg")
                                        .as_slice(),
                                );
                                group.set_folding(
                                    crate::aligned_editor::UnchangedFolds::new(
                                        3,
                                        [svg.clone(), svg],
                                    ),
                                    cx,
                                );
                                group.fold_unchanged(true, cx);
                            }
                            Some(group)
                        } else {
                            None
                        };
                        let host = cx.new(|_| Harness {
                            outer: ScrollHandle::new(),
                            inner: ScrollHandle::new(),
                            editor,
                            tree,
                            group,
                            kind,
                            show: true,
                            painted_rows: Rc::new(Cell::new(0)),
                        });
                        capture.replace(Some(host.clone()));
                        let cached = cx.new(|_| CachedHarness(host.clone()));
                        let measured = cx.new(|_| super::frame_probe::Measured {
                            content: cached,
                            host,
                            samples: frame_output,
                        });
                        window.activate_window();
                        cx.new(|cx| gpui_base::Root::new(measured, window, cx))
                    },
                )
                .expect("the isolated native acceptance window opens");
            // Input dispatch may read Root plugins. An erased handle updates the window without
            // leasing the Root entity, exactly as the native platform's input dispatch does.
            let window: gpui_kit::AnyWindowHandle = window.into();
            let host = slot.borrow_mut().take().unwrap();
            cx.spawn(async move |cx| {
                let _background_leases = background_leases;
                cx.background_executor()
                    .timer(Duration::from_millis(1000))
                    .await;
                let _ = window.update(cx, |_, window, cx| {
                    window.activate_window();
                    if focused {
                        let editor = host.read(cx).editor.clone();
                        editor.update(cx, |editor, cx| editor.focus(window, cx));
                    }
                    // Settle native hover/folding-icon notifications before measuring steady
                    // wheel work. Their conservative complete geometry admission remains enabled.
                    window.dispatch_event(
                        gpui_kit::PlatformInput::MouseMove(gpui_kit::MouseMoveEvent {
                            position,
                            pressed_button: None,
                            modifiers: Default::default(),
                        }),
                        cx,
                    );
                    if start_row != 0 {
                        if let Some(group) = host.read(cx).group.clone() {
                            assert!(
                                group.reveal_offset(0, start_byte, cx),
                                "native comparison location refused: {:?}",
                                group.status(cx)
                            );
                        }
                    }
                });
                cx.background_executor()
                    .timer(Duration::from_millis(500))
                    .await;
                let _ = window.update(cx, |_, window, cx| {
                    eprintln!(
                        "native preparation: subtree_cpu_ms_max={:.3}",
                        moving_frames
                            .borrow()
                            .iter()
                            .map(|sample| sample.cpu.as_secs_f64() * 1000.)
                            .max_by(f64::total_cmp)
                            .unwrap_or(0.)
                    );
                    moving_frames.borrow_mut().clear();
                    window.dispatch_event(
                        gpui_kit::PlatformInput::Scroll(ScrollEvent {
                            // Wheel and trackpad fixtures retain pointer-device semantics.
                            source: gpui_kit::ScrollSource::Pointer,
                            position,
                            delta: ScrollDelta::Lines(point(0., wheel_lines)),
                            modifiers: Default::default(),
                            touch_phase: gpui_kit::TouchPhase::Moved,
                        }),
                        cx,
                    );
                });
                for index in 0..110 {
                    cx.background_executor()
                        .timer(Duration::from_millis(20))
                        .await;
                    let _ = window.update(cx, |_, window, cx| {
                        // Sustained physical input spans enough vsyncs for a cadence measurement.
                        if index > 0 && index < 75 && index % 4 == 0 {
                            window.dispatch_event(
                                gpui_kit::PlatformInput::Scroll(ScrollEvent {
                                    // Wheel and trackpad fixtures retain pointer-device semantics.
                                    source: gpui_kit::ScrollSource::Pointer,
                                    position,
                                    delta: ScrollDelta::Lines(point(0., wheel_lines)),
                                    modifiers: Default::default(),
                                    touch_phase: gpui_kit::TouchPhase::Moved,
                                }),
                                cx,
                            );
                        }
                        let host = host.read(cx);
                        let editor = host.editor.read(cx);
                        let offset = native_offset(host, cx);
                        output.borrow_mut().push((
                            f32::from(-offset.x - offset.y),
                            editor.visible_row_range().map(|range| range.start),
                            editor.value().as_str() == source_text,
                        ));
                    });
                }
                // Release the real platform input delegate and blink task before App's leak
                // audit drops its entity map. Only this isolated acceptance window is closed.
                let _ = window.update(cx, |_, window, cx| {
                    window.blur(cx);
                    window.refresh();
                });
                cx.background_executor()
                    .timer(Duration::from_millis(50))
                    .await;
                let _ = window.update(cx, |_, window, _| window.remove_window());
                drop(host);
                cx.background_executor()
                    .timer(Duration::from_millis(50))
                    .await;
                let _ = cx.update(|cx| cx.quit());
            })
            .detach();
        });
    let samples = samples.borrow();
    eprintln!(
        "real Windows wheel samples (logical distance, first visible row, unchanged text): first={:?}, last={:?}",
        samples.first(),
        samples.last()
    );
    assert_eq!(samples.len(), 110);
    let last = samples.last().unwrap().0;
    assert!(
        last > 0.
            && samples
                .iter()
                .any(|sample| sample.0 > 0. && sample.0 < last),
        "the real native window must paint an intermediate position"
    );
    assert!(samples.windows(2).all(|pair| pair[1].0 + 0.5 >= pair[0].0));
    assert!(
        samples[85].0 > samples[80].0 + 0.5,
        "native scrolling must retain a decelerating tail after the last physical wheel"
    );
    assert_eq!(samples[108].0, last, "the native inertial tail must settle");
    assert!(
        samples.iter().all(|sample| sample.2),
        "native scrolling must preserve the original source text"
    );
    if matches!(kind, Kind::Editor) {
        assert!(
            samples.last().unwrap().1.is_some_and(|row| row > 0),
            "native layout must expose later source rows"
        );
    }
    super::frame_probe::verify(&frames.borrow());
}

/// Add two real alignment gaps and a changed line near `anchor` for peer `index`, retaining a
/// small diff window within the shared admission budget even in three-column native acceptance.
fn comparison_peer(text: &str, index: usize, anchor: usize, collapsed: bool) -> String {
    text.split_inclusive('\n')
        .enumerate()
        .map(|(row, line)| {
            if row == anchor + index * 2 + 1 || row == anchor + index * 2 + 5 {
                format!("inserted peer {index} 中文😀\n{line}")
            } else if row == anchor + 1
                || (collapsed && row > anchor && row < anchor + 400 && row % 40 == 0)
            {
                line.replace("alpha", &format!("variant{index}"))
            } else {
                line.to_owned()
            }
        })
        .collect()
}
