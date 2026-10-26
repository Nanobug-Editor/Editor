//! Draws editor popovers locally while retaining the upstream editing engine.

use crate::*;
use editor_ui::scroll::ScrollableElement as _;
use gpui_base::input::{Backspace, Delete, Enter, Escape, InputOverlayKind, MoveDown, MoveUp};
use gpui_kit::{AnyElement, BoxShadow, Hsla, StyledText, deferred, relative};
use lsp_types::{Documentation, HoverContents, MarkedString};

#[cfg(test)]
#[path = "completion_tests.rs"]
mod completion_tests;

#[path = "completion_refresh.rs"]
mod completion_refresh;

/// Read-only geometry shared by source-owned overlays; text and native session stay in EditorState.
#[derive(Clone)]
pub(super) struct SourceProjection {
    group: editor_ui::aligned_editor::AlignmentGroup,
    index: usize,
}

impl SourceProjection {
    /// Retains the same measured group, never a copied geometry snapshot or editor instance.
    pub(super) fn new(group: editor_ui::aligned_editor::AlignmentGroup, index: usize) -> Self {
        Self { group, index }
    }

    /// Projects a native window rectangle; unavailable/stale layout suppresses that overlay.
    pub(super) fn bounds(&self, native: Bounds<Pixels>, cx: &App) -> Option<Bounds<Pixels>> {
        self.group.project_bounds(self.index, native, cx)
    }

    /// The physical input clip, independent from the complete native authority height.
    pub(super) fn viewport(&self, cx: &App) -> Option<Bounds<Pixels>> {
        self.group.viewport(self.index, cx)
    }

    /// Uses current byte-range geometry for a source anchor, including a collapsed caret.
    fn range(&self, range: std::ops::Range<usize>, cx: &App) -> Option<Bounds<Pixels>> {
        self.group.project_range(self.index, range, cx)
    }
}

/// Retain the selection, viewport and pending request across completion repaints.
#[derive(Default)]
pub(crate) struct CompletionPopupState {
    selection: Cell<(u64, usize)>,
    scroll: ScrollHandle,
    /// A pending response may only replace the menu revision it was requested for.
    refresh_revision: Cell<Option<u64>>,
}

impl CompletionPopupState {
    /// Discard document-local selection and requests when the popup closes or tabs change.
    pub(crate) fn reset(&self) {
        self.selection.set((0, 0));
        self.scroll.set_offset(point(px(0.), px(0.)));
        self.refresh_revision.set(None);
    }
}

/// Retain the visible card through focus transfer into its selectable content.
pub(crate) struct DefinitionPopupFocus {
    handle: FocusHandle,
    /// Last painted card in physical window coordinates, including its resize edges.
    pub(super) bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// A press inside the card owns selection/resizing even after leaving its rectangle.
    pub(super) pointer_gesture: Cell<bool>,
    hover: Rc<std::cell::RefCell<Option<gpui_base::input::HoverPopoverState>>>,
    /// Diagnostic text uses the same focus boundary as type documentation.
    diagnostic: Rc<std::cell::RefCell<Option<gpui_base::input::DiagnosticEntry>>>,
    /// Invalidate deferred focus restoration when details are explicitly closed.
    generation: Rc<Cell<u64>>,
    _subscription: Subscription,
}

