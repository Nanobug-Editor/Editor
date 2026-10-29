//! Standalone, observable alignment/geometry regressions; no GPUI runtime or document copy is used.
use super::*;

/// Cached byte hunks must project every native caret row once, including asymmetric EOF/empty sources.
#[test]
fn shared_byte_comparisons_preserve_all_native_rows() {
    let sources = ["", "a", "a\n", "a\r\n", "\n", "a\nb", "a\nb\n", "新😀\na\n"];
    for left in sources {
        for right in sources {
            for third in sources {
                let plan = AlignedRows::new(&[left, right, third]).unwrap();
                for (pane, slots) in plan.slots.iter().enumerate() {
                    assert_eq!(
                        slots.iter().copied().flatten().collect::<Vec<_>>(),
                        (0..plan.source_rows[pane].len()).collect::<Vec<_>>(),
                        "missing/duplicate row for {left:?}, {right:?}, {third:?}, pane {pane}"
                    );
                }
                assert!(
                    plan.slots
                        .iter()
                        .all(|slots| slots.len() == plan.slots[0].len())
                );
            }
        }
    }
}

/// A real-sized lockfile must retain every native row and align an insertion beyond the old cap.
#[test]
fn large_comparison_aligns_ten_thousand_rows_without_matrix_budget() {
    let before = numbered("package-", 10_500);
    let after = before.replace("package-8000\n", "package-8000\n新增😀\n");
    let plan = AlignedRows::new(&[&before, &after]).unwrap();
    for (pane, slots) in plan.slots.iter().enumerate() {
        assert_eq!(
            slots.iter().copied().flatten().collect::<Vec<_>>(),
            (0..plan.source_rows[pane].len()).collect::<Vec<_>>()
        );
    }
    assert_eq!(plan.slots[0][8001], None);
    assert_eq!(plan.slots[1][8001], Some(8001));
    assert_eq!(plan.slots[0][8002], Some(8001));
}

fn uniform(rows: usize, height: f32) -> Vec<Range<f32>> {
    (0..rows)
        .map(|row| row as f32 * height..(row + 1) as f32 * height)
        .collect()
}

