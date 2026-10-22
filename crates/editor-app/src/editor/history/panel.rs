//! Native history navigation composes local controls around bounded readonly source previews.

use super::*;
use editor_history::ClearScope;
use editor_ui::Input;
use editor_ui::scroll::ScrollableElement as _;
use gpui_kit::AnyElement;

impl EditorApp {
    /// A recovered pending document exposes relocation without making the user repeat recovery.
    pub(crate) fn render_history_recovery_notice(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(file) = self
            .active_tab_index()
            .and_then(|index| self.tabs.get(index))
        else {
            return div().into_any_element();
        };
        if !self
            .history
            .pending_create
            .get(&file.file_id)
            .is_some_and(|recovery| recovery.needs_create)
        {
            return div().into_any_element();
        }
        let id = file.file_id;
        h_flex()
            .w_full()
            .p_2()
            .gap_2()
            .flex_wrap()
            .items_center()
            .bg(cx.theme().popover)
            .child(
                div()
                    .text_sm()
                    .child(t!("history.restored_pending").to_string()),
            )
            .child(
                Button::new("history-recovery-save-as")
                    .debug_selector(|| "history-recovery-save-as".into())
                    .label(t!("history.choose_destination").to_string())
                    .small()
                    .on_click(cx.listener(move |app, _, window, cx| {
                        app.relocate_history_recovery(id, window, cx)
                    })),
            )
            .into_any_element()
    }

