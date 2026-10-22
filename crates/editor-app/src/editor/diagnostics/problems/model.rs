//! Frame-local rows and summary counts borrow each open text document's canonical diagnostic model.
use super::*;
use gpui_base::input::{DiagnosticSeverity, Position};
use std::ops::Range;

/// An immutable selection identity is checked against live rows, never used as a diagnostic cache.
#[derive(Clone, PartialEq, Eq)]
pub(super) struct ProblemKey {
    pub(super) document: api::DocumentVersion,
    pub(super) range: Range<Position>,
    pub(super) source: String,
    pub(super) message: String,
    pub(super) collection: Option<(String, String)>,
}

/// File headers contribute to the native scroll child index, independently of diagnostic selection.
pub(super) struct Row {
    pub(super) key: ProblemKey,
    pub(super) path: PathBuf,
    pub(super) file_label: String,
    pub(super) severity: DiagnosticSeverity,
    pub(super) list_index: usize,
}

impl Row {
    /// Preserve the marker's severity and source, including independent collection owners.
    fn new(
        document: &api::DocumentVersion,
        path: &Path,
        file_label: &str,
        item: Diagnostic,
        collection: Option<(String, String)>,
    ) -> Self {
        Self {
            key: ProblemKey {
                document: document.clone(),
                range: item.range,
                source: item
                    .source
                    .map(|source| source.to_string())
                    .unwrap_or_else(|| t!("problems.language_source").to_string()),
                message: item.message.to_string(),
                collection,
            },
            path: path.to_path_buf(),
            file_label: file_label.to_owned(),
            severity: item.severity,
            list_index: 0,
        }
    }
}

impl EditorApp {
    /// Project known open-file diagnostics without starting extra services or a workspace scan.
    pub(super) fn problem_rows(&self, cx: &App) -> Vec<Row> {
        let mut rows = Vec::new();
        for (index, file) in self.tabs.iter().enumerate() {
            let Some(tab) = &file.text else { continue };
            let Ok(document) = self.plugin_document_version(index) else {
                continue;
            };
            let path = file.path();
            let file_label =
                editor_core::display_path(path.strip_prefix(self.workspace.root()).unwrap_or(path))
                    .to_string();
            for item in tab.diagnostics.legacy_projection() {
                rows.push(Row::new(&document, path, &file_label, item, None));
            }
            let text = tab.editor.read(cx).text();
            for (key, publication) in tab
                .diagnostics
                .direct
                .iter()
                .filter(|(_, item)| item.collection.is_live() && item.document == document)
            {
                for (item, diagnostic) in publication
                    .project(text)
                    .into_iter()
                    .zip(&publication.items)
                {
                    rows.push(Row::new(
                        &document,
                        path,
                        &file_label,
                        item,
                        Some((key.clone(), diagnostic.id.clone())),
                    ));
                }
            }
        }
        rows.sort_by(|a, b| {
            (&a.path, a.key.range.start, &a.key.source, &a.key.message).cmp(&(
                &b.path,
                b.key.range.start,
                &b.key.source,
                &b.key.message,
            ))
        });
        let mut list_index = 0;
        for index in 0..rows.len() {
            if index == 0 || rows[index].path != rows[index - 1].path {
                list_index += 1;
            }
            rows[index].list_index = list_index;
            if !self.problems.collapsed.contains(&rows[index].path) {
                list_index += 1;
            }
        }
        rows
    }

    /// Disclosure changes the keyboard projection while summary counts still consume every live row.
    pub(super) fn visible_problem_rows<'a>(&self, rows: &'a [Row]) -> Vec<&'a Row> {
        rows.iter()
            .filter(|row| !self.problems.collapsed.contains(&row.path))
            .collect()
    }

    /// Keep the same diagnostic selected through reordering; a removed selection falls back to row one.
    pub(super) fn selected_problem_index(&self, rows: &[&Row]) -> usize {
        self.problems
            .selected
            .as_ref()
            .and_then(|key| rows.iter().position(|row| &row.key == key))
            .unwrap_or(0)
    }

    /// Omit the empty footer entry; the shortcut remains available regardless of publication state.
    pub(crate) fn render_syntax_error_indicator(
        &self,
        cx: &Context<Self>,
    ) -> Option<gpui_kit::AnyElement> {
        let rows = self.problem_rows(cx);
        if rows.is_empty() {
            return None;
        }
        let errors = rows
            .iter()
            .filter(|row| row.severity == DiagnosticSeverity::Error)
            .count();
        let warnings = rows
            .iter()
            .filter(|row| row.severity == DiagnosticSeverity::Warning)
            .count();
        let others = rows.len() - errors - warnings;
        let capped = self
            .tabs
            .iter()
            .filter_map(|file| file.text.as_ref())
            .any(|tab| {
                tab.diagnostics.syntax.len() == crate::language::diagnostics::MAX_DIAGNOSTICS
            });
        Some(
            editor_ui::problems::problem_indicator(errors, warnings, others, capped, cx)
                .on_click(cx.listener(|app, _, window, cx| app.show_problems(window, cx)))
                .into_any_element(),
        )
    }
}