fn numbered(prefix: &str, rows: usize) -> String {
    (0..rows)
        .map(|row| format!("{prefix}{row}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn assert_close(actual: f32, expected: f32) {
    assert!((actual - expected).abs() < 0.001, "{actual} != {expected}");
}

/// Two/three sources share one disclosure row, while hidden bytes have no painted coordinates.
#[test]
fn unchanged_folds_keep_context_and_every_changed_slot_in_three_panes() {
    let original = numbered("中文😀 ", 50);
    let left = original.replace("中文😀 2\n", "left change\n");
    let result = original.replace("中文😀 42\n", "result change\nextra\n");
    let third = original.replace("中文😀 25\n", "third change\n");
    let plan = AlignedRows::new(&[&left, &result, &third]).unwrap();
    let regions = plan.unchanged_regions(3);
    assert!(!regions.is_empty());
    let measurements = plan
        .source_rows
        .iter()
        .map(|rows| uniform(rows.len(), 20.))
        .collect::<Vec<_>>();
    let expanded = plan.layout(&measurements).unwrap();
    let folded = plan
        .layout_with_folds(&measurements, &regions, 20.)
        .unwrap();
    assert!(folded.display_height < expanded.display_height);
    assert_eq!(folded.folds.len(), regions.len());
    for (range, display) in &folded.folds {
        assert_eq!(display.end - display.start, 20.);
        for pane in 0..3 {
            let row = plan.slots[pane][range.start].unwrap();
            assert!(
                folded.panes[pane]
                    .source_to_display(row as f32 * 20. + 1.)
                    .is_none()
            );
            let native = folded.panes[pane]
                .display_to_source(display.start + 10.)
                .unwrap();
            assert!(
                !range.clone().any(|slot| {
                    let row = plan.slots[pane][slot].unwrap();
                    native >= row as f32 * 20. && native < (row + 1) as f32 * 20.
                }),
                "disclosures cannot manufacture a native hidden-text pointer target"
            );
        }
    }
    for slot in (0..plan.slots[0].len()).filter(|slot| !plan.unchanged[*slot]) {
        for pane in 0..3 {
            if let Some(row) = plan.slots[pane][slot] {
                assert!(
                    folded.panes[pane]
                        .source_to_display(row as f32 * 20. + 1.)
                        .is_some()
                );
            }
        }
    }
}

/// Unchanged means exact bytes; newline changes and short contexts remain fully visible.
#[test]
fn unchanged_folds_handle_identical_empty_short_and_line_ending_sources() {
    for sources in [
        ["", ""],
        ["a\nb\n", "a\nb\n"],
        ["a\r\nb\r\nc\r\nd\r\ne\r\n", "a\nb\nc\nd\ne\n"],
    ] {
        let plan = AlignedRows::new(&sources).unwrap();
        assert!(plan.unchanged_regions(1).is_empty());
    }
    let text = numbered("same", 25);
    let plan = AlignedRows::new(&[&text, &text]).unwrap();
    assert_eq!(plan.unchanged_regions(3), vec![3..22]);
    let measures = vec![uniform(25, 20.); 2];
    let folded = plan.layout_with_folds(&measures, &[3..22], 20.).unwrap();
    assert_eq!(folded.display_height, 140.);
    assert_eq!(folded.panes[0].source_height, 500.);
    assert!(
        plan.layout_with_folds(&measures, &[3..22, 5..8], 20.)
            .is_err()
    );
    let changed = text.replace("same10\n", "different\n");
    let changed = AlignedRows::new(&[&text, &changed]).unwrap();
    assert!(changed.layout_with_folds(&measures, &[3..22], 20.).is_err());
}

#[test]
fn empty_sources_have_a_real_eof_row_and_no_artificial_text() {
    let plan = AlignedRows::new(&["", ""]).unwrap();
    assert_eq!(plan.source_rows, vec![vec![0..0], vec![0..0]]);
    assert_eq!(plan.slots, vec![vec![Some(0)], vec![Some(0)]]);
    let measured = plan.layout(&[uniform(1, 20.), uniform(1, 20.)]).unwrap();
    assert_eq!(measured.display_height, 20.);
    assert!(measured.panes.iter().all(|pane| pane.gaps.is_empty()));
    assert_eq!(measured.panes[0].slices[0].source, 0_f32..20.);
}

#[test]
fn utf8_crlf_and_trailing_lf_keep_original_offsets_and_eof_caret_rows() {
    let sources = ["中😀\r\n尾", "中😀\n插\n尾"];
    let plan = AlignedRows::new(&sources).unwrap();
    assert_eq!(plan.source_rows[0], vec![0..9, 9..12]);
    assert_eq!(plan.source_rows[1], vec![0..8, 8..12, 12..15]);
    assert_eq!(plan.slots[0], vec![Some(0), None, Some(1)]);
    assert_eq!(plan.slots[1], vec![Some(0), Some(1), Some(2)]);
    for (source, ranges) in sources.iter().zip(&plan.source_rows) {
        for range in ranges {
            assert!(source.is_char_boundary(range.start));
            assert!(source.is_char_boundary(range.end));
        }
    }
    let trailing = AlignedRows::new(&["中😀\r\n", "中😀\n"]).unwrap();
    assert_eq!(
        trailing.source_rows,
        vec![vec![0..9, 9..9], vec![0..8, 8..8]]
    );
    assert_eq!(trailing.slots[0], vec![Some(0), Some(1)]);
    assert_eq!(trailing.slots[0], trailing.slots[1]);
    assert_eq!(
        AlignedRows::new(&["\n", ""]).unwrap().source_rows[0],
        vec![0..1, 1..1]
    );
}

#[test]
fn beginning_and_end_insertions_and_deletions_preserve_native_row_order() {
    let inserted = AlignedRows::new(&["a\nb", "head\na\nb\ntail"]).unwrap();
    assert_eq!(inserted.slots[0], vec![None, Some(0), Some(1), None]);
    assert_eq!(inserted.slots[1], vec![Some(0), Some(1), Some(2), Some(3)]);
    let deleted = AlignedRows::new(&["head\na\nb\ntail", "a\nb"]).unwrap();
    assert_eq!(deleted.slots[0], inserted.slots[1]);
    assert_eq!(deleted.slots[1], inserted.slots[0]);
}

#[test]
fn unmatched_runs_pair_in_order_before_padding_the_extra_native_rows() {
    let plan = AlignedRows::new(&["old1\nold2\nanchor", "new1\nanchor"]).unwrap();
    assert_eq!(plan.slots[0], vec![Some(0), Some(1), Some(2)]);
    assert_eq!(plan.slots[1], vec![Some(0), None, Some(1)]);
}

#[test]
fn three_panes_align_inserted_rows_and_preserve_pivot_anchors() {
    let plan = AlignedRows::new(&[
        "a\nb\nc",
        "head\na\nx\nb\nc\ntail1\ntail2",
        "a\nx\ny\nb\nz\nc",
    ])
    .unwrap();
    assert_eq!(
        plan.slots,
        vec![
            vec![
                None,
                Some(0),
                None,
                None,
                Some(1),
                None,
                Some(2),
                None,
                None
            ],
            vec![
                Some(0),
                Some(1),
                Some(2),
                None,
                Some(3),
                None,
                Some(4),
                Some(5),
                Some(6)
            ],
            vec![
                None,
                Some(0),
                Some(1),
                Some(2),
                Some(3),
                Some(4),
                Some(5),
                None,
                None
            ],
        ]
    );
}

/// Two non-pivot sources can share a new row at different insertion ordinals. Their real shared
/// text must align while the surrounding one-sided additions reserve only blank display space.
#[test]
fn common_peer_insertions_share_display_rows_without_pivot_anchors() {
    let sources = ["a\nz\n", "a\nX\n共同\nz\n", "a\n共同\nY\nz\n"];
    let plan = AlignedRows::new(&sources).unwrap();
    let measured = plan
        .layout(&[uniform(3, 20.), uniform(5, 20.), uniform(5, 20.)])
        .unwrap();
    // The visible order is a, X, 共同, Y, z, EOF. These known source coordinates therefore
    // identify the same actual text row without depending on a particular matching algorithm.
    assert_eq!(measured.panes[1].source_to_display(40.), Some(40.));
    assert_eq!(measured.panes[2].source_to_display(20.), Some(40.));
    assert_eq!(measured.panes[0].source_to_display(20.), Some(80.));
    assert_eq!(measured.panes[1].source_to_display(60.), Some(80.));
    assert_eq!(measured.panes[2].source_to_display(60.), Some(80.));
    for (pane, slots) in plan.slots.iter().enumerate() {
        assert_eq!(
            slots.iter().copied().flatten().collect::<Vec<_>>(),
            (0..plan.source_rows[pane].len()).collect::<Vec<_>>(),
            "alignment cannot insert text rows, duplicate them or change source order",
        );
    }
}

/// Every pivot comparison and three-pane insertion bucket shares one input-row allowance.
#[test]
fn peer_insertions_charge_the_shared_budget_only_for_three_panes() {
    let sources = ["a\nz\n", "a\nX\n共同\nz\n", "a\n共同\nY\nz\n"];
    let mut budget = 20;
    assert!(AlignedRows::new_with_budget(&sources, &mut budget).is_ok());
    assert_eq!(budget, 0);
    let mut budget = 19;
    assert_eq!(
        AlignedRows::new_with_budget(&sources, &mut budget),
        Err(AlignError::ComparisonBudget),
    );
    assert_eq!(budget, 3);
    let mut budget = 8;
    assert!(AlignedRows::new_with_budget(&sources[..2], &mut budget).is_ok());
    assert_eq!(budget, 0);
    let mut budget = 20;
    assert!(
        AlignedRows::new_with_budget(&[sources[0], sources[1], sources[1]], &mut budget).is_ok()
    );
    assert_eq!(budget, 0);
    // Two pivot comparisons spend twenty rows first; the peer bucket needs four more.
    let sources = [
        "a\nold\nz\n",
        "a\nchanged\nX\n共同\nz\n",
        "a\nchanged\n共同\nY\nz\n",
    ];
    let mut budget = 23;
    assert_eq!(
        AlignedRows::new_with_budget(&sources, &mut budget),
        Err(AlignError::ComparisonBudget),
    );
    assert_eq!(budget, 3);
    let mut budget = 24;
    assert!(AlignedRows::new_with_budget(&sources, &mut budget).is_ok());
    assert_eq!(budget, 0);
}

/// A common peer-only line can wrap differently in the two original editors. Its shared slot
/// takes the larger native height so that the following pivot and EOF rows remain aligned.
#[test]
fn common_peer_insertions_keep_later_anchors_aligned_after_soft_wraps() {
    let plan = AlignedRows::new(&["a\nz\n", "a\nX\n共同\nz\n", "a\n共同\nY\nz\n"]).unwrap();
    let measured = plan
        .layout(&[
            uniform(3, 20.),
            vec![
                0_f32..20.,
                20_f32..40.,
                40_f32..80.,
                80_f32..100.,
                100_f32..120.,
            ],
            uniform(5, 20.),
        ])
        .unwrap();
    assert_eq!(measured.panes[1].source_to_display(40.), Some(40.));
    assert_eq!(measured.panes[2].source_to_display(20.), Some(40.));
    for (pane, z, eof) in [(0, 20., 40.), (1, 80., 100.), (2, 60., 80.)] {
        assert_eq!(measured.panes[pane].source_to_display(z), Some(100.));
        assert_eq!(measured.panes[pane].source_to_display(eof), Some(120.));
    }
}

fn wrapped_layout() -> AlignedLayout {
    AlignedRows::new(&["a\nb\nc", "a\nextra\nb\nc"])
        .unwrap()
        .layout(&[
            vec![0_f32..20., 20_f32..60., 60_f32..80.],
            vec![0_f32..40., 40_f32..60., 60_f32..80., 80_f32..100.],
        ])
        .unwrap()
}

#[test]
fn measured_soft_wraps_use_maximum_slot_height_and_coalesce_equal_source_gaps() {
    let measured = wrapped_layout();
    assert_eq!(measured.display_height, 120.);
    assert_eq!(measured.panes[0].source_height, 80.);
    assert_eq!(measured.panes[1].source_height, 100.);
    assert_eq!(
        measured.panes[0].gaps,
        vec![Gap {
            source_y: 20.,
            height: 40.
        }]
    );
    assert_eq!(
        measured.panes[1].gaps,
        vec![Gap {
            source_y: 80.,
            height: 20.
        }]
    );
    assert_eq!(
        measured.panes[0].slices,
        vec![
            SourceSlice {
                source: 0_f32..20.,
                display: 0_f32..20.
            },
            SourceSlice {
                source: 20_f32..80.,
                display: 60_f32..120.
            },
        ]
    );
    assert_eq!(
        measured.panes[1].slices,
        vec![
            SourceSlice {
                source: 0_f32..80.,
                display: 0_f32..80.
            },
            SourceSlice {
                source: 80_f32..100.,
                display: 100_f32..120.
            },
        ]
    );
}

#[test]
fn native_coordinates_round_trip_and_gap_clicks_choose_the_nearest_real_row() {
    let measured = wrapped_layout();
    let pane = &measured.panes[0];
    for source in [0., 10., 19., 20., 21., 50., 79., 80.] {
        let display = pane.source_to_display(source).unwrap();
        assert_close(pane.display_to_source(display).unwrap(), source);
    }
    assert_eq!(pane.source_to_display(20.), Some(60.));
    let upper = pane.display_to_source(30.).unwrap();
    assert!(
        upper < 20. && upper >= 19.5,
        "upper gap must hit the preceding native row"
    );
    let large_origin = -240_000_f32;
    assert!(large_origin + upper < large_origin + 20.);
    assert_eq!(pane.display_to_source(55.), Some(20.));
    assert!(
        pane.display_to_source(40.).unwrap() < 20.,
        "tie chooses the preceding row"
    );
    assert_eq!(pane.display_to_source(60.), Some(20.));
    assert_eq!(pane.display_to_source(-100.), Some(0.));
    assert_eq!(pane.display_to_source(1000.), Some(80.));
    assert_eq!(pane.source_to_display(-1.), Some(0.));
    assert_eq!(pane.source_to_display(1000.), Some(120.));
    assert_eq!(pane.source_to_display(f32::NAN), None);
    assert_eq!(pane.display_to_source(f32::INFINITY), None);
}

#[test]
fn leading_and_trailing_gaps_do_not_invent_native_positions() {
    for (source, other, gap_y) in [("a", "head\na", 0.), ("a", "a\ntail", 20.)] {
        let measured = AlignedRows::new(&[source, other])
            .unwrap()
            .layout(&[uniform(1, 20.), uniform(2, 20.)])
            .unwrap();
        let pane = &measured.panes[0];
        assert_eq!(
            pane.gaps,
            vec![Gap {
                source_y: gap_y,
                height: 20.
            }]
        );
        assert_eq!(pane.source_to_display(20.), Some(40.));
        if gap_y == 0. {
            assert_eq!(pane.source_to_display(0.), Some(20.));
            assert_eq!(pane.display_to_source(10.), Some(0.));
        } else {
            assert_eq!(pane.source_to_display(0.), Some(0.));
            assert!(pane.display_to_source(30.).unwrap() < 20.);
        }
    }
}

#[test]
fn zero_height_native_rows_are_finite_and_do_not_create_paint_slices() {
    let measured = AlignedRows::new(&["a\nb", "a"])
        .unwrap()
        .layout(&[vec![0_f32..0., 0_f32..0.], vec![0_f32..0.]])
        .unwrap();
    assert_eq!(measured.display_height, 0.);
    assert!(
        measured
            .panes
            .iter()
            .all(|pane| pane.slices.is_empty() && pane.gaps.is_empty())
    );
    assert_eq!(measured.panes[0].source_to_display(0.), Some(0.));
    assert_eq!(measured.panes[0].display_to_source(0.), Some(0.));
}

#[test]
fn fractional_native_ranges_keep_inverse_precision_across_three_wrapped_panes() {
    let measured = AlignedRows::new(&["a\nb", "a\nx\nb", "a\nb"])
        .unwrap()
        .layout(&[
            vec![0_f32..10.3, 10.3_f32..41.7],
            vec![0_f32..15.2, 15.2_f32..35.7, 35.7_f32..50.9],
            vec![0_f32..12.1, 12.1_f32..47.7],
        ])
        .unwrap();
    assert_close(measured.display_height, 71.3);
    for pane in &measured.panes {
        for index in 0..100 {
            let source = pane.source_height * index as f32 / 100.;
            let display = pane.source_to_display(source).unwrap();
            assert_close(pane.display_to_source(display).unwrap(), source);
        }
    }
}

#[test]
fn geometry_shape_discontinuity_nonfinite_negative_and_overflow_are_rejected() {
    let plan = AlignedRows::new(&["a\nb", "a\nb"]).unwrap();
    assert_eq!(plan.layout(&[]), Err(AlignError::MeasurementPaneCount));
    assert_eq!(
        plan.layout(&[uniform(1, 20.), uniform(2, 20.)]),
        Err(AlignError::MeasurementRowCount { pane: 0 })
    );
    for invalid in [
        vec![1_f32..20., 20_f32..40.],
        vec![0_f32..20., 21_f32..40.],
        vec![0_f32..20., 20_f32..19.],
        vec![0_f32..-1., -1_f32..20.],
        vec![0_f32..f32::NAN, 20_f32..40.],
        vec![0_f32..20., 20_f32..f32::INFINITY],
    ] {
        assert!(matches!(
            plan.layout(&[invalid, uniform(2, 20.)]),
            Err(AlignError::InvalidGeometry { pane: 0, .. })
        ));
    }
    assert_eq!(
        plan.layout(&[
            vec![0_f32..f32::MAX, f32::MAX..f32::MAX],
            vec![0_f32..0., 0_f32..f32::MAX],
        ]),
        Err(AlignError::GeometryOverflow)
    );
}

#[test]
fn byte_and_native_row_limits_include_the_final_empty_eof_row() {
    assert_eq!(AlignedRows::new(&[""]), Err(AlignError::PaneCount));
    assert_eq!(
        AlignedRows::new(&["", "", "", ""]),
        Err(AlignError::PaneCount)
    );
    let maximum = "a".repeat(MAX_SOURCE_BYTES);
    assert!(AlignedRows::new(&[&maximum, ""]).is_ok());
    assert_eq!(
        AlignedRows::new(&["", &(maximum + "a")]),
        Err(AlignError::SourceBytes { pane: 1 })
    );
    let rows = "\n".repeat(MAX_SOURCE_ROWS - 1);
    assert_eq!(
        AlignedRows::new(&[&rows, &rows]).unwrap().source_rows[0].len(),
        MAX_SOURCE_ROWS
    );
    assert_eq!(
        AlignedRows::new(&[&(rows + "\n"), ""]),
        Err(AlignError::SourceRows { pane: 0 })
    );
}

#[test]
fn histogram_admits_large_changes_and_charges_each_peer_linearly() {
    let pivot = numbered("p", 2001);
    let other = numbered("q", 2001);
    assert!(AlignedRows::new(&[&pivot, &other]).is_ok());
    let pivot = numbered("p", 1500);
    let left = numbered("q", 1500);
    let right = numbered("r", 1500);
    assert!(AlignedRows::new(&[&pivot, &left, &right]).is_ok());
    let mut budget = 5_000;
    assert_eq!(
        AlignedRows::new_with_budget(&[&pivot, &left, &right], &mut budget),
        Err(AlignError::ComparisonBudget)
    );
    assert_eq!(budget, 2_000);
    let pivot = numbered("p", 2001);
    let changed = pivot.replace("p1000\n", "different\n");
    let trimmed = AlignedRows::new(&[&pivot, &changed, &pivot]).unwrap();
    assert_eq!(trimmed.slots[0].len(), 2001);
    assert_eq!(trimmed.slots[0], trimmed.slots[1]);
    assert_eq!(trimmed.slots[0], trimmed.slots[2]);
    // Fully changed sources no longer require a quadratic matrix.
    assert!(AlignedRows::new(&[&numbered("p", 2000), &numbered("q", 2000)]).is_ok());
}

#[test]
fn repeated_short_lines_never_drop_duplicate_or_reorder_any_native_row() {
    let sources = [
        "", "a", "b", "a\na", "a\nb", "b\na", "b\nb", "a\nb\na", "a\n",
    ];
    for a in sources {
        for b in sources {
            for c in sources {
                let plan = AlignedRows::new(&[a, b, c]).unwrap();
                let count = plan.slots[0].len();
                for (pane, slots) in plan.slots.iter().enumerate() {
                    assert_eq!(slots.len(), count);
                    assert_eq!(
                        slots.iter().copied().flatten().collect::<Vec<_>>(),
                        (0..plan.source_rows[pane].len()).collect::<Vec<_>>()
                    );
                }
                assert!((0..count).all(|slot| plan.slots.iter().any(|pane| pane[slot].is_some())));
                let geometry = plan
                    .source_rows
                    .iter()
                    .map(|rows| uniform(rows.len(), 3.))
                    .collect::<Vec<_>>();
                let measured = plan.layout(&geometry).unwrap();
                assert_eq!(measured.display_height, count as f32 * 3.);
                for pane in &measured.panes {
                    for source in 0..pane.source_height as usize {
                        let display = pane.source_to_display(source as f32).unwrap();
                        assert_close(pane.display_to_source(display).unwrap(), source as f32);
                    }
                }
            }
        }
    }
}

/// Separately planned groups charge one input-row ledger, including scans of unchanged sources.
#[test]
fn independent_groups_charge_one_explicit_comparison_budget() {
    let mut budget = 5;
    assert!(AlignedRows::new_with_budget(&["a\nb", "c\nd"], &mut budget).is_ok());
    assert_eq!(budget, 1);
    assert_eq!(
        AlignedRows::new_with_budget(&["e\nf", "g\nh"], &mut budget),
        Err(AlignError::ComparisonBudget),
    );
    assert_eq!(budget, 1);
    assert_eq!(
        AlignedRows::new_with_budget(&["same", "same"], &mut budget),
        Err(AlignError::ComparisonBudget)
    );
    assert_eq!(budget, 1);
}
