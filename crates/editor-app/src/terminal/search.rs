//! Native find controls and read-only Vi key routing, independent from child keyboard input.

use super::*;
use engine::TerminalMode;
use gpui_kit::{AnyElement, KeyDownEvent};

impl TerminalPanel {
    /// Reveal the selected terminal's retained query and focus the native IME-capable input.
    pub(super) fn open_search(&mut self, cx: &mut Context<Self>) {
        self.search_open = true;
        self.search_focus_pending = true;
        cx.notify();
    }

    /// Route new view shortcuts before raw terminal mapping; ordinary Ctrl+C/V stay intact.
    /// Only the focused native grid participates, so find-input editing never sends bytes to a child.
    pub(super) fn navigation_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.prefer_character_input {
            return;
        }
        if !self.grid.as_ref().is_some_and(|grid| {
            grid.read(cx).focus_handle().is_focused(window) && !grid.read(cx).is_composing()
        }) {
            return;
        }
        let key = &event.keystroke;
        let modifiers = key.modifiers;
        let mut handled = true;
        if modifiers.control && modifiers.shift && !modifiers.alt && key.key == "f" {
            self.open_search(cx);
        } else if key.key == "f3" && !modifiers.control && !modifiers.alt {
            self.search_open = true;
            if let Some(index) = self.active_index() {
                self.sessions[index].engine.search_next(modifiers.shift);
            }
        } else if modifiers.control && modifiers.shift && key.key == "space" && !modifiers.alt {
            if let Some(index) = self.active_index() {
                self.sessions[index].engine.toggle_vi();
            }
        } else if modifiers.control && !modifiers.alt && matches!(key.key.as_str(), "c" | "v") {
            handled = false;
        } else if let Some(index) = self.active_index() {
            let vi = self.sessions[index]
                .engine
                .mode()
                .contains(TerminalMode::VI);
            if vi && key.key == "/" {
                self.open_search(cx);
            } else if vi && modifiers.alt && key.key == "v" {
                self.sessions[index].engine.vi_key("v", true, false);
            } else if vi && key.key == "n" && !modifiers.control && !modifiers.alt {
                self.sessions[index].engine.search_next(modifiers.shift);
            } else if vi && key.key == "y" && !modifiers.control && !modifiers.alt {
                self.copy(cx);
                self.sessions[index].engine.clear_selection();
            } else if vi && key.key == "enter" {
                self.link = self.sessions[index].engine.vi_link();
                self.open_link(window, cx);
            } else {
                let text_key = key
                    .key_char
                    .as_deref()
                    .filter(|text| {
                        text.chars().count() == 1
                            && text.chars().any(|ch| ch.is_ascii_punctuation())
                    })
                    .unwrap_or(&key.key);
                handled = self.sessions[index].engine.vi_key(
                    text_key,
                    modifiers.control,
                    modifiers.shift,
                );
            }
        } else {
            handled = false;
        }
        if handled {
            window.prevent_default();
            cx.stop_propagation();
            cx.notify();
        }
    }

    /// Bind the independent terminal library toolbar to this host session and focus policy.
    pub(super) fn search_bar(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !self.search_open {
            return None;
        }
        let index = self.active_index()?;
        let session = &self.sessions[index];
        let search = &session.engine.search;
        let model = editor_terminal::SearchModel {
            session: session.id,
            query: search.query.clone(),
            regex: search.regex,
            case_sensitive: search.case_sensitive,
            invalid: search.invalid,
            found: search.current.is_some(),
        };
        if self.search_view.is_none() {
            let owner = cx.entity().downgrade();
            self.search_view = Some(cx.new(|cx| {
                editor_terminal::SearchBar::new(
                    model.clone(),
                    move |action, cx| {
                        let _ = owner.update(cx, |panel, cx| panel.search_action(action, cx));
                    },
                    window,
                    cx,
                )
            }));
        }
        let view = self.search_view.as_ref().unwrap().clone();
        let activate = std::mem::take(&mut self.search_focus_pending);
        view.update(cx, |view, cx| view.update(model, activate, window, cx));
        Some(view.into_any_element())
    }

    /// Apply only intents stamped with the still-selected session; late UI events cannot retarget.
    fn search_action(&mut self, action: editor_terminal::SearchAction, cx: &mut Context<Self>) {
        use editor_terminal::SearchAction;
        let session = match &action {
            SearchAction::Query(model) => model.session,
            SearchAction::Next { session, .. } | SearchAction::Close { session } => *session,
        };
        if self.active != Some(session) {
            return;
        }
        let Some(index) = self.active_index() else {
            return;
        };
        match action {
            SearchAction::Query(model) => {
                self.sessions[index].engine.set_search(
                    model.query,
                    model.regex,
                    model.case_sensitive,
                );
                self.sessions[index].engine.search_next(false);
            }
            SearchAction::Next { backwards, .. } => {
                self.sessions[index].engine.search_next(backwards);
            }
            SearchAction::Close { .. } => {
                self.search_open = false;
                self.focus_pending = true;
            }
        }
        cx.notify();
    }
}
