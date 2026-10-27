//! Native hierarchical menus retain directory expansion, search IME and captured action identities.
use super::*;
use gpui_base::input::{InputEvent, InputState};
use gpui_kit::{AppContext as _, Entity, EntityInputHandler as _, Subscription};
use plugin_runtime::plugin_protocol::ui::{ActionMenuItem, ActionMenuKind};
use std::{collections::BTreeSet, sync::Arc};
#[cfg(test)]
mod tests;
mod view;

/// Submenu preparation keeps the popup open; only selection or dismissal ends its focus lease.
#[derive(Clone, Debug)]
pub enum ActionMenuEvent {
    /// Execute an explicitly selected action and dismiss the native popup.
    Select(String),
    /// Prepare this stable submenu under the publisher's current command authority.
    Open(String),
    /// Dismiss without executing any domain action.
    Dismiss,
}

/// One native popup combines inline directory groups and cascading action submenus.
/// The publisher supplies immutable rows; callbacks return only their stable action IDs.
pub struct ActionMenu {
    items: Vec<ActionMenuItem>,
    locale: String,
    icons: Arc<BTreeMap<String, Arc<[u8]>>>,
    style: MenuStyle,
    position: Point<Pixels>,
    focus: FocusHandle,
    previous: Option<FocusHandle>,
    closed: bool,
    expanded: BTreeMap<String, bool>,
    /// One stable parent ID per cascading column; unrelated publications truncate removed parents.
    cascade: Vec<String>,
    selected: BTreeMap<usize, String>,
    active_column: usize,
    scrolls: Vec<ScrollHandle>,
    search: Option<Entity<InputState>>,
    _subscription: Option<Subscription>,
    /// Native rows capture this generation so an old painted hit target cannot invoke new meaning.
    generation: u64,
    sink: Rc<dyn Fn(ActionMenuEvent, &mut Window, &mut App)>,
    pointer: Point<Pixels>,
    width: f32,
}

/// Flattened visible rows retain their provider identity and logical directory depth.
#[derive(Clone)]
struct Row {
    item: ActionMenuItem,
    depth: usize,
}

