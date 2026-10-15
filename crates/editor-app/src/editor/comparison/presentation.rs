//! Native comparison navigation, explicit alignment pauses and derived readonly unified text.
use super::*;
use gpui_kit::AnyElement;
use std::cell::RefCell;

/// Executes only explicit independent-pane navigation using current native shaped geometry.
/// Shared aligned scrolling is owned by AlignmentGroup and never competes with this fallback.
pub(super) fn navigate_native_viewports(
    pending: &RefCell<[Option<crate::editor::viewport::Locator>; 2]>,
    editors: &[Entity<EditorState>; 2],
    window: &mut Window,
    cx: &mut App,
) {
    for (side, editor) in editors.iter().enumerate() {
        let step = pending.borrow_mut()[side]
            .as_mut()
            .map(|locator| locator.step(editor.read(cx)));
        match step {
            Some(crate::editor::viewport::Step::Scroll(offset)) => {
                editor.update(cx, |state, cx| state.set_scroll_offset(offset, cx));
                window.request_animation_frame();
            }
            Some(_) => {
                pending.borrow_mut()[side].take();
            }
            None => {}
        }
    }
}

/// Unicode-safe common edges isolate actual changed characters inside paired changed line ranges.
fn inline_ranges(
    left: &str,
    right: &str,
    hunks: &[diff::Hunk],
    colors: [Hsla; 2],
) -> [Vec<RangeDecoration>; 2] {
    let mut marks = [Vec::new(), Vec::new()];
    for hunk in hunks
        .iter()
        .filter(|hunk| !hunk.left.is_empty() && !hunk.right.is_empty())
    {
        let a = &left[hunk.left.clone()];
        let b = &right[hunk.right.clone()];
        let prefix = a
            .chars()
            .zip(b.chars())
            .take_while(|(a, b)| a == b)
            .map(|(ch, _)| ch.len_utf8())
            .sum::<usize>();
        let suffix = a[prefix..]
            .chars()
            .rev()
            .zip(b[prefix..].chars().rev())
            .take_while(|(a, b)| a == b)
            .map(|(ch, _)| ch.len_utf8())
            .sum::<usize>();
        for (side, range) in [&hunk.left, &hunk.right].into_iter().enumerate() {
            let range = (range.start + prefix)..(range.end - suffix);
            if !range.is_empty() {
                marks[side].push(
                    RangeDecoration::new(range)
                        .with_style(RangeDecorationStyle::Fill)
                        .with_color(colors[side]),
                );
            }
        }
    }
    marks
}
/// Each comparison has its own layers, preserving diagnostics and every unrelated marker owner.
pub(super) fn create_inline_marks(
    left: &Entity<EditorState>,
    right: &Entity<EditorState>,
    hunks: &[diff::Hunk],
    cx: &mut App,
) -> [RangeDecorationCollection; 2] {
    let [a, b] = inline_ranges(
        &left.read(cx).value().to_string(),
        &right.read(cx).value().to_string(),
        hunks,
        [
            cx.theme().danger.opacity(0.45),
            cx.theme().success.opacity(0.45),
        ],
    );
    [
        left.update(cx, |editor, cx| {
            editor.create_range_decorations_collection(a, cx)
        }),
        right.update(cx, |editor, cx| {
            editor.create_range_decorations_collection(b, cx)
        }),
    ]
}
/// Recolor/recompute only derived inline layers after a native edit or palette change.
pub(super) fn refresh_inline_marks(comparison: &mut DocumentComparison, cx: &mut App) {
    let [a, b] = inline_ranges(
        &comparison.left_editor.read(cx).value().to_string(),
        &comparison.right_editor.read(cx).value().to_string(),
        &comparison.hunks,
        [
            cx.theme().danger.opacity(0.45),
            cx.theme().success.opacity(0.45),
        ],
    );
    comparison.inline_marks[0].set(a, cx);
    comparison.inline_marks[1].set(b, cx);
}

