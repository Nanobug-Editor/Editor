//! Bounded document comparison over the existing native editor sessions.

use crate::*;
use editor_ui::aligned_editor::{AlignmentGroup, AlignmentPane, AlignmentStatus};
use editor_ui::row_backgrounds::RowBackgroundCollection;
use gpui_base::TestSupportExt as _;
use gpui_base::input::{RangeDecoration, RangeDecorationCollection, RangeDecorationStyle};
use gpui_kit::Hsla;
use plugin_runtime::plugin_protocol::api;
mod folding;
mod navigation;
mod presentation;
#[cfg(test)]
mod render_tests;
mod sessions;
mod third;
pub(crate) use sessions::ComparisonView;

/// Share only bounded immutable byte intervals with native comparison consumers. Editor ownership,
/// presentation and lifecycle stay private to their callers; no document text or mutable state leaks.
pub(crate) fn changed_byte_ranges(
    left: &str,
    right: &str,
) -> Result<Vec<[std::ops::Range<usize>; 2]>, ()> {
    diff::compare(left, right).map(|hunks| {
        hunks
            .into_iter()
            .map(|hunk| [hunk.left, hunk.right])
            .collect()
    })
}

/// Own only comparison decorations and the temporary left-pane input restriction.
pub(crate) struct DocumentComparison {
    /// Two local sources do not carry a virtual handle, so retain the caller's runtime authority.
    authority: plugin_runtime::EditorAuthority,
    left: api::DocumentVersion,
    right: api::DocumentVersion,
    left_editor: Entity<EditorState>,
    right_editor: Entity<EditorState>,
    left_readonly: bool,
    left_title: String,
    right_title: String,
    left_marks: RowBackgroundCollection,
    right_marks: RowBackgroundCollection,
    /// An optional readonly third source and its independently owned result/parent decorations.
    third: Option<third::ThirdComparison>,
    inline_marks: [RangeDecorationCollection; 2],
    // Immutable byte ranges are sufficient to recolor live comparisons; text stays in EditorState.
    hunks: Vec<diff::Hunk>,
    colors: [Hsla; 3],
    /// Only the additive file-bound API retains a view across edits and tab switches.
    persistent: Option<String>,
    /// Only immutable derived unified text is cached; it can never edit either source.
    unified_editor: Option<Entity<EditorState>>,
    unified_marks: Option<RowBackgroundCollection>,
    unified_hunks: Vec<std::ops::Range<usize>>,
    /// Alignment shares immutable row geometry while both EditorStates retain their sessions.
    alignment: AlignmentGroup,
    /// Independent-mode navigation uses measured native viewports and never changes selection.
    navigation: Rc<std::cell::RefCell<[Option<crate::editor::viewport::Locator>; 3]>>,
}

/// Resolve deletion, insertion and modification colors from the current local theme.
fn comparison_colors(cx: &App) -> [Hsla; 3] {
    [
        cx.theme().danger.opacity(0.20),
        cx.theme().success.opacity(0.20),
        cx.theme().warning.opacity(0.20),
    ]
}

/// Refreshes source revisions without replacing native entities or their selection/history.
fn configure_alignment(comparison: &mut DocumentComparison, window: &Window, cx: &mut App) {
    let Some(router) = editor_ui::aligned_pointer::for_window(window, cx) else {
        return;
    };
    let mut panes = vec![
        AlignmentPane::new(
            comparison.left_editor.clone(),
            comparison.left.revision,
            comparison.left_title.clone(),
        ),
        AlignmentPane::new(
            comparison.right_editor.clone(),
            comparison.right.revision,
            comparison.right_title.clone(),
        ),
    ];
    if let Some(third) = &comparison.third {
        panes.push(AlignmentPane::new(
            third.editor.clone(),
            third.version.revision,
            third.title.clone(),
        ));
    }
    let _ = comparison.alignment.configure(panes, &router, cx);
    comparison.alignment.set_inner_gutters(true, cx);
    // Comparison panes always share their viewport; only native overlays may pause alignment.
    comparison.alignment.set_enabled(true, cx);
}

