//! Canonical one/two-stroke capture shared by key search and inline binding drafts.

use gpui_kit::{Keystroke, OperatingSystem};
use std::time::{Duration, Instant};

/// A deadline belongs to the first stroke, never to the most recent repaint.
#[derive(Default)]
pub(super) struct Capture {
    pub strokes: Vec<String>,
    pub waiting: bool,
    pub generation: u64,
    deadline: Option<Instant>,
}

impl Capture {
    /// Invalidate timers across recording sessions instead of reusing their generation numbers.
    pub fn clear(&mut self) {
        self.strokes.clear();
        self.waiting = false;
        self.deadline = None;
        self.generation += 1;
    }

    /// Record real keys using `os` for canonical host modifier names; ignore modifier-only events.
    pub fn record(&mut self, key: &Keystroke, now: Instant, os: OperatingSystem) -> bool {
        if matches!(key.key.as_str(), "shift" | "control" | "alt" | "cmd" | "fn")
            || key.key.is_empty()
        {
            return false;
        }
        if self.waiting && self.deadline.is_some_and(|deadline| now < deadline) {
            self.strokes.push(key.unparse(os));
            self.waiting = false;
            self.deadline = None;
        } else {
            self.strokes = vec![key.unparse(os)];
            self.waiting = true;
            self.deadline = Some(now + Duration::from_secs(2));
        }
        self.generation += 1;
        true
    }
}

/// Convert dynamic binding text into keycap labels using the host `os`, without changing stored steps.
pub(super) fn display(strokes: &[String], os: OperatingSystem) -> Vec<String> {
    strokes
        .iter()
        .map(|stroke| {
            let Ok(key) = Keystroke::parse_with_os(stroke, os) else {
                return stroke.clone();
            };
            let mut parts = Vec::new();
            if key.modifiers.control {
                parts.push("Ctrl".to_owned());
            }
            if key.modifiers.alt {
                parts.push("Alt".to_owned());
            }
            if key.modifiers.shift {
                parts.push("Shift".to_owned());
            }
            if key.modifiers.platform {
                parts.push("Cmd".to_owned());
            }
            if key.modifiers.function {
                parts.push("Fn".to_owned());
            }
            parts.push(match key.key.as_str() {
                "left" => "←".into(),
                "right" => "→".into(),
                "up" => "↑".into(),
                "down" => "↓".into(),
                "escape" => "Esc".into(),
                "space" => "Space".into(),
                "enter" => "Enter".into(),
                _ => key.key.to_uppercase(),
            });
            parts.join("+")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    //! Capture and display share the profile's explicit runtime host conventions.
    use super::*;

    /// Physical platform modifiers retain host notation while secondary aliases use native labels.
    #[test]
    fn shortcut_capture_uses_host_modifier_names() {
        let key = Keystroke {
            modifiers: gpui_kit::Modifiers {
                platform: true,
                ..Default::default()
            },
            key: "p".into(),
            key_char: None,
        };
        for (os, canonical, label) in [
            (OperatingSystem::Windows, "win-p", "Ctrl+P"),
            (OperatingSystem::Mac, "cmd-p", "Cmd+P"),
            (OperatingSystem::Linux, "super-p", "Ctrl+P"),
        ] {
            let mut capture = Capture::default();
            assert!(capture.record(&key, Instant::now(), os));
            assert_eq!(capture.strokes, [canonical]);
            assert_eq!(display(&["secondary-p".to_owned()], os), [label]);
        }
    }
}