impl DefinitionPopupFocus {
    /// Restore after native blur listeners finish, without taking text-selection focus.
    pub(crate) fn new(window: &mut Window, cx: &mut Context<EditorApp>) -> Self {
        let handle = cx.focus_handle();
        let hover = Rc::new(std::cell::RefCell::new(
            None::<gpui_base::input::HoverPopoverState>,
        ));
        let hovered = hover.clone();
        let diagnostic = Rc::new(std::cell::RefCell::new(
            None::<gpui_base::input::DiagnosticEntry>,
        ));
        let diagnosed = diagnostic.clone();
        let generation = Rc::new(Cell::new(0u64));
        let restoration_generation = generation.clone();
        let focused = handle.clone();
        let subscription = cx.on_focus_in(&handle, window, move |app, window, cx| {
            let hover = hovered.borrow().clone();
            let diagnostic = diagnosed.borrow().clone();
            if hover.is_none() && diagnostic.is_none() {
                return;
            }
            let editor = app.editor.clone();
            let generation = restoration_generation.get();
            let restoration_generation = restoration_generation.clone();
            let focused = focused.clone();
            let app = cx.entity().downgrade();
            // EditorState clears hover on blur. Defer until every focus
            // listener has run, regardless of tab/editor creation order.
            window.defer(cx, move |window, cx| {
                if restoration_generation.get() != generation
                    || !focused.contains_focused(window, cx)
                {
                    return;
                }
                let _ = app.update(cx, |app, cx| {
                    if app.editor.entity_id() == editor.entity_id() {
                        editor.update(cx, |editor, cx| {
                            if let Some(hover) = hover {
                                editor.present_hover(hover.symbol_range, hover.hover, cx);
                            }
                            if let Some(diagnostic) = diagnostic {
                                editor.present_diagnostic(diagnostic, cx);
                            }
                        });
                    }
                });
            });
        });
        Self {
            handle,
            bounds: Rc::new(Cell::new(None)),
            pointer_gesture: Cell::new(false),
            hover,
            diagnostic,
            generation,
            _subscription: subscription,
        }
    }

    /// Release both presentations and cancel restoration from an earlier focus event.
    pub(crate) fn clear(&self) {
        self.bounds.set(None);
        self.pointer_gesture.set(false);
        self.generation.set(self.generation.get().wrapping_add(1));
        self.hover.borrow_mut().take();
        self.diagnostic.borrow_mut().take();
    }

    /// Capture the current presentation rather than a potentially older LSP cache.
    fn track(&self, hover: &gpui_base::input::HoverPopoverState) -> &FocusHandle {
        *self.diagnostic.borrow_mut() = None;
        *self.hover.borrow_mut() = Some(hover.clone());
        &self.handle
    }

    /// Restore the error card when selecting its message causes the editor to blur.
    fn track_diagnostic(&self, diagnostic: &gpui_base::input::DiagnosticEntry) -> &FocusHandle {
        *self.hover.borrow_mut() = None;
        *self.diagnostic.borrow_mut() = Some(diagnostic.clone());
        &self.handle
    }
}