/// Build only this comparison's owner ranges; unrelated diagnostics and marks remain untouched.
fn comparison_marks(hunks: &[diff::Hunk], colors: [Hsla; 3], left: bool) -> Vec<RangeDecoration> {
    hunks
        .iter()
        .filter_map(|hunk| {
            let (range, opposite, one_sided) = if left {
                (&hunk.left, &hunk.right, colors[0])
            } else {
                (&hunk.right, &hunk.left, colors[1])
            };
            (!range.is_empty()).then(|| {
                RangeDecoration::new(range.clone())
                    .with_style(RangeDecorationStyle::Fill)
                    .with_color(if opposite.is_empty() {
                        one_sided
                    } else {
                        colors[2]
                    })
            })
        })
        .collect()
}

impl EditorApp {
    /// Compare two exact live versions; bounded snapshots are temporary algorithm inputs only.
    pub(crate) fn compare_plugin_documents(
        &mut self,
        left: &api::DocumentVersion,
        right: &api::DocumentVersion,
        request: &plugin_runtime::EditorRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<api::EditorValue, api::Failure> {
        let value = self.install_document_comparison(
            left,
            right,
            None,
            request.authority().clone(),
            Some(request),
            window,
            cx,
        )?;
        self.reveal_comparison_hunk(0, cx);
        Ok(value)
    }

    /// Native tab activation can reuse already admitted ownership without forging another request.
    fn install_document_comparison(
        &mut self,
        left: &api::DocumentVersion,
        right: &api::DocumentVersion,
        third: Option<&api::DocumentVersion>,
        authority: plugin_runtime::EditorAuthority,
        request: Option<&plugin_runtime::EditorRequest>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<api::EditorValue, api::Failure> {
        let locate = |target: &api::DocumentVersion| {
            (0..self.tabs.len()).find(|index| {
                self.plugin_document_version(*index)
                    .is_ok_and(|version| version == *target)
            })
        };
        let li = locate(left).ok_or_else(|| {
            api::Failure::new(
                api::ErrorCode::StaleRevision,
                "Comparison left document changed or closed",
            )
        })?;
        let ri = locate(right).ok_or_else(|| {
            api::Failure::new(
                api::ErrorCode::StaleRevision,
                "Comparison right document changed or closed",
            )
        })?;
        // Validate every extra source and both pairwise budgets before replacing a live view.
        let third_source = third
            .map(|target| self.prepare_third_comparison(target, left, right, cx))
            .transpose()?;
        if li == ri {
            return Err(api::Failure::new(
                api::ErrorCode::InvalidRequest,
                "Comparison requires distinct sessions",
            ));
        }
        if self.plugin_window_mounts.contains_key(&left.id)
            || self.plugin_window_mounts.contains_key(&right.id)
        {
            return Err(api::Failure::new(
                api::ErrorCode::Conflict,
                "A comparison source is mounted in another work window",
            ));
        }
        let live = |index: usize| {
            self.tabs[index]
                .virtual_document
                .as_ref()
                .is_none_or(|tab| tab.resource.is_live())
        };
        if !live(li) || !live(ri) {
            return Err(api::Failure::new(
                api::ErrorCode::StaleRevision,
                "Comparison resource was revoked",
            ));
        }
        let left_editor = self.tabs[li]
            .text
            .as_ref()
            .ok_or_else(|| {
                api::Failure::new(
                    api::ErrorCode::UnsupportedOperation,
                    "Comparison requires text",
                )
            })?
            .editor
            .clone();
        let right_editor = self.tabs[ri]
            .text
            .as_ref()
            .ok_or_else(|| {
                api::Failure::new(
                    api::ErrorCode::UnsupportedOperation,
                    "Comparison requires text",
                )
            })?
            .editor
            .clone();
        // Check Rope byte lengths before allocating immutable snapshots on the UI thread.
        if left_editor.read(cx).text().len() > 1024 * 1024
            || right_editor.read(cx).text().len() > 1024 * 1024
        {
            return Err(api::Failure::new(
                api::ErrorCode::LimitExceeded,
                "Comparison exceeds 1 MiB per document",
            ));
        }
        let hunks = diff::compare(
            &left_editor.read(cx).value().to_string(),
            &right_editor.read(cx).value().to_string(),
        )
        .map_err(|_| {
            api::Failure::new(
                api::ErrorCode::LimitExceeded,
                "Comparison exceeds 2000 lines or 4M cells",
            )
        })?;
        // Expensive pure comparison is cancellable until native focus/decorations actually change.
        if !authority.is_live() || request.is_some_and(|request| !request.enter_side_effect()) {
            return Err(api::Failure::new(
                api::ErrorCode::Cancelled,
                "Comparison did not execute",
            ));
        }
        self.close_plugin_document_menu(window, cx);
        self.close_document_comparison(cx);
        // Mount the right native session without turning a comparison-only source into a file tab.
        self.activate_tab_with_navigation(ri, false, true, window, cx);
        // Activating a tab can settle linked input. Never attach a pre-activation diff
        // if that settlement advanced either source's capability revision.
        if self.plugin_document_version(li)? != *left
            || self.plugin_document_version(ri)? != *right
            || third.is_some_and(|target| !self.comparison_source_is_exact(target))
        {
            return Err(api::Failure::new(
                api::ErrorCode::StaleRevision,
                "Comparison source changed during activation",
            ));
        }
        // Preserve the readonly flag itself: a separately disabled editor must not become readonly.
        let left_readonly = left_editor.read(cx).presentation().is_readonly();
        let colors = comparison_colors(cx);
        let left_marks = left_editor.update(cx, |editor, cx| {
            editor.set_readonly(true, cx);
            RowBackgroundCollection::new(&cx.entity(), comparison_marks(&hunks, colors, true), cx)
        });
        let right_marks = right_editor.update(cx, |_, cx| {
            RowBackgroundCollection::new(&cx.entity(), comparison_marks(&hunks, colors, false), cx)
        });
        let inline_marks =
            presentation::create_inline_marks(&left_editor, &right_editor, &hunks, cx);
        let third = third_source.map(|source| source.mount(&right_editor, cx));
        let mut comparison = DocumentComparison {
            authority,
            left: left.clone(),
            right: right.clone(),
            left_editor,
            right_editor,
            left_readonly,
            left_title: self.tabs[li].title(),
            right_title: self.tabs[ri].title(),
            left_marks,
            right_marks,
            third,
            inline_marks,
            hunks,
            colors,
            persistent: None,
            unified_editor: None,
            unified_marks: None,
            unified_hunks: Vec::new(),
            alignment: AlignmentGroup::new(cx),
            navigation: Rc::new(std::cell::RefCell::new([None, None, None])),
        };
        configure_alignment(&mut comparison, window, cx);
        self.document_comparison = Some(comparison);
        self.editor_panel.update(cx, |_, cx| cx.notify());
        cx.notify();
        Ok(api::EditorValue::DocumentsCompared {
            left: left.clone(),
            right: right.clone(),
        })
    }

    /// Invalidate changed versions/authority, returning keys only from a pane being unmounted.
    pub(crate) fn sync_document_comparison(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_comparison_views(window, cx);
        self.refresh_persistent_comparison(window, cx);
        if !self.document_comparison_is_current() {
            // Dismiss the popup while its previous left focus is still mounted, then reuse
            // the same selective repair as direct keyboard focus on the retired pane.
            self.close_plugin_document_menu(window, cx);
            // GPUI retains the old FocusHandle after unmounting; focus-lost alone does not
            // give keys to the surviving editor. Leave dialogs and an already-visible
            // editor alone, and never lend a background text entity to a binary tab.
            let restore_focus = self.document_comparison.as_ref().is_some_and(|comparison| {
                [&comparison.left_editor, &comparison.right_editor]
                    .into_iter()
                    .chain(comparison.third.as_ref().map(|third| &third.editor))
                    .any(|editor| {
                        editor != &self.editor
                            && editor
                                .read(cx)
                                .focus_handle(cx)
                                .contains_focused(window, cx)
                    })
            });
            self.close_document_comparison(cx);
            if restore_focus && self.active_text_tab_index().is_some() {
                self.editor
                    .update(cx, |editor, cx| editor.focus(window, cx));
            }
            return;
        }
        let colors = comparison_colors(cx);
        if let Some(comparison) = &mut self.document_comparison
            && comparison.colors != colors
        {
            // Theme changes do not invalidate text versions; replace only owned colors once.
            comparison
                .left_marks
                .set(comparison_marks(&comparison.hunks, colors, true), cx);
            comparison
                .right_marks
                .set(comparison_marks(&comparison.hunks, colors, false), cx);
            comparison.colors = colors;
            presentation::refresh_inline_marks(comparison, cx);
            if let Some(third) = &mut comparison.third {
                third.refresh_marks(&comparison.right_editor, colors, cx);
            }
            self.prepare_unified_comparison(window, cx);
        }
    }

    /// Rendering rechecks because queued requests may mutate documents after shell synchronization.
    fn document_comparison_is_current(&self) -> bool {
        let Some(comparison) = &self.document_comparison else {
            return false;
        };
        let live = |target: &api::DocumentVersion| {
            (0..self.tabs.len()).any(|index| {
                self.plugin_document_version(index)
                    .is_ok_and(|version| version == *target)
                    && self.tabs[index]
                        .virtual_document
                        .as_ref()
                        .is_none_or(|tab| tab.resource.is_live())
            })
        };
        comparison.authority.is_live()
            && !self.plugin_window_mounts.contains_key(&comparison.left.id)
            && !self.plugin_window_mounts.contains_key(&comparison.right.id)
            && live(&comparison.left)
            && live(&comparison.right)
            && comparison.third.as_ref().is_none_or(|third| {
                !self.plugin_window_mounts.contains_key(&third.version.id) && live(&third.version)
            })
            && self.active_tab_index().is_some_and(|index| {
                self.plugin_document_version(index)
                    .is_ok_and(|version| version == comparison.right)
            })
    }

    /// Exposes current split-view geometry to native consumers without lending an unmounted or
    /// stale source projection. Unified mode owns its separate immutable display geometry.
    pub(crate) fn comparison_alignment(&self) -> Option<&AlignmentGroup> {
        if !self.document_comparison_is_current()
            || self.active_comparison.as_ref().is_some_and(|id| {
                self.comparison_views
                    .iter()
                    .any(|view| &view.id == id && view.unified)
            })
        {
            return None;
        }
        self.document_comparison
            .as_ref()
            .map(|comparison| &comparison.alignment)
    }

    /// Dispose only our owners; diagnostics and other editor decorations survive closure.
    pub(crate) fn close_document_comparison(&mut self, cx: &mut Context<Self>) {
        if let Some(comparison) = self.document_comparison.take() {
            // A closed comparison cannot be revived by retained paint/input callbacks. Retirement
            // releases its row budget immediately while the pointer router finishes old gestures.
            comparison.alignment.retire(cx);
            comparison.left_marks.dispose(cx);
            comparison.right_marks.dispose(cx);
            if let Some(third) = comparison.third {
                third.unmount(&comparison.right_editor, cx);
            }
            for marks in comparison.inline_marks {
                marks.dispose(cx);
            }
            if let Some(marks) = comparison.unified_marks {
                marks.dispose(cx);
            }
            comparison.left_editor.update(cx, |editor, cx| {
                editor.set_readonly(comparison.left_readonly, cx)
            });
            self.editor_panel.update(cx, |_, cx| cx.notify());
            cx.notify();
        }
    }

    /// Global Save must respect the actual focused pane, not the active right tab alone.
    pub(crate) fn comparison_readonly_has_focus(&self, window: &Window, cx: &App) -> bool {
        self.document_comparison.as_ref().is_some_and(|comparison| {
            [&comparison.left_editor, &comparison.right_editor]
                .into_iter()
                .chain(comparison.unified_editor.as_ref())
                .chain(comparison.third.as_ref().map(|third| &third.editor))
                .any(|editor| {
                    !editor.read(cx).presentation().is_editable()
                        && editor
                            .read(cx)
                            .focus_handle(cx)
                            .contains_focused(window, cx)
                })
        })
    }

    /// Render one source frame per side over shared alignment geometry and the original sessions.
    pub(crate) fn render_document_comparison(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui_kit::AnyElement> {
        // The shell synchronizes lifetime/version validity before composing this view.
        if !self.document_comparison_is_current() {
            return None;
        }
        let comparison = self.document_comparison.as_ref()?;
        if comparison.third.is_some() {
            return self.render_three_document_comparison(window, cx);
        }
        comparison.alignment.set_enabled(true, cx);
        comparison.alignment.refresh_native_overlays(cx);
        if self.active_comparison.as_ref().is_some_and(|id| {
            self.comparison_views
                .iter()
                .any(|view| &view.id == id && view.unified)
        }) {
            if let Some(editor) = &comparison.unified_editor {
                let navigation = comparison.navigation.clone();
                let editors = vec![editor.clone()];
                let body = editor_ui::source_editor::Editor::new(editor)
                    .identity("unified-diff", Some(t!("editor.diff_unified").to_string()))
                    .tools(false)
                    .into_any_element();
                return Some(
                    div()
                        .flex()
                        .flex_col()
                        .size_full()
                        .min_h_0()
                        .child(self.comparison_toolbar(cx))
                        .child(
                            div()
                                .relative()
                                .flex_1()
                                .min_h_0()
                                .child(body)
                                .child(
                                    gpui_kit::canvas(
                                        |_, _, _| (),
                                        move |_, _, window, cx| {
                                            presentation::navigate_native_viewports(
                                                &navigation,
                                                &editors,
                                                window,
                                                cx,
                                            );
                                        },
                                    )
                                    .absolute()
                                    .size_full(),
                                )
                                .children(self.comparison_boundary_notice(cx)),
                        )
                        .into_any_element(),
                );
            }
        }
        let left = comparison.left_editor.clone();
        let left_title = comparison.left_title.clone();
        let right_title = comparison.right_title.clone();
        let left_label = format!(
            "{} · {} · {}",
            t!("editor.diff_left"),
            left_title,
            t!("editor.readonly_label")
        );
        let right_label = format!("{} · {}", t!("editor.diff_right"), right_title);
        let alignment = self.comparison_alignment()?;
        let left_input = alignment.pane(0).unwrap_or_else(|| {
            editor_ui::source_editor::Editor::new(&left)
                .identity("document-diff-left-input", Some(left_label.clone()))
                .tools(false)
                .into_any_element()
        });
        let right = self.render_native_editor_aligned(alignment, 1, window, cx);
        let menu_target = self.plugin_document_menu_target(&comparison.left);
        let left_focus = left.clone();
        let left_projection = comparison.alignment.clone();
        let navigation = comparison.navigation.clone();
        let sources = vec![
            comparison.left_editor.clone(),
            comparison.right_editor.clone(),
        ];
        Some(
            div()
                .relative()
                .flex()
                .flex_col()
                .size_full()
                .min_h_0()
                .child(self.comparison_toolbar(cx))
                .child(
                    div()
                        .relative()
                        .flex()
                        .flex_1()
                        .min_h_0()
                        .child(
                            div()
                                .id("document-diff-left-region")
                                .role(gpui_kit::Role::Group)
                                .aria_label(left_label)
                                .debug_selector(|| "document-diff-left".into())
                                // Admit host document shortcuts even inside the panel's PluginSurface.
                                .key_context("NativeEditorSource")
                                .relative()
                                .capture_any_mouse_down(cx.listener(
                                    move |this, event: &MouseDownEvent, window, cx| {
                                        if event.button == MouseButton::Right {
                                            // The actual left gesture owns focus/selection; active right-tab
                                            // state is never substituted for its captured resource version.
                                            left_focus
                                                .update(cx, |editor, cx| editor.focus(window, cx));
                                            this.open_plugin_document_menu(
                                                menu_target.clone(),
                                                left_projection
                                                    .project_pointer_bounds(
                                                        0,
                                                        Bounds::new(
                                                            event.position,
                                                            size(px(1.), px(1.)),
                                                        ),
                                                        cx,
                                                    )
                                                    .map_or(event.position, |bounds| bounds.origin),
                                                window,
                                                cx,
                                            );
                                            cx.stop_propagation();
                                        }
                                    },
                                ))
                                .flex()
                                .flex_col()
                                .flex_1()
                                .w_0()
                                .min_h_0()
                                .overflow_hidden()
                                .child(editor_ui::comparison_source_header(
                                    0,
                                    format!("{} · {}", t!("editor.diff_left"), left_title),
                                    cx,
                                ))
                                .child(div().flex_1().min_h_0().child(left_input)),
                        )
                        .child(
                            div()
                                .id("document-diff-right-region")
                                .role(gpui_kit::Role::Group)
                                .aria_label(right_label)
                                .test_support()
                                .debug_selector(|| "document-diff-right".into())
                                .relative()
                                .flex()
                                .flex_col()
                                .flex_1()
                                .w_0()
                                .min_h_0()
                                .overflow_hidden()
                                .child(editor_ui::comparison_source_header(
                                    1,
                                    format!("{} · {}", t!("editor.diff_right"), right_title),
                                    cx,
                                ))
                                .child(right),
                        )
                        .children(self.comparison_boundary_notice(cx)),
                )
                .child(
                    gpui_kit::canvas(
                        |_, _, _| (),
                        move |_, _, window, cx| {
                            presentation::navigate_native_viewports(
                                &navigation,
                                &sources,
                                window,
                                cx,
                            );
                        },
                    )
                    .absolute()
                    .size_full(),
                )
                .into_any_element(),
        )
    }
}

/// Compute byte ranges for line changes without retaining a second document model.
mod diff {
    use std::ops::Range;

    /// A paired hunk is a modification; a one-sided hunk is an insertion or deletion.
    #[derive(Debug, PartialEq)]
    pub(super) struct Hunk {
        pub left: Range<usize>,
        pub right: Range<usize>,
    }

    /// Ignore platform line separators while keeping a missing final newline significant.
    /// Only comparison keys are normalized; hunk offsets still use the original UTF-8 strings.
    fn line_key(line: &str) -> (&str, bool) {
        match line.strip_suffix('\n') {
            Some(content) => (content.strip_suffix('\r').unwrap_or(content), true),
            None => (line, false),
        }
    }

    /// Reject excessive input before allocating the bounded LCS matrix.
    pub(super) fn compare(left: &str, right: &str) -> Result<Vec<Hunk>, ()> {
        if left.len() > 1024 * 1024 || right.len() > 1024 * 1024 {
            return Err(());
        }
        // Count with an early stop before allocating per-line references for hostile input.
        if left.split_inclusive('\n').take(2001).count() > 2000
            || right.split_inclusive('\n').take(2001).count() > 2000
        {
            return Err(());
        }
        let a = left.split_inclusive('\n').collect::<Vec<_>>();
        let b = right.split_inclusive('\n').collect::<Vec<_>>();
        let a_keys = a.iter().map(|line| line_key(line)).collect::<Vec<_>>();
        let b_keys = b.iter().map(|line| line_key(line)).collect::<Vec<_>>();
        if a.len() > 2000 || b.len() > 2000 || a.len() * b.len() > 4_000_000 {
            return Err(());
        }
        // The final row/column are implicit zeroes; u16 holds at most 2000 matched lines.
        let mut table = vec![0u16; a.len() * b.len()];
        let cell = |table: &[u16], i: usize, j: usize| {
            if i < a.len() && j < b.len() {
                table[i * b.len() + j]
            } else {
                0
            }
        };
        for i in (0..a.len()).rev() {
            for j in (0..b.len()).rev() {
                table[i * b.len() + j] = if a_keys[i] == b_keys[j] {
                    1 + cell(&table, i + 1, j + 1)
                } else {
                    cell(&table, i + 1, j).max(cell(&table, i, j + 1))
                };
            }
        }
        let (mut i, mut j, mut left_offset, mut right_offset) = (0, 0, 0, 0);
        let mut hunks = Vec::new();
        while i < a.len() || j < b.len() {
            if i < a.len() && j < b.len() && a_keys[i] == b_keys[j] {
                left_offset += a[i].len();
                right_offset += b[j].len();
                i += 1;
                j += 1;
                continue;
            }
            let (start_left, start_right) = (left_offset, right_offset);
            while i < a.len() || j < b.len() {
                if i < a.len() && j < b.len() && a_keys[i] == b_keys[j] {
                    break;
                }
                if i < a.len() && (j == b.len() || cell(&table, i + 1, j) >= cell(&table, i, j + 1))
                {
                    left_offset += a[i].len();
                    i += 1;
                } else {
                    right_offset += b[j].len();
                    j += 1;
                }
            }
            hunks.push(Hunk {
                left: start_left..left_offset,
                right: start_right..right_offset,
            });
        }
        Ok(hunks)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// Guard limits and empty-side differences must not require an unbounded matrix.
        #[test]
        fn bounds_work_and_handles_empty_and_identical_documents() {
            assert!(compare(&"x".repeat(1024 * 1024 + 1), "").is_err());
            assert!(compare(&"x\n".repeat(2001), "").is_err());
            assert!(
                compare(&"x\n".repeat(2000), &"x\n".repeat(2000))
                    .unwrap()
                    .is_empty()
            );
            assert!(compare("", "").unwrap().is_empty());
            assert_eq!(
                compare("", "新").unwrap(),
                vec![Hunk {
                    left: 0..0,
                    right: 0..3
                }]
            );
            assert_eq!(
                compare("旧", "").unwrap(),
                vec![Hunk {
                    left: 0..3,
                    right: 0..0
                }]
            );
        }

        #[test]
        fn preserves_unicode_crlf_and_classifies_changed_lines() {
            assert_eq!(
                compare("同\r\n旧\r\n尾", "同\r\n新\r\n尾").unwrap(),
                vec![Hunk {
                    left: 5..10,
                    right: 5..10
                }]
            );
            assert_eq!(
                compare("a\n", "a\nb\n").unwrap(),
                vec![Hunk {
                    left: 2..2,
                    right: 2..4
                }]
            );
        }

        /// Git blobs use LF while a Windows worktree may use CRLF; offsets stay source-native.
        #[test]
        fn mixed_line_endings_do_not_turn_unchanged_context_into_changes() {
            assert!(
                compare("same\nlast\n", "same\r\nlast\r\n")
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(
                compare("same\nold\nlast\n", "same\r\nnew\r\nlast\r\n").unwrap(),
                vec![Hunk {
                    left: 5..9,
                    right: 6..11
                }]
            );
            // The absence of a final newline is a real content change.
            assert!(!compare("same\n", "same").unwrap().is_empty());
            assert!(!compare("same\r", "same").unwrap().is_empty());
        }
    }
}
