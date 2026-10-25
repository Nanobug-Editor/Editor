//! Native action-menu columns use the same local row and surface contract as ordinary popups.
use super::*;
use crate::{Icon, scroll::ScrollableElement as _};
use gpui_kit::assets::IconName;

impl ActionMenu {
    /// Draw one bounded column; cards consume pointer input before the outside-dismiss layer.
    fn column(
        &self,
        column: usize,
        width: f32,
        max_height: f32,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        let rows = self.rows(column, cx);
        let header = column == 0 && self.search.is_some();
        let height =
            ((rows.len() as f32 * 29.) + if header { 44. } else { 0. } + 12.).min(max_height);
        let mut body = div()
            .id(format!("action-menu-scroll-{column}"))
            .h(px((height - 12. - if header { 44. } else { 0. }).max(0.)))
            .overflow_y_scrollbar_with(&self.scrolls[column]);
        for row in rows {
            let item = row.item;
            let selected = self.selected.get(&column) == Some(&item.id);
            let id = item.id.clone();
            let hover_id = id.clone();
            let generation = self.generation;
            let disabled = item.disabled;
            let debug = format!("action-menu-{}", item.id);
            let group = matches!(item.kind, ActionMenuKind::Group { .. });
            let submenu = matches!(item.kind, ActionMenuKind::Submenu { .. });
            let expanded = self.expanded.get(&item.id).copied().unwrap_or(false);
            let icon = item
                .icon
                .as_ref()
                .and_then(|icon| self.icons.get(icon.path(cx.theme().is_dark())))
                .map(|bytes| crate::tree::TreeRowIcon::artwork(bytes).element());
            body = body.child(
                self.style
                    .row(
                        SharedString::from(debug.clone()),
                        item.label.for_locale(&self.locale).to_owned(),
                        selected,
                    )
                    .debug_selector(move || debug.clone())
                    .disabled(item.disabled)
                    .when(item.disabled, |row| {
                        row.text_color(cx.theme().muted_foreground)
                    })
                    .when(item.separator_before, |row| {
                        row.border_t_1().border_color(self.style.border)
                    })
                    .on_mouse_move(cx.listener(
                        move |this, event: &gpui_kit::MouseMoveEvent, _, cx| {
                            if !disabled
                                && this.generation == generation
                                && event.position != this.pointer
                            {
                                this.selected.insert(column, hover_id.clone());
                                this.active_column = column;
                                cx.notify();
                            }
                            this.pointer = event.position;
                        },
                    ))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if this.generation == generation {
                            this.activate(column, &id, window, cx);
                        }
                    }))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .w_full()
                            .min_w_0()
                            .gap_1()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .w(px(row.depth as f32 * 14. + 14.))
                                    .flex_shrink_0()
                                    // SVG chevrons share tree sizing and inherit the row's theme color.
                                    .when(group, |slot| {
                                        slot.child(
                                            Icon::new(if expanded {
                                                IconName::ChevronDown
                                            } else {
                                                IconName::ChevronRight
                                            })
                                            .xsmall(),
                                        )
                                    })
                                    .when(!group && item.checked, |slot| slot.child("✓")),
                            )
                            .children(icon)
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .child(item.label.for_locale(&self.locale).to_owned()),
                            )
                            .children(item.detail.as_ref().map(|detail| {
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .max_w(px(width * 0.4))
                                    .truncate()
                                    .child(detail.for_locale(&self.locale).to_owned())
                            }))
                            .when(submenu, |row| {
                                row.child(Icon::new(IconName::ChevronRight).xsmall())
                            }),
                    ),
            );
        }
        self.style
            .card(width)
            .id(format!("action-menu-column-{column}"))
            .debug_selector(move || format!("action-menu-column-{column}").into())
            .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
            .when(header, |card| {
                card.child(
                    div()
                        .h(px(44.))
                        .p_1()
                        .flex_shrink_0()
                        .child(crate::Input::new(self.search.as_ref().unwrap()).w_full()),
                )
            })
            .child(body)
            .into_any_element()
    }
}

impl Render for ActionMenu {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.closed {
            return div().into_any_element();
        }
        let viewport = window.viewport_size();
        let width = self.width.min((viewport.width / px(1.) - 16.).max(0.));
        let left = self
            .position
            .x
            .clamp(px(8.), (viewport.width - px(width + 8.)).max(px(8.)));
        let top = self
            .position
            .y
            .clamp(px(8.), (viewport.height - px(80.)).max(px(8.)));
        let max_height = (viewport.height - top - px(8.)) / px(1.);
        let mut layer = div()
            .absolute()
            .w(viewport.width)
            .h(viewport.height)
            .track_focus(&self.focus)
            .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_up(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .capture_key_down(cx.listener(Self::key))
            .capture_action(
                cx.listener(|this, _: &gpui_base::input::MoveUp, window, cx| {
                    this.search_key("up", window, cx)
                }),
            )
            .capture_action(
                cx.listener(|this, _: &gpui_base::input::MoveDown, window, cx| {
                    this.search_key("down", window, cx)
                }),
            )
            .capture_action(
                cx.listener(|this, _: &gpui_base::input::MoveHome, window, cx| {
                    this.search_key("home", window, cx)
                }),
            )
            .capture_action(
                cx.listener(|this, _: &gpui_base::input::MoveEnd, window, cx| {
                    this.search_key("end", window, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &gpui_base::actions::Confirm, window, cx| {
                    this.activate_selected(window, cx);
                    cx.stop_propagation();
                }),
            )
            .on_action(
                cx.listener(|this, _: &gpui_base::actions::Cancel, window, cx| {
                    this.search_key("escape", window, cx);
                    cx.stop_propagation();
                }),
            )
            .on_any_mouse_down(cx.listener(|this, _, window, cx| {
                this.finish(ActionMenuEvent::Dismiss, window, cx);
                cx.stop_propagation();
            }))
            .child(
                div()
                    .absolute()
                    .left(left)
                    .top(top)
                    .child(self.column(0, width, max_height, cx)),
            );
        let mut previous_left = left;
        for column in 1..=self.cascade.len() {
            let x = if previous_left + px(width * 2. + 8.) <= viewport.width {
                previous_left + px(width)
            } else {
                (previous_left - px(width)).max(px(8.))
            };
            let parent = &self.cascade[column - 1];
            let row = self
                .rows(column - 1, cx)
                .iter()
                .position(|row| &row.item.id == parent)
                .unwrap_or(0);
            let scroll = self.scrolls[column - 1].offset().y;
            let y = (top
                + px(row as f32 * 29.
                    + if column == 1 && self.search.is_some() {
                        44.
                    } else {
                        0.
                    })
                + scroll)
                .clamp(px(8.), (viewport.height - px(80.)).max(px(8.)));
            layer = layer.child(div().absolute().left(x).top(y).child(self.column(
                column,
                width,
                (viewport.height - y - px(8.)) / px(1.),
                cx,
            )));
            previous_left = x;
        }
        deferred(anchored().position(point(px(0.), px(0.))).child(layer))
            .with_priority(21)
            .into_any_element()
    }
}
