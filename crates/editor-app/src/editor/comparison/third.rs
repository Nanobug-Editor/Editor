//! Optional three-source comparisons reuse the original native sessions and alignment group.
use super::*;

/// Project a result line boundary back to its source; changed intervals land at their first line.
fn source_offset(hunks: &[diff::Hunk], result: usize) -> usize {
    let mut source_end = 0;
    let mut result_end = 0;
    for hunk in hunks {
        if result < hunk.right.start {
            break;
        }
        if result < hunk.right.end || result == hunk.right.start {
            return hunk.left.start;
        }
        source_end = hunk.left.end;
        result_end = hunk.right.end;
    }
    source_end + result.saturating_sub(result_end)
}

/// Both comparisons share the same bounded matrix budget; each source is capped independently.
fn row_budget_allows(rows: [usize; 3]) -> bool {
    rows.iter().all(|rows| *rows <= 2000)
        && rows[1].saturating_mul(rows[0].saturating_add(rows[2])) <= 4_000_000
}

/// Navigation covers changes against either outer source, ordered by the shared middle result.
pub(super) fn navigation_targets(comparison: &DocumentComparison) -> Vec<[usize; 3]> {
    let mut targets = comparison
        .hunks
        .iter()
        .map(|hunk| {
            [
                hunk.left.start,
                hunk.right.start,
                comparison
                    .third
                    .as_ref()
                    .map_or(0, |third| source_offset(&third.hunks, hunk.right.start)),
            ]
        })
        .collect::<Vec<_>>();
    if let Some(third) = &comparison.third {
        targets.extend(third.hunks.iter().map(|hunk| {
            [
                source_offset(&comparison.hunks, hunk.right.start),
                hunk.right.start,
                hunk.left.start,
            ]
        }));
        targets.sort_by_key(|target| target[1]);
        targets.dedup();
    }
    targets
}

/// A validated immutable descriptor prepared before any visible comparison is replaced.
pub(super) struct ThirdSource {
    /// Exact issued identity retained across the side-effect admission boundary.
    pub(super) version: api::DocumentVersion,
    /// Original native source entity; no text is retained outside its session.
    editor: Entity<EditorState>,
    /// Accessible source label captured from its native document tab.
    title: String,
    /// Changed intervals from the third input to the middle result.
    pub(super) hunks: Vec<diff::Hunk>,
}

/// Decorations and temporary input restrictions belong to the view, never to a second document.
pub(super) struct ThirdComparison {
    /// Current revision of the third session while this view is mounted.
    pub(super) version: api::DocumentVersion,
    /// Original readonly input surface, shared with the retained source session.
    pub(super) editor: Entity<EditorState>,
    /// Source name used in the visible frame and native accessibility label.
    pub(super) title: String,
    /// Input restriction restored when this derived view leaves the native editor area.
    readonly: bool,
    /// The middle source's prior restriction; three-source views temporarily disable its input.
    result_readonly: bool,
    /// Difference navigation and decorations for the third input against the middle result.
    pub(super) hunks: Vec<diff::Hunk>,
    /// Owned line decorations on the third input.
    marks: RowBackgroundCollection,
    /// Owned line decorations on the middle result, separate from the left-input comparison.
    result_marks: RowBackgroundCollection,
    /// Character decorations belonging only to this input/result pair.
    inline_marks: [RangeDecorationCollection; 2],
}

impl ThirdSource {
    /// Mount the already validated third source; all three panes are readonly until unmounted.
    pub(super) fn mount(self, result: &Entity<EditorState>, cx: &mut App) -> ThirdComparison {
        let colors = comparison_colors(cx);
        let readonly = self.editor.read(cx).presentation().is_readonly();
        let result_readonly = result.read(cx).presentation().is_readonly();
        let marks = self.editor.update(cx, |editor, cx| {
            editor.set_readonly(true, cx);
            RowBackgroundCollection::new(
                &cx.entity(),
                comparison_marks(&self.hunks, colors, true),
                cx,
            )
        });
        let result_marks = result.update(cx, |editor, cx| {
            editor.set_readonly(true, cx);
            RowBackgroundCollection::new(
                &cx.entity(),
                comparison_marks(&self.hunks, colors, false),
                cx,
            )
        });
        let inline_marks = presentation::create_inline_marks(&self.editor, result, &self.hunks, cx);
        ThirdComparison {
            version: self.version,
            editor: self.editor,
            title: self.title,
            readonly,
            result_readonly,
            hunks: self.hunks,
            marks,
            result_marks,
            inline_marks,
        }
    }
}

