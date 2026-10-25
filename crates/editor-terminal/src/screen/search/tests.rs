// SPDX-License-Identifier: Apache-2.0
//! Behavior regressions for regex navigation over wrapped unicode cells without flattening terminal history..
//!
//! Adapted from Alacritty 0.26.0; see this crate's NOTICE and LICENSE-ALACRITTY.

use super::*;

use crate::coordinates::{Column, Line};
use crate::screen::ScreenConfig;
use crate::screen::test_support::{ScreenSize, mock_term};

#[test]
fn regex_right() {
    #[rustfmt::skip]
    let term = mock_term("\
        testing66\r\n\
        Alacritty\n\
        123\r\n\
        Alacritty\r\n\
        123\
    ");

    // Check regex across wrapped and unwrapped lines.
    let mut regex = RegexSearch::new("Ala.*123").unwrap();
    let start = Point::new(Line(1), Column(0));
    let end = Point::new(Line(4), Column(2));
    let match_start = Point::new(Line(1), Column(0));
    let match_end = Point::new(Line(2), Column(2));
    assert_eq!(
        term.regex_search_right(&mut regex, start, end),
        Some(match_start..=match_end)
    );
}

#[test]
fn regex_left() {
    #[rustfmt::skip]
    let term = mock_term("\
        testing66\r\n\
        Alacritty\n\
        123\r\n\
        Alacritty\r\n\
        123\
    ");

    // Check regex across wrapped and unwrapped lines.
    let mut regex = RegexSearch::new("Ala.*123").unwrap();
    let start = Point::new(Line(4), Column(2));
    let end = Point::new(Line(1), Column(0));
    let match_start = Point::new(Line(1), Column(0));
    let match_end = Point::new(Line(2), Column(2));
    assert_eq!(
        term.regex_search_left(&mut regex, start, end),
        Some(match_start..=match_end)
    );
}

#[test]
fn nested_regex() {
    #[rustfmt::skip]
    let term = mock_term("\
        Ala -> Alacritty -> critty\r\n\
        critty\
    ");

    // Greedy stopped at linebreak.
    let mut regex = RegexSearch::new("Ala.*critty").unwrap();
    let start = Point::new(Line(0), Column(0));
    let end = Point::new(Line(0), Column(25));
    assert_eq!(
        term.regex_search_right(&mut regex, start, end),
        Some(start..=end)
    );

    // Greedy stopped at dead state.
    let mut regex = RegexSearch::new("Ala[^y]*critty").unwrap();
    let start = Point::new(Line(0), Column(0));
    let end = Point::new(Line(0), Column(15));
    assert_eq!(
        term.regex_search_right(&mut regex, start, end),
        Some(start..=end)
    );
}

#[test]
fn no_match_right() {
    #[rustfmt::skip]
    let term = mock_term("\
        first line\n\
        broken second\r\n\
        third\
    ");

    let mut regex = RegexSearch::new("nothing").unwrap();
    let start = Point::new(Line(0), Column(0));
    let end = Point::new(Line(2), Column(4));
    assert_eq!(term.regex_search_right(&mut regex, start, end), None);
}

#[test]
fn no_match_left() {
    #[rustfmt::skip]
    let term = mock_term("\
        first line\n\
        broken second\r\n\
        third\
    ");

    let mut regex = RegexSearch::new("nothing").unwrap();
    let start = Point::new(Line(2), Column(4));
    let end = Point::new(Line(0), Column(0));
    assert_eq!(term.regex_search_left(&mut regex, start, end), None);
}

#[test]
fn include_linebreak_left() {
    #[rustfmt::skip]
    let term = mock_term("\
        testing123\r\n\
        xxx\
    ");

    // Make sure the cell containing the linebreak is not skipped.
    let mut regex = RegexSearch::new("te.*123").unwrap();
    let start = Point::new(Line(1), Column(0));
    let end = Point::new(Line(0), Column(0));
    let match_start = Point::new(Line(0), Column(0));
    let match_end = Point::new(Line(0), Column(9));
    assert_eq!(
        term.regex_search_left(&mut regex, start, end),
        Some(match_start..=match_end)
    );
}

#[test]
fn include_linebreak_right() {
    #[rustfmt::skip]
    let term = mock_term("\
        xxx\r\n\
        testing123\
    ");

    // Make sure the cell containing the linebreak is not skipped.
    let mut regex = RegexSearch::new("te.*123").unwrap();
    let start = Point::new(Line(0), Column(2));
    let end = Point::new(Line(1), Column(9));
    let match_start = Point::new(Line(1), Column(0));
    assert_eq!(
        term.regex_search_right(&mut regex, start, end),
        Some(match_start..=end)
    );
}