impl EditorApp {
    /// Shared controls remain native local buttons and operate only on the mounted comparison.
    pub(super) fn comparison_toolbar(&self, cx: &mut Context<Self>) -> AnyElement {
        let comparison = self.document_comparison.as_ref().unwrap();
        let unified = self.active_comparison.as_ref().is_some_and(|id| {
            self.comparison_views
                .iter()
                .any(|view| &view.id == id && view.unified)
        });
        div()
            .flex()
            .items_center()
            .gap_2()
            .flex_wrap()
            .px_2()
            .child(
                Button::new("diff-previous")
                    .label(t!("editor.diff_previous").to_string())
                    .disabled(comparison.hunks.is_empty())
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.navigate_comparison_hunk(false, window, cx)
                    })),
            )
            .child(
                Button::new("diff-next")
                    .debug_selector(|| "diff-next".into())
                    .label(t!("editor.diff_next").to_string())
                    .disabled(comparison.hunks.is_empty())
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.navigate_comparison_hunk(true, window, cx)
                    })),
            )
            .child(
                Button::new("diff-unified")
                    .debug_selector(|| "diff-unified".into())
                    .label(if unified {
                        t!("editor.diff_split").to_string()
                    } else {
                        t!("editor.diff_unified").to_string()
                    })
                    .disabled(self.active_comparison.is_none())
                    .on_click(cx.listener(|this, _, window, cx| {
                        if let Some(id) = &this.active_comparison
                            && let Some(view) =
                                this.comparison_views.iter_mut().find(|view| &view.id == id)
                        {
                            view.unified = !view.unified;
                            if view.unified {
                                if let Some(comparison) = &this.document_comparison {
                                    for region in comparison.alignment.regions() {
                                        region.request_cancel();
                                    }
                                }
                            }
                        }
                        this.prepare_unified_comparison(window, cx);
                        cx.notify();
                    })),
            )
            .child(
                Button::new("diff-sync")
                    .label(t!("editor.diff_sync").to_string())
                    .when(comparison.synchronized.get(), |button| button.primary())
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(view) = &this.document_comparison {
                            let enabled = !view.synchronized.get();
                            view.synchronized.set(enabled);
                            for region in view.alignment.regions() {
                                region.request_cancel();
                            }
                            view.alignment.set_enabled(enabled, cx);
                        }
                        cx.notify();
                    })),
            )
            .children(
                (!unified)
                    .then(|| match comparison.alignment.status(cx) {
                        AlignmentStatus::NativeOverlays => {
                            Some(t!("editor.diff_alignment_paused_overlays").to_string())
                        }
                        AlignmentStatus::Disabled => {
                            Some(t!("editor.diff_alignment_paused_sync").to_string())
                        }
                        AlignmentStatus::Unavailable(_) => {
                            Some(t!("editor.diff_alignment_unavailable").to_string())
                        }
                        _ => None,
                    })
                    .flatten()
                    .map(|message| div().child(message)),
            )
            .child(div().flex_1())
            .child(
                Button::new("close-document-diff")
                    .label(t!("editor.close_diff").to_string())
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.close_plugin_document_menu(window, cx);
                        if let Some(id) = this.active_comparison.clone() {
                            this.close_comparison_view(&id, window, cx);
                        } else {
                            this.close_document_comparison(cx);
                        }
                        if this.active_text_tab_index().is_some() {
                            this.editor
                                .update(cx, |editor, cx| editor.focus(window, cx));
                        }
                    })),
            )
            .into_any_element()
    }
    /// Split navigation reveals source bytes without changing selection/focus or document history.
    fn navigate_comparison_hunk(
        &mut self,
        next: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(comparison) = &self.document_comparison else {
            return;
        };
        let count = comparison.hunks.len();
        if count == 0 {
            return;
        }
        let mut index = 0;
        if let Some(id) = &self.active_comparison
            && let Some(view) = self.comparison_views.iter_mut().find(|view| &view.id == id)
        {
            index = if next {
                (view.hunk + 1) % count
            } else {
                (view.hunk + count - 1) % count
            };
            view.hunk = index;
        }
        let hunk = &comparison.hunks[index];
        if self.active_comparison.as_ref().is_some_and(|id| {
            self.comparison_views
                .iter()
                .any(|view| &view.id == id && view.unified)
        }) {
            if let (Some(editor), Some(range)) = (
                &comparison.unified_editor,
                comparison.unified_hunks.get(index),
            ) {
                editor.update(cx, |editor, cx| {
                    editor.set_selected_range(range.start..range.start, cx);
                    editor.focus(window, cx);
                });
            }
            cx.notify();
            return;
        }
        if !comparison.alignment.reveal_offset(1, hunk.right.start, cx) {
            *comparison.navigation.borrow_mut() = [
                Some(crate::editor::viewport::Locator::new(hunk.left.start, 0.0)),
                Some(crate::editor::viewport::Locator::new(hunk.right.start, 0.0)),
            ];
        }
        cx.notify();
    }
    /// Unified mode is an immutable derived display. Editing resumes in the shared right session.
    pub(super) fn prepare_unified_comparison(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(comparison) = &mut self.document_comparison else {
            return;
        };
        let left = comparison.left_editor.read(cx).value().to_string();
        let right = comparison.right_editor.read(cx).value().to_string();
        let mut text = String::new();
        let mut marks = Vec::new();
        let mut previous = 0;
        let mut unified_hunks = Vec::new();
        let append = |text: &mut String, source: &str, prefix: char| {
            for line in source.split_inclusive('\n') {
                text.push(prefix);
                text.push_str(line);
                if !line.ends_with('\n') {
                    text.push('\n');
                }
            }
        };
        for hunk in &comparison.hunks {
            append(&mut text, &left[previous..hunk.left.start], ' ');
            let start = text.len();
            let hunk_start = start;
            append(&mut text, &left[hunk.left.clone()], '-');
            if text.len() > start {
                marks.push(
                    RangeDecoration::new(start..text.len())
                        .with_style(RangeDecorationStyle::Fill)
                        .with_color(comparison.colors[0]),
                );
            }
            let start = text.len();
            append(&mut text, &right[hunk.right.clone()], '+');
            if text.len() > start {
                marks.push(
                    RangeDecoration::new(start..text.len())
                        .with_style(RangeDecorationStyle::Fill)
                        .with_color(comparison.colors[1]),
                );
            }
            previous = hunk.left.end;
            unified_hunks.push(hunk_start..text.len());
        }
        append(&mut text, &left[previous..], ' ');
        comparison.unified_hunks = unified_hunks;
        if let Some(editor) = &comparison.unified_editor {
            editor.update(cx, |editor, cx| editor.set_value(text, window, cx));
            comparison.unified_marks.as_mut().unwrap().set(marks, cx);
        } else {
            let editor = cx.new(|cx| {
                EditorState::new(window, cx)
                    .language("text")
                    .line_number(false)
            });
            let collection = editor.update(cx, |editor, cx| {
                editor.set_value(text, window, cx);
                editor.set_readonly(true, cx);
                editor.create_range_decorations_collection(marks, cx)
            });
            comparison.unified_editor = Some(editor);
            comparison.unified_marks = Some(collection);
        }
    }
}
