// SPDX-License-Identifier: Apache-2.0
//! Behavior regressions for read-only vi cursor movement on the same terminal grid..
//!
//! Adapted from Alacritty 0.26.0; see this crate's NOTICE and LICENSE-ALACRITTY.

use super::*;

use crate::coordinates::{Column, Line};
use crate::events::VoidListener;
use crate::screen::test_support::ScreenSize;
use crate::screen::{Screen, ScreenConfig};
use vte::ansi::Handler;

fn term() -> Screen<VoidListener> {
    let size = ScreenSize::new(20, 20);
    Screen::new(ScreenConfig::default(), &size, VoidListener)
}

#[test]
fn motion_simple() {
    let mut term = term();

    let mut cursor = ViModeCursor::new(Point::new(Line(0), Column(0)));

    cursor = cursor.motion(&mut term, ViMotion::Right);
    assert_eq!(cursor.point, Point::new(Line(0), Column(1)));

    cursor = cursor.motion(&mut term, ViMotion::Left);
    assert_eq!(cursor.point, Point::new(Line(0), Column(0)));

    cursor = cursor.motion(&mut term, ViMotion::Down);
    assert_eq!(cursor.point, Point::new(Line(1), Column(0)));

    cursor = cursor.motion(&mut term, ViMotion::Up);
    assert_eq!(cursor.point, Point::new(Line(0), Column(0)));
}

#[test]
fn simple_wide() {
    let mut term = term();
    term.grid_mut()[Line(0)][Column(0)].c = 'a';
    term.grid_mut()[Line(0)][Column(1)].c = '汉';
    term.grid_mut()[Line(0)][Column(1)]
        .flags
        .insert(Flags::WIDE_CHAR);
    term.grid_mut()[Line(0)][Column(2)].c = ' ';
    term.grid_mut()[Line(0)][Column(2)]
        .flags
        .insert(Flags::WIDE_CHAR_SPACER);
    term.grid_mut()[Line(0)][Column(3)].c = 'a';

    let mut cursor = ViModeCursor::new(Point::new(Line(0), Column(1)));
    cursor = cursor.motion(&mut term, ViMotion::Right);
    assert_eq!(cursor.point, Point::new(Line(0), Column(3)));

    let mut cursor = ViModeCursor::new(Point::new(Line(0), Column(2)));
    cursor = cursor.motion(&mut term, ViMotion::Left);
    assert_eq!(cursor.point, Point::new(Line(0), Column(0)));
}

#[test]
fn motion_start_end() {
    let mut term = term();

    let mut cursor = ViModeCursor::new(Point::new(Line(0), Column(0)));

    cursor = cursor.motion(&mut term, ViMotion::Last);
    assert_eq!(cursor.point, Point::new(Line(0), Column(19)));

    cursor = cursor.motion(&mut term, ViMotion::First);
    assert_eq!(cursor.point, Point::new(Line(0), Column(0)));
}

#[test]
fn motion_first_occupied() {
    let mut term = term();
    term.grid_mut()[Line(0)][Column(0)].c = ' ';
    term.grid_mut()[Line(0)][Column(1)].c = 'x';
    term.grid_mut()[Line(0)][Column(2)].c = ' ';
    term.grid_mut()[Line(0)][Column(3)].c = 'y';
    term.grid_mut()[Line(0)][Column(19)]
        .flags
        .insert(Flags::WRAPLINE);
    term.grid_mut()[Line(1)][Column(19)]
        .flags
        .insert(Flags::WRAPLINE);
    term.grid_mut()[Line(2)][Column(0)].c = 'z';
    term.grid_mut()[Line(2)][Column(1)].c = ' ';

    let mut cursor = ViModeCursor::new(Point::new(Line(2), Column(1)));

    cursor = cursor.motion(&mut term, ViMotion::FirstOccupied);
    assert_eq!(cursor.point, Point::new(Line(2), Column(0)));

    cursor = cursor.motion(&mut term, ViMotion::FirstOccupied);
    assert_eq!(cursor.point, Point::new(Line(0), Column(1)));
}

#[test]
fn motion_high_middle_low() {
    let mut term = term();

    let mut cursor = ViModeCursor::new(Point::new(Line(0), Column(0)));

    cursor = cursor.motion(&mut term, ViMotion::High);
    assert_eq!(cursor.point, Point::new(Line(0), Column(0)));

    cursor = cursor.motion(&mut term, ViMotion::Middle);
    assert_eq!(cursor.point, Point::new(Line(9), Column(0)));

    cursor = cursor.motion(&mut term, ViMotion::Low);
    assert_eq!(cursor.point, Point::new(Line(19), Column(0)));
}

