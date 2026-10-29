//! Bounded native message history; the host publishes receipts and owns window routing.
use crate::dock::{self, Panel as DockPanel};
use crate::{Button, Icon, theme::ActiveTheme as _};
use gpui_base::dock::PanelEvent;
use gpui_base::{h_flex, v_flex};
use gpui_kit::assets::IconName;
use gpui_kit::{
    App, Context, EventEmitter, FocusHandle, Focusable, Hsla, InteractiveElement as _, IntoElement,
    ParentElement, Render, ScrollHandle, SharedString, StatefulInteractiveElement as _, Styled,
    WeakEntity, Window, div, prelude::FluentBuilder as _,
};
use rust_i18n::t;
use std::collections::VecDeque;

/// Severity of one caller-published receipt, independent of its translated wording.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum MessageLevel {
    /// An informational receipt uses the least urgent, blue reminder.
    Info,
    /// A warning takes priority over information and uses an orange reminder.
    Warning,
    /// A dangerous or failed result takes priority over all other receipts and uses red.
    Error,
}

impl MessageLevel {
    /// Resolve this reminder's blue/orange/red meaning against the active light or dark palette in `cx`.
    pub fn reminder_color(self, cx: &App) -> Hsla {
        let dark = cx.theme().is_dark();
        match self {
            Self::Info => gpui_kit::rgb(if dark { 0x60a5fa } else { 0x2563eb }).into(),
            // Tiny reminders need amber's yellow component to remain distinct from danger red.
            Self::Warning => gpui_kit::rgb(if dark { 0xfbbf24 } else { 0xf59e0b }).into(),
            Self::Error => cx.theme().danger,
        }
    }
}

/// Current-run capacity and progressive disclosure have distinct responsibilities.
const HISTORY_LIMIT: usize = 500;
const DISPLAY_BATCH: usize = 20;

/// Shape supplied by the user; the local icon renderer resolves its size and theme color.
pub fn notification_icon() -> Icon {
    Icon::default()
        .data(include_bytes!("../../assets/status-icons/notification.svg").as_slice())
        .small()
}

/// Immutable receipt identity makes chronological ties and retained rows deterministic.
struct Message {
    id: u64,
    level: MessageLevel,
    text: String,
    /// Captured caller attribution remains readable after a plugin is disabled or removed.
    source: Option<String>,
}

/// Owns one bounded history for the main window; session persistence stores only its presentation.
pub struct MessagePanel {
    visible: bool,
    focus: FocusHandle,
    records: VecDeque<Message>,
    /// Highest unacknowledged severity; only an explicit open or clear resets it, never painting or eviction.
    pending_alert: Option<MessageLevel>,
    next_id: u64,
    display_limit: usize,
    scroll: ScrollHandle,
    /// Base provides the current group through its lifecycle hook; hidden views otherwise lose render dependencies.
    group: Option<WeakEntity<gpui_base::dock::TabGroup>>,
}

impl MessagePanel {
    /// Start a fresh history regardless of saved layout; visibility is the only restored flag.
    pub fn new(visible: bool, cx: &mut Context<Self>) -> Self {
        Self {
            visible,
            focus: cx.focus_handle(),
            records: VecDeque::new(),
            pending_alert: None,
            next_id: 0,
            display_limit: DISPLAY_BATCH,
            scroll: ScrollHandle::new(),
            group: None,
        }
    }

    /// Append a user-facing host result, evicting only the oldest receipt above the global cap.
    pub fn push(&mut self, level: MessageLevel, text: String, cx: &mut Context<Self>) {
        self.push_attributed(level, text, None, cx);
    }

    /// Append `text` at `level` with optional immutable `source`, preserving the same global history cap.
    pub fn push_attributed(
        &mut self,
        level: MessageLevel,
        text: String,
        source: Option<String>,
        cx: &mut Context<Self>,
    ) {
        // Retain the highest receipt even after history eviction; lower levels cannot acknowledge it.
        self.pending_alert = self.pending_alert.max(Some(level));
        self.next_id = self
            .next_id
            .checked_add(1)
            .expect("host message identity exhausted");
        self.records.push_back(Message {
            id: self.next_id,
            level,
            text,
            source,
        });
        if self.records.len() > HISTORY_LIMIT {
            self.records.pop_front();
        }
        cx.notify();
    }

    /// Expose host-panel visibility to the dock and session, without exposing the mutable history.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// The status button reads only reminder state, independently of how many rows are disclosed.
    pub fn has_pending_alert(&self) -> bool {
        self.pending_alert.is_some()
    }

    /// Return the highest severity received since opening or clearing, or None after acknowledgement.
    pub fn pending_level(&self) -> Option<MessageLevel> {
        self.pending_alert
    }

    /// Synchronous acknowledgement cannot later erase a receipt arriving after this user action.
    pub fn acknowledge(&mut self, cx: &mut Context<Self>) {
        self.pending_alert = None;
        cx.notify();
    }

    /// Clear this window's receipts and reminder synchronously; identities remain monotonic for later results.
    /// Original plugin log storage and its import cursor remain untouched by this presentation action.
    fn clear(&mut self, cx: &mut Context<Self>) {
        self.records.clear();
        self.display_limit = DISPLAY_BATCH;
        self.scroll.set_offset(Default::default());
        self.pending_alert = None;
        cx.notify();
    }

    /// Visibility changes publish a layout event so the shell persists and reclaims occupied space.
    pub fn toggle(&mut self, cx: &mut Context<Self>) {
        self.set_visible(!self.visible, cx);
    }

