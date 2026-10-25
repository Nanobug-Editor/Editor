//! Two native toolbar groups route captured targets; all plugin functions and artwork stay in packages.
use super::*;
use crate::app::editor_preferences::TabPlacement;
use crate::outline::ToggleOutline;
use editor_ui::menu::{MenuStyle, PopupMenu};

mod order;
mod render;

/// Native toolbar focus retains this context; reopening a file or replacing a window invalidates it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FunctionContext {
    File(protocol::api::FileVersion),
    Window { key: String, epoch: u64 },
}

#[derive(Clone)]
enum Action {
    Explorer,
    /// Native sessions stay available without an installed terminal package.
    Terminal,
    /// The built-in outline uses the same window controls and overflow behavior as other panels.
    Outline,
    /// File navigation uses the same native window controls when its global placement is docked.
    OpenFiles,
    /// Host messages share the same order and overflow path as every other window.
    Messages,
    Window {
        key: String,
        epoch: u64,
    },
    Function {
        key: String,
        epoch: u64,
        event: protocol::ui::ToolEvent,
    },
}

/// One immutable projection is shared by direct buttons and its group's overflow popup.
#[derive(Clone)]
struct Entry {
    id: String,
    label: String,
    tooltip: String,
    icon: Option<Icon>,
    selected: bool,
    disabled: bool,
    action: Action,
}

impl ExtensionPanel {
    /// Cache each resource once per package digest; repeated native rendering performs no disk reads.
    pub(super) fn refresh_tool_icons(&mut self) {
        let Some(key) = self
            .active
            .as_ref()
            .zip(self.surface_id.as_ref())
            .map(|(id, panel)| format!("{id}/{panel}"))
        else {
            return;
        };
        let Some(entry) = self
            .entries
            .iter()
            .find(|entry| Some(&entry.manifest.id) == self.active.as_ref())
        else {
            self.tool_icons.clear();
            return;
        };
        let wanted = self
            .views
            .get(&key)
            .into_iter()
            .flat_map(|doc| &doc.tools)
            .flat_map(|tool| [tool.icon.light.clone(), tool.icon.dark.clone()])
            .collect::<std::collections::BTreeSet<_>>();
        self.tool_icons.retain(|path, _| wanted.contains(path));
        for path in wanted {
            if !self.tool_icons.contains_key(&path) {
                let icon = entry
                    .tool_icon(&self.root, &path)
                    .map(|bytes| Icon::default().data(&bytes));
                self.tool_icons.insert(path, icon);
            }
        }
    }
}

impl EditorApp {
    /// Keep a target only while native focus is inside the toolbar or its captured overflow popup.
    fn update_function_context(&mut self, window: &Window, cx: &App) {
        let file = self
            .active_tab_index()
            .and_then(|index| self.plugin_file_context(index).ok())
            .map(|file| FunctionContext::File(file.version));
        for (key, panel) in &self.plugin_panels {
            let panel = panel.read(cx);
            if !panel.editor_preview
                && panel.visible.get()
                && (panel.focus.contains_focused(window, cx)
                    || panel
                        .native_ui
                        .as_ref()
                        .is_some_and(|view| view.read(cx).contains_focus(window, cx)))
            {
                self.function_context = Some(FunctionContext::Window {
                    key: key.clone(),
                    epoch: panel.instance_epoch,
                });
                return;
            }
        }
        if self.toolbar_focus.contains_focused(window, cx) || self.tool_overflow.is_some() {
            let valid = match &self.function_context {
                Some(FunctionContext::File(version)) => {
                    file.as_ref() == Some(&FunctionContext::File(version.clone()))
                }
                Some(FunctionContext::Window { key, epoch }) => {
                    self.plugin_panels.get(key).is_some_and(|panel| {
                        let panel = panel.read(cx);
                        !panel.editor_preview
                            && panel.visible.get()
                            && panel.instance_epoch == *epoch
                    })
                }
                None => false,
            };
            if valid {
                return;
            }
        }
        self.function_context = file;
    }

