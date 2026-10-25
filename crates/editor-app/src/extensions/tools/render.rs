//! Toolbar composition keeps native button behavior and delegates gestures to the local UI module.

use super::*;
use editor_ui::reorder_group::ReorderGroup;

impl EditorApp {
    /// Compose one captured entry with its selected appearance and ordinary native activation.
    fn render_bar_button(&self, entry: &Entry, cx: &Context<Self>) -> AnyElement {
        if matches!(entry.action, Action::Messages) {
            return self.render_messages_button(cx).into_any_element();
        }
        let selected = component_styles(cx, ThemeComponent::PanelToggle).selected;
        let id = entry.id.clone();
        let button = Button::new(SharedString::from(id.clone()))
            .small()
            .compact()
            .ghost()
            .w(px(24.))
            .h(px(24.))
            .accessibility_label(entry.label.clone())
            .tooltip(if matches!(entry.action, Action::Function { .. }) {
                entry.tooltip.clone()
            } else {
                format!("{}\n{}", entry.tooltip, t!("toolbar.reorder_hint"))
            })
            .disabled(entry.disabled)
            .when(entry.selected, |button| {
                button
                    .bg(selected.background.unwrap_or(cx.theme().list_active))
                    .text_color(selected.foreground.unwrap_or(cx.theme().foreground))
            });
        let button = if let Some(icon) = &entry.icon {
            button.icon(icon.clone())
        } else {
            button.label("□")
        };
        let action = entry.clone();
        div()
            .debug_selector(move || id.clone())
            .child(button.on_click(
                cx.listener(move |app, _, window, cx| app.activate_bar_entry(&action, window, cx)),
            ))
            .into_any_element()
    }

    /// Visible window buttons share a reorder surface; functions keep their contextual order.
    fn render_bar_group(
        &self,
        entries: Vec<Entry>,
        slots: usize,
        group: &'static str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let visible = if entries.len() <= slots {
            entries.len()
        } else {
            slots.saturating_sub(1)
        };
        let mut row = h_flex()
            .debug_selector(move || format!("plugin-{group}-group"))
            .tab_group()
            .items_center()
            .gap_1()
            .flex_shrink_0();
        if group == "windows" {
            let snapshot: Vec<_> = entries
                .iter()
                .take(visible)
                .map(|entry| entry.id.clone())
                .collect();
            let mut buttons =
                ReorderGroup::new("window-button-order", self.window_button_drag.clone())
                    .gap_1()
                    .on_reorder(cx.listener(move |app, event, _, cx| {
                        app.reorder_window_buttons(event, &snapshot, cx);
                    }));
            for entry in entries.iter().take(visible) {
                buttons = buttons.item(
                    entry.id.clone(),
                    entry.label.clone(),
                    self.render_bar_button(entry, cx),
                );
            }
            row = row.child(buttons);
        } else {
            for entry in entries.iter().take(visible) {
                row = row.child(self.render_bar_button(entry, cx));
            }
        }
        if visible < entries.len() {
            let unread_hidden = self.messages.read(cx).pending_level().filter(|_| {
                group == "windows"
                    && entries[visible..]
                        .iter()
                        .any(|entry| matches!(entry.action, Action::Messages))
            });
            // All windows are available in the reorder popup, including destinations currently visible.
            let hidden = if group == "windows" {
                entries
            } else {
                entries[visible..].to_vec()
            };
            let id = format!("plugin-{group}-overflow");
            let label = t!(if group == "windows" {
                "toolbar.more_windows"
            } else {
                "toolbar.more_tools"
            })
            .to_string();
            row = row.child(
                div().debug_selector(move || id.clone()).child(
                    Button::new(SharedString::from(format!("more-{group}")))
                        .label("⋯")
                        .small()
                        .compact()
                        .ghost()
                        .w(px(24.))
                        .h(px(24.))
                        .relative()
                        .when_some(unread_hidden, |button, level| {
                            button.child(
                                div()
                                    .debug_selector(|| "host-messages-overflow-dot".into())
                                    .absolute()
                                    .top(px(2.))
                                    .right(px(2.))
                                    .size(px(6.))
                                    .rounded_full()
                                    .bg(level.reminder_color(cx)),
                            )
                        })
                        .accessibility_label(label.clone())
                        .tooltip(label)
                        .on_click(cx.listener(
                            move |app, event: &gpui_kit::ClickEvent, window, cx| {
                                app.open_group_overflow(
                                    hidden.clone(),
                                    group == "windows",
                                    event.position(),
                                    window,
                                    cx,
                                )
                            },
                        )),
                ),
            );
        }
        row.into_any_element()
    }

    /// Reserve bounded slots for both groups while keeping the three default host entries together.
    pub(crate) fn render_plugin_toolbar(
        &mut self,
        explorer: Icon,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.update_function_context(window, cx);
        let windows = self.window_entries(explorer, cx);
        let functions = self.function_entries(cx);
        let slots = (((window.viewport_size().width / px(1.) - 220.).max(84.) / 28.).floor()
            as usize)
            .min(64);
        let window_slots = windows.len().min(if functions.is_empty() {
            slots
        } else {
            (slots / 2).max(4).min(slots.saturating_sub(1).max(1))
        });
        let tool_slots = slots.saturating_sub(window_slots).max(1);
        let mut bar = h_flex()
            .items_center()
            .flex_shrink_0()
            .track_focus(&self.toolbar_focus)
            .child(self.render_bar_group(windows, window_slots, "windows", cx));
        if !functions.is_empty() {
            bar = bar
                .child(
                    div()
                        .debug_selector(|| "plugin-tool-group-separator".into())
                        .w(px(1.))
                        .h(px(14.))
                        .mx(px(4.))
                        .bg(cx.theme().border),
                )
                .child(self.render_bar_group(functions, tool_slots, "tools", cx));
        }
        bar.into_any_element()
    }
}
