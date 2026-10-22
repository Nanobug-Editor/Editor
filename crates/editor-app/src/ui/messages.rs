//! Host toolbar routing composes the shared message control with window-specific lifecycle.
use crate::*;
pub(crate) use editor_ui::message_panel::MessagePanel;
use editor_ui::message_panel::notification_icon;

impl EditorApp {
    /// The window toolbar owns this Base-backed button; its reminder remains visible during plugin overflow.
    pub(crate) fn render_messages_button(&self, cx: &Context<Self>) -> impl IntoElement {
        let visible = self.messages_visible(cx);
        let selected_style = component_styles(cx, ThemeComponent::PanelToggle).selected;
        let pending = self.messages.read(cx).has_pending_alert();
        let label = if pending {
            t!("messages.unread")
        } else {
            t!("messages.title")
        }
        .to_string();
        div()
            .id("host-messages-toggle")
            .debug_selector(|| "host-messages-toggle".into())
            .child(
                Button::new("toggle-host-messages")
                    .child(
                        div()
                            .relative()
                            .size(px(14.))
                            .flex_shrink_0()
                            .child(notification_icon())
                            // The circle is decoration inside the Base button, not a separate hit target or count.
                            .when(pending, |icon| {
                                icon.child(
                                    div()
                                        .id("host-messages-dot")
                                        .debug_selector(|| "host-messages-dot".into())
                                        .absolute()
                                        .top(px(-1.))
                                        .right(px(-1.))
                                        .size(px(6.))
                                        .rounded_full()
                                        .bg(cx.theme().danger),
                                )
                            }),
                    )
                    .small()
                    .compact()
                    .ghost()
                    .expanded(visible)
                    // Disclosure metadata alone does not paint Base buttons; match the other window controls.
                    .when(visible, |button| {
                        button
                            .bg(selected_style.background.unwrap_or(cx.theme().list_active))
                            .text_color(selected_style.foreground.unwrap_or(cx.theme().foreground))
                    })
                    .tooltip(label.clone())
                    .accessibility_label(label)
                    .on_click(cx.listener(|this, _, window, cx| {
                        cx.stop_propagation();
                        this.toggle_messages(window, cx);
                    })),
            )
    }
}
