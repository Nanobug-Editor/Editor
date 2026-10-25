//! The Open Files dock leaf projects the editor's existing document tabs without owning documents.

use crate::app::editor_preferences::TabPlacement;
use crate::*;

#[cfg(test)]
mod tests;

/// A native tool window retains only presentation state and a weak link to its document owner.
pub(crate) struct OpenFilesPanel {
    /// The application remains the sole owner of documents, comparison views and their order.
    parent: WeakEntity<EditorApp>,
    /// Effective visibility combines the global placement with this workspace's saved intent.
    visible: bool,
    /// Dock activation gives keys to this list without selecting a different document.
    focus: FocusHandle,
    /// Native dock caching must repaint on document opening, edits, closing and reordering.
    _owner_observer: Subscription,
}

impl OpenFilesPanel {
    /// Bind `parent` and its initial effective `visible` state to a native panel in `cx`.
    pub(crate) fn new(
        parent: WeakEntity<EditorApp>,
        visible: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        let owner = parent
            .upgrade()
            .expect("Open Files belongs to a live editor");
        let observer = cx.observe(&owner, |_, _, cx| cx.notify());
        Self {
            parent,
            visible,
            focus: cx.focus_handle(),
            _owner_observer: observer,
        }
    }

    /// Update only the painted visibility projection; the owner keeps the saved preference.
    fn set_visible(&mut self, visible: bool, cx: &mut Context<Self>) -> bool {
        if self.visible == visible {
            return false;
        }
        self.visible = visible;
        cx.notify();
        true
    }
}

impl EventEmitter<PanelEvent> for OpenFilesPanel {}

impl Focusable for OpenFilesPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl dock::BasePanel for OpenFilesPanel {
    fn panel_name(&self) -> &'static str {
        "OpenFiles"
    }

    fn visible(&self, _: &App) -> bool {
        self.visible
    }

    fn closable(&self, _: &App) -> bool {
        false
    }

    fn zoomable(&self, _: &App) -> bool {
        false
    }
}

impl DockPanel for OpenFilesPanel {
    /// Dock drag cards share the localized heading while omitting its hide control.
    fn drag_title(&self, _: &App) -> Option<gpui_kit::SharedString> {
        Some(t!("open_files.title").to_string().into())
    }

    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let parent = self.parent.clone();
        h_flex()
            .w_full()
            .items_center()
            .justify_between()
            .child(
                h_flex()
                    .flex_1()
                    .min_w(px(0.))
                    .gap_2()
                    .items_center()
                    .child(Icon::new(IconName::Files).small())
                    .child(div().truncate().child(t!("open_files.title").to_string())),
            )
            .child(
                Button::new("open-files-hide")
                    .debug_selector(|| "open-files-hide".into())
                    .icon(Icon::new(IconName::WindowMinimize))
                    .small()
                    .compact()
                    .ghost()
                    .tooltip(t!("open_files.hide").to_string())
                    .accessibility_label(t!("open_files.hide").to_string())
                    .on_click(move |_, window, cx| {
                        cx.stop_propagation();
                        // The App callback can notify this panel after its title borrow has ended.
                        let _ = parent.update(cx, |app, cx| {
                            app.set_open_files_visible(false, window, cx);
                        });
                    }),
            )
    }
}

impl Render for OpenFilesPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = self
            .parent
            .update(cx, |app, cx| app.render_open_files(window, cx))
            .unwrap_or_else(|_| div().into_any_element());
        div()
            .id("open-files-panel")
            .debug_selector(|| "open-files-panel".into())
            .track_focus(&self.focus)
            .size_full()
            .min_w(px(0.))
            .min_h_0()
            .child(content)
    }
}

impl EditorApp {
    /// Add the existing panel to the left band only when absent; saved placements remain authoritative.
    pub(crate) fn ensure_open_files_dock(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let id = gpui_base::dock::PanelId::from(self.open_files_panel.entity_id());
        if self.dock_area.read(cx).panel(id).is_some() {
            return;
        }
        let panel = self
            .dock_presentation
            .panel_handle(self.open_files_panel.clone());
        let width = px(self.session_state.explorer_width);
        self.dock_area.update(cx, |area, cx| {
            local_dock::add_panel_view(
                area,
                panel,
                gpui_base::dock::DockPlacement::Left,
                Some(width),
                window,
                cx,
            );
        });
    }

    /// Project the global placement and local hide/show intent without rewriting either preference.
    pub(crate) fn sync_open_files_visibility(&mut self, cx: &mut Context<Self>) {
        let visible =
            self.tab_placement == TabPlacement::ToolWindow && self.session_state.open_files_visible;
        let changed = self
            .open_files_panel
            .update(cx, |panel, cx| panel.set_visible(visible, cx));
        if changed {
            self.dock_area.update(cx, |_, cx| cx.notify());
            cx.notify();
        }
    }

    /// Reopen a restored native region only when the current mode and saved panel intent require it.
    /// Inline mode may have saved that region closed; reopening preserves its selection and geometry.
    pub(crate) fn restore_open_files_dock_visibility(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.tab_placement != TabPlacement::ToolWindow || !self.session_state.open_files_visible
        {
            return;
        }
        let id = gpui_base::dock::PanelId::from(self.open_files_panel.entity_id());
        self.dock_area.update(cx, |area, cx| {
            for placement in [
                gpui_base::dock::DockPlacement::Left,
                gpui_base::dock::DockPlacement::Right,
                gpui_base::dock::DockPlacement::Bottom,
            ] {
                if !area.is_dock_open(placement)
                    && area
                        .layout(placement)
                        .is_some_and(|tree| tree.find_panel_node(id).is_some())
                {
                    // Do not reveal/select this leaf: a saved exclusive peer remains authoritative.
                    area.toggle_dock(placement, window, cx);
                }
            }
        });
    }

    /// Toggle actual dock display in tool-window mode; captured stale toolbar actions do nothing.
    pub(crate) fn toggle_open_files(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.tab_placement != TabPlacement::ToolWindow {
            return;
        }
        let id = gpui_base::dock::PanelId::from(self.open_files_panel.entity_id());
        let visible = !local_dock::panel_visible(self.dock_area.read(cx), id, cx);
        self.set_open_files_visible(visible, window, cx);
    }

    /// Show or hide only the navigation panel, preserving every file and its saved dock position.
    fn set_open_files_visible(
        &mut self,
        visible: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let focus = self.open_files_panel.read(cx).focus_handle(cx);
        let restore_focus = !visible && focus.contains_focused(window, cx);
        if !visible {
            self.tabs_drag.cancel(window, cx);
        }
        self.session_state.open_files_visible = visible;
        self.sync_open_files_visibility(cx);
        if visible && self.tab_placement == TabPlacement::ToolWindow {
            self.ensure_open_files_dock(window, cx);
            let id = gpui_base::dock::PanelId::from(self.open_files_panel.entity_id());
            self.dock_presentation.activate(id);
            self.dock_area.update(cx, |area, cx| {
                local_dock::reveal_panel(area, id, window, cx);
                cx.notify();
            });
        } else if restore_focus {
            self.focus_tab_body(window, cx);
        }
        self.capture_dock_layout(cx);
        self.persist_session();
        cx.notify();
    }
}
