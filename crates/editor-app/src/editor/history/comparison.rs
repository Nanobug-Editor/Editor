//! Bounded derived diff markers use existing native source entities and their own decoration layers.

use super::*;
use editor_ui::aligned_editor::{AlignmentGroup, AlignmentPane};
use gpui_base::input::{RangeDecoration, RangeDecorationCollection, RangeDecorationStyle};

/// Comparison owns presentation only; both original EditorStates retain all text and Undo authority.
pub(super) struct Comparison {
    /// Independent marker owners never replace syntax, diagnostics or plugin decoration collections.
    marks: [RangeDecorationCollection; 2],
    /// Native source layout/scroll projection, with no padding inserted into document content.
    pub alignment: AlignmentGroup,
    /// Native changes invalidate these derived markers and ask for a fresh comparison.
    _observer: Subscription,
}

impl Comparison {
    /// Retire native input projections before removing panes, then release only this view's markers.
    pub fn retire(self, cx: &mut App) {
        self.alignment.retire(cx);
        for marks in self.marks {
            marks.clear(cx);
        }
    }
}

impl EditorApp {
    /// Diff runs off-thread under the existing 1 MiB/2000-line/matrix bounds; larger sources retain preview.
    pub(super) fn compare_history_selection(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(panel) = &self.history.panel else {
            return;
        };
        let Some(selection) = &panel.selection else {
            return;
        };
        if selection.active_document
            != self
                .active_tab_index()
                .map(|index| self.tabs[index].file_id)
        {
            self.history_notice(t!("history.stale").to_string(), cx);
            return;
        }
        let Some(target) = selection.target.clone() else {
            self.history_notice(t!("history.open_target").to_string(), cx);
            return;
        };
        let Some(index) = super::recovery::target_matches(self, &target, cx) else {
            self.history_notice(t!("history.stale").to_string(), cx);
            return;
        };
        let Some(left) = panel.preview.clone() else {
            return;
        };
        let right = self.text_tab(index).unwrap().editor.clone();
        let old = left.read(cx).value().to_string();
        let current = right.read(cx).value().to_string();
        if old.len() > 1024 * 1024 || current.len() > 1024 * 1024 {
            self.history_notice(t!("history.diff_unavailable").to_string(), cx);
            return;
        }
        // The writable comparison belongs to this file's real active session, including Save.
        // Merely selecting a readonly version keeps its native list focus unchanged.
        self.activate_tab(index, window, cx);
        if let Some(selection) = self
            .history
            .panel
            .as_mut()
            .and_then(|panel| panel.selection.as_mut())
        {
            selection.active_document = Some(target.id);
        }
        let generation = self.history.generation;
        cx.spawn_in(window, async move |app, cx| {
            let ranges = cx
                .background_executor()
                .spawn(
                    async move { crate::editor::comparison::changed_byte_ranges(&old, &current) },
                )
                .await;
            let _ = app.update_in(cx, |app, window, cx| {
                if app.history.generation != generation {
                    return;
                }
                // A matching buffer can remain open after the user changes the active Save target.
                if app.active_tab_index().map(|index| app.tabs[index].file_id) != Some(target.id)
                    || super::recovery::target_matches(app, &target, cx).is_none()
                {
                    app.history_notice(t!("history.stale").to_string(), cx);
                    return;
                }
                let Ok(ranges) = ranges else {
                    app.history_notice(t!("history.diff_unavailable").to_string(), cx);
                    return;
                };
                let mut alignment = AlignmentGroup::new(cx);
                let Some(router) = editor_ui::aligned_pointer::for_window(window, cx) else {
                    return;
                };
                if alignment
                    .configure(
                        vec![
                            AlignmentPane::new(
                                left.clone(),
                                generation,
                                t!("history.historical").to_string(),
                            ),
                            AlignmentPane::new(
                                right.clone(),
                                target.capability,
                                t!("history.current").to_string(),
                            ),
                        ],
                        &router,
                        cx,
                    )
                    .is_err()
                {
                    app.history_notice(t!("history.diff_unavailable").to_string(), cx);
                    return;
                }
                alignment.set_enabled(true, cx);
                let colors = [
                    cx.theme().danger.opacity(0.28),
                    cx.theme().success.opacity(0.28),
                ];
                let marks = [&left, &right].map(|editor| editor.clone());
                let marks = std::array::from_fn(|side| {
                    marks[side].update(cx, |editor, cx| {
                        editor.create_range_decorations_collection(
                            ranges
                                .iter()
                                .filter_map(|range| {
                                    (!range[side].is_empty()).then(|| {
                                        RangeDecoration::new(range[side].clone())
                                            .with_style(RangeDecorationStyle::Fill)
                                            .with_color(colors[side])
                                    })
                                })
                                .collect(),
                            cx,
                        )
                    })
                });
                let observer = cx.observe(&right, move |app, _, cx| {
                    if super::recovery::target_matches(app, &target, cx).is_none() {
                        if let Some(panel) = &mut app.history.panel {
                            if let Some(comparison) = panel.comparison.take() {
                                comparison.retire(cx);
                            }
                            panel.notice = Some(t!("history.stale").to_string());
                        }
                        cx.notify();
                    }
                });
                if let Some(panel) = &mut app.history.panel {
                    if let Some(previous) = panel.comparison.take() {
                        previous.retire(cx);
                    }
                    panel.comparison = Some(Comparison {
                        marks,
                        alignment,
                        _observer: observer,
                    });
                }
                cx.notify();
            });
        })
        .detach();
    }
}
