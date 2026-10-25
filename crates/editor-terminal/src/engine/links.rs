//! Resolve a clicked grid cell to OSC 8 metadata or the surrounding printed token.

use super::*;

/// Untrusted terminal output is only a navigation candidate, never an executable command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    /// Original URI or printed path; the view validates scheme and local existence on activation.
    pub target: String,
}

impl Engine {
    /// Resolve a link at the read-only Vi cursor through the same visible-cell policy.
    pub fn vi_link(&self) -> Option<Link> {
        let point =
            crate::screen::point_to_viewport(self.offset(), self.term.vi_mode_cursor.point)?;
        self.link_at(point.line, point.column.0)
    }
    /// Read `row`/`column` in the visible grid, including wrapped Unicode text and retained history.
    /// OSC 8 metadata takes precedence; plain tokens are bounded to one logical line.
    pub fn link_at(&self, row: usize, column: usize) -> Option<Link> {
        if row >= self.term.screen_lines() || column >= self.term.columns() {
            return None;
        }
        let point = Point::new(Line(row as i32 - self.offset() as i32), Column(column));
        if let Some(link) = self.term.grid()[point].hyperlink() {
            return Some(Link {
                target: link.uri().to_owned(),
            });
        }
        let start = self.term.line_search_left(point);
        let end = self.term.line_search_right(point);
        // Bound hostile no-newline output before allocating a candidate string.
        if end.line.0.saturating_sub(start.line.0) as usize * self.term.columns() > 8192 {
            return None;
        }
        let mut text = String::new();
        let mut clicked = None;
        for line in start.line.0..=end.line.0 {
            for column in 0..self.term.columns() {
                let at = Point::new(Line(line), Column(column));
                let cell = &self.term.grid()[at];
                if at == point {
                    clicked = Some(text.len());
                }
                if cell
                    .flags
                    .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
                {
                    continue;
                }
                text.push(cell.c);
                if let Some(extra) = cell.zerowidth() {
                    text.extend(extra);
                }
            }
        }
        let offset = clicked?;
        let mut begin = 0;
        let mut quote = None;
        // Quoted file names may contain spaces; ordinary punctuation is trimmed only at edges.
        for (index, ch) in text.char_indices() {
            if let Some(delimiter) = quote {
                if ch == delimiter {
                    quote = None;
                }
            } else if ch == '\'' || ch == '"' {
                quote = Some(ch);
            } else if ch.is_whitespace() {
                if index >= offset {
                    return token(&text[begin..index]);
                }
                begin = index + ch.len_utf8();
            }
        }
        token(&text[begin..])
    }
}

/// Strip prose delimiters while retaining the parentheses used by compiler line/column locations.
fn token(text: &str) -> Option<Link> {
    let text = text.trim_matches(|ch: char| matches!(ch, '[' | ']' | '<' | '>' | ',' | ';'));
    let text = text.strip_prefix('(').unwrap_or(text);
    let text = text.trim_end_matches(['.', ',']);
    (!text.is_empty() && text.len() <= 8192).then(|| Link {
        target: text.into(),
    })
}
