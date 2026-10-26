//! Prepare plugin sources for the default Editor without owning host documents or their authority.
use super::*;
use crate::ui::theme::ActiveTheme as _;
pub(super) use editor_ui::source_editor::Snapshot;
use gpui_base::input::{RangeDecoration, RangeDecorationStyle};
use gpui_kit::{AnyElement, IntoElement as _, SharedString};

impl PluginView {
    /// Reconcile source bytes and owned marks; native focus/selection stay in the retained Editor state.
    pub(super) fn sync_editor_sources(
        &mut self,
        nodes: &[ui::Node],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let keep = nodes
            .iter()
            .filter(|node| {
                matches!(
                    &node.kind,
                    Kind::Editor(ui::Editor {
                        source: ui::EditorSource::Snapshot { .. },
                        ..
                    })
                )
            })
            .map(|node| node.id.clone())
            .collect::<BTreeSet<_>>();
        for id in self
            .snapshots
            .keys()
            .filter(|id| !keep.contains(*id))
            .cloned()
            .collect::<Vec<_>>()
        {
            self.snapshots.remove(&id).unwrap().dispose(cx);
        }
        for node in nodes {
            let Kind::Editor(ui::Editor {
                source: ui::EditorSource::Snapshot { text },
                changes,
                ..
            }) = &node.kind
            else {
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
                .collect();
            if let Some(snapshot) = self.snapshots.get_mut(&node.id) {
                snapshot.update(text, marks, window, cx);
            } else {
                self.snapshots.insert(
                    node.id.clone(),
                    Snapshot::new(text.clone(), marks, window, cx),
                );
            }
        }
    }

    /// Source preparation grants no authority; rendering always uses the default local Editor.
    pub(super) fn render_editor_snapshot(
        &self,
        id: &str,
        actions: Option<editor_ui::EditorActionElements>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(snapshot) = self.snapshots.get(id) else {
            return gpui_kit::div().into_any_element();
        };
        let alignment = self.code_alignment_pane(id);
        let body = alignment
            .as_ref()
            .and_then(|(group, index)| group.pane(*index));
        let mut editor = editor_ui::source_editor::Editor::new(&snapshot.editor)
            .identity(
                SharedString::from(format!("plugin-code-{id}")),
                Some(id.into()),
            )
            .tools(false)
            .presentation(body);
        if let Some(actions) = actions {
            let projection = alignment.map(|(group, index)| {
                Rc::new(move |offset, cx: &App| group.project_range(index, offset..offset, cx))
                    as editor_ui::EditorActionProjection
            });
            editor = editor.edge_controls(actions, projection);
        }
        editor.into_any_element()
    }
}