/// Select the host renderer only while a popover needs styling unavailable upstream.
pub(super) fn render(
    editor: &Entity<EditorState>,
    popup: &Rc<CompletionPopupState>,
    hover_focus: &DefinitionPopupFocus,
    style: theme::ResolvedStyle,
    hover_enabled: bool,
    body: &mut Option<AnyElement>,
    projection: Option<&SourceProjection>,
    window: &mut Window,
    cx: &mut Context<EditorApp>,
) -> Option<AnyElement> {
    // Only a card painted in this frame may keep an outside editor gesture alive.
    hover_focus.bounds.set(None);
    let (completion, hover, diagnostic) = {
        let state = editor.read(cx);
        // Search and code actions retain their native controls and action routing.
        if state.search_session().open || state.code_action_menu_state().open {
            return None;
        }
        (
            state.completion_menu_state().clone(),
            state.hover_popover().cloned(),
            state.diagnostic_popover(),
        )
    };
    if !completion.open && hover.is_none() && diagnostic.is_none() {
        // Do not restore an error from a previous edit when this focus boundary is reused.
        *hover_focus.diagnostic.borrow_mut() = None;
        popup.reset();
        return None;
    }
    if popup.selection.get().0 != completion.revision() {
        popup.selection.set((completion.revision(), 0));
        popup.scroll.set_offset(point(px(0.), px(0.)));
        // Typing can replace a deletion request with a newer upstream response.
        if popup.refresh_revision.get() != Some(completion.revision()) {
            popup.refresh_revision.set(None);
        }
    }
    let completion_view = completion
        .open
        .then(|| render_completion(editor, popup, &completion, projection, window, cx))
        .flatten();
    let diagnostic_view = diagnostic
        .as_ref()
        .filter(|_| hover_enabled)
        .and_then(|diagnostic| {
            // Anchor to a single visible glyph even if the diagnostic spans several lines.
            let state = editor.read(cx);
            let start = diagnostic.range.start;
            let length = state.text().slice(start..).chars().next()?.len_utf8();
            let bounds = state.range_to_bounds(&(start..start + length))?;
            let bounds = if let Some(projection) = projection {
                projection.bounds(bounds, cx)?
            } else {
                bounds
            };
            let host = cx.entity().downgrade();
            Some(
                editor_ui::diagnostic_popup(
                    bounds,
                    diagnostic,
                    hover_focus.track_diagnostic(diagnostic),
                    move |cx| {
                        let _ = host.update(cx, |app, cx| app.dismiss_pointer_hover(cx));
                    },
                    {
                        let bounds = hover_focus.bounds.clone();
                        move |painted| bounds.set(Some(painted))
                    },
                    cx,
                )
                .into_any_element(),
            )
        });
    let hover_view = hover
        .as_ref()
        // A deferred details card must stay out of modal masks and their hitboxes.
        .filter(|_| hover_enabled && diagnostic.is_none())
        .and_then(|hover| render_hover(editor, hover, hover_focus, projection, window, cx));
    let hover_visible = (hover_view.is_some() || diagnostic_view.is_some()) && !completion.open;
    if completion.open {
        install_completion_actions(editor, popup, cx);
    }
    // The base editor keeps the same document, cursor, IME and LSP state. Only
    // its component overlay is replaced for the lifetime of these popovers.
    let host_for_escape = cx.entity().downgrade();
    let refresh_after_deletion = {
        let editor = editor.clone();
        let popup = popup.clone();
        move |window: &mut Window, cx: &mut App| {
            completion_refresh::schedule(&editor, &popup, window, cx);
        }
    };
    let refresh_after_delete = refresh_after_deletion.clone();
    let pending_popup = popup.clone();
    Some(
        div()
            .id("custom-editor-popovers")
            .debug_selector(|| "custom-editor-popovers".into())
            .relative()
            .size_full()
            .bg(style.background.unwrap_or(cx.theme().background))
            .text_color(style.foreground.unwrap_or(cx.theme().foreground))
            .font_family(cx.theme().mono_font_family.clone())
            .text_size(
                style
                    .font_size_px
                    .map(px)
                    .unwrap_or(cx.theme().mono_font_size),
            )
            // Keep the local source frame's 1.5 text line height during the renderer switch.
            .line_height(relative(1.5))
            .when(completion.open, |view| {
                // Observe deletion before the engine clears its menu, then let it edit normally.
                view.capture_action(move |_: &Backspace, window, cx| {
                    refresh_after_deletion(window, cx);
                })
                .capture_action(move |_: &Delete, window, cx| {
                    refresh_after_delete(window, cx);
                })
                .capture_action(move |action: &Enter, _, cx| {
                    // Cached rows remain visible while refreshing, but their edits are stale.
                    if Enter::is_primary(action) && pending_popup.refresh_revision.get().is_some() {
                        cx.stop_propagation();
                    }
                })
            })
            .when(hover_visible, |view| {
                view.on_action(move |_: &Escape, _, cx| {
                    // The base input propagates Escape after its own overlays;
                    // dismiss the definition card at the host boundary.
                    let _ = host_for_escape.update(cx, |app, cx| {
                        app.dismiss_pointer_hover(cx);
                    });
                })
            })
            .child(body.take().unwrap_or_else(|| {
                editor_ui::source_editor::Editor::new(editor)
                    .tools(false)
                    .into_any_element()
            }))
            .children(completion_view)
            .children(hover_view)
            // Completion remains the active editing surface until it closes.
            .when(!completion.open, |view| view.children(diagnostic_view))
            .into_any_element(),
    )
}

