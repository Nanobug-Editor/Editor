//! Problems is a native projection of DocumentDiagnostics; no diagnostic or text cache lives here.
use super::*;
use gpui_base::{StyledExt as _, TestSupportExt as _};
use gpui_kit::{AnyElement, FocusHandle, KeyDownEvent};
use plugin_runtime::{
    LanguageFixOrigin,
    plugin_protocol::{api, direct},
};

/// Only modal interaction state is retained; every render and action consults the document model.
#[derive(Default)]
pub(crate) struct ProblemsState {
    open: bool,
    focus: Option<FocusHandle>,
    previous: Option<FocusHandle>,
    /// Focus belongs to each painted Base row; these handles contain no diagnostic payload.
    rows: Vec<FocusHandle>,
    selected: usize,
    document: Option<api::DocumentVersion>,
    scroll: ScrollHandle,
}

/// A frame-local row carries an owned lookup key, never mutable diagnostic or document truth.
struct Row {
    label: String,
    source: String,
    position: gpui_base::input::Position,
    collection: Option<(String, String)>,
}

impl EditorApp {
    /// Open the current document's native problem list and preserve the previous focus lease.
    pub(crate) fn show_problems(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.prune_direct_diagnostics(cx);
        if !self.problems.open {
            self.problems.previous = window.focused(cx);
            self.problems.focus = Some(cx.focus_handle());
            self.problems.open = true;
        }
        self.problems.focus.as_ref().unwrap().focus(window, cx);
        cx.notify();
    }

    /// Return focus only while this modal still owns it; a later user focus change remains intact.
    fn close_problems(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.problems.open = false;
        let owns_focus =
            self.problems.focus.as_ref().is_some_and(|focus| {
                focus.is_focused(window) || focus.contains_focused(window, cx)
            }) || self
                .problems
                .rows
                .iter()
                .any(|focus| focus.is_focused(window));
        if owns_focus && let Some(previous) = self.problems.previous.take() {
            previous.focus(window, cx);
        }
        self.problems.focus = None;
        self.problems.rows.clear();
        cx.notify();
    }

    /// Build the same legacy union and direct source set that feeds native underlines and hovers.
    fn problem_rows(&self, cx: &App) -> Vec<Row> {
        let Some(index) = self.active_text_tab_index() else {
            return Vec::new();
        };
        let tab = self.tabs[index].text.as_ref().unwrap();
        let text = tab.editor.read(cx).text();
        let mut rows: Vec<_> = tab
            .diagnostics
            .legacy_projection()
            .into_iter()
            .map(|item| Row {
                label: format!(
                    "{}:{} · {}",
                    item.range.start.line + 1,
                    item.range.start.character + 1,
                    item.message
                ),
                source: item
                    .source
                    .map(|source| source.to_string())
                    .unwrap_or_else(|| t!("problems.language_source").to_string()),
                position: item.range.start,
                collection: None,
            })
            .collect();
        for (key, publication) in tab
            .diagnostics
            .direct
            .iter()
            .filter(|(_, item)| item.collection.is_live())
        {
            for item in &publication.items {
                let position = text.offset_to_position(item.range.start);
                let severity = t!(match item.severity {
                    plugin_runtime::plugin_protocol::diagnostics::Severity::Error =>
                        "problems.error",
                    plugin_runtime::plugin_protocol::diagnostics::Severity::Warning =>
                        "problems.warning",
                    plugin_runtime::plugin_protocol::diagnostics::Severity::Information =>
                        "problems.information",
                    plugin_runtime::plugin_protocol::diagnostics::Severity::Hint => "problems.hint",
                });
                rows.push(Row {
                    label: format!(
                        "{severity} · {}:{} · {}",
                        position.line + 1,
                        position.character + 1,
                        item.message
                    ),
                    source: publication.source(),
                    position,
                    collection: Some((key.clone(), item.id.clone())),
                });
            }
        }
        rows.sort_by_key(|row| (row.position, row.source.clone()));
        rows
    }