#[test]
fn motion_bracket() {
    let mut term = term();
    term.grid_mut()[Line(0)][Column(0)].c = '(';
    term.grid_mut()[Line(0)][Column(1)].c = 'x';
    term.grid_mut()[Line(0)][Column(2)].c = ')';

    let mut cursor = ViModeCursor::new(Point::new(Line(0), Column(0)));

    cursor = cursor.motion(&mut term, ViMotion::Bracket);
    assert_eq!(cursor.point, Point::new(Line(0), Column(2)));

    cursor = cursor.motion(&mut term, ViMotion::Bracket);
    assert_eq!(cursor.point, Point::new(Line(0), Column(0)));
}

fn motion_semantic_term() -> Screen<VoidListener> {
    let mut term = term();

    term.grid_mut()[Line(0)][Column(0)].c = 'x';
    term.grid_mut()[Line(0)][Column(1)].c = ' ';
    term.grid_mut()[Line(0)][Column(2)].c = 'x';
    term.grid_mut()[Line(0)][Column(3)].c = 'x';
    term.grid_mut()[Line(0)][Column(4)].c = ' ';
    term.grid_mut()[Line(0)][Column(5)].c = ' ';
    term.grid_mut()[Line(0)][Column(6)].c = ':';
    term.grid_mut()[Line(0)][Column(7)].c = ' ';
    term.grid_mut()[Line(0)][Column(8)].c = 'x';
    term.grid_mut()[Line(0)][Column(9)].c = ':';
    term.grid_mut()[Line(0)][Column(10)].c = 'x';
    term.grid_mut()[Line(0)][Column(11)].c = ' ';
    term.grid_mut()[Line(0)][Column(12)].c = ' ';
    term.grid_mut()[Line(0)][Column(13)].c = ':';
    term.grid_mut()[Line(0)][Column(14)].c = ' ';
    term.grid_mut()[Line(0)][Column(15)].c = 'x';

    term
}

#[test]
fn motion_semantic_right_end() {
    let mut term = motion_semantic_term();

    let mut cursor = ViModeCursor::new(Point::new(Line(0), Column(0)));

    cursor = cursor.motion(&mut term, ViMotion::SemanticRightEnd);
    assert_eq!(cursor.point, Point::new(Line(0), Column(3)));

    cursor = cursor.motion(&mut term, ViMotion::SemanticRightEnd);
    assert_eq!(cursor.point, Point::new(Line(0), Column(6)));

    cursor = cursor.motion(&mut term, ViMotion::SemanticRightEnd);
    assert_eq!(cursor.point, Point::new(Line(0), Column(8)));

    cursor = cursor.motion(&mut term, ViMotion::SemanticRightEnd);
    assert_eq!(cursor.point, Point::new(Line(0), Column(9)));

    cursor = cursor.motion(&mut term, ViMotion::SemanticRightEnd);
    assert_eq!(cursor.point, Point::new(Line(0), Column(10)));

    cursor = cursor.motion(&mut term, ViMotion::SemanticRightEnd);
    assert_eq!(cursor.point, Point::new(Line(0), Column(13)));

    cursor = cursor.motion(&mut term, ViMotion::SemanticRightEnd);
    assert_eq!(cursor.point, Point::new(Line(0), Column(15)));
}

#[test]
fn motion_semantic_left_start() {
    let mut term = motion_semantic_term();

    let mut cursor = ViModeCursor::new(Point::new(Line(0), Column(15)));

    cursor = cursor.motion(&mut term, ViMotion::SemanticLeft);
    assert_eq!(cursor.point, Point::new(Line(0), Column(13)));

    cursor = cursor.motion(&mut term, ViMotion::SemanticLeft);
    assert_eq!(cursor.point, Point::new(Line(0), Column(10)));

    cursor = cursor.motion(&mut term, ViMotion::SemanticLeft);
    assert_eq!(cursor.point, Point::new(Line(0), Column(9)));

    cursor = cursor.motion(&mut term, ViMotion::SemanticLeft);
    assert_eq!(cursor.point, Point::new(Line(0), Column(8)));

    cursor = cursor.motion(&mut term, ViMotion::SemanticLeft);
    assert_eq!(cursor.point, Point::new(Line(0), Column(6)));

    cursor = cursor.motion(&mut term, ViMotion::SemanticLeft);
    assert_eq!(cursor.point, Point::new(Line(0), Column(2)));

    cursor = cursor.motion(&mut term, ViMotion::SemanticLeft);
    assert_eq!(cursor.point, Point::new(Line(0), Column(0)));
}