/// Route language selections through resolve and native transactions; keep other completions local.
fn install_completion_actions(
    editor: &Entity<EditorState>,
    popup: &Rc<CompletionPopupState>,
    cx: &mut Context<EditorApp>,
) {
    let host = cx.entity().downgrade();
    let popup = popup.clone();
    let (items, revision) = {
        let state = editor.read(cx);
        let menu = state.completion_menu_state();
        (menu.items.clone(), menu.revision())
    };
    let editor = editor.clone();
    editor.clone().update(cx, |state, _| {
        state.set_overlay_action_handler(move |kind, action, window, cx| {
            if kind != InputOverlayKind::Completion {
                return false;
            }
            if items.is_empty() {
                return false;
            }
            let selected = popup.selection.get().1.min(items.len() - 1);
            if action.partial_eq(&MoveDown) {
                let selected = (selected + 1).min(items.len() - 1);
                popup.selection.set((revision, selected));
                popup.scroll.scroll_to_item(selected);
                cx.notify();
            } else if action.partial_eq(&MoveUp) {
                let selected = selected.saturating_sub(1);
                popup.selection.set((revision, selected));
                popup.scroll.scroll_to_item(selected);
                cx.notify();
            } else if Enter::is_primary(&*action) {
                let selected_revision = popup.selection.get().0;
                let editor = editor.clone();
                let host = host.clone();
                // Defer the mutation until the engine finishes dispatching this key.
                cx.spawn_in(window, async move |_, cx| {
                    let selected_item = editor.update_in(cx, |state, _window, _cx| {
                        let menu = state.completion_menu_state();
                        // A response may arrive before repaint installs a new action handler.
                        let selected = if menu.revision() == selected_revision {
                            selected
                        } else {
                            0
                        };
                        let Some(item) = menu.items.get(selected).cloned() else {
                            return None;
                        };
                        let end = state.cursor();
                        let start = menu.trigger_start_offset.unwrap_or(end);
                        Some((item, start..end))
                    });
                    if let Ok(Some((item, range))) = selected_item {
                        let _ = host.update_in(cx, |app, window, cx| {
                            // Marked language items are consumed even when stale, so Base cannot
                            // apply a partial main insertion after resolve or import validation fails.
                            if !app.accept_language_completion(item.clone(), window, cx) {
                                editor.update(cx, |state, cx| {
                                    state.insert_completion(&item, range, window, cx)
                                });
                            }
                        });
                    }
                })
                .detach();
            } else if action.partial_eq(&Escape) {
                // The engine closes a handled completion on Escape itself.
            } else {
                return false;
            }
            true
        });
    });
}