impl ActionMenu {
    /// Open `items` at a measured native position. `search` optionally supplies a localized
    /// placeholder. `icons` must already be runtime-admitted package artwork for this publisher.
    pub fn new(
        items: Vec<ActionMenuItem>,
        locale: String,
        icons: Arc<BTreeMap<String, Arc<[u8]>>>,
        position: Point<Pixels>,
        search: Option<String>,
        sink: impl Fn(ActionMenuEvent, &mut Window, &mut App) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let previous = window.focused(cx);
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let input = search
            .map(|placeholder| cx.new(|cx| InputState::new(window, cx).placeholder(placeholder)));
        let subscription = input.as_ref().map(|input| {
            cx.subscribe_in(
                input,
                window,
                |this, _, event: &InputEvent, window, cx| match event {
                    InputEvent::Change => {
                        this.cascade.clear();
                        this.active_column = 0;
                        this.selected.clear();
                        for scroll in &this.scrolls {
                            scroll.set_offset(Default::default());
                        }
                        cx.notify();
                    }
                    InputEvent::PressEnter { .. } => this.activate_selected(window, cx),
                    _ => {}
                },
            )
        });
        if let Some(input) = &input {
            input.read(cx).focus_handle(cx).focus(window, cx);
        }
        let mut this = Self {
            items,
            locale,
            icons,
            style: MenuStyle::current(cx),
            position,
            focus,
            previous,
            closed: false,
            expanded: Default::default(),
            cascade: Vec::new(),
            selected: Default::default(),
            active_column: 0,
            scrolls: vec![ScrollHandle::new()],
            search: input,
            _subscription: subscription,
            generation: 1,
            sink: Rc::new(sink),
            pointer: window.mouse_position(),
            width: 340.,
        };
        this.seed_groups();
        this
    }
    /// Bound this caller's menu width in logical pixels without duplicating native popup behavior.
    pub fn width(mut self, width: f32) -> Self {
        if width.is_finite() && width > 0. {
            self.width = width;
        }
        self
    }
    /// Update an open popup while preserving search and user expansion by stable identity.
    /// Callers retain their own publication generation gate before accepting any resulting action.
    pub fn replace(
        &mut self,
        items: Vec<ActionMenuItem>,
        icons: Arc<BTreeMap<String, Arc<[u8]>>>,
        cx: &mut Context<Self>,
    ) {
        self.items = items;
        self.icons = icons;
        self.generation = self.generation.wrapping_add(1);
        self.seed_groups();
        let all = self.all_ids();
        self.expanded.retain(|id, _| all.contains(id));
        self.cascade = self
            .cascade
            .iter()
            .take_while(|id| all.contains(*id))
            .cloned()
            .collect();
        self.selected.retain(|_, id| all.contains(id));
        self.active_column = self.active_column.min(self.cascade.len());
        cx.notify();
    }
    /// Close through the original focus lease; lifecycle withdrawal never steals another modal's focus.
    pub fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input_focused = self
            .search
            .as_ref()
            .is_some_and(|input| input.read(cx).focus_handle(cx).is_focused(window));
        if !self.focus.contains_focused(window, cx) && !input_focused {
            self.previous = None;
        }
        self.finish(ActionMenuEvent::Dismiss, window, cx);
    }
    /// Initial group defaults are applied once; refreshed metadata cannot undo a user's toggle.
    fn seed_groups(&mut self) {
        for item in &self.items {
            item.visit(&mut |item| {
                if let ActionMenuKind::Group { expanded, .. } = item.kind {
                    self.expanded.entry(item.id.clone()).or_insert(expanded);
                }
            });
        }
    }
    /// Build the bounded identity set without treating labels or arrival order as row identity.
    fn all_ids(&self) -> BTreeSet<String> {
        let mut ids = BTreeSet::new();
        for item in &self.items {
            item.visit(&mut |item| {
                ids.insert(item.id.clone());
            });
        }
        ids
    }
    /// Find the immutable source row, including collapsed groups and submenu descendants.
    fn item(&self, id: &str) -> Option<&ActionMenuItem> {
        fn find<'a>(items: &'a [ActionMenuItem], id: &str) -> Option<&'a ActionMenuItem> {
            items.iter().find_map(|item| {
                if item.id == id {
                    Some(item)
                } else {
                    find(item.children(), id)
                }
            })
        }
        find(&self.items, id)
    }
    /// Search the complete source menu; collapsed directories do not hide a matching descendant.
    fn rows(&self, column: usize, cx: &App) -> Vec<Row> {
        let query = if column == 0 {
            self.search
                .as_ref()
                .map(|input| input.read(cx).value().trim().to_lowercase())
                .unwrap_or_default()
        } else {
            String::new()
        };
        let source: &[ActionMenuItem] = if column == 0 {
            self.items.as_slice()
        } else {
            self.cascade
                .get(column - 1)
                .and_then(|id| self.item(id))
                .map_or(&[][..], |item| item.children())
        };
        let mut result = Vec::new();
        let disabled = self
            .cascade
            .iter()
            .take(column)
            .any(|id| self.item(id).is_some_and(|item| item.disabled));
        flatten_rows(
            source,
            0,
            &self.locale,
            &query,
            &self.expanded,
            disabled,
            &[],
            &mut result,
        );
        result
    }
    /// Resolve one enabled stable row; group/submenu clicks never execute their first child action.
    fn activate(&mut self, column: usize, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = self.item(id).cloned().filter(|item| !item.disabled) else {
            return;
        };
        if !self
            .rows(column, cx)
            .iter()
            .any(|row| row.item.id == id && !row.item.disabled)
        {
            return;
        }
        self.active_column = column;
        self.selected.insert(column, id.into());
        match item.kind {
            ActionMenuKind::Group { .. } => {
                let open = self.expanded.entry(id.into()).or_default();
                *open = !*open;
                self.cascade.truncate(column);
            }
            ActionMenuKind::Submenu { on_open, .. } => {
                self.cascade.truncate(column);
                self.cascade.push(id.into());
                if self.scrolls.len() <= column + 1 {
                    self.scrolls.push(ScrollHandle::new());
                }
                self.active_column = column + 1;
                self.select_first(column + 1, cx);
                if on_open.is_some() {
                    (self.sink)(ActionMenuEvent::Open(id.into()), window, cx);
                }
            }
            ActionMenuKind::Action { command } if !command.disabled => {
                self.finish(ActionMenuEvent::Select(id.into()), window, cx)
            }
            _ => {}
        }
        cx.notify();
    }
    /// Initialize only the active column's keyboard selection, keeping parent highlights intact.
    fn select_first(&mut self, column: usize, cx: &App) {
        if let Some(row) = self
            .rows(column, cx)
            .into_iter()
            .find(|row| !row.item.disabled)
        {
            self.selected.insert(column, row.item.id);
        }
    }
    /// Enter from either the search input or native action routing uses the same exact row.
    fn activate_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Base emits its submit event while native composition may still own Enter. The popup
        // must not interpret that IME confirmation as executing a menu command.
        if let Some(input) = self.search.clone() {
            if input.update(cx, |input, cx| {
                input.marked_text_range(window, cx).is_some()
            }) {
                return;
            }
        }
        let column = self.active_column;
        if !self.selected.contains_key(&column) {
            self.select_first(column, cx);
        }
        if let Some(id) = self.selected.get(&column).cloned() {
            self.activate(column, &id, window, cx);
        }
    }
    /// Navigation yields to native composition and keeps ordinary search caret movement intact.
    fn key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        if let Some(input) = self.search.clone() {
            if input.update(cx, |input, cx| {
                input.marked_text_range(window, cx).is_some()
            }) {
                return;
            }
            if !matches!(key, "up" | "down" | "home" | "end" | "escape") {
                return;
            }
        }
        cx.stop_propagation();
        match key {
            "escape" | "left" if !self.cascade.is_empty() => {
                self.cascade.pop();
                self.active_column = self.cascade.len();
                cx.notify();
            }
            "escape" => self.finish(ActionMenuEvent::Dismiss, window, cx),
            "enter" | "space" => self.activate_selected(window, cx),
            "right" => {
                if !self.selected.contains_key(&self.active_column) {
                    self.select_first(self.active_column, cx);
                }
                if self
                    .selected
                    .get(&self.active_column)
                    .and_then(|id| self.item(id))
                    .is_some_and(|item| !matches!(item.kind, ActionMenuKind::Action { .. }))
                {
                    self.activate_selected(window, cx);
                }
            }
            "up" | "down" | "home" | "end" => {
                let column = self.active_column;
                let rows = self
                    .rows(column, cx)
                    .into_iter()
                    .enumerate()
                    .filter(|(_, row)| !row.item.disabled)
                    .collect::<Vec<_>>();
                if rows.is_empty() {
                    return;
                }
                let old = rows
                    .iter()
                    .position(|(_, row)| self.selected.get(&column) == Some(&row.item.id));
                let index = match key {
                    "home" => 0,
                    "end" => rows.len() - 1,
                    "up" => old.map_or(rows.len() - 1, |i| (i + rows.len() - 1) % rows.len()),
                    _ => old.map_or(0, |i| (i + 1) % rows.len()),
                };
                self.selected.insert(column, rows[index].1.item.id.clone());
                self.scrolls[column].scroll_to_item(rows[index].0);
                cx.notify();
            }
            _ => {}
        }
    }
    /// Base consumes fixed list keys as input actions; route them before search, retaining IME ownership.
    /// GPUI's static parser accepts these platform-independent navigation names.
    fn search_key(&mut self, key: &'static str, window: &mut Window, cx: &mut Context<Self>) {
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
    /// Restore the originating native focus once and publish exactly one terminal menu action.
    fn finish(&mut self, action: ActionMenuEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.closed {
            return;
        }
        self.closed = true;
        if let Some(previous) = self.previous.take() {
            previous.focus(window, cx);
        }
        (self.sink)(action, window, cx);
        cx.emit(gpui_kit::DismissEvent);
        cx.notify();
    }
}
impl gpui_kit::Focusable for ActionMenu {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl gpui_kit::EventEmitter<gpui_kit::DismissEvent> for ActionMenu {}

/// Searching projects descendants of every submenu and keeps their original action identity.
/// Breadcrumbs disambiguate identical labels from separate overflow entries or branch groups.
fn flatten_rows(
    items: &[ActionMenuItem],
    depth: usize,
    locale: &str,
    query: &str,
    expanded: &BTreeMap<String, bool>,
    ancestor_disabled: bool,
    parents: &[String],
    out: &mut Vec<Row>,
) {
    for source in items {
        let mut item = source.clone();
        item.disabled |= ancestor_disabled;
        let label = item.label.for_locale(locale).to_owned();
        let matches = label.to_lowercase().contains(query)
            || item
                .detail
                .as_ref()
                .is_some_and(|text| text.for_locale(locale).to_lowercase().contains(query));
        if !query.is_empty() && !parents.is_empty() {
            let breadcrumb = parents.join(" / ");
            item.detail = Some(plugin_runtime::plugin_protocol::ui::LocalizedText::new(
                breadcrumb.clone(),
                breadcrumb,
            ));
        }
        match &source.kind {
            ActionMenuKind::Group { items, .. } | ActionMenuKind::Submenu { items, .. }
                if !query.is_empty() =>
            {
                let mut descendants = Vec::new();
                let mut path = parents.to_vec();
                path.push(label);
                flatten_rows(
                    items,
                    depth + 1,
                    locale,
                    query,
                    expanded,
                    item.disabled,
                    &path,
                    &mut descendants,
                );
                if matches || !descendants.is_empty() {
                    out.push(Row { item, depth });
                    out.extend(descendants);
                }
            }
            ActionMenuKind::Group { items, .. } => {
                let open = expanded.get(&item.id).copied().unwrap_or(false);
                let disabled = item.disabled;
                out.push(Row { item, depth });
                if open {
                    flatten_rows(
                        items,
                        depth + 1,
                        locale,
                        query,
                        expanded,
                        disabled,
                        parents,
                        out,
                    );
                }
            }
            _ if query.is_empty() || matches => out.push(Row { item, depth }),
            _ => {}
        }
    }
}
