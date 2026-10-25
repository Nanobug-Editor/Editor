// SPDX-License-Identifier: Apache-2.0
//! Behavior regressions for ring storage keeps scrollback ordering stable across viewport changes..
//!
//! Adapted from Alacritty 0.26.0; see this crate's NOTICE and LICENSE-ALACRITTY.

use crate::coordinates::{Column, Line};
use crate::grid::GridCell;
use crate::grid::row::Row;
use crate::grid::storage::{MAX_CACHE_SIZE, Storage};
use crate::screen::cell::Flags;

impl GridCell for char {
    fn is_empty(&self) -> bool {
        *self == ' ' || *self == '\t'
    }

    fn reset(&mut self, template: &Self) {
        *self = *template;
    }

    fn flags(&self) -> &Flags {
        unimplemented!();
    }

    fn flags_mut(&mut self) -> &mut Flags {
        unimplemented!();
    }
}

#[test]
fn with_capacity() {
    let storage = Storage::<char>::with_capacity(3, 1);

    assert_eq!(storage.inner.len(), 3);
    assert_eq!(storage.len, 3);
    assert_eq!(storage.zero, 0);
    assert_eq!(storage.visible_lines, 3);
}

#[test]
fn indexing() {
    let mut storage = Storage::<char>::with_capacity(3, 1);

    storage[Line(0)] = filled_row('0');
    storage[Line(1)] = filled_row('1');
    storage[Line(2)] = filled_row('2');

    storage.zero += 1;

    assert_eq!(storage[Line(0)], filled_row('2'));
    assert_eq!(storage[Line(1)], filled_row('0'));
    assert_eq!(storage[Line(2)], filled_row('1'));
}

#[test]
#[should_panic]
#[cfg(debug_assertions)]
fn indexing_above_inner_len() {
    let storage = Storage::<char>::with_capacity(1, 1);
    let _ = &storage[Line(-1)];
}

#[test]
fn rotate() {
    let mut storage = Storage::<char>::with_capacity(3, 1);
    storage.rotate(2);
    assert_eq!(storage.zero, 2);
    storage.shrink_lines(2);
    assert_eq!(storage.len, 1);
    assert_eq!(storage.inner.len(), 3);
    assert_eq!(storage.zero, 2);
}

/// Grow the buffer one line at the end of the buffer.
///
/// Before:
///   0: 0 <- Zero
///   1: 1
///   2: -
/// After:
///   0: 0 <- Zero
///   1: 1
///   2: -
///   3: \0
///   ...
///   MAX_CACHE_SIZE: \0
#[test]
fn grow_after_zero() {
    // Setup storage area.
    let mut storage: Storage<char> = Storage {
        inner: vec![filled_row('0'), filled_row('1'), filled_row('-')],
        zero: 0,
        visible_lines: 3,
        len: 3,
    };

    // Grow buffer.
    storage.grow_visible_lines(4);

    // Make sure the result is correct.
    let mut expected = Storage {
        inner: vec![filled_row('0'), filled_row('1'), filled_row('-')],
        zero: 0,
        visible_lines: 4,
        len: 4,
    };
    expected
        .inner
        .append(&mut vec![filled_row('\0'); MAX_CACHE_SIZE]);

    assert_eq!(storage.visible_lines, expected.visible_lines);
    assert_eq!(storage.inner, expected.inner);
    assert_eq!(storage.zero, expected.zero);
    assert_eq!(storage.len, expected.len);
}

/// Grow the buffer one line at the start of the buffer.
///
/// Before:
///   0: -
///   1: 0 <- Zero
///   2: 1
/// After:
///   0: 0 <- Zero
///   1: 1
///   2: -
///   3: \0
///   ...
///   MAX_CACHE_SIZE: \0
#[test]
fn grow_before_zero() {
    // Setup storage area.
    let mut storage: Storage<char> = Storage {
        inner: vec![filled_row('-'), filled_row('0'), filled_row('1')],
        zero: 1,
        visible_lines: 3,
        len: 3,
    };

    // Grow buffer.
    storage.grow_visible_lines(4);

    // Make sure the result is correct.
    let mut expected = Storage {
        inner: vec![filled_row('0'), filled_row('1'), filled_row('-')],
        zero: 0,
        visible_lines: 4,
        len: 4,
    };
    expected
        .inner
        .append(&mut vec![filled_row('\0'); MAX_CACHE_SIZE]);

    assert_eq!(storage.visible_lines, expected.visible_lines);
    assert_eq!(storage.inner, expected.inner);
    assert_eq!(storage.zero, expected.zero);
    assert_eq!(storage.len, expected.len);
}