    /// Host order is Explorer, optional Open Files, Outline, Messages, Terminal, then installed plugins.
    /// A saved user arrangement overrides this projection without changing panel ownership.
    fn window_entries(&self, explorer: Icon, cx: &App) -> Vec<Entry> {
        let mut windows = self
            .extensions
            .read(cx)
            .entries
            .iter()
            .filter(|entry| entry.enabled)
            .flat_map(|entry| {
                entry
                    .manifest
                    .panels
                    .iter()
                    .filter(|panel| panel.position != "editor" && panel.show_in_window_controls)
                    .enumerate()
                    .map(|(index, panel)| {
                        (
                            entry.install_sequence,
                            entry.manifest.id.clone(),
                            panel.status_order.unwrap_or(1000),
                            index,
                            format!("{}/{}", entry.manifest.id, panel.id),
                        )
                    })
            })
            .collect::<Vec<_>>();
        windows.sort();
        let label = t!("panel.explorer").to_string();
        let mut result = vec![Entry {
            id: "explorer-panel-toggle".into(),
            label: label.clone(),
            tooltip: label,
            icon: Some(explorer),
            selected: local_dock::panel_visible(
                self.dock_area.read(cx),
                gpui_base::dock::PanelId::from(self.explorer_panel.entity_id()),
                cx,
            ),
            disabled: false,
            action: Action::Explorer,
        }];
        if self.tab_placement == TabPlacement::ToolWindow {
            let visible = local_dock::panel_visible(
                self.dock_area.read(cx),
                gpui_base::dock::PanelId::from(self.open_files_panel.entity_id()),
                cx,
            );
            result.push(Entry {
                id: "open-files-toggle".into(),
                label: t!("open_files.title").to_string(),
                tooltip: t!(if visible {
                    "open_files.hide"
                } else {
                    "open_files.show"
                })
                .to_string(),
                icon: Some(Icon::new(IconName::Files)),
                selected: visible,
                disabled: false,
                action: Action::OpenFiles,
            });
        }
        // Every host control reflects actual regional display, including closed and exclusive docks.
        let outline_visible = local_dock::panel_visible(
            self.dock_area.read(cx),
            gpui_base::dock::PanelId::from(self.outline_panel.entity_id()),
            cx,
        );
        result.push(Entry {
            id: "outline-toggle".into(),
            label: t!("panel.outline").to_string(),
            tooltip: t!(if outline_visible {
                "outline.hide"
            } else {
                "outline.show"
            })
            .to_string(),
            icon: Some(Icon::default().path("icons/outline-panel.svg")),
            selected: outline_visible,
            disabled: false,
            action: Action::Outline,
        });
        result.push(Entry {
            id: "host-messages-toggle".into(),
            label: t!(if self.messages.read(cx).has_pending_alert() {
                "messages.unread"
            } else {
                "messages.title"
            })
            .to_string(),
            tooltip: t!("messages.title").to_string(),
            icon: Some(editor_ui::message_panel::notification_icon()),
            selected: self.messages_visible(cx),
            disabled: false,
            action: Action::Messages,
        });
        result.push(Entry {
            id: "terminal-toggle".into(),
            label: t!("terminal.title").to_string(),
            tooltip: t!("terminal.title").to_string(),
            icon: Some(crate::terminal::icon(cx)),
            selected: self.terminal_visible(cx),
            disabled: false,
            action: Action::Terminal,
        });
        for (_, _, _, _, key) in windows {
            let Some(panel) = self.plugin_panels.get(&key) else {
                continue;
            };
            let panel = panel.read(cx);
            result.push(Entry {
                id: format!("plugin-window-{key}"),
                label: panel.panel_title.clone(),
                tooltip: panel.panel_title.clone(),
                icon: panel.panel_icon(cx.theme().is_dark()),
                selected: panel.displayed_in_dock(),
                disabled: false,
                action: Action::Window {
                    key,
                    epoch: panel.instance_epoch,
                },
            });
        }
        result.sort_by_key(|entry| {
            self.session_state
                .window_button_order
                .iter()
                .position(|id| id == &entry.id)
                .unwrap_or(usize::MAX)
        });
        result
    }

