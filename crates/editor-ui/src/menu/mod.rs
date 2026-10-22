//! Shared explorer-style menu appearance and a native popup with keyboard navigation.
use crate::theme::ActiveTheme as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, Context, Div, ElementId, FocusHandle, Focusable, FontWeight, Hsla, InteractiveElement,
    IntoElement, KeyDownEvent, MouseButton, ParentElement, Pixels, Point, Render, Role,
    ScrollHandle, SharedString, StatefulInteractiveElement, Styled, Window, anchored, deferred,
    div, point, px,
};
use plugin_runtime::plugin_protocol::ui::{Action, MenuItem};
use std::collections::BTreeMap;
use std::rc::Rc;
mod action_menu;
#[cfg(test)]
mod tests;
mod view;
pub use action_menu::{ActionMenu, ActionMenuEvent};

/// Resolved local menu colors, spacing and typography shared by native popup renderers.
#[derive(Clone, PartialEq)]
pub struct MenuStyle {
    pub surface: Hsla,
    pub foreground: Hsla,
    pub border: Hsla,
    pub hover: Hsla,
    pub hover_foreground: Hsla,
    pub radius: f32,
    pub font_size: f32,
    pub padding_x: f32,
    pub padding_y: f32,
    pub font_family: SharedString,
    pub bold: bool,
}
impl MenuStyle {
    /// Resolve the menu appearance from the initialized product theme in cx.
    pub fn current(cx: &App) -> Self {
        let styles =
            crate::theme::component_styles(cx, plugin_schema::ThemeComponent::ExplorerMenu);
        Self {
            surface: styles.base.background.unwrap_or(cx.theme().popover),
            foreground: styles
                .base
                .foreground
                .unwrap_or(cx.theme().popover_foreground),
            border: styles.base.border.unwrap_or(cx.theme().border),
            hover: styles.hover.background.unwrap_or(cx.theme().list_hover),
            hover_foreground: styles.hover.foreground.unwrap_or(cx.theme().foreground),
            radius: styles.base.radius_px.unwrap_or(6.),
            font_size: styles.base.font_size_px.unwrap_or(13.),
            padding_x: styles.base.padding_x_px.unwrap_or(6.),
            padding_y: styles.base.padding_y_px.unwrap_or(5.),
            font_family: cx.theme().font_family.clone(),
            bold: false,
        }
    }
    /// Build a local popup surface with logical-pixel width and this resolved appearance.
    pub fn card(&self, width: f32) -> Div {
        div()
            .flex()
            .flex_col()
            .w(px(width))
            .p(px(self.padding_y))
            .rounded(px(self.radius))
            .border_1()
            .border_color(self.border)
            .bg(self.surface)
            .text_color(self.foreground)
            .font_family(self.font_family.clone())
            .text_size(px(self.font_size))
            .font_weight(if self.bold {
                FontWeight::BOLD
            } else {
                FontWeight::NORMAL
            })
            .shadow_md()
    }
    /// Build a Base menu row with stable id, semantic label and owner-selected appearance.
    pub fn row(
        &self,
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        selected: bool,
    ) -> gpui_base::Button {
        // Buttons own their text style so submenu triggers match ordinary menu rows.
        gpui_base::Button::new(id)
            .role(Role::MenuItem)
            .accessibility_label(label)
            .focusable(false)
            .flex()
            .h(px(29.))
            .w_full()
            .items_center()
            .font_family(self.font_family.clone())
            .text_size(px(self.font_size))
            .font_weight(if self.bold {
                FontWeight::BOLD
            } else {
                FontWeight::NORMAL
            })
            .px(px(self.padding_x + 6.))
            .rounded(px((self.radius - 2.).max(2.)))
            .bg(if selected { self.hover } else { self.surface })
            .text_color(if selected {
                self.hover_foreground
            } else {
                self.foreground
            })
    }
}

/// Local row presentations retain the ordinary popup's identity and interaction model.
#[derive(Clone)]
pub enum MenuRowContent {
    /// A non-interactive group label; callers also mark its MenuItem disabled for navigation.
    Heading,
    /// A two-line selectable row with an optional current-selection check.
    Detail { secondary: String, checked: bool },
}