/// Shrink the buffer one line at the start of the buffer.
///
/// Before:
///   0: 2
///   1: 0 <- Zero
///   2: 1
/// After:
///   0: 2 <- Hidden
///   0: 0 <- Zero
///   1: 1
#[test]
fn shrink_before_zero() {
    // Setup storage area.
    let mut storage: Storage<char> = Storage {
        inner: vec![filled_row('2'), filled_row('0'), filled_row('1')],
        zero: 1,
        visible_lines: 3,
        len: 3,
    };

    // Shrink buffer.
    storage.shrink_visible_lines(2);

    // Make sure the result is correct.
    let expected = Storage {
        inner: vec![filled_row('2'), filled_row('0'), filled_row('1')],
        zero: 1,
        visible_lines: 2,
        len: 2,
    };
    assert_eq!(storage.visible_lines, expected.visible_lines);
    assert_eq!(storage.inner, expected.inner);
    assert_eq!(storage.zero, expected.zero);
    assert_eq!(storage.len, expected.len);
}

/// Shrink the buffer one line at the end of the buffer.
///
/// Before:
///   0: 0 <- Zero
///   1: 1
///   2: 2
/// After:
///   0: 0 <- Zero
///   1: 1
///   2: 2 <- Hidden
#[test]
fn shrink_after_zero() {
    // Setup storage area.
    let mut storage: Storage<char> = Storage {
        inner: vec![filled_row('0'), filled_row('1'), filled_row('2')],
        zero: 0,
        visible_lines: 3,
        len: 3,
    };

    // Shrink buffer.
    storage.shrink_visible_lines(2);

    // Make sure the result is correct.
    let expected = Storage {
        inner: vec![filled_row('0'), filled_row('1'), filled_row('2')],
        zero: 0,
        visible_lines: 2,
        len: 2,
    };
    assert_eq!(storage.visible_lines, expected.visible_lines);
    assert_eq!(storage.inner, expected.inner);
    assert_eq!(storage.zero, expected.zero);
    assert_eq!(storage.len, expected.len);
}

/// Shrink the buffer at the start and end of the buffer.
///
/// Before:
///   0: 4
///   1: 5
///   2: 0 <- Zero
///   3: 1
///   4: 2
///   5: 3
/// After:
///   0: 4 <- Hidden
///   1: 5 <- Hidden
///   2: 0 <- Zero
///   3: 1
///   4: 2 <- Hidden
///   5: 3 <- Hidden
#[test]
fn shrink_before_and_after_zero() {
    // Setup storage area.
    let mut storage: Storage<char> = Storage {
        inner: vec![
            filled_row('4'),
            filled_row('5'),
            filled_row('0'),
            filled_row('1'),
            filled_row('2'),
            filled_row('3'),
        ],
        zero: 2,
        visible_lines: 6,
        len: 6,
    };

    // Shrink buffer.
    storage.shrink_visible_lines(2);

    // Make sure the result is correct.
    let expected = Storage {
        inner: vec![
            filled_row('4'),
            filled_row('5'),
            filled_row('0'),
            filled_row('1'),
            filled_row('2'),
            filled_row('3'),
        ],
        zero: 2,
        visible_lines: 2,
        len: 2,
    };
    assert_eq!(storage.visible_lines, expected.visible_lines);
    assert_eq!(storage.inner, expected.inner);
    assert_eq!(storage.zero, expected.zero);
    assert_eq!(storage.len, expected.len);
}

/// Check that when truncating all hidden lines are removed from the raw buffer.
///
/// Before:
///   0: 4 <- Hidden
///   1: 5 <- Hidden
///   2: 0 <- Zero
///   3: 1
///   4: 2 <- Hidden
///   5: 3 <- Hidden
/// After:
///   0: 0 <- Zero
///   1: 1
#[test]
fn truncate_invisible_lines() {
    // Setup storage area.
    let mut storage: Storage<char> = Storage {
        inner: vec![
            filled_row('4'),
            filled_row('5'),
            filled_row('0'),
            filled_row('1'),
            filled_row('2'),
            filled_row('3'),
        ],
        zero: 2,
        visible_lines: 1,
        len: 2,
    };

    // Truncate buffer.
    storage.truncate();

    // Make sure the result is correct.
    let expected = Storage {
        inner: vec![filled_row('0'), filled_row('1')],
        zero: 0,
        visible_lines: 1,
        len: 2,
    };
    assert_eq!(storage.visible_lines, expected.visible_lines);
    assert_eq!(storage.inner, expected.inner);
    assert_eq!(storage.zero, expected.zero);
    assert_eq!(storage.len, expected.len);
}