    /// All eligible auxiliary and selected file surfaces contribute; window tools follow their own focus.
    fn function_entries(&self, cx: &App) -> Vec<Entry> {
        let mut tools = Vec::new();
        let locale = rust_i18n::locale();
        for (key, panel) in &self.plugin_panels {
            let panel = panel.read(cx);
            if !panel.visible.get() {
                continue;
            }
            let Some(document) = panel.current_document() else {
                continue;
            };
            for tool in &document.tools {
                let applies = match (&self.function_context, &tool.target) {
                    (
                        Some(FunctionContext::File(current)),
                        protocol::ui::ToolTarget::File { version },
                    ) => current == version,
                    (
                        Some(FunctionContext::Window {
                            key: current,
                            epoch,
                        }),
                        protocol::ui::ToolTarget::Window { panel: target },
                    ) => {
                        current == key
                            && *epoch == panel.instance_epoch
                            && panel.surface_id.as_ref() == Some(target)
                    }
                    _ => false,
                };
                if !applies || !tool.visible {
                    continue;
                }
                let Some(icon) = panel
                    .tool_icons
                    .get(tool.icon.path(cx.theme().is_dark()))
                    .and_then(Clone::clone)
                else {
                    continue;
                };
                tools.push((
                    tool.order,
                    key.clone(),
                    Entry {
                        id: format!("plugin-tool-{key}/{}", tool.id),
                        label: tool.label.for_locale(&locale).into(),
                        tooltip: tool.tooltip.for_locale(&locale).into(),
                        icon: Some(icon),
                        selected: tool.selected,
                        disabled: tool.disabled
                            || document.dialog.is_some()
                            || document.menu.is_some(),
                        action: Action::Function {
                            key: key.clone(),
                            epoch: panel.instance_epoch,
                            event: protocol::ui::ToolEvent {
                                revision: document.revision,
                                tool: tool.id.clone(),
                                target: tool.target.clone(),
                            },
                        },
                    },
                ));
            }
        }
        tools.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.id.cmp(&b.2.id)));
        tools.into_iter().map(|(_, _, entry)| entry).collect()
    }

    /// Revalidate native ownership after focus transfer; stale actions produce no plugin work.
    fn activate_bar_entry(&mut self, entry: &Entry, window: &mut Window, cx: &mut Context<Self>) {
        if entry.disabled {
            return;
        }
        match &entry.action {
            Action::Explorer => self.toggle_explorer(window, cx),
            Action::Terminal => self.toggle_terminal(window, cx),
            Action::Outline => self.toggle_outline(&ToggleOutline, window, cx),
            Action::OpenFiles => self.toggle_open_files(window, cx),
            Action::Messages => self.toggle_messages(window, cx),
            Action::Window { key, epoch } => {
                let Some(panel) = self.plugin_panels.get(key).cloned() else {
                    return;
                };
                if panel.read(cx).instance_epoch != *epoch || panel.read(cx).editor_preview {
                    return;
                }
                panel.update(cx, |panel, cx| {
                    if panel.displayed_in_dock() {
                        panel.hide(cx);
                    } else {
                        panel.show(window, cx);
                    }
                    cx.notify();
                });
                let visible = panel.read(cx).visible.get();
                self.function_context = visible.then(|| FunctionContext::Window {
                    key: key.clone(),
                    epoch: *epoch,
                });
                self.session_state
                    .plugin_panel_visibility
                    .insert(key.clone(), visible);
                self.persist_session();
                self.dock_area.update(cx, |area, cx| {
                    if visible {
                        local_dock::reveal_panel(
                            area,
                            gpui_base::dock::PanelId::from(panel.entity_id()),
                            window,
                            cx,
                        );
                    }
                    cx.notify();
                });
            }
            Action::Function { key, epoch, event } => {
                let expected = match &event.target {
                    protocol::ui::ToolTarget::File { version } => {
                        Some(FunctionContext::File(version.clone()))
                    }
                    protocol::ui::ToolTarget::Window { .. } => Some(FunctionContext::Window {
                        key: key.clone(),
                        epoch: *epoch,
                    }),
                };
                if self.function_context != expected {
                    return;
                }
                if let protocol::ui::ToolTarget::File { version } = &event.target
                    && self
                        .active_tab_index()
                        .and_then(|index| self.plugin_file_context(index).ok())
                        .map(|file| file.version)
                        .as_ref()
                        != Some(version)
                {
                    return;
                }
                let Some(panel) = self.plugin_panels.get(key) else {
                    return;
                };
                let panel = panel.read(cx);
                if panel.instance_epoch != *epoch
                    || !panel.visible.get()
                    || panel
                        .current_document()
                        .is_none_or(|document| document.validate_tool_event(event).is_err())
                {
                    return;
                }
                panel.send(PluginEvent::Tool(event.clone()));
            }
        }
        cx.notify();
    }

    /// A group's hidden entries keep their state, target and native keyboard navigation in one popup.
    fn open_group_overflow(
        &mut self,
        entries: Vec<Entry>,
        reorderable: bool,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let items = entries
            .iter()
            .map(|entry| protocol::ui::MenuItem {
                id: entry.id.clone(),
                label: if entry.selected {
                    format!("✓ {}", entry.label)
                } else {
                    entry.label.clone()
                },
                disabled: entry.disabled,
                separator_before: false,
            })
            .collect();
        let parent = cx.entity().downgrade();
        let reorder_owner = parent.clone();
        let snapshot: Vec<_> = entries.iter().map(|entry| entry.id.clone()).collect();
        self.tool_overflow = Some(cx.new(|cx| {
            let menu = PopupMenu::new(
                items,
                MenuStyle::current(cx),
                position,
                move |action, window, cx| {
                    let _ = parent.update(cx, |app, cx| {
                        if let protocol::ui::Action::Select(id) = action
                            && let Some(entry) = entries.iter().find(|entry| entry.id == id)
                        {
                            app.activate_bar_entry(entry, window, cx);
                        }
                        app.tool_overflow = None;
                        cx.notify();
                    });
                },
                window,
                cx,
            );
            if reorderable {
                menu.reorderable(move |event, _, cx| {
                    reorder_owner
                        .update(cx, |app, cx| {
                            // This popup contains every window. A newly added contribution also
                            // invalidates its captured list; reopening projects the latest members.
                            if app.window_entries(Icon::default(), cx).len() != snapshot.len() {
                                return false;
                            }
                            app.reorder_window_buttons(event, &snapshot, cx)
                        })
                        .unwrap_or(false)
                })
            } else {
                menu
            }
        }));
        cx.notify();
    }
}
