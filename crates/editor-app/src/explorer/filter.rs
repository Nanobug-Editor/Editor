//! Transient, case-insensitive name filtering over the existing workspace tree.

use super::tree::root_items;
use crate::*;
use editor_ui::Input;
use gpui_base::input::InputState;

/// Keeps one native input and the original hierarchy while a filtered projection is displayed.
pub(crate) struct ExplorerFilter {
    /// Base owns the query's text, focus, selection and IME composition.
    input: Entity<InputState>,
    /// Visibility is session-local and starts hidden for every workspace window.
    pub(crate) visible: bool,
    /// Hover admission includes empty tree space without moving keyboard focus.
    pub(crate) body_hovered: bool,
    /// The dock title is a separate native hover surface above the tree.
    pub(crate) title_hovered: bool,
    /// Lowercase query applied to the displayed projection; empty means ordinary browsing.
    query: String,
    /// Original shared expansion state is isolated from the projection's newly created items.
    original: Option<Vec<TreeItem>>,
    /// Retain query updates for this workspace's lifetime.
    _changed: Subscription,
}

impl ExplorerFilter {
    /// Create the workspace's hidden input in `window`, subscribing through its owning app `cx`.
    pub(crate) fn new(window: &mut Window, cx: &mut Context<EditorApp>) -> Self {
        let input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(t!("explorer.filter_placeholder").to_string())
        });
        let changed = cx.subscribe(&input, |app, input, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                let query = input.read(cx).value().to_lowercase();
                app.apply_explorer_filter(query, cx);
            }
        });
        Self {
            input,
            visible: false,
            body_hovered: false,
            title_hovered: false,
            query: String::new(),
            original: None,
            _changed: changed,
        }
    }

    /// Report whether the live tree is currently a temporary filtered projection.
    pub(crate) fn is_active(&self) -> bool {
        !self.query.is_empty()
    }

    /// Project freshly scanned `items`, retaining their ordinary expansion state for restoration.
    pub(crate) fn project(&mut self, items: Vec<TreeItem>) -> Vec<TreeItem> {
        if self.is_active() {
            let filtered = filter_items(&items, &self.query);
            self.original = Some(items);
            filtered
        } else {
            items
        }
    }
}

/// Match names in `items` against lowercase `query`, retaining ancestors and matched folder contents.
/// New items prevent temporary expansion from mutating the original shared TreeItem state.
fn filter_items(items: &[TreeItem], query: &str) -> Vec<TreeItem> {
    /// Retain `item` when its name, an ancestor (`inherited`), or a descendant matches `query`.
    /// `root` keeps the workspace heading without treating its name as a subtree-wide match.
    fn visit(item: &TreeItem, query: &str, root: bool, inherited: bool) -> Option<TreeItem> {
        // The workspace root remains a heading; its name must not make every descendant match.
        let matched = inherited || (!root && item.label.to_lowercase().contains(query));
        let children = item
            .children
            .iter()
            .filter_map(|child| visit(child, query, false, matched))
            .collect::<Vec<_>>();
        (root || matched || !children.is_empty()).then(|| {
            TreeItem::new(item.id.clone(), item.label.clone())
                .children(children)
                .expanded(true)
                .disabled(item.is_disabled())
        })
    }
    items
        .iter()
        .filter_map(|item| visit(item, query, true, false))
        .collect()
}

