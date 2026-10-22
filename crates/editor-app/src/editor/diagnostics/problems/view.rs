//! Compose file groups and version-gated actions inside the existing native dock bounds.
use super::*;
use editor_ui::{problems as appearance, vertical_scrollbar};
use gpui_kit::AnyElement;

impl EditorApp {
    /// Render a bounded nonmodal list; the local UI layer owns the panel's appearance.
    pub(crate) fn render_problems(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if !self.problems.open.get() {
            return div().into_any_element();
        }
        let all_rows = self.problem_rows(cx);
        let rows = self.visible_problem_rows(&all_rows);
        let selected = self.selected_problem_index(&rows);
        let selection = rows.get(selected).map(|row| row.key.clone());
        let owns_focus = self.problems.owns_focus(window, cx);
        let group_had_focus = self
            .problems
            .groups
            .values()
            .any(|focus| focus.is_focused(window));
        // Closed files lose disclosure state. A temporary clear/republication keeps the user's choice.
        self.problems
            .collapsed
            .retain(|path| self.tabs.iter().any(|tab| tab.path() == path));
        self.problems
            .groups
            .retain(|path, _| all_rows.iter().any(|row| &row.path == path));
        for group in all_rows.chunk_by(|a, b| a.path == b.path) {
            self.problems
                .groups
                .entry(group[0].path.clone())
                .or_insert_with(|| cx.focus_handle());
        }
        let group_focused = self
            .problems
            .groups
            .values()
            .any(|focus| focus.is_focused(window));
        let changed = self.problems.selected != selection
            || self.problems.selected_index != selected
            || self.problems.rows.len() != rows.len()
            || (group_had_focus && !group_focused);
        self.problems.selected = selection;
        self.problems.selected_index = selected;
        self.problems
            .rows
            .resize_with(rows.len(), || cx.focus_handle());
        // Revoked rows may own keyboard focus. Repair it only within the panel's current focus lease.
        if self.problems.focus.is_focused(window) || (changed && owns_focus && !group_focused) {
            self.problems
                .rows
                .get(selected)
                .unwrap_or(&self.problems.focus)
                .focus(window, cx);
        }
        if changed
            && !group_focused
            && let Some(row) = rows.get(selected)
        {
            self.problems.scroll.scroll_to_item(row.list_index);
        }
        let list = self.render_problem_list(&all_rows, selected, cx);
        let actions = self.render_problem_actions(rows.get(selected).copied(), cx);
        appearance::problem_body(
            &self.problems.focus,
            list,
            vertical_scrollbar(&self.problems.scroll, cx).into_any_element(),
            actions,
            cx,
        )
        .capture_key_down(cx.listener(Self::problems_key))
        .into_any_element()
    }

    /// Keep file headers in the scroll sequence while only expanded diagnostics receive row focus slots.
    fn render_problem_list(&self, rows: &[Row], selected: usize, cx: &Context<Self>) -> AnyElement {
        let mut list = appearance::problem_list(&self.problems.scroll, cx);
        let mut index = 0;
        for (group_index, group) in rows.chunk_by(|a, b| a.path == b.path).enumerate() {
            let path = group[0].path.clone();
            let expanded = !self.problems.collapsed.contains(&path);
            let icon = crate::ui::icons::file_icon(
                &self.providers,
                &path,
                false,
                &theme::active_theme(self.dark_theme),
            );
            list = list.child(
                appearance::problem_file(
                    group_index,
                    &group[0].file_label,
                    group.len(),
                    expanded,
                    &self.problems.groups[&path],
                    icon.into_any_element(),
                    cx,
                )
                .on_click(cx.listener(move |app, _, window, cx| {
                    app.set_problem_file_expanded(&path, !expanded, window, cx);
                })),
            );
            if !expanded {
                continue;
            }
            for row in group {
                let key = row.key.clone();
                list = list.child(
                    appearance::problem_row(
                        index,
                        &self.problems.rows[index],
                        row.severity,
                        &row.key.message,
                        &row.key.source,
                        row.key.range.start,
                        index == selected,
                        cx,
                    )
                    .on_click(
                        cx.listener(move |app, _, window, cx| app.select_problem(&key, window, cx)),
                    ),
                );
                index += 1;
            }
        }
        if rows.is_empty() {
            list = list.child(appearance::problem_empty(cx));
        }
        editor_ui::scroll::SmoothScroll::new(
            "problems-wheel",
            &self.problems.scroll,
            editor_ui::scroll::ScrollAxis::Vertical,
            list,
        )
        .into_any_element()
    }

    /// Direct publications expose their existing fixes and related locations for the selected file.
    fn render_problem_actions(&self, row: Option<&Row>, cx: &Context<Self>) -> Option<AnyElement> {
        let mut actions = appearance::problem_actions(cx);
        let mut has_actions = false;
        if let Some(row) = row
            && let Some((key, id)) = &row.key.collection
            && let Some(index) = (0..self.tabs.len()).find(|index| {
                self.plugin_document_version(*index).ok().as_ref() == Some(&row.key.document)
            })
            && let Some(publication) = self.tabs[index]
                .text
                .as_ref()
                .unwrap()
                .diagnostics
                .direct
                .get(key)
            && let Some(item) = publication.items.iter().find(|item| &item.id == id)
        {
            has_actions = !item.related.is_empty() || !item.fixes.is_empty();
            for (index, related) in item.related.iter().enumerate() {
                let (document, key, id) = (row.key.document.clone(), key.clone(), id.clone());
                actions = actions.child(
                    Button::new(format!("problem-related-{index}"))
                        .label(related.message.clone())
                        .accessibility_label(related.message.clone())
                        .small()
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
                let (document, key, id) = (row.key.document.clone(), key.clone(), id.clone());
                actions = actions.child(
                    Button::new(format!("problem-fix-{index}"))
                        .label(fix.title.clone())
                        .accessibility_label(fix.title.clone())
                        .small()
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
        has_actions.then(|| actions.into_any_element())
    }
}