#[test]
fn motion_semantic_right_start() {
    let mut term = motion_semantic_term();

    let mut cursor = ViModeCursor::new(Point::new(Line(0), Column(0)));

    cursor = cursor.motion(&mut term, ViMotion::SemanticRight);
    assert_eq!(cursor.point, Point::new(Line(0), Column(2)));

    cursor = cursor.motion(&mut term, ViMotion::SemanticRight);
    assert_eq!(cursor.point, Point::new(Line(0), Column(6)));

    cursor = cursor.motion(&mut term, ViMotion::SemanticRight);
    assert_eq!(cursor.point, Point::new(Line(0), Column(8)));

    cursor = cursor.motion(&mut term, ViMotion::SemanticRight);
    assert_eq!(cursor.point, Point::new(Line(0), Column(9)));

    cursor = cursor.motion(&mut term, ViMotion::SemanticRight);
    assert_eq!(cursor.point, Point::new(Line(0), Column(10)));

    cursor = cursor.motion(&mut term, ViMotion::SemanticRight);
    assert_eq!(cursor.point, Point::new(Line(0), Column(13)));

    cursor = cursor.motion(&mut term, ViMotion::SemanticRight);
    assert_eq!(cursor.point, Point::new(Line(0), Column(15)));
}

#[test]
fn motion_semantic_left_end() {
    let mut term = motion_semantic_term();

    let mut cursor = ViModeCursor::new(Point::new(Line(0), Column(15)));

    cursor = cursor.motion(&mut term, ViMotion::SemanticLeftEnd);
    assert_eq!(cursor.point, Point::new(Line(0), Column(13)));

    cursor = cursor.motion(&mut term, ViMotion::SemanticLeftEnd);
    assert_eq!(cursor.point, Point::new(Line(0), Column(10)));

    cursor = cursor.motion(&mut term, ViMotion::SemanticLeftEnd);
    assert_eq!(cursor.point, Point::new(Line(0), Column(9)));

    cursor = cursor.motion(&mut term, ViMotion::SemanticLeftEnd);
    assert_eq!(cursor.point, Point::new(Line(0), Column(8)));

    cursor = cursor.motion(&mut term, ViMotion::SemanticLeftEnd);
    assert_eq!(cursor.point, Point::new(Line(0), Column(6)));

    cursor = cursor.motion(&mut term, ViMotion::SemanticLeftEnd);
    assert_eq!(cursor.point, Point::new(Line(0), Column(3)));

    cursor = cursor.motion(&mut term, ViMotion::SemanticLeftEnd);
    assert_eq!(cursor.point, Point::new(Line(0), Column(0)));
}

#[test]
fn scroll_semantic() {
    let mut term = term();
    term.grid_mut().scroll_up(&(Line(0)..Line(20)), 5);

    let mut cursor = ViModeCursor::new(Point::new(Line(0), Column(0)));

    cursor = cursor.motion(&mut term, ViMotion::SemanticLeft);
    assert_eq!(cursor.point, Point::new(Line(-5), Column(0)));
    assert_eq!(term.grid().display_offset(), 5);

    cursor = cursor.motion(&mut term, ViMotion::SemanticRight);
    assert_eq!(cursor.point, Point::new(Line(19), Column(19)));
    assert_eq!(term.grid().display_offset(), 0);

    cursor = cursor.motion(&mut term, ViMotion::SemanticLeftEnd);
    assert_eq!(cursor.point, Point::new(Line(-5), Column(0)));
    assert_eq!(term.grid().display_offset(), 5);

    cursor = cursor.motion(&mut term, ViMotion::SemanticRightEnd);
    assert_eq!(cursor.point, Point::new(Line(19), Column(19)));
    assert_eq!(term.grid().display_offset(), 0);
}