    /// Render metadata and readonly history; the ordinary document remains the sole writable session.
    pub(crate) fn render_history_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(panel) = &self.history.panel else {
            return div().into_any_element();
        };
        let categories = [
            Category::Workspace,
            Category::Deleted,
            Category::Standalone,
            Category::Legacy,
            Category::All,
        ];
        let category_buttons = categories.into_iter().enumerate().map(|(index, category)| {
            let label = category.label();
            let selected =
                std::mem::discriminant(&panel.category) == std::mem::discriminant(&category);
            Button::new(format!("history-category-{index}"))
                .label(label)
                .small()
                .ghost()
                .selected(selected)
                .on_click(cx.listener(move |app, _, window, cx| {
                    app.open_history_category(category.clone(), window, cx)
                }))
        });
        let rows =
            panel.rows.iter().enumerate().map(|(index, item)| {
                let title = item.title();
                Button::new(format!("history-item-{index}"))
                    .debug_selector(move || format!("history-item-{index}").into())
                    .accessibility_label(format!("{} · {} · {}", item.time(), item.source(), title))
                    .tooltip(title.clone())
                    .selected(panel.selected == Some(index))
                    .ghost()
                    .content_full_width()
                    .w_full()
                    .h_auto()
                    .py_2()
                    .child(
                        v_flex()
                            .w_full()
                            .min_w_0()
                            .gap_1()
                            .child(div().w_full().truncate().text_xs().child(item.time()))
                            .child(div().w_full().truncate().text_xs().child(item.source()))
                            .child(div().w_full().truncate().text_sm().child(item.path_label())),
                    )
                    .on_click(cx.listener(move |app, _, window, cx| {
                        app.select_history_item(index, window, cx)
                    }))
            });
        let selected = panel.selected.and_then(|index| panel.rows.get(index));
        let exportable = selected.is_some_and(Item::has_content);
        let restorable = panel.preview.is_some() && matches!(selected, Some(Item::Version(_)));
        let one_scope = selected.map(|item| match item {
            Item::Version(row) => ClearScope::Version(row.id),
            Item::Legacy(row) => ClearScope::Legacy(row.id.clone()),
        });
        let file_scope = selected.and_then(|item| match item {
            Item::Version(row) => Some(ClearScope::File(row.file)),
            _ => None,
        });
        let body = if let Some(comparison) = &panel.comparison {
            h_flex()
                .size_full()
                .min_h_0()
                .min_w_0()
                .children(
                    (0..2)
                        .filter_map(|side| comparison.alignment.pane(side))
                        .map(|pane| div().flex_1().min_w_0().h_full().child(pane)),
                )
                .into_any_element()
        } else if let Some(preview) = &panel.preview {
            div()
                .size_full()
                .min_h_0()
                .child(Editor::new(preview).size_full())
                .into_any_element()
        } else {
            div()
                .size_full()
                .p_4()
                .text_color(cx.theme().muted_foreground)
                .child(if panel.busy {
                    t!("history.loading").to_string()
                } else {
                    t!("history.select_version").to_string()
                })
                .into_any_element()
        };
        v_flex()
            .debug_selector(|| "history-panel".into())
            .size_full()
            .min_h_0()
            .min_w_0()
            .bg(cx.theme().background)
            .key_context("LocalHistory")
            .on_key_down(
                cx.listener(|app, event: &gpui_kit::KeyDownEvent, window, cx| {
                    if event.keystroke.key == "escape" {
                        cx.stop_propagation();
                        app.close_history(window, cx);
                    }
                }),
            )
            .child(
                h_flex()
                    .w_full()
                    .flex_wrap()
                    .items_center()
                    .gap_1()
                    .p_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .font_semibold()
                            .mr_2()
                            .child(t!("history.title").to_string()),
                    )
                    .when(matches!(panel.category, Category::File(_)), |bar| {
                        bar.child(div().text_sm().mr_2().child(panel.category.label()))
                    })
                    .children(category_buttons)
                    .child(div().flex_1())
                    .child(
                        Button::new("history-refresh")
                            .label(t!("history.refresh").to_string())
                            .small()
                            .on_click(cx.listener(|app, _, _, cx| app.refresh_history(cx))),
                    )
                    .child(
                        Button::new("history-close")
                            .debug_selector(|| "history-close".into())
                            .label(t!("history.close").to_string())
                            .small()
                            .on_click(
                                cx.listener(|app, _, window, cx| app.close_history(window, cx)),
                            ),
                    ),
            )
            .children(panel.clear.as_ref().map(|confirmation| {
                v_flex()
                    .p_3()
                    .gap_2()
                    .bg(cx.theme().popover)
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        t!(
                            "history.clear_preview",
                            scope = &confirmation.label,
                            count = confirmation.preview.versions + confirmation.preview.legacy,
                            bytes = confirmation.preview.reclaimable_bytes
                        )
                        .to_string(),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("history-clear-confirm")
                                    .debug_selector(|| "history-clear-confirm".into())
                                    .label(t!("history.confirm_clear").to_string())
                                    .danger()
                                    .on_click(
                                        cx.listener(|app, _, _, cx| app.confirm_history_clear(cx)),
                                    ),
                            )
                            .child(
                                Button::new("history-clear-cancel")
                                    .label(t!("history.cancel").to_string())
                                    .on_click(cx.listener(|app, _, _, cx| {
                                        if let Some(panel) = &mut app.history.panel {
                                            panel.clear = None;
                                        }
                                        cx.notify();
                                    })),
                            ),
                    )
            }))
            .children(panel.error.as_ref().map(|error| {
                div()
                    .p_2()
                    .text_sm()
                    .text_color(cx.theme().danger)
                    .child(error.clone())
            }))
            .children(panel.notice.as_ref().map(|notice| {
                div()
                    .p_2()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(notice.clone())
            }))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .child(
                        v_flex()
                            .w(px(290.))
                            .min_w(px(180.))
                            .h_full()
                            .flex_shrink_0()
                            .border_r_1()
                            .border_color(cx.theme().border)
                            .child(
                                div().p_2().child(
                                    Input::new(&panel.filter)
                                        .accessibility_label(t!("history.filter").to_string()),
                                ),
                            )
                            .child(
                                v_flex()
                                    .id("history-rows")
                                    .debug_selector(|| "history-rows".into())
                                    .flex_1()
                                    .min_h_0()
                                    .p_1()
                                    .track_scroll(&panel.scroll)
                                    .overflow_y_scrollbar()
                                    .gap_1()
                                    .children(rows)
                                    .when(panel.rows.is_empty(), |list| {
                                        list.child(div().p_3().child(if panel.busy {
                                            t!("history.loading").to_string()
                                        } else {
                                            t!("history.empty").to_string()
                                        }))
                                    }),
                            )
                            .child(
                                h_flex()
                                    .p_2()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        Button::new("history-previous")
                                            .label(t!("history.previous").to_string())
                                            .small()
                                            .disabled(panel.busy || panel.pages.len() <= 1)
                                            .on_click(cx.listener(|app, _, _, cx| {
                                                app.history_page(false, cx)
                                            })),
                                    )
                                    .child(div().text_xs().child(panel.pages.len().to_string()))
                                    .child(
                                        Button::new("history-next")
                                            .debug_selector(|| "history-next".into())
                                            .label(t!("history.next").to_string())
                                            .small()
                                            .disabled(panel.busy || panel.rows.len() < 50)
                                            .on_click(cx.listener(|app, _, _, cx| {
                                                app.history_page(true, cx)
                                            })),
                                    ),
                            ),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .min_h_0()
                            .child(
                                h_flex()
                                    .p_2()
                                    .gap_2()
                                    .flex_wrap()
                                    .child(
                                        Button::new("history-compare")
                                            .label(t!("history.compare").to_string())
                                            .small()
                                            .disabled(!restorable || panel.busy)
                                            .on_click(cx.listener(|app, _, window, cx| {
                                                app.compare_history_selection(window, cx)
                                            })),
                                    )
                                    .child(
                                        Button::new("history-restore")
                                            .debug_selector(|| "history-restore".into())
                                            .label(t!("history.restore").to_string())
                                            .primary()
                                            .small()
                                            .disabled(!restorable || panel.busy)
                                            .on_click(cx.listener(|app, _, window, cx| {
                                                app.restore_history_selection(window, cx)
                                            })),
                                    )
                                    .child(
                                        Button::new("history-export")
                                            .label(t!("history.export").to_string())
                                            .small()
                                            .disabled(!exportable || panel.busy)
                                            .on_click(cx.listener(|app, _, window, cx| {
                                                app.export_history_selection(window, cx)
                                            })),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_h_0()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .child(body),
                            ),
                    ),
            )
            .child(
                h_flex()
                    .w_full()
                    .p_2()
                    .gap_2()
                    .flex_wrap()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(
                        Button::new("history-clear-entry")
                            .label(t!("history.clear_entry").to_string())
                            .small()
                            .disabled(one_scope.is_none() || panel.busy)
                            .on_click(cx.listener(move |app, _, _, cx| {
                                if let Some(scope) = one_scope.clone() {
                                    app.preview_history_clear(
                                        scope,
                                        t!("history.clear_entry").to_string(),
                                        cx,
                                    );
                                }
                            })),
                    )
                    .child(
                        Button::new("history-clear-file")
                            .label(t!("history.clear_file").to_string())
                            .small()
                            .disabled(file_scope.is_none() || panel.busy)
                            .on_click(cx.listener(move |app, _, _, cx| {
                                if let Some(scope) = file_scope.clone() {
                                    app.preview_history_clear(
                                        scope,
                                        t!("history.clear_file").to_string(),
                                        cx,
                                    );
                                }
                            })),
                    )
                    .child(
                        Button::new("history-clear-workspace")
                            .label(t!("history.clear_workspace").to_string())
                            .small()
                            .disabled(panel.busy)
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.preview_history_clear(
                                    ClearScope::Workspace(app.workspace.root().to_owned()),
                                    t!("history.clear_workspace").to_string(),
                                    cx,
                                )
                            })),
                    )
                    .child(
                        Button::new("history-clear-all")
                            .label(t!("history.clear_all").to_string())
                            .small()
                            .disabled(panel.busy)
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.preview_history_clear(
                                    ClearScope::All,
                                    t!("history.clear_all").to_string(),
                                    cx,
                                )
                            })),
                    ),
            )
            .into_any_element()
    }
}