#[test]
fn skip_dead_cell() {
    let term = mock_term("alacritty");

    // Make sure dead state cell is skipped when reversing.
    let mut regex = RegexSearch::new("alacrit").unwrap();
    let start = Point::new(Line(0), Column(0));
    let end = Point::new(Line(0), Column(6));
    assert_eq!(
        term.regex_search_right(&mut regex, start, end),
        Some(start..=end)
    );
}

#[test]
fn reverse_search_dead_recovery() {
    let term = mock_term("zooo lense");

    // Make sure the reverse DFA operates the same as a forward DFA.
    let mut regex = RegexSearch::new("zoo").unwrap();
    let start = Point::new(Line(0), Column(9));
    let end = Point::new(Line(0), Column(0));
    let match_start = Point::new(Line(0), Column(0));
    let match_end = Point::new(Line(0), Column(2));
    assert_eq!(
        term.regex_search_left(&mut regex, start, end),
        Some(match_start..=match_end)
    );
}

#[test]
fn multibyte_unicode() {
    let term = mock_term("testвосибing");

    let mut regex = RegexSearch::new("te.*ing").unwrap();
    let start = Point::new(Line(0), Column(0));
    let end = Point::new(Line(0), Column(11));
    assert_eq!(
        term.regex_search_right(&mut regex, start, end),
        Some(start..=end)
    );

    let mut regex = RegexSearch::new("te.*ing").unwrap();
    let start = Point::new(Line(0), Column(11));
    let end = Point::new(Line(0), Column(0));
    assert_eq!(
        term.regex_search_left(&mut regex, start, end),
        Some(end..=start)
    );
}

#[test]
fn end_on_multibyte_unicode() {
    let term = mock_term("testвосиб");

    let mut regex = RegexSearch::new("te.*и").unwrap();
    let start = Point::new(Line(0), Column(0));
    let end = Point::new(Line(0), Column(8));
    let match_end = Point::new(Line(0), Column(7));
    assert_eq!(
        term.regex_search_right(&mut regex, start, end),
        Some(start..=match_end)
    );
}

#[test]
fn fullwidth() {
    let term = mock_term("a🦇x🦇");

    let mut regex = RegexSearch::new("[^ ]*").unwrap();
    let start = Point::new(Line(0), Column(0));
    let end = Point::new(Line(0), Column(5));
    assert_eq!(
        term.regex_search_right(&mut regex, start, end),
        Some(start..=end)
    );

    let mut regex = RegexSearch::new("[^ ]*").unwrap();
    let start = Point::new(Line(0), Column(5));
    let end = Point::new(Line(0), Column(0));
    assert_eq!(
        term.regex_search_left(&mut regex, start, end),
        Some(end..=start)
    );
}

#[test]
fn singlecell_fullwidth() {
    let term = mock_term("🦇");

    let mut regex = RegexSearch::new("🦇").unwrap();
    let start = Point::new(Line(0), Column(0));
    let end = Point::new(Line(0), Column(1));
    assert_eq!(
        term.regex_search_right(&mut regex, start, end),
        Some(start..=end)
    );

    let mut regex = RegexSearch::new("🦇").unwrap();
    let start = Point::new(Line(0), Column(1));
    let end = Point::new(Line(0), Column(0));
    assert_eq!(
        term.regex_search_left(&mut regex, start, end),
        Some(end..=start)
    );
}

#[test]
fn end_on_fullwidth() {
    let term = mock_term("jarr🦇");

    let start = Point::new(Line(0), Column(0));
    let end = Point::new(Line(0), Column(4));

    // Ensure ending without a match doesn't loop indefinitely.
    let mut regex = RegexSearch::new("x").unwrap();
    assert_eq!(term.regex_search_right(&mut regex, start, end), None);

    let mut regex = RegexSearch::new("x").unwrap();
    let match_end = Point::new(Line(0), Column(5));
    assert_eq!(term.regex_search_right(&mut regex, start, match_end), None);

    // Ensure match is captured when only partially inside range.
    let mut regex = RegexSearch::new("jarr🦇").unwrap();
    assert_eq!(
        term.regex_search_right(&mut regex, start, end),
        Some(start..=match_end)
    );
}