impl EditorApp {
    /// Replace the live projection with lowercase `query`, without filesystem reads or layout saves.
    fn apply_explorer_filter(&mut self, query: String, cx: &mut Context<Self>) {
        if query == self.explorer_filter.query {
            return;
        }
        if !self.explorer_filter.is_active() {
            self.capture_explorer_state(cx);
            self.explorer_filter.original = Some(root_items(self.tree_state.read(cx)));
        }
        self.explorer_filter.query = query;
        let items = if self.explorer_filter.is_active() {
            filter_items(
                self.explorer_filter.original.as_deref().unwrap_or_default(),
                &self.explorer_filter.query,
            )
        } else {
            self.explorer_filter.original.take().unwrap_or_default()
        };
        self.tree_state.update(cx, |state, cx| {
            let selected = state.selected_item().map(|item| item.id.clone());
            state.set_items(items, cx);
            // Restoring a collapsed hierarchy must not reveal a previously hidden descendant.
            let index = selected.as_ref().and_then(|id| state.index_of(id));
            state.set_selected_index(index, cx);
            state.scroll_to_item(index.unwrap_or(0), ScrollStrategy::Top);
        });
        cx.notify();
    }

    /// Toggle the compact input and move focus to it on opening, or back to the tree on closing.
    pub(crate) fn toggle_explorer_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.explorer_filter.visible {
            self.hide_explorer_filter(window, cx);
        } else {
            self.explorer_filter.visible = true;
            self.explorer_filter
                .input
                .read(cx)
                .focus_handle(cx)
                .focus(window, cx);
            cx.notify();
        }
    }

    /// Clear the temporary filter and return input-owned focus to the tree in `window`.
    pub(crate) fn hide_explorer_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.explorer_filter.visible {
            return;
        }
        self.explorer_filter.visible = false;
        self.apply_explorer_filter(String::new(), cx);
        let focused = self
            .explorer_filter
            .input
            .read(cx)
            .focus_handle(cx)
            .is_focused(window);
        self.explorer_filter.input.update(cx, |input, cx| {
            input.set_value("", window, cx);
        });
        if focused {
            self.tree_state
                .update(cx, |tree, cx| tree.focus(window, cx));
        }
        cx.notify();
    }

    /// Consume Ctrl+F only for a visible Explorer's focused or hovered scope in `window`.
    /// Return whether `key` toggled the filter, allowing the shared resolver to stop dispatch.
    pub(crate) fn handle_explorer_filter_shortcut(
        &mut self,
        key: &gpui_kit::Keystroke,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let modifiers = key.modifiers;
        if key.key != "f"
            || !modifiers.control
            || modifiers.alt
            || modifiers.shift
            || modifiers.platform
        {
            return false;
        }
        if !self.explorer_visible
            || self.explorer_edit.is_some()
            || self.explorer_delete.is_some()
            || self.explorer_menu.is_some()
            || self.shortcut_panel.is_some()
        {
            return false;
        }
        let focused = self
            .explorer_panel
            .focus_handle(cx)
            .contains_focused(window, cx);
        if !focused && !self.explorer_filter.body_hovered && !self.explorer_filter.title_hovered {
            return false;
        }
        self.toggle_explorer_filter(window, cx);
        true
    }

    /// Render the optional input above the tree, preserving Base editing and keyboard semantics.
    pub(crate) fn render_explorer_filter(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<gpui_kit::AnyElement> {
        self.explorer_filter.visible.then(|| {
            div()
                .id("explorer-filter")
                .debug_selector(|| "explorer-filter".into())
                .flex_shrink_0()
                .p_1()
                .capture_key_down(
                    cx.listener(|app, event: &gpui_kit::KeyDownEvent, window, cx| {
                        if event.keystroke.key == "escape" {
                            app.hide_explorer_filter(window, cx);
                            cx.stop_propagation();
                        } else if event.keystroke.key == "down" {
                            app.tree_state.update(cx, |tree, cx| {
                                tree.set_selected_index(tree.entry(1).map(|_| 1), cx);
                                tree.focus(window, cx);
                            });
                            cx.stop_propagation();
                        }
                    }),
                )
                .child(
                    Input::new(&self.explorer_filter.input)
                        .small()
                        .accessibility_label(t!("explorer.filter_placeholder").to_string()),
                )
                .into_any_element()
        })
    }
}

#[cfg(test)]
mod tests;

#[cfg(all(test, windows))]
mod native_tests;
