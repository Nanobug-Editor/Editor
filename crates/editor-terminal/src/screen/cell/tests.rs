// SPDX-License-Identifier: Apache-2.0
//! Behavior regressions for styled cells preserve persistent grid data and fullwidth/combining metadata..
//!
//! Adapted from Alacritty 0.26.0; see this crate's NOTICE and LICENSE-ALACRITTY.

use super::*;

use std::mem;

use crate::coordinates::Column;
use crate::grid::Row;

#[test]
fn cell_size_is_below_cap() {
    // Expected cell size on 64-bit architectures.
    const EXPECTED_CELL_SIZE: usize = 24;

    // Ensure that cell size isn't growing by accident.
    assert!(mem::size_of::<Cell>() <= EXPECTED_CELL_SIZE);
}

#[test]
fn line_length_works() {
    let mut row = Row::<Cell>::new(10);
    row[Column(5)].c = 'a';

    assert_eq!(row.line_length(), Column(6));
}

#[test]
fn line_length_works_with_wrapline() {
    let mut row = Row::<Cell>::new(10);
    row[Column(9)].flags.insert(super::Flags::WRAPLINE);

    assert_eq!(row.line_length(), Column(10));
}