    /// Opening a closed dock may retain the same panel flag; Base owns the region's separate state.
    pub fn set_visible(&mut self, visible: bool, cx: &mut Context<Self>) {
        if self.visible == visible {
            return;
        }
        self.visible = visible;
        if let Some(group) = self.group.as_ref().and_then(WeakEntity::upgrade) {
            group.update(cx, |_, cx| cx.notify());
        }
        cx.emit(PanelEvent::LayoutChanged);
        cx.notify();
    }
}

impl EventEmitter<PanelEvent> for MessagePanel {}
impl Focusable for MessagePanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl dock::BasePanel for MessagePanel {
    fn panel_name(&self) -> &'static str {
        "HostMessages"
    }
    fn closable(&self, _: &App) -> bool {
        false
    }
    fn zoomable(&self, _: &App) -> bool {
        false
    }
    fn visible(&self, _: &App) -> bool {
        self.visible
    }
    fn on_added_to(
        &mut self,
        group: WeakEntity<gpui_base::dock::TabGroup>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) {
        self.group = Some(group);
    }
    fn on_removed(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.group = None;
    }
}
impl DockPanel for MessagePanel {
    /// The drag card names this panel without duplicating its toolbar or list.
    fn drag_title(&self, _: &App) -> Option<gpui_kit::SharedString> {
        Some(t!("messages.title").to_string().into())
    }

    fn title(&mut self, _: bool, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .w_full()
            .justify_between()
            .items_center()
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(notification_icon())
                    .child(t!("messages.title").to_string()),
            )
            .child(
                h_flex()
                    .gap_1()
                    .child(
                        div()
                            .id("host-messages-clear")
                            .debug_selector(|| "host-messages-clear".into())
                            .child(
                                Button::new("clear-host-messages")
                                    // The catalog broom keeps the clear action compact in either theme.
                                    .icon(
                                        Icon::default()
                                            .path(gpui_kit::assets::IconName::Broom.path())
                                            .small(),
                                    )
                                    .tooltip(t!("messages.clear").to_string())
                                    .accessibility_label(t!("messages.clear").to_string())
                                    .small()
                                    .compact()
                                    .ghost()
                                    .disabled(self.records.is_empty())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.clear(cx);
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .id("host-messages-hide")
                            .debug_selector(|| "host-messages-hide".into())
                            .child(
                                Button::new("hide-host-messages")
                                    .icon(IconName::WindowMinimize)
                                    .small()
                                    .compact()
                                    .ghost()
                                    .tooltip(t!("messages.hide").to_string())
                                    .accessibility_label(t!("messages.hide").to_string())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.toggle(cx);
                                    })),
                            ),
                    ),
            )
    }
}
impl Render for MessagePanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut list = v_flex()
            .id("host-message-list")
            .flex_1()
            .min_h_0()
            .min_w_0()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .p_3()
            .gap_3();
        if self.records.is_empty() {
            list = list.child(
                div()
                    .debug_selector(|| "host-messages-empty".into())
                    .text_color(cx.theme().muted_foreground)
                    .child(t!("messages.empty").to_string()),
            );
        }
        for message in self.records.iter().rev().take(self.display_limit) {
            let (label, badge, selector) = match message.level {
                MessageLevel::Info => (t!("messages.info").to_string(), None, "host-message-info"),
                MessageLevel::Warning => (
                    t!("messages.warning").to_string(),
                    Some(crate::StatusIcon::Warning),
                    "host-message-warning",
                ),
                MessageLevel::Error => (
                    t!("messages.error").to_string(),
                    Some(crate::StatusIcon::Error),
                    "host-message-error",
                ),
            };
            let id = message.id;
            list = list.child(
                v_flex()
                    .id(SharedString::from(format!("host-message-{id}")))
                    .w_full()
                    .min_w_0()
                    .debug_selector(move || format!("host-message-{id}"))
                    .gap_1()
                    .pb_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .when_some(badge, |row, badge| row.child(badge.icon(cx)))
                            .child(div().debug_selector(move || selector.into()).child(label)),
                    )
                    .when_some(message.source.as_ref(), |row, source| {
                        row.child(
                            div()
                                .debug_selector(move || format!("host-message-source-{id}"))
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(source.clone()),
                        )
                    })
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().foreground)
                            .child(message.text.clone()),
                    ),
            );
        }
        // Expansion is an action on this list, not a replacement page or a second copy of messages.
        if self.records.len() > self.display_limit {
            list = list.child(
                div()
                    .id("host-messages-more")
                    .debug_selector(|| "host-messages-more".into())
                    .child(
                        Button::new("more-host-messages")
                            .label(t!("messages.more").to_string())
                            .accessibility_label(t!("messages.more").to_string())
                            .ghost()
                            .small()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.display_limit =
                                    (this.display_limit + DISPLAY_BATCH).min(HISTORY_LIMIT);
                                cx.notify();
                            })),
                    ),
            );
        }
        v_flex()
            .id("host-messages-panel")
            .debug_selector(|| "host-messages-panel".into())
            .track_focus(&self.focus)
            .size_full()
            .min_w_0()
            .min_h_0()
            .bg(cx.theme().background)
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .child(super::scroll::SmoothScroll::new(
                        "host-messages-wheel",
                        &self.scroll,
                        super::scroll::ScrollAxis::Vertical,
                        list,
                    ))
                    .child(crate::vertical_scrollbar(&self.scroll, cx)),
            )
    }
}