#[test]
fn wrapping() {
    #[rustfmt::skip]
    let term = mock_term("\
        xxx\r\n\
        xxx\
    ");

    let mut regex = RegexSearch::new("xxx").unwrap();
    let start = Point::new(Line(0), Column(2));
    let end = Point::new(Line(1), Column(2));
    let match_start = Point::new(Line(1), Column(0));
    assert_eq!(
        term.regex_search_right(&mut regex, start, end),
        Some(match_start..=end)
    );

    let mut regex = RegexSearch::new("xxx").unwrap();
    let start = Point::new(Line(1), Column(0));
    let end = Point::new(Line(0), Column(0));
    let match_end = Point::new(Line(0), Column(2));
    assert_eq!(
        term.regex_search_left(&mut regex, start, end),
        Some(end..=match_end)
    );
}

#[test]
fn wrapping_into_fullwidth() {
    #[rustfmt::skip]
    let term = mock_term("\
        🦇xx\r\n\
        xx🦇\
    ");

    let mut regex = RegexSearch::new("🦇x").unwrap();
    let start = Point::new(Line(0), Column(0));
    let end = Point::new(Line(1), Column(3));
    let match_start = Point::new(Line(0), Column(0));
    let match_end = Point::new(Line(0), Column(2));
    assert_eq!(
        term.regex_search_right(&mut regex, start, end),
        Some(match_start..=match_end)
    );

    let mut regex = RegexSearch::new("x🦇").unwrap();
    let start = Point::new(Line(1), Column(2));
    let end = Point::new(Line(0), Column(0));
    let match_start = Point::new(Line(1), Column(1));
    let match_end = Point::new(Line(1), Column(3));
    assert_eq!(
        term.regex_search_left(&mut regex, start, end),
        Some(match_start..=match_end)
    );
}

#[test]
fn multiline() {
    #[rustfmt::skip]
    let term = mock_term("\
        test \r\n\
        test\
    ");

    const PATTERN: &str = "[a-z]*";
    let mut regex = RegexSearch::new(PATTERN).unwrap();
    let start = Point::new(Line(0), Column(0));
    let end = Point::new(Line(0), Column(3));
    let match_start = Point::new(Line(0), Column(0));
    assert_eq!(
        term.regex_search_right(&mut regex, start, end),
        Some(match_start..=end)
    );

    let mut regex = RegexSearch::new(PATTERN).unwrap();
    let start = Point::new(Line(0), Column(4));
    let end = Point::new(Line(0), Column(0));
    let match_start = Point::new(Line(1), Column(0));
    let match_end = Point::new(Line(1), Column(3));
    assert_eq!(
        term.regex_search_right(&mut regex, start, end),
        Some(match_start..=match_end)
    );
}

#[test]
fn empty_match() {
    #[rustfmt::skip]
    let term = mock_term(" abc ");

    const PATTERN: &str = "[a-z]*";
    let mut regex = RegexSearch::new(PATTERN).unwrap();
    let start = Point::new(Line(0), Column(0));
    let end = Point::new(Line(0), Column(4));
    let match_start = Point::new(Line(0), Column(1));
    let match_end = Point::new(Line(0), Column(3));
    assert_eq!(
        term.regex_search_right(&mut regex, start, end),
        Some(match_start..=match_end)
    );
}

#[test]
fn empty_match_multibyte() {
    #[rustfmt::skip]
    let term = mock_term(" ↑");

    const PATTERN: &str = "[a-z]*";
    let mut regex = RegexSearch::new(PATTERN).unwrap();
    let start = Point::new(Line(0), Column(0));
    let end = Point::new(Line(0), Column(1));
    assert_eq!(term.regex_search_right(&mut regex, start, end), None);
}

#[test]
fn empty_match_multiline() {
    #[rustfmt::skip]
    let term = mock_term("abc          \nxxx");

    const PATTERN: &str = "[a-z]*";
    let mut regex = RegexSearch::new(PATTERN).unwrap();
    let start = Point::new(Line(0), Column(3));
    let end = Point::new(Line(1), Column(2));
    let match_start = Point::new(Line(1), Column(0));
    let match_end = Point::new(Line(1), Column(2));
    assert_eq!(
        term.regex_search_right(&mut regex, start, end),
        Some(match_start..=match_end)
    );
}