#[test]
fn semantic_wide() {
    let mut term = term();
    term.grid_mut()[Line(0)][Column(0)].c = 'a';
    term.grid_mut()[Line(0)][Column(1)].c = ' ';
    term.grid_mut()[Line(0)][Column(2)].c = '汉';
    term.grid_mut()[Line(0)][Column(2)]
        .flags
        .insert(Flags::WIDE_CHAR);
    term.grid_mut()[Line(0)][Column(3)].c = ' ';
    term.grid_mut()[Line(0)][Column(3)]
        .flags
        .insert(Flags::WIDE_CHAR_SPACER);
    term.grid_mut()[Line(0)][Column(4)].c = ' ';
    term.grid_mut()[Line(0)][Column(5)].c = 'a';

    let mut cursor = ViModeCursor::new(Point::new(Line(0), Column(2)));
    cursor = cursor.motion(&mut term, ViMotion::SemanticRight);
    assert_eq!(cursor.point, Point::new(Line(0), Column(5)));

    let mut cursor = ViModeCursor::new(Point::new(Line(0), Column(3)));
    cursor = cursor.motion(&mut term, ViMotion::SemanticLeft);
    assert_eq!(cursor.point, Point::new(Line(0), Column(0)));
}

#[test]
fn motion_word() {
    let mut term = term();
    term.grid_mut()[Line(0)][Column(0)].c = 'a';
    term.grid_mut()[Line(0)][Column(1)].c = ';';
    term.grid_mut()[Line(0)][Column(2)].c = ' ';
    term.grid_mut()[Line(0)][Column(3)].c = ' ';
    term.grid_mut()[Line(0)][Column(4)].c = 'a';
    term.grid_mut()[Line(0)][Column(5)].c = ';';

    let mut cursor = ViModeCursor::new(Point::new(Line(0), Column(0)));

    cursor = cursor.motion(&mut term, ViMotion::WordRightEnd);
    assert_eq!(cursor.point, Point::new(Line(0), Column(1)));

    cursor = cursor.motion(&mut term, ViMotion::WordRightEnd);
    assert_eq!(cursor.point, Point::new(Line(0), Column(5)));

    cursor = cursor.motion(&mut term, ViMotion::WordLeft);
    assert_eq!(cursor.point, Point::new(Line(0), Column(4)));

    cursor = cursor.motion(&mut term, ViMotion::WordLeft);
    assert_eq!(cursor.point, Point::new(Line(0), Column(0)));

    cursor = cursor.motion(&mut term, ViMotion::WordRight);
    assert_eq!(cursor.point, Point::new(Line(0), Column(4)));

    cursor = cursor.motion(&mut term, ViMotion::WordLeftEnd);
    assert_eq!(cursor.point, Point::new(Line(0), Column(1)));
}

#[test]
fn scroll_word() {
    let mut term = term();
    term.grid_mut().scroll_up(&(Line(0)..Line(20)), 5);

    let mut cursor = ViModeCursor::new(Point::new(Line(0), Column(0)));

    cursor = cursor.motion(&mut term, ViMotion::WordLeft);
    assert_eq!(cursor.point, Point::new(Line(-5), Column(0)));
    assert_eq!(term.grid().display_offset(), 5);

    cursor = cursor.motion(&mut term, ViMotion::WordRight);
    assert_eq!(cursor.point, Point::new(Line(19), Column(19)));
    assert_eq!(term.grid().display_offset(), 0);

    cursor = cursor.motion(&mut term, ViMotion::WordLeftEnd);
    assert_eq!(cursor.point, Point::new(Line(-5), Column(0)));
    assert_eq!(term.grid().display_offset(), 5);

    cursor = cursor.motion(&mut term, ViMotion::WordRightEnd);
    assert_eq!(cursor.point, Point::new(Line(19), Column(19)));
    assert_eq!(term.grid().display_offset(), 0);
}

#[test]
fn word_wide() {
    let mut term = term();
    term.grid_mut()[Line(0)][Column(0)].c = 'a';
    term.grid_mut()[Line(0)][Column(1)].c = ' ';
    term.grid_mut()[Line(0)][Column(2)].c = '汉';
    term.grid_mut()[Line(0)][Column(2)]
        .flags
        .insert(Flags::WIDE_CHAR);
    term.grid_mut()[Line(0)][Column(3)].c = ' ';
    term.grid_mut()[Line(0)][Column(3)]
        .flags
        .insert(Flags::WIDE_CHAR_SPACER);
    term.grid_mut()[Line(0)][Column(4)].c = ' ';
    term.grid_mut()[Line(0)][Column(5)].c = 'a';

    let mut cursor = ViModeCursor::new(Point::new(Line(0), Column(2)));
    cursor = cursor.motion(&mut term, ViMotion::WordRight);
    assert_eq!(cursor.point, Point::new(Line(0), Column(5)));

    let mut cursor = ViModeCursor::new(Point::new(Line(0), Column(3)));
    cursor = cursor.motion(&mut term, ViMotion::WordLeft);
    assert_eq!(cursor.point, Point::new(Line(0), Column(0)));
}