/// Position the styled completion list at the cursor using upstream layout data.
fn render_completion(
    editor: &Entity<EditorState>,
    popup: &Rc<CompletionPopupState>,
    menu: &gpui_base::input::CompletionMenuState,
    projection: Option<&SourceProjection>,
    window: &mut Window,
    cx: &mut Context<EditorApp>,
) -> Option<AnyElement> {
    let state = editor.read(cx);
    let (cursor, line_height) = state.cursor_layout()?;
    let viewport = if let Some(projection) = projection {
        projection.viewport(cx)?
    } else {
        state.input_bounds()
    };
    let caret = if let Some(projection) = projection {
        projection.range(state.cursor()..state.cursor(), cx)?.origin
    } else {
        state.scroll_offset() + cursor.origin
    };
    let origin = caret - viewport.origin + point(-px(4.), line_height + px(4.));
    let width = state.lsp().completion_menu.max_width;
    let origin_x = origin
        .x
        .max(px(0.))
        .min((window.bounds().size.width - viewport.origin.x - px(120.)).max(px(0.)));
    let absolute_x = viewport.origin.x + origin_x;
    let max_width = width
        .min(window.bounds().size.width - absolute_x)
        .max(px(120.));
    // Move documentation below the list when there is no room for two columns.
    let vertical_layout = absolute_x + width + px(4.) + width > window.bounds().size.width;
    let start = menu.trigger_start_offset.unwrap_or(state.cursor());
    let end = state.cursor();
    let selected = popup
        .selection
        .get()
        .1
        .min(menu.items.len().saturating_sub(1));
    let revision = menu.revision();
    let host = cx.entity().downgrade();
    let row_styles = component_styles(cx, ThemeComponent::ExplorerRow);
    let foreground = cx.theme().foreground;
    let muted = cx.theme().muted_foreground;
    let match_color = cx.theme().primary;
    let hover_bg = row_styles.hover.background.unwrap_or(cx.theme().list_hover);
    let selected_bg = row_styles
        .selected
        .background
        .unwrap_or(cx.theme().selection);
    let selected_fg = row_styles.selected.foreground.unwrap_or(foreground);
    let selected_border = row_styles.selected.border.unwrap_or(cx.theme().primary);
    let query = crate::language::completion::identifier_prefix(&state.text().to_string(), end);
    let rows = menu.items.iter().cloned().enumerate().map(|(index, item)| {
        let editor = editor.clone();
        let popup = popup.clone();
        let pending_popup = popup.clone();
        let host = host.clone();
        let click_host = host.clone();
        let deprecated = item.deprecated.unwrap_or(false);
        // Abbreviations highlight their matching letters, including gaps in fuzzy matches.
        let highlights = crate::language::completion::matching_ranges(&query, &item.label)
            .unwrap_or_default()
            .into_iter()
            .map(|range| {
                (
                    range,
                    HighlightStyle {
                        color: Some(match_color),
                        ..Default::default()
                    },
                )
            });
        let label = StyledText::new(item.label.clone()).with_highlights(highlights);
        h_flex()
            .id(("editor-completion", index))
            .debug_selector(move || format!("editor-completion-row-{index}").into())
            .gap_2()
            .px_2()
            .py_1()
            .when(deprecated, |row| row.line_through())
            .when(index == selected, |row| {
                row.bg(selected_bg)
                    .text_color(selected_fg)
                    .border_l_1()
                    .border_color(selected_border)
            })
            .when(index != selected, |row| row.hover(|row| row.bg(hover_bg)))
            .child(div().text_color(foreground).child(label))
            .when_some(item.detail.clone(), |row, detail| {
                row.child(
                    div()
                        .text_color(muted)
                        // Details inherit the candidate font family at two pixels smaller.
                        .text_size(typography::editor_font_size(cx) - px(2.))
                        .italic()
                        .when(deprecated, |detail| detail.line_through())
                        .child(detail),
                )
            })
            .on_hover(move |hovered, _, cx| {
                if *hovered && popup.selection.get().1 != index {
                    // Selection and documentation follow the hovered completion.
                    popup.selection.set((revision, index));
                    let _ = host.update(cx, |app, cx| {
                        let _ = app.editor_panel.update(cx, |_, cx| cx.notify());
                    });
                }
            })
            .on_click(move |_, window, cx| {
                if pending_popup.refresh_revision.get().is_some() {
                    // Only a fresh server response can supply edits for the current document.
                    cx.stop_propagation();
                    return;
                }
                if editor.read(cx).completion_menu_state().revision() != revision {
                    // Wait for repaint if a refresh has replaced this row's edit data.
                    return;
                }
                let _ = click_host.update(cx, |app, cx| {
                    if !app.accept_language_completion(item.clone(), window, cx) {
                        editor.update(cx, |state, cx| {
                            state.insert_completion(&item, start..end, window, cx);
                            state.dismiss_completion_overlay(cx);
                        });
                    }
                });
                cx.stop_propagation();
            })
    });
    let documentation = menu
        .items
        .get(selected)
        .and_then(|item| item.documentation.as_ref());
    let documentation = documentation.map(|doc| match doc {
        Documentation::String(text) => text.clone(),
        Documentation::MarkupContent(markup) => markup.value.clone(),
    });
    let panel = card(cx, "editor-completion-card")
        .p_1()
        .max_w(max_width.max(px(120.)))
        .min_w(px(120.))
        .max_h(px(240.))
        .overflow_y_scrollbar_with(&popup.scroll)
        .children(rows);
    let mut layout = div()
        .flex()
        .flex_row()
        .items_start()
        .gap_1()
        .when(vertical_layout, |layout| layout.flex_col())
        .child(panel);
    if let Some(documentation) = documentation {
        let documentation = if vertical_layout {
            documentation.lines().next().unwrap_or_default().to_string()
        } else {
            documentation
        };
        layout = layout.child(
            card(cx, "editor-completion-documentation-card")
                .p_1()
                .px_2()
                .w(if vertical_layout { max_width } else { width })
                .max_h(px(240.))
                .overflow_y_scrollbar()
                .child(markdown_view(
                    "editor-completion-documentation",
                    documentation,
                    cx,
                )),
        );
    }
    Some(
        deferred(
            div()
                .absolute()
                .left(origin_x)
                .top(origin.y)
                .font_family(cx.theme().mono_font_family.clone())
                .text_size(typography::editor_font_size(cx))
                .child(layout),
        )
        .into_any_element(),
    )
}

