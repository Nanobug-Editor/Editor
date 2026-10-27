//! Native authority follows semantic overlay ancestry, independently of drawing-only placement.
use plugin_protocol::ui::{self, Document, Kind, Node};

/// One semantic foreground owner; moving its rectangle never replaces this incarnation.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct OverlayOwner {
    canvas: String,
    overlay: String,
    identity: u64,
    target: ui::CanvasTargetRef,
}
/// Outer-to-inner ownership is captured by native frame proofs and transient transfer leases.
pub(crate) type Lineage = Vec<OverlayOwner>;

/// Find a declaration, including inactive tabs, without granting input authority.
pub(crate) fn find<'a>(document: &'a Document, id: &str) -> Option<&'a Node> {
    locate(document, id).map(|(node, _)| node)
}
/// Return the exact enclosing overlay incarnations; an absent declaration has no lineage.
pub(crate) fn lineage(document: &Document, id: &str) -> Option<Lineage> {
    locate(document, id).map(|(_, lineage)| lineage)
}
/// Share one complete declaration traversal so editor action groups and nested canvases cannot diverge.
fn locate<'a>(document: &'a Document, id: &str) -> Option<(&'a Node, Lineage)> {
    let mut path = Vec::new();
    let node = descend(&document.root, id, &mut path)
        .or_else(|| {
            document
                .editor_toolbar
                .as_ref()
                .and_then(|node| descend(node, id, &mut path))
        })
        .or_else(|| {
            document
                .dialog
                .as_ref()
                .and_then(|dialog| descend(&dialog.content, id, &mut path))
        })?;
    Some((node, path))
}
/// Only Canvas overlay edges extend ownership; ordinary layout and inactive tab discovery do not.
fn descend<'a>(node: &'a Node, id: &str, path: &mut Lineage) -> Option<&'a Node> {
    if node.id == id {
        return Some(node);
    }
    match &node.kind {
        Kind::Column { children } | Kind::Row { children } => {
            children.iter().find_map(|child| descend(child, id, path))
        }
        Kind::Scroll { content } => descend(content, id, path),
        Kind::Tabs { tabs, .. } => tabs.iter().find_map(|tab| descend(&tab.content, id, path)),
        Kind::Editor(editor) => editor
            .left_actions()
            .iter()
            .chain(editor.right_actions())
            .find_map(|action| descend(&action.content, id, path)),
        Kind::Canvas(canvas) => {
            for overlay in &canvas.overlays {
                path.push(OverlayOwner {
                    canvas: node.id.clone(),
                    overlay: overlay.id.clone(),
                    identity: overlay.anchor.identity,
                    target: overlay.anchor.target.clone(),
                });
                if let Some(found) = descend(&overlay.child, id, path) {
                    return Some(found);
                }
                path.pop();
            }
            None
        }
        _ => None,
    }
}
