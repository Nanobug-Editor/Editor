//! Immutable plugin snapshots use Base selection and scrolling, with no editable document session.
use super::*;
use crate::ui::theme::ActiveTheme as _;
use editor_ui::row_backgrounds::RowBackgroundCollection;
use gpui_base::input::{EditorState, RangeDecoration, RangeDecorationStyle};
use gpui_kit::{AnyElement, IntoElement as _, SharedString};

pub(super) struct Snapshot {
    pub(super) editor: Entity<EditorState>,
    text: String,
    pub(super) generation: u64,
    marks: RowBackgroundCollection,
}
impl PluginView {
    /// Retain native scroll/focus while replacing only supplied immutable bytes and owned colors.
    pub(super) fn sync_readonly_code(
        &mut self,
        nodes: &[ui::Node],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let keep = nodes
            .iter()
            .filter(|node| matches!(node.kind, Kind::ReadonlyCode { .. }))
            .map(|node| node.id.clone())
            .collect::<BTreeSet<_>>();
        for id in self
            .readonly_code
            .keys()
            .filter(|id| !keep.contains(*id))
            .cloned()
            .collect::<Vec<_>>()
        {
            self.readonly_code.remove(&id).unwrap().marks.dispose(cx);
        }
        for node in nodes {
            let Kind::ReadonlyCode { text, changes, .. } = &node.kind else {
                continue;
            };
            let color = cx.theme().warning.opacity(0.22);
            let marks = changes
                .iter()
                .filter(|range| range.start < range.end)
                .map(|range| {
                    RangeDecoration::new(range.start..range.end)
                        .with_style(RangeDecorationStyle::Fill)
                        .with_color(color)
                })
                .collect::<Vec<_>>();
            if let Some(snapshot) = self.readonly_code.get_mut(&node.id) {
                if snapshot.text != *text {
                    snapshot
                        .editor
                        .update(cx, |editor, cx| editor.set_value(text.clone(), window, cx));
                    snapshot.text = text.clone();
                    snapshot.generation = snapshot.generation.wrapping_add(1);
                }
                snapshot.marks.set(marks, cx);
            } else {
                let editor = cx.new(|cx| {
                    EditorState::new(window, cx)
                        .language("text")
                        .line_number(true)
                        .soft_wrap(false)
                });
                let marks = editor.update(cx, |editor, cx| {
                    editor.set_value(text.clone(), window, cx);
                    editor.set_readonly(true, cx);
                    RowBackgroundCollection::new(&cx.entity(), marks, cx)
                });
                self.readonly_code.insert(
                    node.id.clone(),
                    Snapshot {
                        editor,
                        text: text.clone(),
                        generation: 0,
                        marks,
                    },
                );
            }
        }
    }
    /// Copy/select/scroll stay in Base; this source can never receive an editor write request.
    pub(super) fn render_readonly_code(
        &self,
        id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(snapshot) = self.readonly_code.get(id) else {
            return gpui_kit::div().into_any_element();
        };
        let body = self
            .code_alignment_pane(id)
            .and_then(|(group, index)| group.pane(index));
        let source = editor_ui::readonly_editor_body(
            SharedString::from(format!("plugin-code-{id}")),
            id.into(),
            &snapshot.editor,
            body,
            window,
            cx,
        );
        self.code_sync_pane(id, source)
    }
}
