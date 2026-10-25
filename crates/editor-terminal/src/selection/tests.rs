// SPDX-License-Identifier: Apache-2.0
//! Behavior regressions for selection anchors, expansion and rotation in the owned screen..
//!
//! Adapted from Alacritty 0.26.0; see this crate's NOTICE and LICENSE-ALACRITTY.

use super::*;

use crate::coordinates::{Column, Point, Side};
use crate::screen::test_support::ScreenSize;
use crate::screen::{Screen, ScreenConfig};

fn term(height: usize, width: usize) -> Screen<()> {
    let size = ScreenSize::new(width, height);
    Screen::new(ScreenConfig::default(), &size, ())
}

/// Test case of single cell selection.
///
/// 1. [  ]
/// 2. [B ]
/// 3. [BE]
#[test]
fn single_cell_left_to_right() {
    let location = Point::new(Line(0), Column(0));
    let mut selection = Selection::new(SelectionType::Simple, location, Side::Left);
    selection.update(location, Side::Right);

    assert_eq!(
        selection.to_range(&term(1, 2)).unwrap(),
        SelectionRange {
            start: location,
            end: location,
            is_block: false
        }
    );
}

/// Test case of single cell selection.
///
/// 1. [  ]
/// 2. [ B]
/// 3. [EB]
#[test]
fn single_cell_right_to_left() {
    let location = Point::new(Line(0), Column(0));
    let mut selection = Selection::new(SelectionType::Simple, location, Side::Right);
    selection.update(location, Side::Left);

    assert_eq!(
        selection.to_range(&term(1, 2)).unwrap(),
        SelectionRange {
            start: location,
            end: location,
            is_block: false
        }
    );
}

/// Test adjacent cell selection from left to right.
///
/// 1. [  ][  ]
/// 2. [ B][  ]
/// 3. [ B][E ]
#[test]
fn between_adjacent_cells_left_to_right() {
    let mut selection = Selection::new(
        SelectionType::Simple,
        Point::new(Line(0), Column(0)),
        Side::Right,
    );
    selection.update(Point::new(Line(0), Column(1)), Side::Left);

    assert_eq!(selection.to_range(&term(1, 2)), None);
}

/// Test adjacent cell selection from right to left.
///
/// 1. [  ][  ]
/// 2. [  ][B ]
/// 3. [ E][B ]
#[test]
fn between_adjacent_cells_right_to_left() {
    let mut selection = Selection::new(
        SelectionType::Simple,
        Point::new(Line(0), Column(1)),
        Side::Left,
    );
    selection.update(Point::new(Line(0), Column(0)), Side::Right);

    assert_eq!(selection.to_range(&term(1, 2)), None);
}

#[rustfmt::skip]
/// Test selection across adjacent lines.
///
/// 1.  [  ][  ][  ][  ][  ]
///     [  ][  ][  ][  ][  ]
/// 2.  [  ][ B][  ][  ][  ]
///     [  ][  ][  ][  ][  ]
/// 3.  [  ][ B][XX][XX][XX]
///     [XX][XE][  ][  ][  ]
#[test]
fn across_adjacent_lines_upward_final_cell_exclusive() {
    let mut selection =
        Selection::new(SelectionType::Simple, Point::new(Line(0), Column(1)), Side::Right);
    selection.update(Point::new(Line(1), Column(1)), Side::Right);

    assert_eq!(selection.to_range(&term(2, 5)).unwrap(), SelectionRange {
        start: Point::new(Line(0), Column(2)),
        end: Point::new(Line(1), Column(1)),
        is_block: false,
    });
}

#[rustfmt::skip]
/// Test selection across adjacent lines.
///
/// 1.  [  ][  ][  ][  ][  ]
///     [  ][  ][  ][  ][  ]
/// 2.  [  ][  ][  ][  ][  ]
///     [  ][ B][  ][  ][  ]
/// 3.  [  ][ E][XX][XX][XX]
///     [XX][XB][  ][  ][  ]
/// 4.  [ E][XX][XX][XX][XX]
///     [XX][XB][  ][  ][  ]
#[test]
fn selection_bigger_then_smaller() {
    let mut selection =
        Selection::new(SelectionType::Simple, Point::new(Line(1), Column(1)), Side::Right);
    selection.update(Point::new(Line(0), Column(1)), Side::Right);
    selection.update(Point::new(Line(0), Column(0)), Side::Right);

    assert_eq!(selection.to_range(&term(2, 5)).unwrap(), SelectionRange {
        start: Point::new(Line(0), Column(1)),
        end: Point::new(Line(1), Column(1)),
        is_block: false,
    });
}

#[test]
fn line_selection() {
    let size = (10, 5);
    let mut selection = Selection::new(
        SelectionType::Lines,
        Point::new(Line(9), Column(1)),
        Side::Left,
    );
    selection.update(Point::new(Line(4), Column(1)), Side::Right);
    selection = selection
        .rotate(&size, &(Line(0)..Line(size.0 as i32)), 4)
        .unwrap();

    assert_eq!(
        selection.to_range(&term(size.0, size.1)).unwrap(),
        SelectionRange {
            start: Point::new(Line(0), Column(0)),
            end: Point::new(Line(5), Column(4)),
            is_block: false,
        }
    );
}

#[test]
fn semantic_selection() {
    let size = (10, 5);
    let mut selection = Selection::new(
        SelectionType::Semantic,
        Point::new(Line(9), Column(3)),
        Side::Left,
    );
    selection.update(Point::new(Line(4), Column(1)), Side::Right);
    selection = selection
        .rotate(&size, &(Line(0)..Line(size.0 as i32)), 4)
        .unwrap();

    assert_eq!(
        selection.to_range(&term(size.0, size.1)).unwrap(),
        SelectionRange {
            start: Point::new(Line(0), Column(1)),
            end: Point::new(Line(5), Column(3)),
            is_block: false,
        }
    );
}