impl ThirdComparison {
    /// Replace owned colors/ranges after a version or theme change without touching source text.
    pub(super) fn refresh_marks(
        &mut self,
        result: &Entity<EditorState>,
        colors: [Hsla; 3],
        cx: &mut App,
    ) {
        self.marks
            .set(comparison_marks(&self.hunks, colors, true), cx);
        self.result_marks
            .set(comparison_marks(&self.hunks, colors, false), cx);
        for mark in &self.inline_marks {
            mark.dispose(cx);
        }
        self.inline_marks =
            presentation::create_inline_marks(&self.editor, result, &self.hunks, cx);
    }

    /// Restore only the view's input restrictions and dispose its own decoration collections.
    pub(super) fn unmount(self, result: &Entity<EditorState>, cx: &mut App) {
        self.marks.dispose(cx);
        self.result_marks.dispose(cx);
        for marks in self.inline_marks {
            marks.dispose(cx);
        }
        self.editor
            .update(cx, |editor, cx| editor.set_readonly(self.readonly, cx));
        result.update(cx, |editor, cx| {
            editor.set_readonly(self.result_readonly, cx)
        });
    }
}

impl EditorApp {
    /// Check an exact live incarnation after activation or before a third source is mounted.
    pub(super) fn comparison_source_is_exact(&self, source: &api::DocumentVersion) -> bool {
        (0..self.tabs.len()).any(|index| {
            self.plugin_document_version(index)
                .is_ok_and(|version| version == *source)
                && self.tabs[index]
                    .virtual_document
                    .as_ref()
                    .is_none_or(|tab| tab.resource.is_live())
        })
    }

    /// Resolve a distinct third source and reject stale, mounted or oversized inputs atomically.
    pub(super) fn prepare_third_comparison(
        &self,
        third: &api::DocumentVersion,
        left: &api::DocumentVersion,
        result: &api::DocumentVersion,
        cx: &App,
    ) -> Result<ThirdSource, api::Failure> {
        if third.id == left.id || third.id == result.id {
            return Err(api::Failure::new(
                api::ErrorCode::InvalidRequest,
                "Comparison requires three distinct sessions",
            ));
        }
        let index = (0..self.tabs.len())
            .find(|index| {
                self.plugin_document_version(*index)
                    .is_ok_and(|version| version == *third)
            })
            .ok_or_else(|| {
                api::Failure::new(
                    api::ErrorCode::StaleRevision,
                    "Third comparison source changed or closed",
                )
            })?;
        if !self.comparison_source_is_exact(third) {
            return Err(api::Failure::new(
                api::ErrorCode::StaleRevision,
                "Third comparison source was revoked",
            ));
        }
        if self.plugin_window_mounts.contains_key(&third.id) {
            return Err(api::Failure::new(
                api::ErrorCode::Conflict,
                "Third comparison source is mounted in another work window",
            ));
        }
        let editor = self.tabs[index]
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
        let find = |source: &api::DocumentVersion| {
            (0..self.tabs.len())
                .find(|index| {
                    self.plugin_document_version(*index)
                        .is_ok_and(|version| version == *source)
                })
                .and_then(|index| {
                    self.tabs[index]
                        .text
                        .as_ref()
                        .map(|text| text.editor.clone())
                })
        };
        let left_editor = find(left).ok_or_else(|| {
            api::Failure::new(
                api::ErrorCode::StaleRevision,
                "Comparison left source changed",
            )
        })?;
        let result_editor = find(result).ok_or_else(|| {
            api::Failure::new(api::ErrorCode::StaleRevision, "Comparison result changed")
        })?;
        let valid = [&left_editor, &result_editor, &editor]
            .iter()
            .all(|editor| editor.read(cx).text().len() <= 1024 * 1024);
        if !valid {
            return Err(api::Failure::new(
                api::ErrorCode::LimitExceeded,
                "Comparison exceeds 1 MiB per source",
            ));
        }
        let texts = [&left_editor, &result_editor, &editor]
            .map(|editor| editor.read(cx).value().to_string());
        let rows = texts
            .each_ref()
            .map(|text| text.split_inclusive('\n').take(2001).count());
        if !row_budget_allows(rows) {
            return Err(api::Failure::new(
                api::ErrorCode::LimitExceeded,
                "Three-source comparison exceeds aggregate 4M cells or 2000 lines",
            ));
        }
        let hunks = diff::compare(&texts[2], &texts[1]).map_err(|_| {
            api::Failure::new(
                api::ErrorCode::LimitExceeded,
                "Third comparison source exceeds limits",
            )
        })?;
        Ok(ThirdSource {
            version: third.clone(),
            editor,
            title: self.tabs[index].title(),
            hunks,
        })
    }