    /// Arrow selection scrolls and focuses the live Base row; Base owns Enter and Escape delivery.
    fn problems_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.modifiers != Default::default() {
            return;
        }
        let length = self.problem_rows(cx).len();
        match event.keystroke.key.as_str() {
            "up" => self.problems.selected = self.problems.selected.saturating_sub(1),
            "down" => {
                self.problems.selected = (self.problems.selected + 1).min(length.saturating_sub(1))
            }
            "home" => self.problems.selected = 0,
            "end" => self.problems.selected = length.saturating_sub(1),
            _ => return,
        }
        if let Some(focus) = self.problems.rows.get(self.problems.selected) {
            focus.focus(window, cx);
        }
        self.problems.scroll.scroll_to_item(self.problems.selected);
        cx.stop_propagation();
        cx.notify();
    }

    /// Reveal only a row reconstructed from the live model, rather than a stale rendered payload.
    fn reveal_problem(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(position) = self
            .problem_rows(cx)
            .get(self.problems.selected)
            .map(|row| row.position)
        else {
            return;
        };
        self.close_problems(window, cx);
        self.editor.update(cx, |editor, cx| {
            editor.unfold_at(position, cx);
            editor.set_cursor_position(position, window, cx);
        });
        self.editor.focus_handle(cx).focus(window, cx);
    }

    /// A fix/related action re-resolves collection, diagnostic ID, source revision and original owner.
    fn problem_action(
        &mut self,
        document: api::DocumentVersion,
        key: String,
        id: String,
        fix: Option<usize>,
        related: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let result = (|| {
            let index = (0..self.tabs.len())
                .find(|index| self.plugin_document_version(*index).ok().as_ref() == Some(&document))
                .ok_or_else(|| {
                    api::Failure::new(
                        api::ErrorCode::StaleRevision,
                        "Problem source changed or closed",
                    )
                })?;
            let publication = self.tabs[index]
                .text
                .as_ref()
                .unwrap()
                .diagnostics
                .direct
                .get(&key)
                .filter(|item| item.collection.is_live() && item.document == document)
                .ok_or_else(|| {
                    api::Failure::new(
                        api::ErrorCode::InvalidHandle,
                        "Problem collection was revoked",
                    )
                })?;
            let diagnostic = publication
                .items
                .iter()
                .find(|item| item.id == id)
                .ok_or_else(|| {
                    api::Failure::new(api::ErrorCode::StaleRevision, "Problem was replaced")
                })?;
            let origin = LanguageFixOrigin::Diagnostics(publication.collection.clone());
            let action = fix.and_then(|index| diagnostic.fixes.get(index)).cloned();
            let location = related
                .and_then(|index| diagnostic.related.get(index))
                .map(|item| direct::Location {
                    resource: item.resource.clone(),
                    position: item.position,
                    label: Some(item.message.clone()),
                });
            self.close_problems(window, cx);
            if let Some(fix) = action {
                self.submit_native_language_fix(origin, document, fix, cx)?;
            } else if let Some(location) = location {
                self.navigate_language_location(&origin, &document, location, window, cx)?;
            }
            Ok::<(), api::Failure>(())
        })();
        if let Err(error) = result {
            self.status = error.message;
            cx.notify();
        }
    }

    /// Base supplies focus trapping and dismissal; local buttons and theme tokens own all appearance.
    pub(crate) fn render_problems(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if !self.problems.open {
            return div().into_any_element();
        }
        let document = self
            .active_text_tab_index()
            .and_then(|index| self.plugin_document_version(index).ok());
        let document_changed = self.problems.document != document;
        if document_changed {
            self.problems.document = document.clone();
            self.problems.selected = 0;
            self.problems.scroll.scroll_to_item(0);
        }
        let rows = self.problem_rows(cx);
        self.problems.selected = self.problems.selected.min(rows.len().saturating_sub(1));
        let dialog_focus = self.problems.focus.as_ref().unwrap();
        let owns_focus = dialog_focus.is_focused(window)
            || dialog_focus.contains_focused(window, cx)
            || self
                .problems
                .rows
                .iter()
                .any(|focus| focus.is_focused(window));
        let rows_changed = self.problems.rows.len() != rows.len();
        self.problems
            .rows
            .resize_with(rows.len(), || cx.focus_handle());
        // The initial Dialog focus is above the body, so its keys cannot reach a descendant
        // listener. Enter the selected native row, including before the deferred first paint.
        // If live publications remove a focused row, keep focus inside this modal without
        // pulling it back from an unrelated control the user selected in the meantime.
        if dialog_focus.is_focused(window) || ((rows_changed || document_changed) && owns_focus) {
            self.problems
                .rows
                .get(self.problems.selected)
                .unwrap_or(dialog_focus)
                .focus(window, cx);
        }
        let mut sources = BTreeMap::<String, usize>::new();
        for row in &rows {
            *sources.entry(row.source.clone()).or_default() += 1;
        }
        let mut list = div()
            .id("native-problems-list")
            .debug_selector(|| "native-problems-list".into())
            .h(px(260.))
            .overflow_y_scroll()
            .track_scroll(&self.problems.scroll)
            .flex()
            .flex_col();
        for (index, row) in rows.iter().enumerate() {
            list = list.child(
                Button::new(format!("native-problem-{index}"))
                    .track_focus(&self.problems.rows[index])
                    .label(row.label.clone())
                    .accessibility_label(format!("{} · {}", row.source, row.label))
                    .w_full()
                    .h(px(40.))
                    .flex_shrink_0()
                    .ghost()
                    .when(index == self.problems.selected, |button| button.primary())
                    .debug_selector(move || format!("native-problem-{index}"))
                    .on_click(
                        cx.listener(move |app, event: &gpui_kit::ClickEvent, window, cx| {
                            app.problems.selected = index;
                            // Pointer selection exposes details; a complete native keyboard click
                            // reveals the selected position, without stealing Enter from fix buttons.
                            if event.is_keyboard() {
                                app.reveal_problem(window, cx);
                            } else {
                                cx.notify();
                            }
                        }),
                    ),
            );
        }
        let mut body = div()
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .bg(cx.theme().popover)
            .text_color(cx.theme().popover_foreground)
            .capture_key_down(cx.listener(Self::problems_key))
            .child(
                div()
                    .id("native-problems-heading")
                    .role(gpui_kit::Role::Heading)
                    .aria_label(t!("problems.title", count = rows.len()).to_string())
                    .font_semibold()
                    .child(t!("problems.title", count = rows.len()).to_string())
                    // Observe the same native heading in headless tests, without alternate properties.
                    .test_support(),
            )
            .children(
                sources
                    .into_iter()
                    .enumerate()
                    .map(|(index, (source, count))| {
                        let label = format!("{source}: {count}");
                        div()
                            .id(format!("native-problems-source-{index}"))
                            .role(gpui_kit::Role::Label)
                            .aria_label(label.clone())
                            .text_sm()
                            .child(label)
                            .test_support()
                    }),
            )
            .child(list);
        if rows.is_empty() {
            body = body.child(t!("problems.empty").to_string());
        }
        if let Some((key, id)) = rows
            .get(self.problems.selected)
            .and_then(|row| row.collection.as_ref())
            && let Some(document) = &document
            && let Some(index) = self.active_text_tab_index()
            && let Some(publication) = self.tabs[index]
                .text
                .as_ref()
                .unwrap()
                .diagnostics
                .direct
                .get(key)
            && let Some(item) = publication.items.iter().find(|item| &item.id == id)
        {
            for (index, related) in item.related.iter().enumerate() {
                let (document, key, id) = (document.clone(), key.clone(), id.clone());
                body = body.child(
                    Button::new(format!("problem-related-{index}"))
                        .label(related.message.clone())
                        .accessibility_label(related.message.clone())
                        .ghost()
                        .debug_selector(move || format!("problem-related-{index}"))
                        .on_click(cx.listener(move |app, _, window, cx| {
                            app.problem_action(
                                document.clone(),
                                key.clone(),
                                id.clone(),
                                None,
                                Some(index),
                                window,
                                cx,
                            )
                        })),
                );
            }
            for (index, fix) in item.fixes.iter().enumerate() {
                let (document, key, id) = (document.clone(), key.clone(), id.clone());
                body = body.child(
                    Button::new(format!("problem-fix-{index}"))
                        .label(fix.title.clone())
                        .accessibility_label(fix.title.clone())
                        .debug_selector(move || format!("problem-fix-{index}"))
                        .on_click(cx.listener(move |app, _, window, cx| {
                            app.problem_action(
                                document.clone(),
                                key.clone(),
                                id.clone(),
                                Some(index),
                                None,
                                window,
                                cx,
                            )
                        })),
                );
            }
        }
        body = body.child(
            div()
                .flex()
                .gap_2()
                .child(
                    Button::new("problem-reveal")
                        .label(t!("problems.reveal").to_string())
                        .accessibility_label(t!("problems.reveal").to_string())
                        .debug_selector(|| "problem-reveal".into())
                        .disabled(rows.is_empty())
                        .on_click(cx.listener(|app, _, window, cx| app.reveal_problem(window, cx))),
                )
                .child(
                    Button::new("problem-close")
                        .label(t!("problems.close").to_string())
                        .accessibility_label(t!("problems.close").to_string())
                        .debug_selector(|| "problem-close".into())
                        .on_click(cx.listener(|app, _, window, cx| app.close_problems(window, cx))),
                ),
        );
        let weak = cx.entity().downgrade();
        gpui_base::Dialog::new(cx)
            .focus_handle(self.problems.focus.as_ref().unwrap().clone())
            .close_on_backdrop_press(false)
            .on_cancel(move |_, window, cx| {
                let _ = weak.update(cx, |app, cx| app.close_problems(window, cx));
                true
            })
            .on_ok(|_, _, _| false)
            .top(px(54.))
            .popup(
                gpui_base::DialogPopup::new()
                    .w(px(680.))
                    .max_h(px(650.))
                    .rounded(cx.theme().radius_lg)
                    .border_1()
                    .border_color(cx.theme().border)
                    .child(body),
            )
            .into_any_element()
    }
}
