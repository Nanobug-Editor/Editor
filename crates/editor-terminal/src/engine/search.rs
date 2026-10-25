//! Literal/regex search and viewport highlights use Alacritty's wrapped Unicode grid search.

use super::*;
use crate::{
    coordinates::{Boundary, Direction},
    screen::search::{Match, RegexIter, RegexSearch},
};

/// A session's transient query and current result; no process or persistent text ownership.
#[derive(Default)]
pub struct Search {
    /// Original user query, retained when the find bar closes.
    pub query: String,
    /// Interpret the query as a regex instead of literal text.
    pub regex: bool,
    /// Explicit case policy overrides Alacritty's default smart-case behavior.
    pub case_sensitive: bool,
    /// Invalid expressions are visible in the find bar and never reach the PTY.
    pub invalid: bool,
    /// Inclusive grid coordinates are valid only until output, resize or query replacement.
    pub current: Option<Match>,
    compiled: Option<RegexSearch>,
}

impl Engine {
    /// Compile `query` with the requested literal/regex and case policy, retiring the old result.
    pub fn set_search(&mut self, query: String, regex: bool, case_sensitive: bool) {
        self.search_visible = true;
        let source = if regex {
            query.clone()
        } else {
            regex::escape(&query)
        };
        let source = format!("(?{}i:{source})", if case_sensitive { "-" } else { "" });
        let compiled = (!query.is_empty()).then(|| RegexSearch::new(&source));
        self.search = Search {
            query,
            regex,
            case_sensitive,
            invalid: compiled.as_ref().is_some_and(Result::is_err),
            compiled: compiled.and_then(Result::ok),
            current: None,
        };
    }

    /// Find the next/previous match, wrapping over retained history and revealing its start.
    /// `backwards` controls direction; search never alters selection, process input or undo state.
    pub fn search_next(&mut self, backwards: bool) -> bool {
        let direction = if backwards {
            Direction::Left
        } else {
            Direction::Right
        };
        let origin = match &self.search.current {
            Some(found) if backwards => found.start().sub(&self.term, Boundary::None, 1),
            Some(found) => found.end().add(&self.term, Boundary::None, 1),
            None if backwards => Point::new(
                Line(self.term.screen_lines() as i32 - 1 - self.offset() as i32),
                self.term.last_column(),
            ),
            None => Point::new(Line(-(self.offset() as i32)), Column(0)),
        };
        let Some(regex) = &mut self.search.compiled else {
            return false;
        };
        let found = self
            .term
            .search_next(regex, origin, direction, Side::Left, None);
        if let Some(found) = &found {
            if self.mode().contains(TerminalMode::VI) {
                self.term.vi_goto_point(*found.start());
            } else {
                self.term.scroll_to_point(*found.start());
            }
            self.follow_screen = self.offset() == 0;
        }
        self.search.current = found;
        self.search.current.is_some()
    }

    /// Search only logical lines intersecting the viewport for paint; no full-history match vector.
    /// Returned inclusive ranges can include off-screen parts of a wrapped logical line.
    pub(super) fn visible_matches(&mut self) -> Vec<Match> {
        if !self.search_visible {
            return vec![];
        }
        let start = Point::new(Line(-(self.offset() as i32)), Column(0));
        let end = Point::new(
            start.line + self.term.screen_lines() - 1,
            self.term.last_column(),
        );
        let start = self.term.line_search_left(start);
        let end = self.term.line_search_right(end);
        let Some(regex) = &mut self.search.compiled else {
            return vec![];
        };
        RegexIter::new(start, end, Direction::Right, &self.term, regex).collect()
    }
}