/// Anchor the definition card to the measured symbol in window coordinates.
fn render_hover(
    editor: &Entity<EditorState>,
    hover: &gpui_base::input::HoverPopoverState,
    hover_focus: &DefinitionPopupFocus,
    projection: Option<&SourceProjection>,
    _window: &mut Window,
    cx: &mut Context<EditorApp>,
) -> Option<AnyElement> {
    let state = editor.read(cx);
    let bounds = state.range_to_bounds(&hover.symbol_range)?;
    let bounds = if let Some(projection) = projection {
        projection.bounds(bounds, cx)?
    } else {
        bounds
    };
    let markdown = hover_markdown(&hover.hover.contents);
    let painted_bounds = hover_focus.bounds.clone();
    let host_for_key = cx.entity().downgrade();
    Some(
        editor_ui::definition_popup(
            bounds,
            editor_ui::definition_popup::details_card("editor-definition-card", cx)
                // The card and Markdown share one focus boundary; entering it
                // restores the presentation cleared by the editor's blur.
                .track_focus(hover_focus.track(hover))
                .p_3()
                // The popup frame owns natural limits and user-selected size.
                // Let this card shrink, grow, and wrap within that frame.
                .flex_grow_1()
                .flex_shrink_1()
                .min_w_0()
                .min_h_0()
                .overflow_y_scrollbar()
                .text_size(typography::editor_font_size(cx))
                // Opaque hit testing protects the editor while the window's
                // selection layer receives mouse down, move, and drag events.
                .on_mouse_move(|event, _, cx| {
                    if event.pressed_button.is_none() {
                        cx.stop_propagation();
                    }
                })
                .on_key_down(move |event: &gpui_kit::KeyDownEvent, _, cx| {
                    if event.keystroke.key == "escape" {
                        // Markdown receives focus during selection, so Escape
                        // must also be handled on the card's focus path.
                        let _ = host_for_key.update(cx, |app, cx| {
                            app.dismiss_pointer_hover(cx);
                        });
                        cx.stop_propagation();
                    }
                })
                .child(
                    div()
                        .debug_selector(|| "editor-definition-text".into())
                        .child(editor_ui::text::hover_markdown_view(
                            "editor-definition-details",
                            markdown,
                            cx,
                        )),
                )
                .into_any_element(),
            move |bounds| painted_bounds.set(Some(bounds)),
        )
        .into_any_element(),
    )
}

/// Match the editor font in markdown and fenced code without changing other UI text.
fn markdown_view(id: &'static str, markdown: String, cx: &App) -> gpui_base::TextView {
    editor_ui::markdown_view(id, markdown, typography::editor_font_size(cx), cx)
}

/// Keep editor popovers on the live theme surface used by their Markdown content.
fn card(cx: &App, id: &'static str) -> gpui_kit::Stateful<gpui_kit::Div> {
    let shadow = vec![
        BoxShadow {
            color: Hsla {
                h: 0.,
                s: 0.,
                l: 0.,
                a: 0.18,
            },
            blur_radius: px(10.),
            spread_radius: px(-1.),
            offset: point(px(0.), px(2.)),
            inset: false,
        },
        BoxShadow {
            color: Hsla {
                h: 0.,
                s: 0.,
                l: 0.,
                a: 0.18,
            },
            blur_radius: px(3.),
            spread_radius: px(0.),
            offset: point(px(0.), px(1.)),
            inset: false,
        },
    ];
    div()
        .id(id)
        .debug_selector(move || id.into())
        .flex_none()
        .occlude()
        // Markdown supplies its own theme foreground, so a fixed white surface
        // would make documentation unreadable after switching to a dark theme.
        .bg(cx.theme().popover)
        .text_color(cx.theme().popover_foreground)
        .border_1()
        .border_color(cx.theme().border)
        .shadow(shadow)
}

