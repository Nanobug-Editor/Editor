//! Product find/replace presentation over the original Base search session and input drafts.
use super::*;

impl EditorTools {
    /// Close search and return focus to the source, retaining Base's query for the next activation.
    fn close_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.activation = None;
        self.editor.update(cx, |state, cx| {
            state.close_search(cx);
            state.focus(window, cx);
        });
    }
    /// Rebuild the local search controls while retaining input focus and Base query state.
    pub(super) fn render_search(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui_kit::AnyElement> {
        let session = self.editor.read(cx).search_session().clone();
        let activation = self.editor.read(cx).search_activation_revision();
        if session.open && self.activation != Some(activation) {
            self.activation = Some(activation);
            self.query.update(cx, |input, cx| {
                input.set_value(session.query.clone(), window, cx);
                input.focus(window, cx);
                input.select_all(window, cx);
            });
        }
        if !session.open {
            self.activation = None;
        }
        let search = session.open.then(|| {
            let input = Input::new(&self.query)
                .accessibility_label(rust_i18n::t!("source.find").to_string());
            gpui_base::v_flex()
                .id("local-editor-search")
                .debug_selector(|| "local-editor-search".into())
                .occlude()
                .absolute()
                .right_2()
                .top_2()
                .w(px(440.))
                .max_w_full()
                .p_2()
                .gap_2()
                .rounded(cx.theme().radius)
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().popover)
                .shadow_md()
                .capture_key_down(cx.listener(
                    |this, event: &gpui_kit::KeyDownEvent, window, cx| {
                        if event.keystroke.key == "escape" {
                            this.close_search(window, cx);
                            cx.stop_propagation();
                        } else if event.keystroke.key == "enter" {
                            this.editor.update(cx, |state, cx| {
                                if event.keystroke.modifiers.shift {
                                    state.previous_search_match(cx);
                                } else {
                                    state.next_search_match(cx);
                                }
                            });
                            cx.stop_propagation();
                        }
                    },
                ))
                .child(
                    gpui_base::h_flex()
                        .gap_1()
                        .child(
                            div()
                                .debug_selector(|| "local-find-query".into())
                                .flex_1()
                                .child(input),
                        )
                        .child(
                            Button::new("find-previous")
                                .compact()
                                .icon(Icon::new(gpui_kit::assets::IconName::ChevronUp))
                                .tooltip(rust_i18n::t!("source.previous").to_string())
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.editor.update(cx, |state, cx| {
                                        state.previous_search_match(cx);
                                    });
                                })),
                        )
                        .child(
                            Button::new("find-next")
                                .compact()
                                .icon(Icon::new(gpui_kit::assets::IconName::ChevronDown))
                                .tooltip(rust_i18n::t!("source.next").to_string())
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.editor.update(cx, |state, cx| {
                                        state.next_search_match(cx);
                                    });
                                })),
                        )
                        .child(
                            Button::new("find-close")
                                .compact()
                                .icon(Icon::new(gpui_kit::assets::IconName::Close))
                                .tooltip(rust_i18n::t!("source.close").to_string())
                                .on_click(
                                    cx.listener(|this, _, window, cx| {
                                        this.close_search(window, cx)
                                    }),
                                ),
                        ),
                )
                .child(
                    gpui_base::h_flex()
                        .gap_2()
                        .child(
                            Button::new("find-case")
                                .compact()
                                .label("Aa")
                                .selected(!session.case_insensitive)
                                .tooltip(rust_i18n::t!("source.match_case").to_string())
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let query = this.query.read(cx).value().to_string();
                                    let insensitive =
                                        !this.editor.read(cx).search_session().case_insensitive;
                                    this.editor.update(cx, |state, cx| {
                                        state.set_search_query(query, insensitive, cx)
                                    });
                                })),
                        )
                        .child(
                            Button::new("find-replace-mode")
                                .compact()
                                .label(rust_i18n::t!("source.replace"))
                                .selected(session.replace_mode)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let replace =
                                        !this.editor.read(cx).search_session().replace_mode;
                                    this.editor.update(cx, |state, cx| {
                                        state.set_search_replace_mode(replace, cx)
                                    });
                                })),
                        )
                        .child(
                            div()
                                .flex_1()
                                .text_color(cx.theme().muted_foreground)
                                .child(session.matcher.label()),
                        ),
                )
                .when(session.replace_mode, |panel| {
                    panel.child(
                        gpui_base::h_flex()
                            .gap_1()
                            .child(
                                div()
                                    .debug_selector(|| "local-find-replacement".into())
                                    .flex_1()
                                    .child(Input::new(&self.replacement).accessibility_label(
                                        rust_i18n::t!("source.replace").to_string(),
                                    )),
                            )
                            .child(
                                Button::new("replace-current")
                                    .debug_selector(|| "local-replace-current".into())
                                    .label(rust_i18n::t!("source.replace"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        let text = this.replacement.read(cx).value().to_string();
                                        this.editor.update(cx, |state, cx| {
                                            state.replace_current_search_match(&text, window, cx);
                                        });
                                    })),
                            )
                            .child(
                                Button::new("replace-all")
                                    .label(rust_i18n::t!("source.replace_all"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        let text = this.replacement.read(cx).value().to_string();
                                        this.editor.update(cx, |state, cx| {
                                            state.replace_all_search_matches(&text, window, cx);
                                        });
                                    })),
                            ),
                    )
                })
        });
        search.map(IntoElement::into_any_element)
    }
}
