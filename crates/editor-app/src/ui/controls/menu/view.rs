//! Native menu drawing is kept beside its state and interaction owner.

use super::*;

impl Render for PopupMenu {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.closed {
            return div().into_any_element();
        }
        let viewport = window.viewport_size();
        let width = self.width.min((viewport.width / px(1.) - 16.).max(0.));
        let height = (self
            .items
            .iter()
            .map(|item| self.row_height(item))
            .sum::<f32>()
            + 2. * self.style.padding_y
            + 2.)
            + if self.search.is_some() { 44. } else { 0. };
        let height = height
            .min(self.max_height)
            .min((viewport.height / px(1.) - 16.).max(0.));
        let left = self
            .position
            .x
            .clamp(px(8.), (viewport.width - px(width + 8.)).max(px(8.)));
        let top = self.position.y.clamp(
            px(8.),
            if self.below_anchor {
                (viewport.height - px(8.)).max(px(8.))
            } else {
                (viewport.height - px(height + 8.)).max(px(8.))
            },
        );
        let height = height.min((viewport.height - top - px(8.)) / px(1.));
        let split = self.items.len().saturating_sub(self.footer_items);
        let footer_height = self.items[split..]
            .iter()
            .map(|item| self.row_height(item))
            .sum::<f32>();
        let mut rows = div()
            .id("menu-scroll")
            .debug_selector(|| "native-menu-scroll".into())
            .h(px((height
                - footer_height
                - if self.search.is_some() { 44. } else { 0. }
                - 2. * self.style.padding_y
                - 2.)
                .max(0.)))
            .overflow_y_scroll()
            .track_scroll(&self.scroll);
        let mut footer = div().flex().flex_col().flex_shrink_0();
        for (index, item) in self.items.iter().enumerate() {
            let id = item.id.clone();
            let debug = format!("native-menu-{}", id);
            if matches!(
                self.row_content.get(&item.id),
                Some(MenuRowContent::Heading)
            ) {
                // Headings have no hit target and never enter the enabled navigation list.
                rows = rows.child(
                    div()
                        .id(SharedString::from(debug.clone()))
                        .debug_selector(move || debug.clone())
                        .flex()
                        .items_center()
                        .h(px(self.row_height(item)))
                        .px(px(self.style.padding_x + 6.))
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(cx.theme().muted_foreground)
                        .when(item.separator_before, |row| {
                            row.border_t_1().border_color(self.style.border)
                        })
                        .child(item.label.clone()),
                );
                continue;
            }
            let hover_id = id.clone();
            let detail = self.row_content.get(&item.id);
            let selected = self.selected == Some(index);
            let primary_debug = format!("native-menu-label-{}", item.id);
            let row = self
                .style
                .row(
                    SharedString::from(debug.clone()),
                    item.label.clone(),
                    self.selected == Some(index),
                )
                .debug_selector(move || debug.clone())
                .h(px(self.row_height(item)))
                .flex_shrink_0()
                .disabled(item.disabled)
                // Non-actionable placeholders stay readable in both themes without looking enabled.
                .when(item.disabled, |row| {
                    row.text_color(cx.theme().muted_foreground)
                })
                .when_some(self.notices.get(&item.id).cloned(), |row, reason| {
                    row.text_color(cx.theme().danger)
                        .tooltip(move |window, cx| {
                            crate::ui::controls::Tooltip::new(reason.clone()).build(window, cx)
                        })
                })
                .when(item.separator_before, |row| {
                    row.border_t_1().border_color(self.style.border)
                })
                .on_mouse_move(
                    cx.listener(move |this, event: &gpui_kit::MouseMoveEvent, _, cx| {
                        if event.position != this.pointer_position {
                            if let Some(index) = this
                                .items
                                .iter()
                                .position(|item| item.id == hover_id && !item.disabled)
                            {
                                this.selected = Some(index);
                                cx.notify();
                            }
                        }
                        this.pointer_position = event.position;
                    }),
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    if this
                        .items
                        .iter()
                        .any(|item| item.id == id && !item.disabled)
                    {
                        this.finish(Action::Select(id.clone()), window, cx)
                    }
                }))
                .map(|row| match detail {
                    Some(MenuRowContent::Detail { secondary, checked }) => {
                        let tooltip = secondary.clone();
                        row.accessibility_label(format!("{}\n{}", item.label, secondary))
                            .tooltip(move |window, cx| {
                                crate::ui::controls::Tooltip::new(tooltip.clone()).build(window, cx)
                            })
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .w_full()
                                    .min_w_0()
                                    .child(div().w(px(14.)).flex_shrink_0().child(if *checked {
                                        "✓"
                                    } else {
                                        ""
                                    }))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .flex_1()
                                            .min_w_0()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .id(SharedString::from(primary_debug.clone()))
                                                    // Publish the actual highlighted label's painted bounds.
                                                    .debug_selector(move || {
                                                        if selected {
                                                            "native-menu-selected-label".into()
                                                        } else {
                                                            primary_debug.clone()
                                                        }
                                                    })
                                                    .truncate()
                                                    .child(item.label.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .truncate()
                                                    .child(secondary.clone()),
                                            ),
                                    ),
                            )
                    }
                    _ => row.child(item.label.clone()),
                });
            let row = if let Some(action) = self.row_actions.get(&item.id) {
                let action_id = action.id.clone();
                let debug = format!("native-menu-{}", action_id);
                div()
                    .flex()
                    .items_center()
                    .group("menu-action-row")
                    .child(div().flex_1().min_w_0().child(row))
                    .child(
                        crate::ui::controls::Button::new(SharedString::from(action.id.clone()))
                            .debug_selector(move || debug.clone())
                            .icon(gpui_kit::assets::IconName::Close)
                            .small()
                            .compact()
                            .ghost()
                            .accessibility_label(action.label.clone())
                            .tooltip(action.label.clone())
                            .on_click(cx.listener(move |this, _, window, cx| {
                                cx.stop_propagation();
                                (this.sink)(Action::Select(action_id.clone()), window, cx);
                            })),
                    )
                    .into_any_element()
            } else {
                row.into_any_element()
            };
            if index < split {
                rows = rows.child(row);
            } else {
                footer = footer.child(row);
            }
        }
        let card = self
            .style
            .card(width)
            .id("native-popup-menu")
            .debug_selector(|| "native-popup-menu".into())
            .role(Role::Menu)
            .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
            .when_some(self.search.as_ref(), |card, input| {
                card.child(
                    div()
                        .h(px(44.))
                        .p_1()
                        .flex_shrink_0()
                        .debug_selector(|| "project-search".into())
                        .child(crate::ui::controls::Input::new(input).w_full()),
                )
            })
            .child(rows)
            .when(self.footer_items > 0, |card| card.child(footer));
        deferred(
            anchored().position(point(px(0.), px(0.))).child(
                div()
                    .absolute()
                    .w(viewport.width)
                    .h(viewport.height)
                    .track_focus(&self.focus)
                    .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_mouse_up(MouseButton::Right, |_, _, cx| cx.stop_propagation())
                    .capture_key_down(cx.listener(Self::key))
                    .capture_action(cx.listener(
                        |this, _: &gpui_base::input::MoveUp, window, cx| {
                            this.search_navigation("up", window, cx)
                        },
                    ))
                    .capture_action(cx.listener(
                        |this, _: &gpui_base::input::MoveDown, window, cx| {
                            this.search_navigation("down", window, cx)
                        },
                    ))
                    .capture_action(cx.listener(
                        |this, _: &gpui_base::input::MoveHome, window, cx| {
                            this.search_navigation("home", window, cx)
                        },
                    ))
                    .capture_action(cx.listener(
                        |this, _: &gpui_base::input::MoveEnd, window, cx| {
                            this.search_navigation("end", window, cx)
                        },
                    ))
                    // A popup inside a dialog owns these actions as well as raw key events.
                    // Without this boundary, Enter selects a row and also confirms the parent modal.
                    .on_action(
                        cx.listener(|this, _: &gpui_base::actions::Confirm, window, cx| {
                            if let Some(item) = this
                                .selected
                                .and_then(|index| this.items.get(index))
                                .filter(|item| !item.disabled)
                            {
                                this.finish(Action::Select(item.id.clone()), window, cx);
                            }
                            cx.stop_propagation();
                        }),
                    )
                    .on_action(
                        cx.listener(|this, _: &gpui_base::actions::Cancel, window, cx| {
                            this.finish(Action::Dismiss, window, cx);
                            cx.stop_propagation();
                        }),
                    )
                    .on_any_mouse_down(cx.listener(|this, _, window, cx| {
                        this.finish(Action::Dismiss, window, cx);
                        cx.stop_propagation();
                    }))
                    .child(div().absolute().left(left).top(top).child(card)),
            ),
        )
        .with_priority(20)
        .into_any_element()
    }
}
