//! Lexicographic child paths preserve native foreground order even when parent probes paint last.
use super::*;

/// Ordinary descendants outrank a parent background; later overlay siblings outrank an entire earlier subtree.
pub(super) fn collect(document: &ui::Document) -> BTreeMap<String, Vec<usize>> {
    fn visit(node: &ui::Node, path: &mut Vec<usize>, paths: &mut BTreeMap<String, Vec<usize>>) {
        paths.insert(node.id.clone(), path.clone());
        let children: Vec<&ui::Node> = match &node.kind {
            ui::Kind::Column { children } | ui::Kind::Row { children } => children.iter().collect(),
            ui::Kind::Scroll { content } => vec![content],
            ui::Kind::Tabs { tabs, .. } => tabs.iter().map(|tab| &tab.content).collect(),
            ui::Kind::Canvas(canvas) => canvas
                .overlays
                .iter()
                .map(|overlay| overlay.child.as_ref())
                .collect(),
            ui::Kind::Editor(editor) => editor
                .left_actions()
                .iter()
                .chain(editor.right_actions())
                .map(|action| action.content.as_ref())
                .collect(),
            _ => Vec::new(),
        };
        for (index, child) in children.into_iter().enumerate() {
            path.push(index);
            visit(child, path, paths);
            path.pop();
        }
    }
    let mut paths = BTreeMap::new();
    visit(&document.root, &mut vec![0], &mut paths);
    if let Some(toolbar) = &document.editor_toolbar {
        visit(toolbar, &mut vec![1], &mut paths);
    }
    if let Some(dialog) = &document.dialog {
        visit(&dialog.content, &mut vec![2], &mut paths);
    }
    paths
}
