//! Renders the explorer, document tabs, and editor.

use crate::*;
use editor_ui::scroll::{ScrollAxis, ScrollableElement as _, SmoothScrollable as _};

impl EditorApp {
    pub(crate) fn render_file_tree(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        let tree_style = component_styles(cx, ThemeComponent::ExplorerTree).base;
        let row_styles = component_styles(cx, ThemeComponent::ExplorerRow);
        let tree_scroll = self.tree_state.read(cx).scroll_handle().clone();
        // All visible rows belong to this render's theme. Resolve and clone its
        // definition once, rather than repeating catalog locks and cloning per row.
        let icon_theme = theme::active_theme(self.dark_theme);
        let tree = gpui_base::Tree::new(&self.tree_state)
            .item(move |index, entry, entry_state, _window, cx| {
                let selected = entry_state.is_selected();
                view.update(cx, |app, cx| {
                    let item = entry.item();
                    // Only explicit selection changes the row appearance; pointer hover is inert.
                    let row_style = if selected {
                        row_styles.selected
                    } else {
                        row_styles.base
                    };
                    // Keep the last project selection visible but muted while an external tab is active.
                    let inactive_selection = selected
                        && app
                            .active_path
                            .as_ref()
                            .is_some_and(|path| !path.starts_with(app.workspace.root()));
                    let row_background = if inactive_selection {
                        cx.theme().list_hover
                    } else {
                        row_style.background.unwrap_or(cx.theme().background)
                    };
                    let row_path = PathBuf::from(item.id.as_str());
                    // Classification shares the displayed tree's scan generation, including empty folders.
                    let is_folder = app.workspace_directories.contains(row_path.as_path());
                    let decoration = app.explorer_file_decoration(&row_path, is_folder, cx);
                    let drop_target = app.explorer_drag.target() == Some(row_path.as_path());
                    let icon = file_icon(
                        &app.providers,
                        Path::new(item.id.as_str()),
                        is_folder,
                        &icon_theme,
                    )
                    // Slightly enlarge explorer icons without changing tab-strip icon sizing.
                    .size(px(16.));
                    let disclosure = if is_folder {
                        Icon::new(if entry.is_expanded() {
                            IconName::ChevronDown
                        } else {
                            IconName::ChevronRight
                        })
                        .xsmall()
                        .into_any_element()
                    } else {
                        div().size(px(12.)).into_any_element()
                    };
                    div()
                        .id(format!("explorer-row-{index}"))
                        .debug_selector(move || format!("explorer-row-{index}").into())
                        .w_full()
                        .min_h(px(24.))
                        .relative()
                        // Center the content within the whole row, including its minimum height.
                        .flex()
                        .items_center()
                        .bg(row_background)
                        .when(drop_target, |row| {
                            row.border_1().border_color(cx.theme().primary)
                        })
                        .when(selected, |this| {
                            this.rounded(px(row_style.radius_px.unwrap_or(5.)))
                        })
                        .text_color(row_style.foreground.unwrap_or(cx.theme().foreground))
                        .py(px(row_style.padding_y_px.unwrap_or(0.3)))
                        .px(px(row_style.padding_x_px.unwrap_or(4.)))
                        .pl(px(14.) * entry.depth() + px(8.))
                        // Selection is indicated by its background, without a leading accent border.
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, window, cx| {
                                // Select on press; expansion waits for a full click so dragging stays inert.
                                cx.stop_propagation();
                                this.select_explorer_row(index, window, cx);
                            }),
                        )
                        .on_mouse_down(
                            MouseButton::Right,
                            cx.listener({
                                let path = PathBuf::from(item.id.as_str());
                                move |this, event: &MouseDownEvent, window, cx| {
                                    cx.stop_propagation();
                                    this.select_explorer_row(index, window, cx);
                                    this.open_explorer_menu(
                                        Some(path.clone()),
                                        event.position,
                                        window,
                                        cx,
                                    );
                                }
                            }),
                        )
                        .when(row_path != app.workspace.root(), |row| {
                            row.on_drag(
                                explorer::drag::TreeDrag {
                                    path: row_path.clone(),
                                },
                                |drag, _, _, cx| cx.new(|_| drag.clone()),
                            )
                        })
                        .child(app.render_tree_row_geometry(&row_path, is_folder, cx))
                        .on_drag_move(cx.listener({
                            let path = row_path.clone();
                            move |app,
                                  event: &gpui_kit::DragMoveEvent<explorer::drag::TreeDrag>,
                                  window,
                                  cx| {
                                app.tree_drag_row(
                                    &path,
                                    event.bounds,
                                    event.event.position,
                                    window,
                                    cx,
                                );
                            }
                        }))
                        .on_drag_move(cx.listener({
                            let path = row_path;
                            move |app,
                                  event: &gpui_kit::DragMoveEvent<gpui_kit::ExternalPaths>,
                                  window,
                                  cx| {
                                app.tree_drag_row(
                                    &path,
                                    event.bounds,
                                    event.event.position,
                                    window,
                                    cx,
                                );
                            }
                        }))
                        .child(
                            h_flex()
                                .w_full()
                                .min_w(px(0.))
                                .items_center()
                                .gap_1()
                                .child(
                                    div()
                                        .id(format!("explorer-disclosure-{index}"))
                                        .debug_selector(move || {
                                            format!("explorer-disclosure-{index}").into()
                                        })
                                        // Center the 12 px glyph inside a slightly larger click target.
                                        .size(px(16.))
                                        .flex_shrink_0()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .when(is_folder, |arrow| {
                                            arrow
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    cx.listener(move |this, _, window, cx| {
                                                        cx.stop_propagation();
                                                        this.select_explorer_row(index, window, cx);
                                                    }),
                                                )
                                                .on_click(cx.listener({
                                                    let path = PathBuf::from(item.id.as_str());
                                                    move |this, _, _, cx| {
                                                        // Consume arrow clicks so the row cannot toggle twice.
                                                        cx.stop_propagation();
                                                        this.toggle_explorer_directory(&path, cx);
                                                    }
                                                }))
                                        })
                                        .child(disclosure),
                                )
                                .child(
                                    h_flex()
                                        .id(format!("explorer-icon-{index}"))
                                        .debug_selector(move || {
                                            format!("explorer-icon-{index}").into()
                                        })
                                        .size(px(16.))
                                        .flex_shrink_0()
                                        .items_center()
                                        .justify_center()
                                        .child(icon),
                                )
                                .child(
                                    div()
                                        .id(format!("explorer-label-{index}"))
                                        .debug_selector(move || {
                                            format!("explorer-label-{index}").into()
                                        })
                                        .flex_1()
                                        .min_w(px(0.))
                                        .truncate()
                                        .when_some(decoration, |label, (color, explanation)| {
                                            label.text_color(color).tooltip(move |window, cx| {
                                                editor_ui::Tooltip::new(explanation.clone())
                                                    .build(window, cx)
                                            })
                                        })
                                        .child(item.label.clone()),
                                ),
                        )
                        .on_click(cx.listener({
                            let item = item.clone();
                            move |this, event: &ClickEvent, window, cx| {
                                // The host owns activation; Base's parent click must not toggle again.
                                cx.stop_propagation();
                                if is_folder {
                                    let click_count =
                                        if this.session_state.explorer_expand_on_double_click {
                                            2
                                        } else {
                                            1
                                        };
                                    // Ignore the second press in single-click mode instead of undoing the first.
                                    if event.click_count() == click_count {
                                        this.toggle_explorer_directory(
                                            Path::new(item.id.as_str()),
                                            cx,
                                        );
                                    }
                                } else {
                                    let path = PathBuf::from(item.id.as_str());
                                    if this.explorer_filter.visible {
                                        this.hide_explorer_filter(window, cx);
                                        this.select_file_in_tree(&path, cx);
                                    }
                                    if event.click_count() == 2 {
                                        this.open_file(path, window, cx);
                                    } else {
                                        // Browsing keeps the tree's focus and reuses one disposable tab.
                                        this.open_preview_file(path, window, cx);
                                    }
                                }
                            }
                        }))
                        .into_any_element()
                })
            })
            .list_style(StyleRefinement::default().flex_grow_1().size_full())
            .p_1()
            // Explorer and editor text share the user's live font size by default.
            .text_size(
                tree_style
                    .font_size_px
                    .map(px)
                    .unwrap_or(typography::font_size(cx)),
            )
            .font_family(cx.theme().mono_font_family.clone())
            .flex_1()
            .min_h_0()
            .bg(tree_style.background.unwrap_or(cx.theme().background))
            .text_color(tree_style.foreground.unwrap_or(cx.theme().foreground))
            .smooth_scroll(&tree_scroll, ScrollAxis::Vertical);

        v_flex()
            .id("explorer-root")
            .key_context("ExplorerFiles")
            .debug_selector(|| "explorer-root".into())
            .size_full()
            .min_h_0()
            .bg(tree_style.background.unwrap_or(cx.theme().background))
            .relative()
            .on_hover(cx.listener(|app, hovered, _, _| {
                app.explorer_filter.body_hovered = *hovered;
            }))
            .on_drag_move(cx.listener(Self::tree_drag_move))
            .on_drag_move(cx.listener(Self::external_tree_drag_move))
            .capture_key_down(cx.listener(Self::cancel_explorer_drag))
            .capture_key_down(
                cx.listener(|app, event: &gpui_kit::KeyDownEvent, window, cx| {
                    if event.keystroke.key == "enter" && app.explorer_filter.visible {
                        let path = app
                            .tree_state
                            .read(cx)
                            .selected_item()
                            .map(|item| PathBuf::from(item.id.as_str()));
                        if let Some(path) =
                            path.filter(|path| !app.workspace_directories.contains(path))
                        {
                            app.hide_explorer_filter(window, cx);
                            app.open_file(path, window, cx);
                            cx.stop_propagation();
                        }
                    }
                }),
            )
            .on_action(cx.listener(
                |app, _: &explorer::transfer::shortcuts::UndoFiles, window, cx| {
                    app.undo_file_transfer(window, cx)
                },
            ))
            .on_action(cx.listener(
                |app, _: &explorer::transfer::shortcuts::RedoFiles, window, cx| {
                    app.redo_file_transfer(window, cx)
                },
            ))
            .on_action(cx.listener(
                |app, _: &explorer::transfer::shortcuts::PasteFiles, window, cx| {
                    let selected = app
                        .tree_state
                        .read(cx)
                        .selected_item()
                        .map(|item| PathBuf::from(item.id.as_str()));
                    app.paste_file_offer(selected.as_deref(), window, cx);
                },
            ))
            .children(self.render_explorer_filter(cx))
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .child(self.render_tree_geometry_start(cx))
                    .child(tree)
                    .child(
                        div()
                            .absolute()
                            .right_0()
                            .top_0()
                            .bottom_0()
                            .w(px(8.))
                            .child(editor_ui::vertical_scrollbar(&tree_scroll, cx)),
                    ),
            )
            .child(self.render_tree_drag_events(cx))
            .when_some(self.explorer_drag.intent(), |root, copy| {
                root.child(div().absolute().bottom_0().left_0().child(
                    editor_ui::file_operation::drag_preview(String::new(), copy, cx),
                ))
            })
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    this.open_explorer_menu(None, event.position, window, cx);
                }),
            )
    }

    pub(crate) fn render_editor_panel(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        if self.history.panel.is_some() {
            return self.render_editor_with_tabs(self.render_history_panel(cx), window, cx);
        }
        let style = component_styles(cx, ThemeComponent::Editor).base;
        // No document means no input handler, gutter, tab strip or drop caret.
        if self.tabs.is_empty() {
            return v_flex()
                .debug_selector(|| "editor-panel-content".into())
                .size_full()
                .min_h_0()
                .child(editor_ui::empty_editor_canvas(
                    t!("editor.select_file").to_string(),
                    style.background.unwrap_or(cx.theme().background),
                    cx.theme().muted_foreground,
                ))
                .into_any_element();
        }
        if self.active_text_tab_index().is_none() {
            // A file-only body never mounts the retained background editor or its native input handler.
            if self.editor.focus_handle(cx).is_focused(window) {
                window.blur(cx);
            }
            let body = if let Some(preview) = self.active_editor_preview(cx) {
                let failed = preview.read(cx).file_preview_failed();
                let file_id = self
                    .active_tab_index()
                    .map(|index| self.tabs[index].file_id);
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .relative()
                    .child(preview)
                    .when(failed, |body| {
                        body.child(
                            div().absolute().top_2().right_2().child(
                                Button::new("retry-image-resource")
                                    .label(t!("file_view.retry").to_string())
                                    .small()
                                    .on_click(cx.listener(move |app, _, _, cx| {
                                        if app
                                            .active_tab_index()
                                            .map(|index| app.tabs[index].file_id)
                                            == file_id
                                        {
                                            app.retry_file_view(cx);
                                        }
                                    })),
                            ),
                        )
                    })
                    .into_any_element()
            } else {
                let message = self.file_view_unavailable_reason(cx);
                let file_id = self
                    .active_tab_index()
                    .map(|index| self.tabs[index].file_id);
                v_flex()
                    .debug_selector(|| "file-view-unavailable".into())
                    .flex_1()
                    .min_h_0()
                    .p_4()
                    .gap_3()
                    // A native fault may include a long WASM trace. Keep recovery outside the
                    // bounded diagnostic scroller so the real button remains inside the viewport.
                    .child(
                        div()
                            .id("file-view-diagnostic")
                            .debug_selector(|| "file-view-diagnostic".into())
                            .flex_1()
                            .min_h_0()
                            .w_full()
                            .overflow_y_scrollbar()
                            .text_color(cx.theme().muted_foreground)
                            .child(message),
                    )
                    .child(
                        // Recovery remains separately measurable even when the failure message changes size.
                        div()
                            .flex_shrink_0()
                            .flex()
                            .justify_center()
                            .w_full()
                            .debug_selector(|| "file-view-retry".into())
                            .child(
                                Button::new("retry-file-view")
                                    // Native acceptance clicks the button's own hitbox, not its full-width wrapper.
                                    .debug_selector(|| "file-view-retry-button".into())
                                    .label(t!("file_view.retry").to_string())
                                    .small()
                                    .on_click(cx.listener(move |app, _, _, cx| {
                                        if app
                                            .active_tab_index()
                                            .map(|index| app.tabs[index].file_id)
                                            == file_id
                                        {
                                            app.retry_file_view(cx);
                                        }
                                    })),
                            ),
                    )
                    .into_any_element()
            };
            return self.render_editor_with_tabs(body, window, cx);
        }
        let body = if let Some(comparison) = self.render_document_comparison(window, cx) {
            comparison
        } else if let Some(preview) = self.active_editor_preview(cx) {
            self.render_editor_preview_body(preview, window, cx)
        } else {
            self.render_native_editor(window, cx)
        };
        let body = v_flex()
            .size_full()
            .min_w_0()
            .min_h_0()
            // Keep the editor's flexible height when the body is either a source pane or a split.
            .child(self.render_history_recovery_notice(cx))
            .child(v_flex().flex_1().min_w_0().min_h_0().child(body));
        self.render_editor_with_tabs(body, window, cx)
    }
}
