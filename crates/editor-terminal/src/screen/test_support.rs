// SPDX-License-Identifier: Apache-2.0
//! Small internal screen fixtures shared by local VT algorithm regressions.
//!
//! Adapted from Alacritty 0.26.0; see this crate's NOTICE and LICENSE-ALACRITTY.

use super::*;

use serde::{Deserialize, Serialize};

use crate::events::VoidListener;

#[derive(Serialize, Deserialize)]
pub struct ScreenSize {
    pub columns: usize,
    pub screen_lines: usize,
}

impl ScreenSize {
    pub fn new(columns: usize, screen_lines: usize) -> Self {
        Self {
            columns,
            screen_lines,
        }
    }
}

impl Dimensions for ScreenSize {
    fn total_lines(&self) -> usize {
        self.screen_lines()
    }

    fn screen_lines(&self) -> usize {
        self.screen_lines
    }

    fn columns(&self) -> usize {
        self.columns
    }
}

/// Construct a terminal from its content as string.
///
/// A `\n` will break line and `\r\n` will break line without wrapping.
///
pub fn mock_term(content: &str) -> Screen<VoidListener> {
    let lines: Vec<&str> = content.split('\n').collect();
    let num_cols = lines
        .iter()
        .map(|line| {
            line.chars()
                .filter(|c| *c != '\r')
                .map(|c| c.width().unwrap())
                .sum()
        })
        .max()
        .unwrap_or(0);

    // Create terminal with the appropriate dimensions.
    let size = ScreenSize::new(num_cols, lines.len());
    let mut term = Screen::new(ScreenConfig::default(), &size, VoidListener);

    // Fill terminal with content.
    for (line, text) in lines.iter().enumerate() {
        let line = Line(line as i32);
        if !text.ends_with('\r') && line + 1 != lines.len() {
            term.grid[line][Column(num_cols - 1)]
                .flags
                .insert(Flags::WRAPLINE);
        }

        let mut index = 0;
        for c in text.chars().take_while(|c| *c != '\r') {
            term.grid[line][Column(index)].c = c;

            // Handle fullwidth characters.
            let width = c.width().unwrap();
            if width == 2 {
                term.grid[line][Column(index)]
                    .flags
                    .insert(Flags::WIDE_CHAR);
                term.grid[line][Column(index + 1)]
                    .flags
                    .insert(Flags::WIDE_CHAR_SPACER);
            }

            index += width;
        }
    }

    term
}
