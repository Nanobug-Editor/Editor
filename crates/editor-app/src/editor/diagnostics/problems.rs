//! Docked Problems projects live diagnostics; selection, disclosure, scrolling and focus are presentation state.
mod model;
mod panel;
mod view;

use super::*;
use gpui_kit::{FocusHandle, KeyDownEvent};
use model::{ProblemKey, Row};
use panel::ProblemsPanel;
use plugin_runtime::{
    LanguageFixOrigin,
    plugin_protocol::{api, direct},
};
use std::collections::{BTreeMap, BTreeSet};

/// One workspace's interaction state shares a dock leaf, without copying diagnostic or text truth.
pub(crate) struct ProblemsState {
    pub(crate) panel: Entity<ProblemsPanel>,
    open: Rc<Cell<bool>>,
    focus: FocusHandle,
    close_focus: FocusHandle,
    previous: Option<FocusHandle>,
    /// A source editor may be replaced by cross-file navigation while this nonmodal panel stays open.
    return_to_editor: bool,
    /// Base row handles contain no diagnostic payload; selection retains only an exact lookup key.
    rows: Vec<FocusHandle>,
    /// File paths retain disclosure state through diagnostic replacement, without retaining markers.
    collapsed: BTreeSet<PathBuf>,
    groups: BTreeMap<PathBuf, FocusHandle>,
    selected: Option<ProblemKey>,
    /// Track the selected row's focus slot through same-length publication reordering.
    selected_index: usize,
    scroll: ScrollHandle,
}

impl ProblemsState {
    /// Bind the native leaf to its editor owner and restore only saved presentation state.
    pub(crate) fn new(parent: WeakEntity<EditorApp>, visible: bool, cx: &mut App) -> Self {
        let open = Rc::new(Cell::new(visible));
        let focus = cx.focus_handle();
        let close_focus = cx.focus_handle();
        let panel = cx.new(|cx| {
            ProblemsPanel::new(parent, open.clone(), focus.clone(), close_focus.clone(), cx)
        });
        Self {
            panel,
            open,
            focus,
            close_focus,
            previous: None,
            return_to_editor: false,
            rows: Vec::new(),
            collapsed: BTreeSet::new(),
            groups: BTreeMap::new(),
            selected: None,
            selected_index: 0,
            scroll: ScrollHandle::new(),
        }
    }

    /// Hide returns focus only if this panel still owns the user's current focus lease.
    fn owns_focus(&self, window: &Window, cx: &App) -> bool {
        self.focus.is_focused(window)
            || self.focus.contains_focused(window, cx)
            || self.close_focus.is_focused(window)
            || self.rows.iter().any(|focus| focus.is_focused(window))
            || self.groups.values().any(|focus| focus.is_focused(window))
    }
}

