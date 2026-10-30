//! Local notification cards own their appearance, dismissal and lifetime without modal focus.

use super::{Button, Icon};
use crate::message_panel::MessageLevel;
use crate::scroll::ScrollableElement as _;
use crate::theme::ActiveTheme as _;
use gpui_base::{StyledExt as _, v_flex};
use gpui_kit::assets::IconName;
use gpui_kit::{
    App, ClickEvent, Context, InteractiveElement as _, IntoElement, MouseButton, ParentElement,
    Render, SharedString, StatefulInteractiveElement as _, Styled, Window, div,
    prelude::FluentBuilder as _, px,
};
use rust_i18n::t;
use std::rc::Rc;
use std::time::Duration;

/// Host and plugin-attributed notices share appearance and pointer boundaries; owners keep their lifetimes.
pub fn message_card(
    id: impl Into<gpui_kit::ElementId>,
    cx: &gpui_kit::App,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    div()
        .id(id)
        .role(gpui_kit::Role::Status)
        .occlude()
        .w_full()
        .items_start()
        .gap_2()
        .p_3()
        .rounded(px(8.))
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().popover)
        .text_color(cx.theme().popover_foreground)
        .shadow_lg()
        .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
        .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_scroll(|_, _, cx| cx.stop_propagation())
}

/// One dismissible nonmodal card; its owner chooses placement and the bounded visible collection.
pub struct Notification {
    title: SharedString,
    message: SharedString,
    dismissed: bool,
    /// Alert cards use severity artwork; ordinary transient notices retain their informational style.
    level: Option<MessageLevel>,
    /// The owner routes explicit activation without giving this control access to message history.
    on_click: Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
}

impl Notification {
    /// File-operation failures remain readable until explicitly dismissed; they never take focus.
    pub fn persistent(title: impl Into<SharedString>, message: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            dismissed: false,
            level: None,
            on_click: None,
        }
    }

    /// Retain an attributed warning or error until dismissal; `level` controls only its native appearance.
    pub fn alert(
        level: MessageLevel,
        title: impl Into<SharedString>,
        message: impl Into<SharedString>,
    ) -> Self {
        Self {
            level: Some(level),
            ..Self::persistent(title, message)
        }
    }

    /// Let the bounded owner discard closed cards without treating dismissal as history acknowledgement.
    pub fn is_dismissed(&self) -> bool {
        self.dismissed
    }

    /// Route a body click to `handler` with the original event, owning window and app; closing stays separate.
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    /// Display a message for 1.2 seconds; replacing the entity also replaces its expiration timer.
    pub fn new(
        title: impl Into<SharedString>,
        message: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1200))
                .await;
            // A dropped older card cannot dismiss the newer notification in the same window.
            let _ = this.update(cx, |this, cx| {
                this.dismissed = true;
                cx.notify();
            });
        })
        .detach();
        Self {
            title: title.into(),
            message: message.into(),
            dismissed: false,
            level: None,
            on_click: None,
        }
    }
}

impl Render for Notification {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.dismissed {
            return div().into_any_element();
        }
        let alert = self.level.is_some();
        let icon = match self.level {
            Some(MessageLevel::Warning) => super::StatusIcon::Warning.icon(cx),
            Some(MessageLevel::Error) => super::StatusIcon::Error.icon(cx),
            _ => Icon::new(IconName::Info)
                .small()
                .text_color(cx.theme().primary),
        };
        // History-backed alerts use a two-line preview; standalone notices retain their own body.
        let body = div()
            .id("notification-message-body")
            .debug_selector(|| "notification-message-body".into())
            .text_sm()
            .child(self.message.clone());
        let body = if alert {
            body.line_clamp(2).text_ellipsis().into_any_element()
        } else {
            body.max_h(px(80.))
                .overflow_y_scrollbar()
                .into_any_element()
        };
        message_card("local-notification", cx)
            .flex()
            .flex_shrink_0()
            .when_some(self.on_click.clone(), |card, handler| {
                card.cursor_pointer().on_click(move |event, window, cx| {
                    cx.stop_propagation();
                    handler(event, window, cx);
                })
            })
            .debug_selector(move || {
                if alert {
                    "host-message-toast"
                } else {
                    "local-notification"
                }
                .into()
            })
            .child(icon)
            .child(
                v_flex()
                    .flex_1()
                    .min_w(px(0.))
                    .gap_1()
                    .child(
                        div()
                            .font_semibold()
                            .text_sm()
                            .truncate()
                            .child(self.title.clone()),
                    )
                    .child(body),
            )
            .child(
                div()
                    .id("local-notification-close")
                    .debug_selector(move || {
                        if alert {
                            "host-message-toast-close"
                        } else {
                            "local-notification-close"
                        }
                        .into()
                    })
                    .flex_shrink_0()
                    .child(
                        Button::new("dismiss-local-notification")
                            .icon(IconName::Close)
                            .small()
                            .compact()
                            .ghost()
                            .tooltip(t!("notification.close").to_string())
                            .accessibility_label(t!("notification.close").to_string())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.dismissed = true;
                                cx.stop_propagation();
                                cx.notify();
                            })),
                    ),
            )
            .into_any_element()
    }
}
