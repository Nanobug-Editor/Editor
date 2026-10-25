//! Owns window chrome, dock panels, settings UI, and shell rendering.

use crate::*;
use editor_ui::scroll::ScrollableElement as _;
pub(super) use editor_ui::window::title_bar_window_control;

/// Share one document scope between default keys and unbound shortcut metadata.
/// Embedded native source panes receive document commands without granting surrounding plugin controls that authority.
pub(crate) const DOCUMENT_COMMAND_CONTEXT: &str =
    "EditorShell && !PluginSurface || EditorShell > NativeEditorSource";

/// Keep repeated window/fixture initialization from superseding already applied user bindings.
struct ShellBindingsInitialized;
impl gpui_kit::Global for ShellBindingsInitialized {}

/// Register shell commands in the current app after Base initialization.
/// Startup and native shell fixtures share this table, including the plugin-surface scope boundary.
pub(crate) fn bind_editor_shell_keys(cx: &mut App) {
    if cx.has_global::<ShellBindingsInitialized>() {
        return;
    }
    // A plugin layout can embed the host's native source pane. Its descendant context admits
    // document commands without granting the surrounding guest controls those same shortcuts.
    let document_context = Some(DOCUMENT_COMMAND_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("shift-alt-f", FormatDocument, document_context),
        KeyBinding::new("f2", RenameSymbol, document_context),
        KeyBinding::new("f9", ToggleSourceBreakpoint, document_context),
        KeyBinding::new("ctrl-s", SaveDocument, document_context),
        // Match the tree's redo chord when text input owns focus; Base keeps its existing Ctrl+Y binding.
        KeyBinding::new("ctrl-shift-z", gpui_base::input::Redo, Some("Input")),
        KeyBinding::new("ctrl-shift-r", RefreshWorkspace, document_context),
        KeyBinding::new("ctrl-alt-t", ToggleTheme, Some("EditorShell")),
        KeyBinding::new("f12", NavigateToDefinition, document_context),
        KeyBinding::new("ctrl-i", ShowDefinitionDetails, document_context),
        // Error navigation follows the active document and wraps at either end.
        KeyBinding::new("f8", NextSyntaxError, document_context),
        KeyBinding::new("shift-f8", PreviousSyntaxError, document_context),
        KeyBinding::new("ctrl-shift-m", ShowProblems, document_context),
        KeyBinding::new("shift-f12", ShowReferences, document_context),
        KeyBinding::new("ctrl-.", ShowCodeActions, document_context),
        KeyBinding::new("ctrl-shift-space", ShowSignatureHelp, document_context),
    ]);
    cx.set_global(ShellBindingsInitialized);
}

#[cfg(target_os = "windows")]
const WINDOWS_TIMER_RESOLUTION_MS: u32 = 1;

#[cfg(target_os = "windows")]
#[link(name = "winmm")]
unsafe extern "system" {
    fn timeBeginPeriod(period: u32) -> u32;
    fn timeEndPeriod(period: u32) -> u32;
}

#[cfg(target_os = "windows")]
pub(crate) struct WindowsTimerResolution {
    enabled: bool,
}

#[cfg(target_os = "windows")]
impl WindowsTimerResolution {
    pub(crate) fn enable_for_window_drag() -> Self {
        // GPUI's Win32 modal move loop uses a short SetTimer interval to keep
        // processing and painting while the OS owns the title-bar drag loop.
        let enabled = unsafe { timeBeginPeriod(WINDOWS_TIMER_RESOLUTION_MS) } == 0;
        Self { enabled }
    }
}