#[test]
fn leading_spacer() {
    #[rustfmt::skip]
    let mut term = mock_term("\
        xxx \n\
        🦇xx\
    ");
    term.grid[Line(0)][Column(3)]
        .flags
        .insert(Flags::LEADING_WIDE_CHAR_SPACER);

    let mut regex = RegexSearch::new("🦇x").unwrap();
    let start = Point::new(Line(0), Column(0));
    let end = Point::new(Line(1), Column(3));
    let match_start = Point::new(Line(0), Column(3));
    let match_end = Point::new(Line(1), Column(2));
    assert_eq!(
        term.regex_search_right(&mut regex, start, end),
        Some(match_start..=match_end)
    );

    let mut regex = RegexSearch::new("🦇x").unwrap();
    let start = Point::new(Line(1), Column(3));
    let end = Point::new(Line(0), Column(0));
    let match_start = Point::new(Line(0), Column(3));
    let match_end = Point::new(Line(1), Column(2));
    assert_eq!(
        term.regex_search_left(&mut regex, start, end),
        Some(match_start..=match_end)
    );

    let mut regex = RegexSearch::new("x🦇").unwrap();
    let start = Point::new(Line(0), Column(0));
    let end = Point::new(Line(1), Column(3));
    let match_start = Point::new(Line(0), Column(2));
    let match_end = Point::new(Line(1), Column(1));
    assert_eq!(
        term.regex_search_right(&mut regex, start, end),
        Some(match_start..=match_end)
    );

    let mut regex = RegexSearch::new("x🦇").unwrap();
    let start = Point::new(Line(1), Column(3));
    let end = Point::new(Line(0), Column(0));
    let match_start = Point::new(Line(0), Column(2));
    let match_end = Point::new(Line(1), Column(1));
    assert_eq!(
        term.regex_search_left(&mut regex, start, end),
        Some(match_start..=match_end)
    );
}

#[test]
fn wide_without_spacer() {
    let size = ScreenSize::new(2, 2);
    let mut term = Screen::new(ScreenConfig::default(), &size, ());
    term.grid[Line(0)][Column(0)].c = 'x';
    term.grid[Line(0)][Column(1)].c = '字';
    term.grid[Line(0)][Column(1)].flags = Flags::WIDE_CHAR;

    let mut regex = RegexSearch::new("test").unwrap();

    let start = Point::new(Line(0), Column(0));
    let end = Point::new(Line(0), Column(1));

    let mut iter = RegexIter::new(start, end, Direction::Right, &term, &mut regex);
    assert_eq!(iter.next(), None);
}

#[test]
fn wrap_around_to_another_end() {
    #[rustfmt::skip]
    let term = mock_term("\
        abc\r\n\
        def\
    ");

    // Bottom to top.
    let mut regex = RegexSearch::new("abc").unwrap();
    let start = Point::new(Line(1), Column(0));
    let end = Point::new(Line(0), Column(2));
    let match_start = Point::new(Line(0), Column(0));
    let match_end = Point::new(Line(0), Column(2));
    assert_eq!(
        term.regex_search_right(&mut regex, start, end),
        Some(match_start..=match_end)
    );

    // Top to bottom.
    let mut regex = RegexSearch::new("def").unwrap();
    let start = Point::new(Line(0), Column(2));
    let end = Point::new(Line(1), Column(0));
    let match_start = Point::new(Line(1), Column(0));
    let match_end = Point::new(Line(1), Column(2));
    assert_eq!(
        term.regex_search_left(&mut regex, start, end),
        Some(match_start..=match_end)
    );
}

#[test]
fn nfa_compile_error() {
    assert!(RegexSearch::new("[0-9A-Za-z]{9999999}").is_err());
}

#[test]
fn runtime_cache_error() {
    let term = mock_term(&str::repeat("i", 9999));

    let mut regex = RegexSearch::new("[0-9A-Za-z]{9999}").unwrap();
    let start = Point::new(Line(0), Column(0));
    let end = Point::new(Line(0), Column(9999));
    assert_eq!(term.regex_search_right(&mut regex, start, end), None);
}

#[test]
fn greed_is_good() {
    #[rustfmt::skip]
    let term = mock_term("https://github.com");

    // Bottom to top.
    let mut regex = RegexSearch::new("/github.com|https://github.com").unwrap();
    let start = Point::new(Line(0), Column(0));
    let end = Point::new(Line(0), Column(17));
    assert_eq!(
        term.regex_search_right(&mut regex, start, end),
        Some(start..=end)
    );
}

#[test]
fn anchored_empty() {
    #[rustfmt::skip]
    let term = mock_term("rust");

    // Bottom to top.
    let mut regex = RegexSearch::new(";*|rust").unwrap();
    let start = Point::new(Line(0), Column(0));
    let end = Point::new(Line(0), Column(3));
    assert_eq!(
        term.regex_search_right(&mut regex, start, end),
        Some(start..=end)
    );
}

