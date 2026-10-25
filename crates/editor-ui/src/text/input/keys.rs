//! Opt-in native editing changes only this sealed draft; domain undo stays in the guest.
use super::*;
use gpui_kit::{Keystroke, point};
use plugin_runtime::plugin_protocol::ui::grapheme_boundaries;

impl CanvasInputState {
    /// Apply declared native editing keys after their independent raw observation was delivered.
    /// `stroke` uses native logical key names. IME owns keys while marked text exists; unrelated
    /// shortcuts return no observation. The owner emits the returned event after releasing this
    /// entity, avoiding a reentrant borrow of the Canvas during native key dispatch.
    pub fn handle_key(
        &mut self,
        stroke: &Keystroke,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<CanvasInputKind> {
        let modifiers = stroke.modifiers;
        if !self.active(window)
            || !self.binding().editing_keys
            || self.model.marked_range().is_some()
            || modifiers.alt
            || modifiers.platform
            || modifiers.function
        {
            return None;
        }
        let key = stroke.key.as_str();
        if modifiers.control {
            if key == "a" {
                return Some(self.key_selection(
                    CanvasTextRange {
                        start: 0,
                        end: self.model.text().len(),
                    },
                    false,
                    window,
                    cx,
                ));
            }
            // Undo, clipboard and other business shortcuts must never run an editor-document action.
            return None;
        }
        match key {
            "backspace" | "delete" if !self.binding().read_only => {
                let mut range = self.model.selection();
                if range.start == range.end {
                    if key == "backspace" {
                        range.start = grapheme_boundaries(self.model.text())
                            .take_while(|byte| *byte < range.start)
                            .last()
                            .unwrap_or(0);
                    } else {
                        range.end = grapheme_boundaries(self.model.text())
                            .find(|byte| *byte > range.end)
                            .unwrap_or(self.model.text().len());
                    }
                }
                if range.start != range.end {
                    let units = range
                        .to_utf16(self.model.text())
                        .expect("native grapheme deletion");
                    return self.key_commit(Some(units), "", window, cx);
                }
                None
            }
            "enter" if !self.binding().read_only => self.key_commit(None, "\n", window, cx),
            "left" | "right" | "up" | "down" | "home" | "end" => {
                self.move_caret(key, modifiers.shift, window, cx)
            }
            _ => None,
        }
    }
    /// Move the active native selection end; vertical navigation uses the same shaped visual lines.
    fn move_caret(
        &mut self,
        key: &str,
        extend: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<CanvasInputKind> {
        let selection = self.model.selection();
        let active = if self.reversed {
            selection.start
        } else {
            selection.end
        };
        let anchor = if self.reversed {
            selection.end
        } else {
            selection.start
        };
        let next = if !extend && selection.start != selection.end && matches!(key, "left" | "right")
        {
            if key == "left" {
                selection.start
            } else {
                selection.end
            }
        } else if key == "left" {
            grapheme_boundaries(self.model.text())
                .take_while(|byte| *byte < active)
                .last()
                .unwrap_or(0)
        } else if key == "right" {
            grapheme_boundaries(self.model.text())
                .find(|byte| *byte > active)
                .unwrap_or(self.model.text().len())
        } else {
            let layout = self.shaped(window).ok()?;
            let caret = layout.caret_bounds(active, true)?;
            let lines = layout.visible_lines().ok()?;
            let index = lines
                .iter()
                .position(|line| active >= line.range.start && active <= line.range.end)?;
            match key {
                "home" => lines[index].range.start,
                "end" => lines[index].range.end,
                _ => {
                    let next = if key == "up" {
                        index.saturating_sub(1)
                    } else {
                        (index + 1).min(lines.len() - 1)
                    };
                    layout.byte_for_point(point(
                        caret.origin.x,
                        px(lines[next].rect.y + lines[next].rect.h / 2.),
                    ))
                }
            }
        };
        let anchor = if extend { anchor } else { next };
        let range = CanvasTextRange {
            start: anchor.min(next),
            end: anchor.max(next),
        };
        Some(self.key_selection(range, next < anchor, window, cx))
    }
    /// Update selection locally and return its separate original-byte observation to the owner.
    fn key_selection(
        &mut self,
        range: CanvasTextRange,
        reversed: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> CanvasInputKind {
        let units = range
            .to_utf16(self.model.text())
            .expect("native selection boundaries");
        let range = self.model.select(units);
        self.reversed = reversed;
        cx.notify();
        window.invalidate_character_coordinates();
        CanvasInputKind::Selection {
            text_revision: self.binding().text_revision,
            base_sequence: self.model.base_sequence(),
            range,
        }
    }
    /// Native editing keys share OS input's bounded draft chain without invoking an editor action.
    fn key_commit(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<CanvasInputKind> {
        let sequence = self.sequence.get().checked_add(1)?;
        match self.model.commit(range, text, sequence) {
            Ok((edit, ended)) => {
                debug_assert!(ended.is_none(), "IME owns keys during composition");
                self.reversed = false;
                self.error = None;
                cx.notify();
                window.invalidate_character_coordinates();
                Some(CanvasInputKind::Text(edit))
            }
            Err(error) => {
                let message = error.message.chars().take(128).collect::<String>();
                let condition = (error.code, message.clone());
                if self.error.as_ref() == Some(&condition) {
                    return None;
                }
                self.error = Some(condition);
                Some(CanvasInputKind::TextError {
                    code: error.code,
                    message,
                })
            }
        }
    }
}
