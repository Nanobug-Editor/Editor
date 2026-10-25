//! Terminal-specific find controls over generic local inputs and buttons.

use editor_ui::{Button, Icon, Input, theme::ActiveTheme as _};
use gpui_base::input::{InputEvent, InputState};
use gpui_kit::assets::IconName;
use gpui_kit::{
    App, AppContext as _, Context, Entity, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, Render, Styled, Subscription, Window, div,
};
use rust_i18n::t;
use std::rc::Rc;

#[cfg(test)]
mod tests;

/// Immutable projection of the selected terminal's search state; no terminal content is copied.
#[derive(Clone, Default, PartialEq)]
pub struct SearchModel {
    /// Stable owner identity returned with every action, rejecting late events after tab switches.
    pub session: u64,
    /// Literal or regular-expression draft text.
    pub query: String,
    /// Whether the query uses regular-expression syntax.
    pub regex: bool,
    /// Explicit case sensitivity, independent from Alacritty's implicit smart-case default.
    pub case_sensitive: bool,
    /// The owner rejected compilation of the current expression.
    pub invalid: bool,
    /// The last explicit navigation found a current-grid result.
    pub found: bool,
}

/// Direct native find intent. The host validates the session before changing its terminal state.
pub enum SearchAction {
    /// Replace the owning session's query and explicit search policies.
    Query(SearchModel),
    /// Navigate forward or backward within one still-current session.
    Next { session: u64, backwards: bool },
    /// Close the bar and return focus to its owning terminal.
    Close { session: u64 },
}

/// Terminal search UI owns its draft input/IME; the terminal Engine owns matching and navigation.
pub struct SearchBar {
    model: SearchModel,
    query: Entity<InputState>,
    sink: Rc<dyn Fn(SearchAction, &mut App)>,
    _subscription: Subscription,
}

impl SearchBar {
    /// Mount the current `model` and return user actions to `sink`; `window` hosts native input/IME.
    pub fn new(
        model: SearchModel,
        sink: impl Fn(SearchAction, &mut App) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let query = cx.new(|cx| InputState::new(window, cx).default_value(model.query.clone()));
        let subscription = cx.subscribe(&query, |bar, input, event, cx| {
            if matches!(event, InputEvent::Change) {
                let query = input.read(cx).value().to_string();
                if bar.model.query != query {
                    bar.model.query = query;
                    (bar.sink)(SearchAction::Query(bar.model.clone()), cx);
                }
            }
        });
        Self {
            model,
            query,
            sink: Rc::new(sink),
            _subscription: subscription,
        }
    }

    /// Apply the authoritative `model`; `activate` schedules query focus/selection after rendering.
    /// Cached dock bodies can render after a focused toolbar was painted, so focus must not change
    /// midway through that frame's accessibility tree. A replaced session cancels the old request.
    pub fn update(
        &mut self,
        model: SearchModel,
        activate: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let changed = self.model != model;
        self.model = model;
        if self.query.read(cx).value().as_ref() != self.model.query {
            let query = self.model.query.clone();
            self.query
                .update(cx, |input, cx| input.set_value(query, window, cx));
        }
        if activate {
            let session = self.model.session;
            cx.defer_in(window, move |bar, window, cx| {
                if bar.model.session != session {
                    return;
                }
                bar.query.update(cx, |input, cx| {
                    input.focus(window, cx);
                    input.select_all(window, cx);
                });
            });
        }
        if changed || activate {
            cx.notify();
        }
    }

    /// Toggle one explicit query policy and immediately notify the owning session.
    fn option(&mut self, regex: bool, cx: &mut Context<Self>) {
        if regex {
            self.model.regex = !self.model.regex;
        } else {
            self.model.case_sensitive = !self.model.case_sensitive;
        }
        (self.sink)(SearchAction::Query(self.model.clone()), cx);
        cx.notify();
    }
}

impl Render for SearchBar {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let status = if self.model.invalid {
            t!("terminal.invalid_regex").to_string()
        } else if self.model.query.is_empty() {
            String::new()
        } else if self.model.found {
            t!("terminal.match_found").to_string()
        } else {
            t!("terminal.no_match").to_string()
        };
        gpui_base::h_flex()
            .id("terminal-find")
            .debug_selector(|| "terminal-find".into())
            .flex_shrink_0()
            .gap_1()
            .p_1()
            .items_center()
            .border_b_1()
            .border_color(cx.theme().border)
            .capture_key_down(cx.listener(|bar, event: &KeyDownEvent, _, cx| {
                let action = match event.keystroke.key.as_str() {
                    "escape" => SearchAction::Close {
                        session: bar.model.session,
                    },
                    "enter" => SearchAction::Next {
                        session: bar.model.session,
                        backwards: event.keystroke.modifiers.shift,
                    },
                    _ => return,
                };
                (bar.sink)(action, cx);
                cx.stop_propagation();
            }))
            .child(
                div()
                    .debug_selector(|| "terminal-find-query".into())
                    .flex_1()
                    .min_w_0()
                    .child(
                        Input::new(&self.query)
                            .accessibility_label(t!("terminal.find").to_string()),
                    ),
            )
            .child(
                Button::new("terminal-find-case")
                    .compact()
                    .label("Aa")
                    .selected(self.model.case_sensitive)
                    .tooltip(t!("source.match_case").to_string())
                    .on_click(cx.listener(|bar, _, _, cx| bar.option(false, cx))),
            )
            .child(
                Button::new("terminal-find-regex")
                    .compact()
                    .label(".*")
                    .selected(self.model.regex)
                    .tooltip(t!("terminal.regex").to_string())
                    .on_click(cx.listener(|bar, _, _, cx| bar.option(true, cx))),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(if self.model.invalid {
                        cx.theme().danger
                    } else {
                        cx.theme().muted_foreground
                    })
                    .child(status),
            )
            .child(
                Button::new("terminal-find-previous")
                    .compact()
                    .icon(Icon::new(IconName::ChevronUp).small())
                    .tooltip(t!("source.previous").to_string())
                    .on_click(cx.listener(|bar, _, _, cx| {
                        (bar.sink)(
                            SearchAction::Next {
                                session: bar.model.session,
                                backwards: true,
                            },
                            cx,
                        )
                    })),
            )
            .child(
                Button::new("terminal-find-next")
                    .compact()
                    .icon(Icon::new(IconName::ChevronDown).small())
                    .tooltip(t!("source.next").to_string())
                    .on_click(cx.listener(|bar, _, _, cx| {
                        (bar.sink)(
                            SearchAction::Next {
                                session: bar.model.session,
                                backwards: false,
                            },
                            cx,
                        )
                    })),
            )
            .child(
                Button::new("terminal-find-close")
                    .compact()
                    .icon(Icon::new(IconName::Close).small())
                    .tooltip(t!("source.close").to_string())
                    .on_click(cx.listener(|bar, _, _, cx| {
                        (bar.sink)(
                            SearchAction::Close {
                                session: bar.model.session,
                            },
                            cx,
                        )
                    })),
            )
    }
}