#[test]
fn newline_breaking_semantic() {
    #[rustfmt::skip]
    let term = mock_term("\
        test abc\r\n\
        def test\
    ");

    // Start at last character.
    let start = term.semantic_search_left(Point::new(Line(0), Column(7)));
    let end = term.semantic_search_right(Point::new(Line(0), Column(7)));
    assert_eq!(start, Point::new(Line(0), Column(5)));
    assert_eq!(end, Point::new(Line(0), Column(7)));

    // Start at first character.
    let start = term.semantic_search_left(Point::new(Line(1), Column(0)));
    let end = term.semantic_search_right(Point::new(Line(1), Column(0)));
    assert_eq!(start, Point::new(Line(1), Column(0)));
    assert_eq!(end, Point::new(Line(1), Column(2)));
}

#[test]
fn inline_word_search() {
    #[rustfmt::skip]
    let term = mock_term("\
        word word word word w\n\
        ord word word word\
    ");

    let mut regex = RegexSearch::new("word").unwrap();
    let start = Point::new(Line(1), Column(4));
    let end = Point::new(Line(0), Column(0));
    let match_start = Point::new(Line(0), Column(20));
    let match_end = Point::new(Line(1), Column(2));
    assert_eq!(
        term.regex_search_left(&mut regex, start, end),
        Some(match_start..=match_end)
    );
}

#[test]
fn fullwidth_semantic() {
    #[rustfmt::skip]
    let mut term = mock_term("test－x－test");
    term.config.semantic_escape_chars = "－".into();

    let start = term.semantic_search_left(Point::new(Line(0), Column(6)));
    let end = term.semantic_search_right(Point::new(Line(0), Column(6)));
    assert_eq!(start, Point::new(Line(0), Column(6)));
    assert_eq!(end, Point::new(Line(0), Column(6)));
}

#[test]
fn fullwidth_across_lines() {
    let term = mock_term("a🦇\n🦇b");

    let mut regex = RegexSearch::new("🦇🦇").unwrap();
    let start = Point::new(Line(0), Column(0));
    let end = Point::new(Line(1), Column(2));
    let match_start = Point::new(Line(0), Column(1));
    let match_end = Point::new(Line(1), Column(1));
    assert_eq!(
        term.regex_search_right(&mut regex, start, end),
        Some(match_start..=match_end)
    );

    let mut regex = RegexSearch::new("🦇🦇").unwrap();
    let start = Point::new(Line(1), Column(2));
    let end = Point::new(Line(0), Column(0));
    let match_start = Point::new(Line(1), Column(1));
    let match_end = Point::new(Line(0), Column(1));
    assert_eq!(
        term.regex_search_left(&mut regex, start, end),
        Some(match_end..=match_start)
    );
}

#[test]
fn fullwidth_into_halfwidth_across_lines() {
    let term = mock_term("a🦇\nxab");

    let mut regex = RegexSearch::new("🦇x").unwrap();
    let start = Point::new(Line(0), Column(0));
    let end = Point::new(Line(1), Column(2));
    let match_start = Point::new(Line(0), Column(1));
    let match_end = Point::new(Line(1), Column(0));
    assert_eq!(
        term.regex_search_right(&mut regex, start, end),
        Some(match_start..=match_end)
    );

    let mut regex = RegexSearch::new("🦇x").unwrap();
    let start = Point::new(Line(1), Column(2));
    let end = Point::new(Line(0), Column(0));
    let match_start = Point::new(Line(1), Column(0));
    let match_end = Point::new(Line(0), Column(1));
    assert_eq!(
        term.regex_search_left(&mut regex, start, end),
        Some(match_end..=match_start)
    );
}

#[test]
fn no_spacer_fullwidth_linewrap() {
    let mut term = mock_term("abY\nxab");
    term.grid_mut()[Line(0)][Column(2)].c = '🦇';

    let mut regex = RegexSearch::new("🦇x").unwrap();
    let start = Point::new(Line(0), Column(0));
    let end = Point::new(Line(1), Column(2));
    let match_start = Point::new(Line(0), Column(2));
    let match_end = Point::new(Line(1), Column(0));
    assert_eq!(
        term.regex_search_right(&mut regex, start, end),
        Some(match_start..=match_end)
    );

    let mut regex = RegexSearch::new("🦇x").unwrap();
    let start = Point::new(Line(1), Column(2));
    let end = Point::new(Line(0), Column(0));
    let match_start = Point::new(Line(1), Column(0));
    let match_end = Point::new(Line(0), Column(2));
    assert_eq!(
        term.regex_search_left(&mut regex, start, end),
        Some(match_end..=match_start)
    );
}