    /// Draw three readonly source frames; all panes retain native focus, selection and Copy.
    pub(super) fn render_three_document_comparison(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui_kit::AnyElement> {
        let comparison = self.document_comparison.as_ref()?;
        let third = comparison.third.as_ref()?;
        comparison.alignment.set_enabled(true, cx);
        comparison.alignment.refresh_native_overlays(cx);
        let sources = [
            (
                &comparison.left,
                &comparison.left_editor,
                &comparison.left_title,
                "document-diff-left",
            ),
            (
                &comparison.right,
                &comparison.right_editor,
                &comparison.right_title,
                "document-diff-right",
            ),
            (
                &third.version,
                &third.editor,
                &third.title,
                "document-diff-third",
            ),
        ];
        let panes = sources
            .into_iter()
            .enumerate()
            .map(|(index, (source, editor, title, selector))| {
                let label = format!("{title} · {}", t!("editor.readonly_label"));
                let input = comparison.alignment.pane(index).unwrap_or_else(|| {
                    editor_ui::readonly_editor(selector, label.clone(), editor, window, cx)
                });
                let target = self.plugin_document_menu_target(source);
                let editor = editor.clone();
                let projection = comparison.alignment.clone();
                div()
                    .id(selector)
                    .debug_selector(move || selector.into())
                    .role(gpui_kit::Role::Group)
                    .aria_label(label)
                    .key_context("NativeEditorSource")
                    .relative()
                    .capture_any_mouse_down(cx.listener(
                        move |this, event: &MouseDownEvent, window, cx| {
                            if event.button == MouseButton::Right {
                                editor.update(cx, |editor, cx| editor.focus(window, cx));
                                this.open_plugin_document_menu(
                                    target.clone(),
                                    projection
                                        .project_pointer_bounds(
                                            index,
                                            Bounds::new(event.position, size(px(1.), px(1.))),
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
                        index,
                        title.clone(),
                        cx,
                    ))
                    .child(div().flex_1().min_h_0().child(input))
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        let navigation = comparison.navigation.clone();
        let editors = vec![
            comparison.left_editor.clone(),
            comparison.right_editor.clone(),
            third.editor.clone(),
        ];
        Some(
            div()
                .relative()
                .flex()
                .flex_col()
                .size_full()
                .min_h_0()
                .child(self.comparison_toolbar(cx))
                .child(div().flex().flex_1().min_h_0().children(panes))
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
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The second pair cannot silently double the comparison allocation ceiling.
    #[test]
    fn three_source_comparison_shares_aggregate_work_budget() {
        assert!(row_budget_allows([1000, 2000, 1000]));
        assert!(!row_budget_allows([1001, 2000, 1000]));
        assert!(!row_budget_allows([2001, 0, 0]));
        assert!(row_budget_allows([0, 2000, 0]));
    }

    /// Byte projections preserve unchanged boundaries after Unicode replacements and deletions.
    #[test]
    fn three_source_navigation_projects_result_boundaries() {
        let hunks = diff::compare("同\n旧😀\n末\n", "同\n新\n末\n").unwrap();
        assert_eq!(source_offset(&hunks, 0), 0);
        assert_eq!(source_offset(&hunks, "同\n".len()), "同\n".len());
        assert_eq!(source_offset(&hunks, "同\n新\n".len()), "同\n旧😀\n".len());
        assert_eq!(
            source_offset(&hunks, "同\n新\n末\n".len()),
            "同\n旧😀\n末\n".len()
        );
        let deleted = diff::compare("a\nremoved\nb\n", "a\nb\n").unwrap();
        assert_eq!(source_offset(&deleted, 2), 2);
        assert_eq!(source_offset(&deleted, 4), 12);
    }
}