impl EditorApp {
    /// Add the default bottom leaf only when absent, including migration of older saved layouts.
    pub(crate) fn ensure_problems_dock(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let panel = self
            .dock_presentation
            .panel_handle(self.problems.panel.clone());
        let id = gpui_base::dock::PanelId::from(self.problems.panel.entity_id());
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

    /// Reveal the workspace list in its current dock position and preserve the previous focus target.
    pub(crate) fn show_problems(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.dock_presentation
            .activate(gpui_base::dock::PanelId::from(
                self.problems.panel.entity_id(),
            ));
        self.prune_direct_diagnostics(cx);
        if !self.problems.owns_focus(window, cx) {
            self.problems.previous = window.focused(cx);
            self.problems.return_to_editor = self.editor.focus_handle(cx).is_focused(window);
        }
        self.problems.open.set(true);
        self.session_state.problems_visible = true;
        self.ensure_problems_dock(window, cx);
        let id = gpui_base::dock::PanelId::from(self.problems.panel.entity_id());
        self.dock_area.update(cx, |area, cx| {
            // A user may have moved this leaf; reopen its actual dock instead of forcing it below.
            let placement = [
                gpui_base::dock::DockPlacement::Left,
                gpui_base::dock::DockPlacement::Right,
                gpui_base::dock::DockPlacement::Bottom,
            ]
            .into_iter()
            .find(|placement| {
                area.layout(*placement)
                    .is_some_and(|layout| layout.find_panel_node(id).is_some())
            });
            if let Some(placement) = placement
                && !area.is_dock_open(placement)
            {
                area.toggle_dock(placement, window, cx);
            }
            area.select_panel(id, window, cx);
            cx.notify();
        });
        self.problems.focus.focus(window, cx);
        self.capture_dock_layout(cx);
        self.persist_session();
        cx.notify();
    }

    /// Hiding this leaf leaves peer panels and their running resources intact.
    fn close_problems(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let owns_focus = self.problems.owns_focus(window, cx);
        self.problems.open.set(false);
        self.session_state.problems_visible = false;
        let previous = self.problems.previous.take();
        let previous = if std::mem::take(&mut self.problems.return_to_editor) {
            Some(self.editor.focus_handle(cx))
        } else {
            previous
        };
        if owns_focus {
            previous
                .unwrap_or_else(|| self.editor.focus_handle(cx))
                .focus(window, cx);
        }
        self.problems.rows.clear();
        self.dock_area.update(cx, |_, cx| cx.notify());
        self.capture_dock_layout(cx);
        self.persist_session();
        cx.notify();
    }

    /// File disclosure keeps its own focus so collapsing cannot activate a hidden diagnostic.
    fn set_problem_file_expanded(
        &mut self,
        path: &Path,
        expanded: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.problem_rows(cx).iter().any(|row| row.path == path) {
            return;
        }
        if expanded {
            self.problems.collapsed.remove(path);
        } else {
            self.problems.collapsed.insert(path.to_path_buf());
        }
        if let Some(focus) = self.problems.groups.get(path) {
            focus.focus(window, cx);
        }
        cx.notify();
    }

    /// Arrows skip collapsed diagnostics; a focused header also supports explicit left/right disclosure.
    fn problems_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.modifiers != Default::default() {
            return;
        }
        if event.keystroke.key == "escape" {
            self.close_problems(window, cx);
            cx.stop_propagation();
            return;
        }
        let group = self
            .problems
            .groups
            .iter()
            .find(|(_, focus)| focus.is_focused(window))
            .map(|(path, _)| path.clone());
        if let Some(path) = &group
            && matches!(event.keystroke.key.as_str(), "left" | "right")
        {
            self.set_problem_file_expanded(path, event.keystroke.key == "right", window, cx);
            cx.stop_propagation();
            return;
        }
        let all_rows = self.problem_rows(cx);
        let rows = self.visible_problem_rows(&all_rows);
        if rows.is_empty() {
            // With every group collapsed, arrows still reach each Base disclosure trigger.
            let groups = self.problems.groups.values().collect::<Vec<_>>();
            let selected = groups
                .iter()
                .position(|focus| focus.is_focused(window))
                .unwrap_or(0);
            let index = match event.keystroke.key.as_str() {
                "up" => selected.saturating_sub(1),
                "down" => (selected + 1).min(groups.len().saturating_sub(1)),
                "home" => 0,
                "end" => groups.len().saturating_sub(1),
                _ => return,
            };
            if let Some(focus) = groups.get(index) {
                focus.focus(window, cx);
            }
            cx.stop_propagation();
            return;
        }
        let selected = self.selected_problem_index(&rows);
        let index = match event.keystroke.key.as_str() {
            "up" if group.is_some() => rows
                .iter()
                .rposition(|row| row.path < *group.as_ref().unwrap())
                .unwrap_or(0),
            "down" if group.is_some() => rows
                .iter()
                .position(|row| row.path >= *group.as_ref().unwrap())
                .unwrap_or(rows.len() - 1),
            "up" => selected.saturating_sub(1),
            "down" => (selected + 1).min(rows.len().saturating_sub(1)),
            "home" => 0,
            "end" => rows.len().saturating_sub(1),
            _ => return,
        };
        if let Some(row) = rows.get(index) {
            self.problems.selected = Some(row.key.clone());
            self.problems.scroll.scroll_to_item(row.list_index);
        }
        if let Some(focus) = self.problems.rows.get(index) {
            focus.focus(window, cx);
        }
        cx.stop_propagation();
        cx.notify();
    }

    /// Re-resolve a painted key; replacement or disclosure cannot activate a changed/hidden row.
    fn select_problem(&mut self, key: &ProblemKey, window: &mut Window, cx: &mut Context<Self>) {
        if self
            .problem_rows(cx)
            .iter()
            .any(|row| &row.key == key && !self.problems.collapsed.contains(&row.path))
        {
            self.problems.selected = Some(key.clone());
            self.reveal_problem(window, cx);
        }
    }

    /// Navigate the selected live document without hiding the list, keeping other files reachable.
    fn reveal_problem(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(key) = self.problems.selected.as_ref() else {
            return;
        };
        let Some(row) = self
            .problem_rows(cx)
            .into_iter()
            .find(|row| &row.key == key)
        else {
            return;
        };
        let Some(index) = (0..self.tabs.len()).find(|index| {
            self.plugin_document_version(*index).ok().as_ref() == Some(&row.key.document)
        }) else {
            return;
        };
        let editor = self.tabs[index].text.as_ref().unwrap().editor.clone();
        self.activate_tab(index, window, cx);
        // Native linked input can defer activation. Recheck the target after that deferred switch.
        if self.editor.entity_id() == editor.entity_id() {
            self.reveal_problem_location(&row.key, window, cx);
        } else {
            let owner = cx.entity().downgrade();
            window.defer(cx, move |window, cx| {
                let _ = owner.update(cx, |app, cx| {
                    app.reveal_problem_location(&row.key, window, cx);
                });
            });
        }
        cx.notify();
    }

    /// A deferred tab switch may outlive its publication; never navigate the outgoing or newer document.
    fn reveal_problem_location(
        &mut self,
        key: &ProblemKey,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let current = self
            .active_text_tab_index()
            .and_then(|index| self.plugin_document_version(index).ok());
        if current.as_ref() != Some(&key.document)
            || !self.problem_rows(cx).iter().any(|row| &row.key == key)
        {
            return;
        }
        self.editor.update(cx, |editor, cx| {
            editor.unfold_at(key.range.start, cx);
            editor.set_cursor_position(key.range.start, window, cx);
        });
        self.editor.focus_handle(cx).focus(window, cx);
        self.problems.return_to_editor = true;
    }

    /// Fixes and related locations retain their original collection, document revision and owner gates.
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
            if let Some(fix) = action {
                self.submit_native_language_fix(origin, document, fix, cx)?;
            } else if let Some(location) = location {
                self.navigate_language_location(&origin, &document, location, window, cx)?;
                self.problems.return_to_editor = true;
            }
            Ok::<(), api::Failure>(())
        })();
        if let Err(error) = result {
            self.status = error.message;
            cx.notify();
        }
    }
}
