//! Native comparison arrows, default shared scrolling and derived readonly comparison text.
use super::*;
use gpui_kit::AnyElement;
use std::cell::RefCell;

/// Executes only explicit independent-pane navigation using current native shaped geometry.
/// Shared aligned scrolling is owned by AlignmentGroup and never competes with this fallback.
pub(super) fn navigate_native_viewports(
    pending: &RefCell<[Option<crate::editor::viewport::Locator>; 3]>,
    editors: &[Entity<EditorState>],
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
    /// Reveal hunk `index` in the actual shared viewport, with a measured independent-mode fallback.
    /// Source selections, focus and Undo stay owned by their native editor sessions.
    pub(super) fn reveal_comparison_hunk(&self, index: usize, cx: &mut App) {
        let Some(comparison) = &self.document_comparison else {
            return;
        };
        // A new location supersedes any independent-pane request, including after alignment resumes.
        *comparison.navigation.borrow_mut() = [None, None, None];
        let targets = third::navigation_targets(comparison);
        let Some(target) = targets.get(index.min(targets.len().saturating_sub(1))) else {
            return;
        };
        if !comparison.alignment.reveal_offset_at(1, target[1], 0.5, cx) {
            *comparison.navigation.borrow_mut() =
                target.map(|offset| Some(crate::editor::viewport::Locator::centered(offset)));
        }
    }

    /// Shared controls remain native local buttons and operate only on the mounted comparison.
    pub(super) fn comparison_toolbar(&self, cx: &mut Context<Self>) -> AnyElement {
        let comparison = self.document_comparison.as_ref().unwrap();
        let no_changes = comparison.hunks.is_empty()
            && comparison
                .third
                .as_ref()
                .is_none_or(|third| third.hunks.is_empty());
        let unified = self.active_comparison.as_ref().is_some_and(|id| {
            self.comparison_views
                .iter()
                .any(|view| &view.id == id && view.unified)
        });
        editor_ui::comparison_header_surface(cx)
            .id("comparison-toolbar")
            .debug_selector(|| "comparison-toolbar".into())
            .flex()
            .items_center()
            .gap_1()
            .flex_wrap()
            .px_2()
            .child(
                Button::new("diff-file-previous")
                    .debug_selector(|| "diff-file-previous".into())
                    .icon(IconName::ArrowLeft)
                    .tooltip(t!("editor.diff_file_previous").to_string())
                    .small()
                    .compact()
                    .ghost()
                    .disabled(self.comparison_file_navigation(false).is_none())
                    .on_click(
                        cx.listener(|this, _, _, cx| this.navigate_comparison_file(false, cx)),
                    ),
            )
            .child(
                Button::new("diff-file-next")
                    .debug_selector(|| "diff-file-next".into())
                    .icon(IconName::ArrowRight)
                    .tooltip(t!("editor.diff_file_next").to_string())
                    .small()
                    .compact()
                    .ghost()
                    .disabled(self.comparison_file_navigation(true).is_none())
                    .on_click(
                        cx.listener(|this, _, _, cx| this.navigate_comparison_file(true, cx)),
                    ),
            )
            .child(
                Button::new("diff-previous")
                    .debug_selector(|| "diff-previous".into())
                    .icon(IconName::ArrowUp)
                    .tooltip(t!("editor.diff_previous").to_string())
                    .small()
                    .compact()
                    .ghost()
                    .disabled(no_changes)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.navigate_comparison_hunk(false, window, cx)
                    })),
            )
            .child(
                Button::new("diff-next")
                    .debug_selector(|| "diff-next".into())
                    .icon(IconName::ArrowDown)
                    .tooltip(t!("editor.diff_next").to_string())
                    .small()
                    .compact()
                    .ghost()
                    .disabled(no_changes)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.navigate_comparison_hunk(true, window, cx)
                    })),
            )
            .children(self.comparison_fold_buttons(cx))
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
            .into_any_element()
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
        // A unified two-source patch cannot represent both parents of a three-source view.
        if comparison.third.is_some() {
            return;
        }
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
                RowBackgroundCollection::new(&cx.entity(), marks, cx)
            });
            comparison.unified_editor = Some(editor);
            comparison.unified_marks = Some(collection);
        }
    }
}
