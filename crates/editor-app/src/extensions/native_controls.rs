//! Panel title commands use the same native popup as guest-supplied menus.
use super::*;
use crate::ui::controls::menu::{MenuStyle, PopupMenu};
impl ExtensionPanel {
    /// Revoke pointer projections before dropping either native surface. Old rendered frames may
    /// retain their pane entities until the router has delivered the original editor's MouseUp.
    pub(super) fn retire_native_inputs(&mut self, cx: &mut App) {
        self.retire_native_ui(cx);
        self.retire_native_toolbar(cx);
    }

    /// Withdraw the body while preserving any independently mounted source toolbar.
    pub(super) fn retire_native_ui(&mut self, cx: &mut App) {
        self.main_code_source = None;
        if let Some(view) = self.native_ui.take() {
            view.update(cx, |view, cx| view.retire_code_panes(cx));
        }
    }

    /// Withdraw a source toolbar without retiring a same-source body during publication gaps.
    pub(super) fn retire_native_toolbar(&mut self, cx: &mut App) {
        if let Some(view) = self.native_toolbar.take() {
            view.update(cx, |view, cx| view.retire_code_panes(cx));
        }
    }

    /// Release native focus/IME targets with the hidden surface, while the guest keeps its own view state.
    pub(super) fn hide(&mut self, cx: &mut App) {
        self.source_viewport.withdraw();
        self.viewport_sync_enabled = false;
        self.visible.set(false);
        self.view_demand.borrow_mut().cancel();
        self.retire_native_inputs(cx);
        self.command_popup = None;
    }
    pub(super) fn command_popup(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Entity<PopupMenu>> {
        if !self.commands_open {
            self.command_popup = None;
            return None;
        }
        let items: Vec<_> = self
            .entries
            .iter()
            .filter(|entry| entry.enabled && Some(&entry.manifest.id) == self.active.as_ref())
            .flat_map(|entry| {
                entry
                    .manifest
                    .commands
                    .iter()
                    .filter(|command| command.menu)
            })
            .map(|command| protocol::ui::MenuItem {
                id: command.id.clone(),
                label: command.title.clone(),
                disabled: false,
                separator_before: false,
            })
            .collect();
        let style = MenuStyle::current(cx);
        if let Some(popup) = &self.command_popup {
            popup.update(cx, |popup, cx| {
                popup.style = style;
                popup.items = items;
                cx.notify();
            });
        } else {
            let owner = cx.entity().downgrade();
            // Declared commands retain metadata identity across first birth. A replacement,
            // disable/enable cycle or changed scope advances these epochs and rejects this menu.
            let plugin = self.active.clone();
            let epochs = self.shortcut_snapshot().commands;
            let position = self.bounds.bottom_left();
            self.command_popup = Some(cx.new(|cx| {
                PopupMenu::new(
                    items,
                    style,
                    position,
                    move |action, _, cx| {
                        let _ = owner.update(cx, |this, cx| {
                            if this.active != plugin {
                                return;
                            }
                            if let protocol::ui::Action::Select(id) = action {
                                if let Some(plugin) = &plugin
                                    && let Some(epoch) = epochs.get(&(plugin.clone(), id.clone()))
                                {
                                    // Capture the actual clicked document/selection before any demand;
                                    // a later receipt must never substitute the newly focused editor.
                                    let guard = this
                                        .parent
                                        .update(cx, |app, cx| {
                                            match app.capture_current_command_guard(cx) {
                                                Ok(guard) => Some(guard),
                                                Err(error) => {
                                                    app.status = error.to_string();
                                                    cx.notify();
                                                    None
                                                }
                                            }
                                        })
                                        .ok()
                                        .flatten();
                                    if let Some(guard) = guard {
                                        let _ = this.enqueue_command(
                                            plugin,
                                            &id,
                                            serde_json::Value::Null,
                                            Some(*epoch),
                                            None,
                                            Some(guard),
                                        );
                                    }
                                }
                            }
                            this.commands_open = false;
                            cx.notify();
                        });
                    },
                    window,
                    cx,
                )
            }));
        }
        // New and retained popups share the measured trigger; the menu owns width and clamping.
        if let Some(popup) = &self.command_popup {
            popup.update(cx, |popup, cx| popup.anchor_to(self.bounds, cx));
        }
        self.command_popup.clone()
    }
}