/// The popup owns focus/navigation/scrolling; callers receive stable item identities.
pub struct PopupMenu {
    pub items: Vec<MenuItem>,
    pub style: MenuStyle,
    pub position: Point<Pixels>,
    focus: FocusHandle,
    previous: Option<FocusHandle>,
    selected: Option<usize>,
    closed: bool,
    scroll: ScrollHandle,
    /// The caller chooses a local layout width; actual rendering still clamps it to the viewport.
    width: f32,
    /// Optional local height limit keeps navigation dropdowns compact without truncating records.
    max_height: f32,
    row_content: BTreeMap<String, MenuRowContent>,
    /// Optional trailing actions stay reachable while only the preceding items scroll.
    footer_items: usize,
    /// Dropdowns retain their trigger's lower edge rather than moving above it when long.
    below_anchor: bool,
    /// Scroll-induced hover changes must not replace a keyboard selection with a stationary pointer.
    pointer_position: Point<Pixels>,
    /// Generic row diagnostics supplied by a caller; keyboard selection remains available for repair.
    notices: std::collections::BTreeMap<String, String>,
    sink: Rc<dyn Fn(Action, &mut Window, &mut App)>,
    /// Search remains a Base input; the immutable snapshot is filtered without losing row identity.
    search: Option<gpui_kit::Entity<gpui_base::input::InputState>>,
    search_subscription: Option<gpui_kit::Subscription>,
    source_items: Vec<MenuItem>,
    /// Secondary actions do not select their containing row or dismiss the popup.
    row_actions: BTreeMap<String, MenuItem>,
}
impl PopupMenu {
    /// Align a dropdown with the trigger's lower-right corner in window coordinates.
    /// Call on measured layout changes as well as opening, so dock moves do not leave a stale popup.
    pub fn anchor_to(&mut self, trigger: gpui_kit::Bounds<Pixels>, cx: &mut Context<Self>) {
        let position = point(trigger.right() - px(self.width), trigger.bottom());
        if self.position != position || !self.below_anchor {
            self.position = position;
            self.below_anchor = true;
            cx.notify();
        }
    }
    /// Focus items styled by style at window-local position; sink receives selected actions in window/cx.
    pub fn new(
        items: Vec<MenuItem>,
        style: MenuStyle,
        position: Point<Pixels>,
        sink: impl Fn(Action, &mut Window, &mut App) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let previous = window.focused(cx);
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let selected = items.iter().position(|item| !item.disabled);
        Self {
            items,
            style,
            position,
            focus,
            previous,
            selected,
            closed: false,
            scroll: ScrollHandle::new(),
            width: 230.,
            max_height: f32::INFINITY,
            row_content: BTreeMap::new(),
            footer_items: 0,
            below_anchor: false,
            pointer_position: window.mouse_position(),
            notices: Default::default(),
            sink: Rc::new(sink),
            search: None,
            search_subscription: None,
            source_items: Vec::new(),
            row_actions: BTreeMap::new(),
        }
    }
    fn finish(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        if self.closed {
            return;
        }
        self.closed = true;
        if let Some(focus) = self.previous.take() {
            focus.focus(window, cx);
        }
        (self.sink)(action, window, cx);
        cx.emit(gpui_kit::DismissEvent);
        cx.notify();
    }
    /// Retire an owned popup through its normal dismissal gate. Restore the origin only
    /// while this popup owns focus; external lifecycle cleanup must not steal a modal's keys.
    pub fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.focus.contains_focused(window, cx) {
            self.previous = None;
        }
        self.finish(Action::Dismiss, window, cx);
    }
    /// Configure width without duplicating popup focus, scroll or button behavior in callers.
    pub fn width(mut self, width: f32) -> Self {
        if width.is_finite() && width > 0. {
            self.width = width;
        }
        self
    }
    /// Add local group labels and two-line rows without changing public plugin menu contracts.
    pub fn row_content(mut self, content: BTreeMap<String, MenuRowContent>) -> Self {
        self.row_content = content;
        self
    }
    /// Add a searchable header while retaining Base IME and the popup's original focus lease.
    pub fn searchable(
        mut self,
        input: gpui_kit::Entity<gpui_base::input::InputState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        self.source_items = self.items.clone();
        self.search_subscription = Some(cx.subscribe_in(
            &input,
            window,
            |this, _, event: &gpui_base::input::InputEvent, window, cx| {
                match event {
                    gpui_base::input::InputEvent::Change => this.filter(cx),
                    gpui_base::input::InputEvent::PressEnter { .. } => {
                        if let Some(item) = this
                            .selected
                            .and_then(|index| this.items.get(index))
                            .filter(|item| !item.disabled)
                        {
                            this.finish(Action::Select(item.id.clone()), window, cx);
                        }
                    }
                    _ => {}
                }
            },
        ));
        input.read(cx).focus_handle(cx).focus(window, cx);
        self.search = Some(input);
        self.filter(cx);
        self
    }
    /// Keep auxiliary row actions outside the primary button's hit target.
    pub fn row_actions(&mut self, actions: BTreeMap<String, MenuItem>, cx: &mut Context<Self>) {
        self.row_actions = actions;
        cx.notify();
    }
    /// Filter only the scrollable records; pinned management commands remain available.
    fn filter(&mut self, cx: &mut Context<Self>) {
        let Some(input) = &self.search else {
            return;
        };
        let query = input.read(cx).text().to_string().trim().to_lowercase();
        let split = self.source_items.len().saturating_sub(self.footer_items);
        let selected = self
            .selected
            .and_then(|index| self.items.get(index))
            .map(|item| item.id.clone());
        self.items = self.source_items.iter().enumerate().filter(|(index, item)| {
            *index >= split || matches!(self.row_content.get(&item.id), Some(MenuRowContent::Heading))
                || item.label.to_lowercase().contains(&query)
                || matches!(self.row_content.get(&item.id), Some(MenuRowContent::Detail { secondary, .. }) if secondary.to_lowercase().contains(&query))
        }).map(|(_, item)| item.clone()).collect();
        self.selected = selected
            .and_then(|id| {
                self.items
                    .iter()
                    .position(|item| item.id == id && !item.disabled)
            })
            .or_else(|| self.items.iter().position(|item| !item.disabled));
        self.scroll.set_offset(Default::default());
        cx.notify();
    }
    /// Bound the full card height in logical pixels; invalid limits retain the viewport-only bound.
    pub fn max_height(mut self, height: f32) -> Self {
        if height.is_finite() && height > 0. {
            self.max_height = height;
        }
        self
    }
    /// Replace a live snapshot while retaining focus and selection by identity. Removed selections
    /// fall back to the first action; minimal scrolling keeps that highlight visible after layout.
    pub fn replace_rows(
        &mut self,
        items: Vec<MenuItem>,
        content: BTreeMap<String, MenuRowContent>,
        cx: &mut Context<Self>,
    ) {
        let selected = self
            .selected
            .and_then(|index| self.items.get(index))
            .map(|item| item.id.clone());
        self.selected = selected
            .and_then(|id| {
                items
                    .iter()
                    .position(|item| item.id == id && !item.disabled)
            })
            .or_else(|| items.iter().position(|item| !item.disabled));
        self.items = items;
        self.row_content = content;
        if self.search.is_some() {
            self.source_items = self.items.clone();
            self.filter(cx);
        }
        if let Some(index) = self
            .selected
            .filter(|index| *index < self.items.len().saturating_sub(self.footer_items))
        {
            // The new row geometry is resolved during prepaint; old offsets cannot follow a row
            // moving between groups or disappearing in a peer window.
            self.scroll.scroll_to_item(index);
        }
        cx.notify();
    }
    /// Content determines its actual row height, including fixed footers and viewport clipping.
    fn row_height(&self, item: &MenuItem) -> f32 {
        match self.row_content.get(&item.id) {
            Some(MenuRowContent::Heading) => 26.,
            Some(MenuRowContent::Detail { .. }) => 50.,
            None => 29.,
        }
    }
    /// Pin a bounded number of final actions; callers still use one identity/navigation list.
    pub fn fixed_footer(mut self, count: usize) -> Self {
        self.footer_items = count.min(self.items.len());
        self
    }
    /// Keep the menu directly below its supplied trigger position, constraining body height instead.
    pub fn below_anchor(mut self) -> Self {
        self.below_anchor = true;
        self
    }
    fn key(&mut self, key: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(input) = &self.search {
            use gpui_kit::EntityInputHandler as _;
            let composing = input.update(cx, |state, cx| {
                state.marked_text_range(window, cx).is_some()
            });
            if composing
                || !matches!(
                    key.keystroke.key.as_str(),
                    "escape" | "up" | "down" | "home" | "end"
                )
            {
                return;
            }
        }
        cx.stop_propagation();
        match key.keystroke.key.as_str() {
            "escape" => self.finish(Action::Dismiss, window, cx),
            "enter" | "space" => {
                if let Some(item) = self
                    .selected
                    .and_then(|i| self.items.get(i))
                    .filter(|i| !i.disabled)
                {
                    self.finish(Action::Select(item.id.clone()), window, cx);
                }
            }
            "up" | "down" | "home" | "end" => {
                let enabled: Vec<_> = self
                    .items
                    .iter()
                    .enumerate()
                    .filter_map(|(i, item)| (!item.disabled).then_some(i))
                    .collect();
                if enabled.is_empty() {
                    return;
                }
                let old = enabled
                    .iter()
                    .position(|i| Some(*i) == self.selected)
                    .unwrap_or(0);
                let index = match key.keystroke.key.as_str() {
                    "home" => 0,
                    "end" => enabled.len() - 1,
                    "up" => (old + enabled.len() - 1) % enabled.len(),
                    _ => (old + 1) % enabled.len(),
                };
                self.selected = Some(enabled[index]);
                if enabled[index] < self.items.len().saturating_sub(self.footer_items) {
                    self.scroll.scroll_to_item(enabled[index]);
                }
                cx.notify();
            }
            _ => {}
        }
    }
    /// Base single-line bindings consume arrow/Home/End actions before raw key events.
    /// Capture only list navigation while search owns focus; composition retains the input's actions.
    fn search_navigation(
        &mut self,
        key: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use gpui_kit::EntityInputHandler as _;
        let Some(input) = self.search.clone() else {
            cx.propagate();
            return;
        };
        if input.update(cx, |state, cx| {
            state.marked_text_range(window, cx).is_some()
        }) {
            cx.propagate();
            return;
        }
        self.key(
            &KeyDownEvent {
                keystroke: gpui_kit::Keystroke::parse(key).unwrap(),
                is_held: false,
                prefer_character_input: false,
            },
            window,
            cx,
        );
    }
    /// Color diagnostic rows with the local danger token and expose their full reason on hover.
    pub fn notices(mut self, notices: std::collections::BTreeMap<String, String>) -> Self {
        self.notices = notices;
        self
    }
}

/// The shell can retain focus and dismiss a superseded popup by its own entity identity.
impl gpui_kit::Focusable for PopupMenu {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl gpui_kit::EventEmitter<gpui_kit::DismissEvent> for PopupMenu {}
