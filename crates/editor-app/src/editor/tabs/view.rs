//! Binds document identities and actions to the local file-tab strip.

use crate::*;
use app::editor_preferences::TabPlacement;
use editor_ui::tabs::{FileTabMove, FileTabPreview, FileTabStrip};
use gpui_kit::AnyElement;

impl EditorApp {
    /// Switch only the mounted presentation, retaining every document, preview and undo history.
    pub(crate) fn apply_tab_placement(
        &mut self,
        placement: TabPlacement,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.tab_placement == placement {
            return;
        }
        let restore_editor_focus = self.tab_placement == TabPlacement::ToolWindow
            && self
                .open_files_panel
                .read(cx)
                .focus_handle(cx)
                .contains_focused(window, cx);
        self.tabs_drag.cancel(window, cx);
        self.tabs_scroll.set_offset(point(px(0.), px(0.)));
        self.tab_placement = placement;
        if placement == TabPlacement::ToolWindow {
            // Choosing the window presentation must reveal a usable list even if it was closed.
            self.session_state.open_files_visible = true;
        }
        self.sync_open_files_visibility(cx);
        self.ensure_open_files_dock(window, cx);
        if placement == TabPlacement::ToolWindow {
            let id = gpui_base::dock::PanelId::from(self.open_files_panel.entity_id());
            self.dock_presentation.activate(id);
            self.dock_area.update(cx, |area, cx| {
                local_dock::reveal_panel(area, id, window, cx);
            });
        } else if restore_editor_focus {
            self.focus_tab_body(window, cx);
        }
        let owner = cx.entity().downgrade();
        // GPUI consumes pending reveal before refreshing the scroll handle's viewport/axis.
        // Queue against the next frame's fresh geometry, and resolve the current active identity
        // then so a file switch or close during the layout transition cannot reveal a stale index.
        window.on_next_frame(move |window, cx| {
            let _ = owner.update(cx, |app, _| {
                if app.tab_placement == placement {
                    app.reveal_active_tab();
                    window.refresh();
                }
            });
        });
        self.editor_panel.update(cx, |_, cx| cx.notify());
        self.capture_dock_layout(cx);
        self.persist_session();
        cx.notify();
    }

    /// Reveal the live active file or derived comparison after a tab viewport has been laid out.
    fn reveal_active_tab(&self) {
        let visible = self.tabs.iter().filter(|tab| tab.is_visible());
        let selected = if let Some(id) = &self.active_comparison {
            self.comparison_views
                .iter()
                .position(|view| &view.id == id)
                .map(|index| visible.count() + index)
        } else {
            visible.enumerate().find_map(|(index, tab)| {
                (self.active_path.as_deref() == Some(tab.path())).then_some(index)
            })
        };
        if let Some(index) = selected {
            self.tabs_scroll.scroll_to_item(index);
        }
    }

    /// Return keyboard input to the mounted body, never to a background text editor.
    pub(crate) fn focus_tab_body(&self, window: &mut Window, cx: &mut App) {
        if let Some(focus) = self
            .history_focus_handle(cx)
            .or_else(|| self.unified_comparison_focus_handle(cx))
        {
            focus.focus(window, cx);
        } else if self.active_text_tab_index().is_some() {
            self.editor.focus_handle(cx).focus(window, cx);
        } else {
            window.blur(cx);
        }
    }

    /// Place one shared tab strip around any editor body, including history and file-only previews.
    pub(in crate::editor) fn render_editor_with_tabs(
        &self,
        body: impl IntoElement,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let placement = self.tab_placement;
        let body = div()
            .debug_selector(|| "editor-tab-body".into())
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .size_full()
            .overflow_hidden()
            .child(body);
        let layout = div()
            .debug_selector(|| "editor-panel-content".into())
            .flex()
            .size_full()
            .min_w_0()
            .min_h_0()
            .when(
                !matches!(placement, TabPlacement::Left | TabPlacement::Right),
                |el| el.flex_col(),
            );
        match placement {
            TabPlacement::ToolWindow => layout.child(body),
            TabPlacement::Top | TabPlacement::Left => layout
                .child(self.render_tabs(placement, window, cx))
                .child(body),
            TabPlacement::Bottom | TabPlacement::Right => layout
                .child(body)
                .child(self.render_tabs(placement, window, cx)),
        }
        .into_any_element()
    }