#[cfg(target_os = "windows")]
impl Drop for WindowsTimerResolution {
    fn drop(&mut self) {
        if self.enabled {
            unsafe {
                timeEndPeriod(WINDOWS_TIMER_RESOLUTION_MS);
            }
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum EditorDockPanelKind {
    Explorer,
    Editor,
}

/// Keep the title bar and bottom toggle on the same theme-specific Explorer artwork.
fn explorer_panel_icon(cx: &App) -> Icon {
    Icon::default().data(if cx.theme().is_dark() {
        include_bytes!("../../assets/status-icons/explorer_dark.svg").as_slice()
    } else {
        include_bytes!("../../assets/status-icons/explorer_light.svg").as_slice()
    })
}

pub(crate) struct EditorDockPanel {
    parent: WeakEntity<EditorApp>,
    kind: EditorDockPanelKind,
    explorer_visible: Rc<Cell<bool>>,
    focus_handle: FocusHandle,
    /// Retain native focus observers so the Explorer title repaints on entry and exit.
    focus_subscriptions: Vec<Subscription>,
}

impl EditorDockPanel {
    pub(crate) fn new(
        parent: WeakEntity<EditorApp>,
        kind: EditorDockPanelKind,
        explorer_visible: Rc<Cell<bool>>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            parent,
            kind,
            explorer_visible,
            focus_handle: cx.focus_handle(),
            focus_subscriptions: Vec::new(),
        }
    }

    /// Watch this panel's focus scope once and retain both repaint subscriptions.
    fn observe_explorer_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.focus_subscriptions.is_empty() {
            return;
        }
        self.focus_subscriptions
            .push(cx.on_focus_in(&self.focus_handle, window, |_, _, cx| cx.notify()));
        self.focus_subscriptions.push(cx.on_focus_out(
            &self.focus_handle,
            window,
            |_, _, _, cx| cx.notify(),
        ));
    }
}

impl EventEmitter<PanelEvent> for EditorDockPanel {}

impl Focusable for EditorDockPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl dock::BasePanel for EditorDockPanel {
    fn panel_name(&self) -> &'static str {
        match self.kind {
            EditorDockPanelKind::Explorer => "Explorer",
            EditorDockPanelKind::Editor => "Editor",
        }
    }

    fn closable(&self, _: &App) -> bool {
        false
    }

    fn zoomable(&self, _: &App) -> bool {
        false
    }

    fn visible(&self, _: &App) -> bool {
        match self.kind {
            EditorDockPanelKind::Explorer => self.explorer_visible.get(),
            EditorDockPanelKind::Editor => true,
        }
    }
}

impl DockPanel for EditorDockPanel {
    /// Compact drag cards retain the same localized name as the native dock heading.
    fn drag_title(&self, _: &App) -> Option<gpui_kit::SharedString> {
        Some(match self.kind {
            EditorDockPanelKind::Explorer => t!("panel.explorer").to_string().into(),
            EditorDockPanelKind::Editor => t!("panel.editor").to_string().into(),
        })
    }