/// Truncate buffer only at the beginning.
///
/// Before:
///   0: 1
///   1: 2 <- Hidden
///   2: 0 <- Zero
/// After:
///   0: 1
///   0: 0 <- Zero
#[test]
fn truncate_invisible_lines_beginning() {
    // Setup storage area.
    let mut storage: Storage<char> = Storage {
        inner: vec![filled_row('1'), filled_row('2'), filled_row('0')],
        zero: 2,
        visible_lines: 1,
        len: 2,
    };

    // Truncate buffer.
    storage.truncate();

    // Make sure the result is correct.
    let expected = Storage {
        inner: vec![filled_row('0'), filled_row('1')],
        zero: 0,
        visible_lines: 1,
        len: 2,
    };
    assert_eq!(storage.visible_lines, expected.visible_lines);
    assert_eq!(storage.inner, expected.inner);
    assert_eq!(storage.zero, expected.zero);
    assert_eq!(storage.len, expected.len);
}

/// First shrink the buffer and then grow it again.
///
/// Before:
///   0: 4
///   1: 5
///   2: 0 <- Zero
///   3: 1
///   4: 2
///   5: 3
/// After Shrinking:
///   0: 4 <- Hidden
///   1: 5 <- Hidden
///   2: 0 <- Zero
///   3: 1
///   4: 2
///   5: 3 <- Hidden
/// After Growing:
///   0: 4
///   1: 5
///   2: -
///   3: 0 <- Zero
///   4: 1
///   5: 2
///   6: 3
#[test]
fn shrink_then_grow() {
    // Setup storage area.
    let mut storage: Storage<char> = Storage {
        inner: vec![
            filled_row('4'),
            filled_row('5'),
            filled_row('0'),
            filled_row('1'),
            filled_row('2'),
            filled_row('3'),
        ],
        zero: 2,
        visible_lines: 0,
        len: 6,
    };

    // Shrink buffer.
    storage.shrink_lines(3);

    // Make sure the result after shrinking is correct.
    let shrinking_expected = Storage {
        inner: vec![
            filled_row('4'),
            filled_row('5'),
            filled_row('0'),
            filled_row('1'),
            filled_row('2'),
            filled_row('3'),
        ],
        zero: 2,
        visible_lines: 0,
        len: 3,
    };
    assert_eq!(storage.inner, shrinking_expected.inner);
    assert_eq!(storage.zero, shrinking_expected.zero);
    assert_eq!(storage.len, shrinking_expected.len);

    // Grow buffer.
    storage.initialize(1, 1);

    // Make sure the previously freed elements are reused.
    let growing_expected = Storage {
        inner: vec![
            filled_row('4'),
            filled_row('5'),
            filled_row('0'),
            filled_row('1'),
            filled_row('2'),
            filled_row('3'),
        ],
        zero: 2,
        visible_lines: 0,
        len: 4,
    };

    assert_eq!(storage.inner, growing_expected.inner);
    assert_eq!(storage.zero, growing_expected.zero);
    assert_eq!(storage.len, growing_expected.len);
}

#[test]
fn initialize() {
    // Setup storage area.
    let mut storage: Storage<char> = Storage {
        inner: vec![
            filled_row('4'),
            filled_row('5'),
            filled_row('0'),
            filled_row('1'),
            filled_row('2'),
            filled_row('3'),
        ],
        zero: 2,
        visible_lines: 0,
        len: 6,
    };

    // Initialize additional lines.
    let init_size = 3;
    storage.initialize(init_size, 1);

    // Generate expected grid.
    let mut expected_inner = vec![
        filled_row('0'),
        filled_row('1'),
        filled_row('2'),
        filled_row('3'),
        filled_row('4'),
        filled_row('5'),
    ];
    let expected_init_size = std::cmp::max(init_size, MAX_CACHE_SIZE);
    expected_inner.append(&mut vec![filled_row('\0'); expected_init_size]);
    let expected_storage = Storage {
        inner: expected_inner,
        zero: 0,
        visible_lines: 0,
        len: 9,
    };

    assert_eq!(storage.len, expected_storage.len);
    assert_eq!(storage.zero, expected_storage.zero);
    assert_eq!(storage.inner, expected_storage.inner);
}

#[test]
fn rotate_wrap_zero() {
    let mut storage: Storage<char> = Storage {
        inner: vec![filled_row('-'), filled_row('-'), filled_row('-')],
        zero: 2,
        visible_lines: 0,
        len: 3,
    };

    storage.rotate(2);

    assert!(storage.zero < storage.inner.len());
}

fn filled_row(content: char) -> Row<char> {
    let mut row = Row::new(1);
    row[Column(0)] = content;
    row
}
