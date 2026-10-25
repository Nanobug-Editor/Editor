//! Alacritty owns Vi motion, selection expansion and wide-cell boundaries.

use super::*;
use crate::vi::ViMotion;

impl Engine {
    /// Turn the pointer's existing selection into a rectangle without changing its anchors.
    pub fn block_selection(&mut self) {
        if let Some(selection) = &mut self.term.selection {
            selection.ty = SelectionType::Block;
        }
    }
    /// Read the current logical size, including off-focus split panes.
    pub fn size(&self) -> GridSize {
        GridSize {
            columns: self.term.columns(),
            rows: self.term.screen_lines(),
        }
    }

    /// Enter/leave read-only Vi navigation without sending an escape or any other byte to the child.
    pub fn toggle_vi(&mut self) {
        self.term.toggle_vi_mode();
        self.term.selection = None;
        self.follow_screen = !self.mode().contains(TerminalMode::VI) && self.offset() == 0;
    }

    /// Apply `key`/modifiers in Vi mode. All text is consumed by this mode, including unknown keys.
    /// Returning false means normal terminal mode is active and normal input should proceed.
    pub fn vi_key(&mut self, key: &str, ctrl: bool, shift: bool) -> bool {
        if !self.mode().contains(TerminalMode::VI) {
            return false;
        }
        let motion = match (key, ctrl, shift) {
            ("h", false, false) | ("left", false, _) => Some(ViMotion::Left),
            ("j" | "down", false, _) => Some(ViMotion::Down),
            ("k" | "up", false, _) => Some(ViMotion::Up),
            ("l", false, false) | ("right", false, _) => Some(ViMotion::Right),
            ("b", false, false) => Some(ViMotion::SemanticLeft),
            ("w", false, false) => Some(ViMotion::SemanticRight),
            ("e", false, false) => Some(ViMotion::SemanticRightEnd),
            ("b", false, true) => Some(ViMotion::WordLeft),
            ("w", false, true) => Some(ViMotion::WordRight),
            ("e", false, true) => Some(ViMotion::WordRightEnd),
            ("0" | "home", false, _) => Some(ViMotion::First),
            ("$" | "end", false, _) => Some(ViMotion::Last),
            ("^", false, _) => Some(ViMotion::FirstOccupied),
            ("h", false, true) => Some(ViMotion::High),
            ("m", false, true) => Some(ViMotion::Middle),
            ("l", false, true) => Some(ViMotion::Low),
            ("%", false, _) => Some(ViMotion::Bracket),
            ("{", false, _) => Some(ViMotion::ParagraphUp),
            ("}", false, _) => Some(ViMotion::ParagraphDown),
            _ => None,
        };
        if let Some(motion) = motion {
            self.term.vi_motion(motion);
        } else {
            match (key, ctrl, shift) {
                ("i", false, _) => self.toggle_vi(),
                ("escape", _, _) => self.clear_selection(),
                ("g", false, _) => {
                    let line = if shift {
                        self.term.bottommost_line()
                    } else {
                        self.term.topmost_line()
                    };
                    self.term.vi_goto_point(Point::new(line, Column(0)));
                }
                ("v", false, _) | ("v", true, _) => {
                    let kind = if ctrl {
                        SelectionType::Block
                    } else if shift {
                        SelectionType::Lines
                    } else {
                        SelectionType::Simple
                    };
                    if self.term.selection.as_ref().is_some_and(|s| s.ty == kind) {
                        self.clear_selection();
                    } else {
                        let point = self.term.vi_mode_cursor.point;
                        let mut selection = Selection::new(kind, point, Side::Left);
                        selection.include_all();
                        self.term.selection = Some(selection);
                    }
                }
                ("u" | "b" | "pageup", true, _) | ("pageup", false, _) => {
                    self.scroll(self.term.screen_lines() as i32 / if key == "u" { 2 } else { 1 });
                }
                ("d" | "f" | "pagedown", true, _) | ("pagedown", false, _) => {
                    self.scroll(
                        -(self.term.screen_lines() as i32) / if key == "d" { 2 } else { 1 },
                    );
                }
                _ => {}
            }
        }
        // A mode exit may resume following, but navigation and selection never do.
        if self.mode().contains(TerminalMode::VI) {
            self.follow_screen = false;
        }
        true
    }
}
