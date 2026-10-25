//! Application dock and workspace integration for the independent terminal implementation.

use super::*;

impl EditorApp {
    /// Restore the leaf after loading an older dock tree, keeping unrelated peers and their sizes.
    pub(crate) fn ensure_terminal_dock(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let panel = self.dock_presentation.panel_handle(self.terminal.clone());
        let id = gpui_base::dock::PanelId::from(self.terminal.entity_id());
        let height = px(self
            .session_state
            .plugin_dock_sizes
            .get("bottom")
            .copied()
            .unwrap_or(260.));
        self.dock_area.update(cx, |area, cx| {
            if area.panel(id).is_none() {
                local_dock::add_panel_view(
                    area,
                    panel,
                    gpui_base::dock::DockPlacement::Bottom,
                    Some(height),
                    window,
                    cx,
                );
            }
        });
    }

    /// Dock-level hiding also affects the footer, without changing whether Shell resources are alive.
    pub(crate) fn terminal_visible(&self, cx: &App) -> bool {
        local_dock::panel_visible(
            self.dock_area.read(cx),
            gpui_base::dock::PanelId::from(self.terminal.entity_id()),
            cx,
        )
    }

    /// Reveal the native dock leaf; opening it never depends on installed package contributions.
    pub(crate) fn toggle_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let visible = !self.terminal_visible(cx);
        let trusted = self.extensions.read(cx).workspace_trusted();
        self.terminal.update(cx, |panel, cx| {
            panel.set_trusted(trusted, cx);
            panel.set_visible(visible, cx);
        });
        if visible {
            self.reveal_terminal(window, cx);
        }
        self.dock_area.update(cx, |_, cx| cx.notify());
        cx.notify();
    }
}
