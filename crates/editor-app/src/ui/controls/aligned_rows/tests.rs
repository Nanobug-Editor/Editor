//! Standalone, observable alignment/geometry regressions; no GPUI runtime or document copy is used.
use super::*;

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
fn three_panes_share_each_insertion_bucket_by_ordinal_and_maximum_length() {
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
fn lcs_budget_is_shared_across_peers_and_checked_after_common_prefix_suffix() {
    let pivot = numbered("p", 2001);
    let other = numbered("q", 2001);
    assert_eq!(
        AlignedRows::new(&[&pivot, &other]),
        Err(AlignError::ComparisonBudget)
    );
    let pivot = numbered("p", 1500);
    let left = numbered("q", 1500);
    let right = numbered("r", 1500);
    assert_eq!(
        AlignedRows::new(&[&pivot, &left, &right]),
        Err(AlignError::ComparisonBudget)
    );
    let pivot = numbered("p", 2001);
    let changed = pivot.replace("p1000\n", "different\n");
    let trimmed = AlignedRows::new(&[&pivot, &changed, &pivot]).unwrap();
    assert_eq!(trimmed.slots[0].len(), 2001);
    assert_eq!(trimmed.slots[0], trimmed.slots[1]);
    assert_eq!(trimmed.slots[0], trimmed.slots[2]);
    // Exactly four million middle comparisons are permitted, rather than counting the DP border.
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

/// Two separately planned groups must charge the same caller ledger rather than each spending 4M.
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
    // Matching common edges consume no matrix cells, even when another group used the budget.
    assert!(AlignedRows::new_with_budget(&["same", "same"], &mut budget).is_ok());
    assert_eq!(budget, 1);
}
