// SPDX-License-Identifier: Apache-2.0
//! Behavior regressions for typed terminal cell coordinates and bounded movement..
//!
//! Adapted from Alacritty 0.26.0; see this crate's NOTICE and LICENSE-ALACRITTY.

use super::*;

#[test]
fn location_ordering() {
    assert!(Point::new(Line(0), Column(0)) == Point::new(Line(0), Column(0)));
    assert!(Point::new(Line(1), Column(0)) > Point::new(Line(0), Column(0)));
    assert!(Point::new(Line(0), Column(1)) > Point::new(Line(0), Column(0)));
    assert!(Point::new(Line(1), Column(1)) > Point::new(Line(0), Column(0)));
    assert!(Point::new(Line(1), Column(1)) > Point::new(Line(0), Column(1)));
    assert!(Point::new(Line(1), Column(1)) > Point::new(Line(1), Column(0)));
    assert!(Point::new(Line(0), Column(0)) > Point::new(Line(-1), Column(0)));
}

#[test]
fn sub() {
    let size = (10, 42);
    let point = Point::new(Line(0), Column(13));

    let result = point.sub(&size, Boundary::Cursor, 1);

    assert_eq!(result, Point::new(Line(0), point.column - 1));
}

#[test]
fn sub_wrap() {
    let size = (10, 42);
    let point = Point::new(Line(1), Column(0));

    let result = point.sub(&size, Boundary::Cursor, 1);

    assert_eq!(result, Point::new(Line(0), size.last_column()));
}

#[test]
fn sub_clamp() {
    let size = (10, 42);
    let point = Point::new(Line(0), Column(0));

    let result = point.sub(&size, Boundary::Cursor, 1);

    assert_eq!(result, point);
}

#[test]
fn sub_grid_clamp() {
    let size = (0, 42);
    let point = Point::new(Line(0), Column(0));

    let result = point.sub(&size, Boundary::Grid, 1);

    assert_eq!(result, point);
}

#[test]
fn sub_none_clamp() {
    let size = (10, 42);
    let point = Point::new(Line(0), Column(0));

    let result = point.sub(&size, Boundary::None, 1);

    assert_eq!(result, Point::new(Line(9), Column(41)));
}

#[test]
fn add() {
    let size = (10, 42);
    let point = Point::new(Line(0), Column(13));

    let result = point.add(&size, Boundary::Cursor, 1);

    assert_eq!(result, Point::new(Line(0), point.column + 1));
}

#[test]
fn add_wrap() {
    let size = (10, 42);
    let point = Point::new(Line(0), size.last_column());

    let result = point.add(&size, Boundary::Cursor, 1);

    assert_eq!(result, Point::new(Line(1), Column(0)));
}

#[test]
fn add_clamp() {
    let size = (10, 42);
    let point = Point::new(Line(9), Column(41));

    let result = point.add(&size, Boundary::Cursor, 1);

    assert_eq!(result, point);
}

#[test]
fn add_grid_clamp() {
    let size = (10, 42);
    let point = Point::new(Line(9), Column(41));

    let result = point.add(&size, Boundary::Grid, 1);

    assert_eq!(result, point);
}

#[test]
fn add_none_clamp() {
    let size = (10, 42);
    let point = Point::new(Line(9), Column(41));

    let result = point.add(&size, Boundary::None, 1);

    assert_eq!(result, Point::new(Line(0), Column(0)));
}