#[test]
fn simple_selection() {
    let size = (10, 5);
    let mut selection = Selection::new(
        SelectionType::Simple,
        Point::new(Line(9), Column(3)),
        Side::Right,
    );
    selection.update(Point::new(Line(4), Column(1)), Side::Right);
    selection = selection
        .rotate(&size, &(Line(0)..Line(size.0 as i32)), 4)
        .unwrap();

    assert_eq!(
        selection.to_range(&term(size.0, size.1)).unwrap(),
        SelectionRange {
            start: Point::new(Line(0), Column(2)),
            end: Point::new(Line(5), Column(3)),
            is_block: false,
        }
    );
}

#[test]
fn block_selection() {
    let size = (10, 5);
    let mut selection = Selection::new(
        SelectionType::Block,
        Point::new(Line(9), Column(3)),
        Side::Right,
    );
    selection.update(Point::new(Line(4), Column(1)), Side::Right);
    selection = selection
        .rotate(&size, &(Line(0)..Line(size.0 as i32)), 4)
        .unwrap();

    assert_eq!(
        selection.to_range(&term(size.0, size.1)).unwrap(),
        SelectionRange {
            start: Point::new(Line(0), Column(2)),
            end: Point::new(Line(5), Column(3)),
            is_block: true
        }
    );
}

#[test]
fn simple_is_empty() {
    let mut selection = Selection::new(
        SelectionType::Simple,
        Point::new(Line(1), Column(0)),
        Side::Right,
    );
    assert!(selection.is_empty());
    selection.update(Point::new(Line(1), Column(1)), Side::Left);
    assert!(selection.is_empty());
    selection.update(Point::new(Line(0), Column(0)), Side::Right);
    assert!(!selection.is_empty());
}

#[test]
fn block_is_empty() {
    let mut selection = Selection::new(
        SelectionType::Block,
        Point::new(Line(1), Column(0)),
        Side::Right,
    );
    assert!(selection.is_empty());
    selection.update(Point::new(Line(1), Column(1)), Side::Left);
    assert!(selection.is_empty());
    selection.update(Point::new(Line(1), Column(1)), Side::Right);
    assert!(!selection.is_empty());
    selection.update(Point::new(Line(0), Column(0)), Side::Right);
    assert!(selection.is_empty());
    selection.update(Point::new(Line(0), Column(1)), Side::Left);
    assert!(selection.is_empty());
    selection.update(Point::new(Line(0), Column(1)), Side::Right);
    assert!(!selection.is_empty());
}

#[test]
fn rotate_in_region_up() {
    let size = (10, 5);
    let mut selection = Selection::new(
        SelectionType::Simple,
        Point::new(Line(7), Column(3)),
        Side::Right,
    );
    selection.update(Point::new(Line(4), Column(1)), Side::Right);
    selection = selection
        .rotate(&size, &(Line(1)..Line(size.0 as i32 - 1)), 4)
        .unwrap();

    assert_eq!(
        selection.to_range(&term(size.0, size.1)).unwrap(),
        SelectionRange {
            start: Point::new(Line(1), Column(0)),
            end: Point::new(Line(3), Column(3)),
            is_block: false,
        }
    );
}

#[test]
fn rotate_in_region_down() {
    let size = (10, 5);
    let mut selection = Selection::new(
        SelectionType::Simple,
        Point::new(Line(4), Column(3)),
        Side::Right,
    );
    selection.update(Point::new(Line(1), Column(1)), Side::Left);
    selection = selection
        .rotate(&size, &(Line(1)..Line(size.0 as i32 - 1)), -5)
        .unwrap();

    assert_eq!(
        selection.to_range(&term(size.0, size.1)).unwrap(),
        SelectionRange {
            start: Point::new(Line(6), Column(1)),
            end: Point::new(Line(8), size.last_column()),
            is_block: false,
        }
    );
}

#[test]
fn rotate_in_region_up_block() {
    let size = (10, 5);
    let mut selection = Selection::new(
        SelectionType::Block,
        Point::new(Line(7), Column(3)),
        Side::Right,
    );
    selection.update(Point::new(Line(4), Column(1)), Side::Right);
    selection = selection
        .rotate(&size, &(Line(1)..Line(size.0 as i32 - 1)), 4)
        .unwrap();

    assert_eq!(
        selection.to_range(&term(size.0, size.1)).unwrap(),
        SelectionRange {
            start: Point::new(Line(1), Column(2)),
            end: Point::new(Line(3), Column(3)),
            is_block: true,
        }
    );
}

#[test]
fn range_intersection() {
    let mut selection = Selection::new(
        SelectionType::Lines,
        Point::new(Line(3), Column(1)),
        Side::Left,
    );
    selection.update(Point::new(Line(6), Column(1)), Side::Right);

    assert!(selection.intersects_range(..));
    assert!(selection.intersects_range(Line(2)..));
    assert!(selection.intersects_range(Line(2)..=Line(4)));
    assert!(selection.intersects_range(Line(2)..=Line(7)));
    assert!(selection.intersects_range(Line(4)..=Line(5)));
    assert!(selection.intersects_range(Line(5)..Line(8)));

    assert!(!selection.intersects_range(..=Line(2)));
    assert!(!selection.intersects_range(Line(7)..=Line(8)));
}