    fn title(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match self.kind {
            // Order explorer actions as locate, collapse all, expand all, then minimize.
            EditorDockPanelKind::Explorer => {
                self.observe_explorer_focus(window, cx);
                let focused = self.focus_handle.contains_focused(window, cx);
                // Base samples the current opacity, so a focus reversal resumes from the
                // visible fraction instead of restarting or flashing the whole button group.
                let opacity = gpui_base::motion::transition(
                    "explorer-title-actions-opacity",
                    if focused { 1. } else { 0. },
                    gpui_base::motion::Transition::new(Duration::from_millis(140)),
                    window,
                    cx,
                );
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .font_normal()
                    .child(
                        h_flex()
                            .id("explorer-title")
                            .debug_selector(|| "explorer-title".into())
                            .flex_1()
                            .min_w(px(0.))
                            .items_center()
                            .gap_2()
                            .child(explorer_panel_icon(cx).small())
                            .child(div().truncate().child(t!("panel.explorer").to_string())),
                    )
                    .child(
                        h_flex()
                            .id("explorer-actions")
                            .debug_selector(|| "explorer-actions".into())
                            // Reserve four compact slots and their height so focus changes never move the title.
                            .w(px(96.))
                            .h(px(24.))
                            .opacity(opacity)
                            .when(opacity > 0., |actions| {
                                actions.child(
                    div()
                        .id("explorer-reveal-active-file")
                        .debug_selector(|| "explorer-reveal-active-file".into())
                        .child(
                            Button::new("explorer-reveal")
                                // The custom target inherits the current title-bar text color.
                                .icon(Icon::default().path("icons/explorer-locate.svg"))
                                .tooltip(t!("explorer.reveal_active_file").to_string())
                                .accessibility_label(t!("explorer.reveal_active_file").to_string())
                                .small()
                                .compact()
                                .ghost()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    // Consume the title action so it cannot start a dock gesture.
                                    cx.stop_propagation();
                                    let _ = this.parent.update(cx, |app, cx| {
                                        app.reveal_active_file_in_explorer(window, cx);
                                    });
                                })),
                        ),
                )
                .child(
                    div()
                        .id("explorer-collapse-all")
                        .debug_selector(|| "explorer-collapse-all".into())
                        .child(
                            Button::new("explorer-collapse-all-button")
                                .icon(Icon::default().path("icons/explorer-collapse-all.svg"))
                                .tooltip(t!("explorer.collapse_all").to_string())
                                .accessibility_label(t!("explorer.collapse_all").to_string())
                                .small()
                                .compact()
                                .ghost()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    // Consume header actions so they cannot start a dock gesture.
                                    cx.stop_propagation();
                                    let _ = this.parent.update(cx, |app, cx| {
                                        app.set_all_explorer_directories_expanded(false, cx);
                                    });
                                })),
                        ),
                )
                .child(
                    div()
                        .id("explorer-expand-all")
                        .debug_selector(|| "explorer-expand-all".into())
                        .child(
                            Button::new("explorer-expand-all-button")
                                .icon(Icon::default().path("icons/explorer-expand-all.svg"))
                                .tooltip(t!("explorer.expand_all").to_string())
                                .accessibility_label(t!("explorer.expand_all").to_string())
                                .small()
                                .compact()
                                .ghost()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    cx.stop_propagation();
                                    let _ = this.parent.update(cx, |app, cx| {
                                        app.set_all_explorer_directories_expanded(true, cx);
                                    });
                                })),
                        ),
                )
                .child(
                    Button::new("explorer-hide")
                        .icon(Icon::new(IconName::WindowMinimize))
                        .tooltip(t!("panel.minimize_explorer").to_string())
                        .accessibility_label(t!("panel.minimize_explorer").to_string())
                        .small()
                        .compact()
                        .ghost()
                        .on_click(cx.listener(|this, _, window, cx| {
                            cx.stop_propagation();
                            let _ = this.parent.update(cx, |app, cx| {
                                // Use the existing visibility path to persist hiding and update the bottom toggle.
                                if app.explorer_visible {
                                    app.toggle_explorer(window, cx);
                                }
                            });
                        })),
                )
                            }),
                    )
                    .into_any_element()
            }
            EditorDockPanelKind::Editor => div()
                .child(t!("panel.editor").to_string())
                .into_any_element(),
        }
    }

    fn title_bar(&self, _: &App) -> bool {
        !matches!(self.kind, EditorDockPanelKind::Editor)
    }
}

impl Render for EditorDockPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let kind = self.kind;
        self.parent
            .update(cx, |app, cx| match kind {
                EditorDockPanelKind::Explorer => app.render_file_tree(cx).into_any_element(),
                EditorDockPanelKind::Editor => {
                    app.render_editor_panel(window, cx).into_any_element()
                }
            })
            .unwrap_or_else(|_| div().into_any_element())
    }
}

impl EditorApp {
    /// Apply `window`'s latest size to layout immediately, then persist after resize input settles.
    /// The quiet period only batches disk I/O; GPUI retains the display-driven paint cadence.
    /// Pure moves need no saved size change. Close already flushes the latest in-memory session.
    fn update_window_geometry(&mut self, window: &Window, cx: &mut Context<Self>) {
        let size = window.bounds().size;
        let width = size.width / px(1.);
        let height = size.height / px(1.);
        if self.session_state.window_width == width && self.session_state.window_height == height {
            return;
        }
        self.session_state.window_width = width;
        self.session_state.window_height = height;
        // Prepare editor-local column pixels on the next native frame without waiting for I/O.
        cx.notify();
        self.pending_window_geometry_save = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(200))
                .await;
            let _ = this.update(cx, |this, _| {
                if !this.shutting_down {
                    // Read current state after settling, never a stale snapshot captured mid-drag.
                    this.session_state.save();
                }
            });
        }));
    }
}