/// Normalize LSP hover formats before the project renderer parses Markdown.
fn hover_markdown(contents: &HoverContents) -> String {
    match contents {
        HoverContents::Markup(markup) => match markup.kind {
            lsp_types::MarkupKind::PlainText => editor_ui::text::plain_markdown(&markup.value),
            lsp_types::MarkupKind::Markdown => markup.value.clone(),
        },
        HoverContents::Scalar(MarkedString::String(text)) => text.clone(),
        HoverContents::Scalar(MarkedString::LanguageString(item)) => {
            format!("```{}\n{}\n```", item.language, item.value)
        }
        HoverContents::Array(items) => items
            .iter()
            .map(|item| match item {
                MarkedString::String(text) => text.clone(),
                MarkedString::LanguageString(item) => {
                    format!("```{}\n{}\n```", item.language, item.value)
                }
            })
            .collect::<Vec<_>>()
            .join("\n\n"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use editor_core::Workspace;
    use gpui_base::Root;
    use gpui_kit::TestAppContext;
    use gpui_kit::{gpui, size};
    use lsp_types::{CompletionItem, Hover};
    use std::{cell::RefCell, rc::Rc};

    /// Both popovers render in the project while the upstream editor owns their state.
    #[gpui::test]
    fn completion_and_hover_use_project_cards(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            typography::init(cx);
            theme::apply_theme(theme::builtin_theme(false), cx);
            cx.set_reduce_motion(true);
        });
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("sample.rs");
        std::fs::write(&path, "sample").unwrap();
        let workspace = Workspace::open(directory.path()).unwrap();
        let slot = Rc::new(RefCell::new(None));
        let capture = slot.clone();
        let (_, cx) = cx.add_window_view(move |window, cx| {
            let view = cx.new(|cx| EditorApp::new(workspace, Some(path), window, cx));
            *capture.borrow_mut() = Some(view.clone());
            Root::new(view, window, cx)
        });
        let view = slot.borrow_mut().take().unwrap();
        cx.simulate_resize(size(px(1000.), px(800.)));
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        // Popover rendering must preserve the editor's existing text row height.
        let base_line_height = cx.update(|_, cx| {
            view.read(cx)
                .editor
                .read(cx)
                .line_height()
                .expect("editor should have a measured layout")
        });

        cx.update(|_, cx| {
            let editor = view.read(cx).editor.clone();
            editor.update(cx, |state, cx| {
                state.present_completion_items(
                    0,
                    "",
                    vec![CompletionItem {
                        label: "completion".into(),
                        ..Default::default()
                    }],
                    cx,
                );
            });
        });
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert!(cx.debug_bounds("editor-completion-card").is_some());
        let completion_line_height = cx.update(|_, cx| {
            view.read(cx)
                .editor
                .read(cx)
                .line_height()
                .expect("completion should keep the editor layout")
        });
        assert_eq!(completion_line_height, base_line_height);
        let row = cx.debug_bounds("editor-completion-row-0").unwrap();
        cx.simulate_click(row.center(), Default::default());
        cx.run_until_parked();
        cx.update(|_, cx| {
            let editor = view.read(cx).editor.clone();
            assert!(editor.read(cx).text().to_string().contains("completion"));
        });

        cx.update(|_, cx| {
            let editor = view.read(cx).editor.clone();
            editor.update(cx, |state, cx| {
                state.present_completion_items(
                    state.cursor(),
                    "",
                    vec![CompletionItem {
                        label: "keyboard".into(),
                        ..Default::default()
                    }],
                    cx,
                );
            });
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        cx.update(|_, cx| {
            let editor = view.read(cx).editor.clone();
            assert!(editor.read(cx).text().to_string().contains("keyboard"));
        });

        cx.update(|_, cx| {
            let editor = view.read(cx).editor.clone();
            editor.update(cx, |state, cx| {
                state.dismiss_completion_overlay(cx);
                state.present_hover(
                    0..6,
                    Hover {
                        contents: HoverContents::Scalar(MarkedString::String("details".into())),
                        range: None,
                    },
                    cx,
                );
            });
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert!(cx.debug_bounds("editor-definition-card").is_some());
        let hover_line_height = cx.update(|_, cx| {
            view.read(cx)
                .editor
                .read(cx)
                .line_height()
                .expect("hover should keep the editor layout")
        });
        assert_eq!(hover_line_height, base_line_height);
        cx.simulate_keystrokes("escape");
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert!(cx.debug_bounds("editor-definition-card").is_none());
    }
}