    /// The dock panel borrows this exact list instead of maintaining its own document model.
    pub(crate) fn render_open_files(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.render_tabs(TabPlacement::ToolWindow, window, cx)
            .into_any_element()
    }

    /// Project current document metadata into the local strip; reorder callbacks commit only on release.
    fn render_tabs(
        &self,
        placement: TabPlacement,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let vertical = matches!(
            placement,
            TabPlacement::Left | TabPlacement::Right | TabPlacement::ToolWindow
        );
        let axis = if vertical {
            gpui_kit::Axis::Vertical
        } else {
            gpui_kit::Axis::Horizontal
        };
        let tab_styles = component_styles(cx, ThemeComponent::EditorTab);
        let close_styles = component_styles(cx, ThemeComponent::EditorTabClose);
        let visible_count = self.tabs.iter().filter(|tab| tab.is_visible()).count();
        let total_count = visible_count + self.comparison_views.len();
        let tabs = self
            .tabs
            .iter()
            .enumerate()
            .filter(|(_, tab)| tab.is_visible())
            .map(|(index, tab)| {
                let path = tab.path().to_path_buf();
                let is_active =
                    self.active_comparison.is_none() && self.active_path.as_ref() == Some(&path);
                let is_external = !path.starts_with(self.workspace.root());
                let is_dirty = tab.is_dirty();
                let disk_state = tab
                    .text
                    .as_ref()
                    .map_or(DiskState::Synced, |text| text.disk_state);
                let name = if tab.virtual_document.is_some() {
                    format!("{} · {}", tab.title(), t!("editor.readonly_label"))
                } else {
                    tab.title()
                };
                let icon_source = crate::ui::icons::project_icon_path(
                    &self.providers,
                    &path,
                    false,
                    &theme::active_theme(self.dark_theme),
                );
                let icon = gpui_kit::img(icon_source.clone())
                    .debug_selector(move || format!("editor-tab-file-icon-{index}").into())
                    .size_4()
                    .flex_shrink_0();
                let activate_path = path.clone();
                let context_path = path.clone();
                let close_path = path.clone();
                let middle_close_path = path.clone();
                let display_name = format!(
                    "{}{}{}",
                    name,
                    if is_dirty { " ●" } else { "" },
                    match disk_state {
                        DiskState::Synced => "",
                        DiskState::Conflict => " ⚠",
                        DiskState::Deleted => " ×",
                    },
                );
                let drag_preview = FileTabPreview {
                    label: display_name.clone().into(),
                    icon: icon_source.into(),
                    preview: tab.preview,
                    external: is_external,
                };
                // Keep the full document path in the tooltip even when the tab label is truncated.
                let tooltip_path = editor_core::display_path(&path).to_string();
                // Both project and external files use the theme's selected underline color.
                let selected_color = tab_styles.selected.border.unwrap_or(cx.theme().primary);
                let separator_color = tab_styles.base.border.unwrap_or(cx.theme().border);
                let external_background = gpui_kit::rgb(0xfff4c2);
                let element = editor_ui::file_tab(
                    format!("editor-tab:{}", path.to_string_lossy()),
                    name.clone(),
                    is_active,
                    index,
                    total_count,
                )
                .debug_selector(move || format!("editor-tab-{index}").into())
                .relative()
                .h_full()
                .gap_2()
                .px(px(tab_styles.base.padding_x_px.unwrap_or(8.)))
                .text_size(px(if is_active {
                    tab_styles
                        .selected
                        .font_size_px
                        .or(tab_styles.base.font_size_px)
                } else {
                    tab_styles.base.font_size_px
                }
                .unwrap_or(14.)))
                .when(!vertical, |style| style.border_r_1())
                .when(vertical, |style| style.border_b_1())
                .border_color(separator_color)
                .when(is_active, |style| {
                    // The selected edge always faces the editor; dock lists use their right edge.
                    let style = style.border_r_0().border_b_0().border_color(selected_color);
                    match placement {
                        TabPlacement::Top => style.border_b_1(),
                        TabPlacement::Bottom => style.border_t_1(),
                        TabPlacement::Left | TabPlacement::ToolWindow => style.border_r_1(),
                        TabPlacement::Right => style.border_l_1(),
                    }
                })
                .bg(if is_external {
                    external_background.into()
                } else if is_active {
                    tab_styles
                        .selected
                        .background
                        .unwrap_or(cx.theme().background)
                } else {
                    tab_styles.base.background.unwrap_or(cx.theme().tab_bar)
                })
                .text_color(if is_external {
                    cx.theme().muted_foreground
                } else if is_active {
                    selected_color
                } else {
                    tab_styles
                        .base
                        .foreground
                        .unwrap_or(cx.theme().tab_foreground)
                })
                .hover(|style| {
                    style
                        .bg(if is_external {
                            external_background.into()
                        } else {
                            tab_styles.hover.background.unwrap_or(if is_active {
                                tab_styles
                                    .selected
                                    .background
                                    .unwrap_or(cx.theme().background)
                            } else {
                                tab_styles.base.background.unwrap_or(cx.theme().tab_bar)
                            })
                        })
                        .text_color(if is_external {
                            cx.theme().muted_foreground
                        } else if is_active {
                            selected_color
                        } else {
                            tab_styles.hover.foreground.unwrap_or(cx.theme().foreground)
                        })
                })
                .tooltip(move |window, cx| Tooltip::new(tooltip_path.clone()).build(window, cx))
                .tooltip_show_delay(Duration::from_millis(1_200))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(px(16.))
                        .flex_shrink_0()
                        .child(icon),
                )
                // Draw the right separator independently so it never inherits blue.
                .when(is_active && !vertical, |this| {
                    this.child(
                        div()
                            .absolute()
                            .right_0()
                            .top_0()
                            .bottom(px(1.))
                            .w(px(1.))
                            .bg(separator_color),
                    )
                })
                .child(
                    editor_ui::FileTabTitle::new(format!("editor-tab-title-{index}"), display_name)
                        .debug_selector(move || format!("editor-tab-title-{index}").into())
                        .when(vertical, |style| style.flex_1())
                        .text_sm()
                        .when(is_external, |style| style.italic()),
                )
                .child(editor_ui::file_tab_actions(
                    tab.preview,
                    div()
                        .id(format!("close-editor-tab:{}", path.to_string_lossy()))
                        .debug_selector(move || format!("editor-tab-close-{index}").into())
                        .flex()
                        .items_center()
                        .justify_center()
                        .flex_shrink_0()
                        .rounded(px(close_styles.base.radius_px.unwrap_or(3.)))
                        .p_1()
                        .hover(|style| {
                            style.bg(close_styles
                                .hover
                                .background
                                .unwrap_or(cx.theme().list_hover))
                        })
                        .child(Icon::new(IconName::Close).xsmall())
                        .on_click(cx.listener(move |this, _, window, cx| {
                            cx.stop_propagation();
                            this.close_tab(close_path.clone(), window, cx);
                        })),
                    cx,
                ))
                .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                    // A tab click activates existing content and follows the explorer preference.
                    if let Some(index) =
                        this.tabs.iter().position(|tab| tab.path() == activate_path)
                    {
                        if event.click_count() == 2 {
                            this.keep_tab(index);
                        }
                        this.activate_tab(index, window, cx);
                    }
                }))
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |app, event: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        if let Some(index) =
                            app.tabs.iter().position(|tab| tab.path() == context_path)
                        {
                            app.activate_tab(index, window, cx);
                            app.open_file_provider_menu(event.position, window, cx);
                        }
                    }),
                )
                .on_mouse_down(
                    MouseButton::Middle,
                    cx.listener(move |this, _, window, cx| {
                        window.prevent_default();
                        cx.stop_propagation();
                        this.close_tab(middle_close_path.clone(), window, cx);
                    }),
                );
                (tab.file_id, drag_preview, element)
            });

        let mut strip = FileTabStrip::new(
            "editor-tabs-scroll",
            self.tabs_drag.clone(),
            self.tabs_scroll.clone(),
        )
        .axis(axis)
        .on_reorder(cx.listener(|app, event: &FileTabMove, _, cx| {
            app.reorder_file_tab(event.source, event.before, cx);
        }));
        for (id, preview, tab) in tabs {
            strip = strip.tab(id, preview, tab);
        }
        for (index, view) in self.comparison_views.iter().enumerate() {
            let activate = cx.entity().downgrade();
            let close = activate.clone();
            let activate_id = view.id.clone();
            let close_id = view.id.clone();
            // Derived views retain their own close/activation semantics and follow file tabs.
            strip = strip.trailing(
                editor_ui::comparison_tab(
                    view.id.clone(),
                    view.title.clone(),
                    self.active_comparison.as_ref() == Some(&view.id),
                    axis,
                    view.preview,
                    visible_count + index,
                    total_count,
                    move |pin, window, cx| {
                        let _ = activate.update(cx, |app, cx| {
                            app.activate_comparison_view(&activate_id, pin, window, cx)
                        });
                    },
                    move |window, cx| {
                        let _ = close.update(cx, |app, cx| {
                            app.close_comparison_view(&close_id, window, cx)
                        });
                    },
                    cx,
                )
                .when(self.active_comparison.as_ref() == Some(&view.id), |tab| {
                    // Derived views share the same inward-facing selection edge as ordinary files.
                    let tab = tab.border_b_0().border_r_0();
                    match placement {
                        TabPlacement::Top => tab.border_b_1(),
                        TabPlacement::Bottom => tab.border_t_1(),
                        TabPlacement::Left | TabPlacement::ToolWindow => tab.border_r_1(),
                        TabPlacement::Right => tab.border_l_1(),
                    }
                }),
            );
        }

        div()
            .id("editor-tabs-container")
            .debug_selector(|| "editor-tabs-container".into())
            .relative()
            .flex_shrink_0()
            .min_w_0()
            .min_h_0()
            .when(!vertical, |el| el.w_full().h(px(PANEL_HEADER_HEIGHT)))
            .when(vertical && placement != TabPlacement::ToolWindow, |el| {
                el.w(px(200.)).max_w(gpui_kit::relative(0.4)).h_full()
            })
            .when(placement == TabPlacement::ToolWindow, |el| el.size_full())
            .map(|el| match placement {
                TabPlacement::Top => el.border_b_1(),
                TabPlacement::Bottom => el.border_t_1(),
                TabPlacement::Left => el.border_r_1(),
                TabPlacement::Right => el.border_l_1(),
                TabPlacement::ToolWindow => el,
            })
            .border_color(
                component_styles(cx, ThemeComponent::EditorTabs)
                    .base
                    .border
                    .unwrap_or(cx.theme().border),
            )
            .bg(component_styles(cx, ThemeComponent::EditorTabs)
                .base
                .background
                .unwrap_or(cx.theme().tab_bar))
            .child(strip)
            // The shared overlay retains native dragging and contracts vertically at idle.
            .child(
                editor_ui::vertical_scrollbar(&self.tabs_scroll, cx).axis(if vertical {
                    gpui_base::ScrollbarAxis::Vertical
                } else {
                    gpui_base::ScrollbarAxis::Horizontal
                }),
            )
    }
}