impl Render for EditorApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.dark_theme = cx.theme().is_dark();
        if self.provider_generation != self.providers.feature_generation() && !self.shutting_down {
            self.sync_dynamic_languages(cx);
        }
        if std::mem::take(&mut self.pending_contribution_sync) {
            // The render frame provides the window needed to attach LSP providers to open tabs.
            self.sync_runtime_contributions(window, cx);
        }
        if self._bounds_subscription.is_none() {
            self._bounds_subscription =
                Some(cx.observe_window_bounds(window, |this, window, cx| {
                    this.update_window_geometry(window, cx);
                }));
        }
        // Open plugin-owned settings through the normal editor document path.
        self.sync_plugin_panels(window, cx);
        self.sync_virtual_documents(window, cx);
        self.sync_plugin_document_menu(window, cx);
        self.sync_document_comparison(window, cx);
        self.sync_plugin_documents(cx);
        self.sync_dynamic_language_servers(cx);
        self.sync_run_controls(window, cx);
        self.sync_project_menu(window, cx);
        self.dispatch_editor_requests(window, cx);
        self.sync_plugin_document_menu(window, cx);
        self.sync_document_comparison(window, cx);
        self.sync_outline(window, cx);
        self.prune_direct_diagnostics(cx);
        self.dock_area.update(cx, |area, cx| {
            self.dock_ui
                .prepare_frame(area, window.viewport_size().width, window, cx)
        });
        if let Some(path) = self.pending_plugin_file.take() {
            self.open_file(path, window, cx);
        }
        let cursor = self.editor.read(cx).cursor_position();

        let shell_style = component_styles(cx, ThemeComponent::AppShell).base;
        let status_style = component_styles(cx, ThemeComponent::StatusBar).base;
        // Observe the exact entry chosen for this frame before confirmation can rebuild the same severity later.
        let plugin_indicator = self.plugin_indicator(cx);
        let services_loading = self.plugin_count(PluginPopupKind::Services, cx) > 0;
        if !services_loading
            && self
                .plugin_popup
                .is_some_and(|(kind, _)| kind == PluginPopupKind::Services)
        {
            // Ready, failed, cancelled or withdrawn engines leave no stale blocking popup behind.
            self.close_plugin_popup(window, cx);
        }
        self.track_plugin_indicator_frame(plugin_indicator, services_loading);
        v_flex()
            .id("editor-shell")
            .debug_selector(|| "editor-shell".into())
            .key_context("EditorShell")
            // Escape must reach the gesture even while the document, tree, or a popup owns focus.
            .capture_key_down(
                cx.listener(|app, event: &gpui_kit::KeyDownEvent, window, cx| {
                    if event.keystroke.key == "escape" && app.tabs_drag.active() {
                        app.tabs_drag.cancel(window, cx);
                        cx.stop_propagation();
                    }
                }),
            )
            .capture_key_down(cx.listener(Self::cancel_pending_language_feature))
            .relative()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    // A click outside the anchored card dismisses its transient details.
                    if this.plugin_popup.take().is_some() {
                        this.close_plugin_popup(window, cx);
                    }
                    if this.explorer_menu.take().is_some() {
                        cx.notify();
                    }
                }),
            )
            .on_action(cx.listener(Self::on_save_action))
            .on_action(
                cx.listener(|app, _: &app::shortcuts::OpenShortcuts, window, cx| {
                    app.open_shortcuts(window, cx);
                }),
            )
            .on_action(cx.listener(|this, _: &extensions::QuitEditor, window, cx| {
                if this.project_window.is_some() {
                    // Release the child editor's update lease before the owner examines it.
                    window.defer(cx, move |_, cx| {
                        app::projects::quit_application(cx);
                    });
                } else {
                    this.shutdown_plugins(cx);
                }
            }))
            .on_action(
                cx.listener(|this, _: &app::projects::CloseWindow, window, cx| {
                    if let Some(owner) = this.project_window.clone() {
                        window.defer(cx, move |window, cx| {
                            let _ = owner.update(cx, |host, cx| {
                                host.request_close(app::projects::CloseIntent::Window, window, cx);
                            });
                        });
                    } else {
                        this.shutdown_plugins(cx);
                    }
                }),
            )
            .on_action(
                cx.listener(|this, _: &app::projects::OpenFolder, window, cx| {
                    if let Some(owner) = this.project_window.clone() {
                        window.defer(cx, move |window, cx| {
                            let _ = owner.update(cx, |host, cx| host.choose_folder(window, cx));
                        });
                    }
                }),
            )
            .on_action(
                cx.listener(|this, _: &app::projects::ShowHome, window, cx| {
                    if let Some(owner) = this.project_window.clone() {
                        window.defer(cx, move |window, cx| {
                            let _ = owner.update(cx, |host, cx| {
                                host.show_home(window, cx);
                            });
                        });
                    }
                }),
            )
            .on_action(
                cx.listener(|this, _: &extensions::ToggleExtensions, window, cx| {
                    this.toggle_extensions(window, cx)
                }),
            )
            .on_action(cx.listener(Self::on_refresh_action))
            .on_action(cx.listener(Self::on_toggle_theme_action))
            .on_action(cx.listener(Self::on_navigate_to_definition))
            .on_action(cx.listener(Self::on_show_definition_details))
            .on_action(cx.listener(Self::format_document_action))
            .on_action(cx.listener(Self::format_selection_action))
            .on_action(cx.listener(Self::rename_symbol_action))
            .on_action(cx.listener(|app, _: &ToggleSourceBreakpoint, _, cx| {
                app.toggle_source_breakpoint(cx)
            }))
            .on_action(cx.listener(Self::toggle_outline))
            .on_action(
                cx.listener(|app, _: &ShowProblems, window, cx| app.show_problems(window, cx)),
            )
            .on_action(cx.listener(|app, _: &ShowReferences, window, cx| {
                app.request_language_feature(
                    plugin_runtime::plugin_protocol::direct::Feature::References,
                    window,
                    cx,
                )
            }))
            .on_action(cx.listener(|app, _: &ShowCodeActions, window, cx| {
                app.request_language_feature(
                    plugin_runtime::plugin_protocol::direct::Feature::CodeActions,
                    window,
                    cx,
                )
            }))
            .on_action(cx.listener(|app, _: &ShowSignatureHelp, window, cx| {
                app.request_language_feature(
                    plugin_runtime::plugin_protocol::direct::Feature::SignatureHelp,
                    window,
                    cx,
                )
            }))
            .on_action(cx.listener(|app, _: &NextSyntaxError, window, cx| {
                app.navigate_syntax_error(false, window, cx);
            }))
            .on_action(cx.listener(|app, _: &PreviousSyntaxError, window, cx| {
                app.navigate_syntax_error(true, window, cx);
            }))
            .size_full()
            .bg(shell_style.background.unwrap_or(cx.theme().background))
            .text_color(shell_style.foreground.unwrap_or(cx.theme().foreground))
            .child(
                editor_ui::window::editor_title_bar(cx)
                    .child(
                        h_flex()
                            .h_full()
                            .items_center()
                            .gap_2()
                            .px_3()
                            // Keep the dropdown outside the native drag region so its button owns input.
                            .child(self.render_application_menu_trigger(cx))
                            .child(self.render_project_menu_trigger(cx))
                            .child(self.render_workspace_entries(window, cx)),
                    )
                    .child(
                        div()
                            .id("title-bar-drag-region")
                            .debug_selector(|| "title-bar-drag-region".into())
                            .flex_1()
                            .h_full()
                            .window_control_area(WindowControlArea::Drag)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, _| this.titlebar_should_move = true),
                            )
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|this, _, _, _| this.titlebar_should_move = false),
                            )
                            .on_mouse_move(cx.listener(|this, _, window, _| {
                                if this.titlebar_should_move {
                                    this.titlebar_should_move = false;
                                    window.start_window_move();
                                }
                            })),
                    )
                    .child(self.render_run_controls(cx))
                    .child(self.render_extensions_button(cx))
                    .child(self.render_settings_dialog(cx))
                    .child(self.render_window_controls(window, cx)),
            )
            .child(h_flex().flex_1().min_h_0().child(self.dock_area.clone()))
            .child(
                div()
                    .w_full()
                    .bg(status_style.background.unwrap_or(cx.theme().background))
                    .text_color(status_style.foreground.unwrap_or(cx.theme().foreground))
                    .text_size(px(status_style.font_size_px.unwrap_or(12.)))
                    .border_t_1()
                    .border_color(status_style.border.unwrap_or(cx.theme().border))
                    .child(
                        StatusBar::new()
                            .left(self.render_plugin_toolbar(explorer_panel_icon(cx), window, cx))
                            // Any open file may contribute; an entirely empty projection has no counter entry.
                            .when_some(self.render_syntax_error_indicator(cx), |bar, indicator| {
                                bar.right(indicator)
                            })
                            // External startup sits immediately left of the independent plugin status.
                            .when(services_loading, |bar| {
                                bar.right(
                                    self.render_plugin_indicator(PluginPopupKind::Services, cx),
                                )
                            })
                            .when_some(plugin_indicator, |bar, kind| {
                                bar.right(self.render_plugin_indicator(kind, cx))
                            })
                            .right(if self.active_text_tab_index().is_some() {
                                t!(
                                    "status.cursor",
                                    line = cursor.line + 1,
                                    column = cursor.character + 1
                                )
                                .to_string()
                            } else {
                                t!("status.cursor_empty").to_string()
                            }),
                    ),
            )
            .child(self.render_plugin_popup_blocker(cx))
            .when_some(self.shortcut_panel.as_ref(), |shell, panel| {
                shell.child(panel.clone())
            })
            .when_some(self.render_application_menu(), |shell, menu| {
                shell.child(menu)
            })
            .when_some(self.render_project_menu(), |shell, menu| shell.child(menu))
            // A pending leave decision covers the shell until the user keeps or stops the sessions.
            .when_some(self.render_leave_confirmation(cx), |shell, confirm| {
                shell.child(confirm)
            })
            .child(self.render_run_menu(window, cx))
            .child(self.render_plugin_popup(window, cx))
            .child(self.render_explorer_menu(window, cx))
            .when_some(self.render_plugin_document_menu(cx), |shell, menu| {
                shell.child(menu)
            })
            .when_some(self.file_view_menu.as_ref(), |body, menu| {
                body.child(menu.clone())
            })
            .when_some(self.tool_overflow.as_ref(), |body, menu| {
                body.child(menu.clone())
            })
            .child(self.render_explorer_edit(cx))
            .child(self.render_explorer_delete(cx))
            .child(self.render_file_transfer(cx))
            .child(self.render_workspace_receipts(cx))
            .child(self.render_workspace_plan(window, cx))
            .children(self.render_language_features(window, cx))
            .child(
                div()
                    .id("plugin-notice-stack")
                    .absolute()
                    .bottom(px(36.))
                    .right(px(12.))
                    .max_h(px(420.))
                    .overflow_y_scrollbar()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .children(
                        self.plugin_interactions
                            .iter()
                            .filter(|prompt| !prompt.read(cx).is_modal())
                            .cloned(),
                    ),
            )
            .children(
                self.plugin_interactions
                    .iter()
                    .filter(|prompt| prompt.read(cx).is_modal())
                    .cloned(),
            )
            .when_some(self.notification.as_ref(), |this, notification| {
                // Center the card near the top, keeping a margin when the window is narrow.
                let width = px(380.).min((window.viewport_size().width - px(24.)).max(px(0.)));
                this.child(
                    div()
                        .absolute()
                        .left((window.viewport_size().width - width) / 2.)
                        .top(px(64.))
                        .w(width)
                        .child(notification.clone()),
                )
            })
            .when_some(self.definition_notice, |this, notice| {
                let left = px((notice.position.x / px(1.) - 28.).max(4.));
                let top = px((notice.position.y / px(1.) - 38.).max(4.));
                this.child(
                    div()
                        .absolute()
                        .left(left)
                        .top(top)
                        .px_3()
                        .py_2()
                        .rounded_md()
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().popover)
                        .text_color(cx.theme().foreground)
                        .text_sm()
                        .child(t!("editor.no_definition").to_string()),
                )
            })
    }
}

impl EditorApp {
    fn render_window_controls(&self, window: &Window, cx: &Context<Self>) -> impl IntoElement {
        editor_ui::window::native_window_controls(window, cx)
    }
}