#[test]
fn scroll_simple() {
    let mut term = term();

    // Create 1 line of scrollback.
    for _ in 0..20 {
        term.newline();
    }

    let mut cursor = ViModeCursor::new(Point::new(Line(0), Column(0)));

    cursor = cursor.scroll(&term, -1);
    assert_eq!(cursor.point, Point::new(Line(1), Column(0)));

    cursor = cursor.scroll(&term, 1);
    assert_eq!(cursor.point, Point::new(Line(0), Column(0)));

    cursor = cursor.scroll(&term, 1);
    assert_eq!(cursor.point, Point::new(Line(-1), Column(0)));
}

#[test]
fn scroll_over_top() {
    let mut term = term();

    // Create 40 lines of scrollback.
    for _ in 0..59 {
        term.newline();
    }

    let mut cursor = ViModeCursor::new(Point::new(Line(19), Column(0)));

    cursor = cursor.scroll(&term, 20);
    assert_eq!(cursor.point, Point::new(Line(-1), Column(0)));

    cursor = cursor.scroll(&term, 20);
    assert_eq!(cursor.point, Point::new(Line(-21), Column(0)));

    cursor = cursor.scroll(&term, 20);
    assert_eq!(cursor.point, Point::new(Line(-40), Column(0)));

    cursor = cursor.scroll(&term, 20);
    assert_eq!(cursor.point, Point::new(Line(-40), Column(0)));
}

#[test]
fn scroll_over_bottom() {
    let mut term = term();

    // Create 40 lines of scrollback.
    for _ in 0..59 {
        term.newline();
    }

    let mut cursor = ViModeCursor::new(Point::new(Line(-40), Column(0)));

    cursor = cursor.scroll(&term, -20);
    assert_eq!(cursor.point, Point::new(Line(-20), Column(0)));

    cursor = cursor.scroll(&term, -20);
    assert_eq!(cursor.point, Point::new(Line(0), Column(0)));

    cursor = cursor.scroll(&term, -20);
    assert_eq!(cursor.point, Point::new(Line(19), Column(0)));

    cursor = cursor.scroll(&term, -20);
    assert_eq!(cursor.point, Point::new(Line(19), Column(0)));
}

#[test]
fn wide_semantic_char() {
    let mut term = term();
    term.set_semantic_escape_chars("－");
    term.grid_mut()[Line(0)][Column(0)].c = 'x';
    term.grid_mut()[Line(0)][Column(1)].c = 'x';
    term.grid_mut()[Line(0)][Column(2)].c = '－';
    term.grid_mut()[Line(0)][Column(2)]
        .flags
        .insert(Flags::WIDE_CHAR);
    term.grid_mut()[Line(0)][Column(3)].c = ' ';
    term.grid_mut()[Line(0)][Column(3)]
        .flags
        .insert(Flags::WIDE_CHAR_SPACER);
    term.grid_mut()[Line(0)][Column(4)].c = 'x';
    term.grid_mut()[Line(0)][Column(5)].c = 'x';

    // Test motion to the right.

    let mut cursor = ViModeCursor::new(Point::new(Line(0), Column(0)));
    cursor = cursor.motion(&mut term, ViMotion::SemanticRight);
    assert_eq!(cursor.point, Point::new(Line(0), Column(2)));

    let mut cursor = ViModeCursor::new(Point::new(Line(0), Column(2)));
    cursor = cursor.motion(&mut term, ViMotion::SemanticRight);
    assert_eq!(cursor.point, Point::new(Line(0), Column(4)));

    // Test motion to the left.

    let mut cursor = ViModeCursor::new(Point::new(Line(0), Column(5)));
    cursor = cursor.motion(&mut term, ViMotion::SemanticLeft);
    assert_eq!(cursor.point, Point::new(Line(0), Column(4)));

    let mut cursor = ViModeCursor::new(Point::new(Line(0), Column(4)));
    cursor = cursor.motion(&mut term, ViMotion::SemanticLeft);
    assert_eq!(cursor.point, Point::new(Line(0), Column(2)));

    let mut cursor = ViModeCursor::new(Point::new(Line(0), Column(2)));
    cursor = cursor.motion(&mut term, ViMotion::SemanticLeft);
    assert_eq!(cursor.point, Point::new(Line(0), Column(0)));
}
