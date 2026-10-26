//! Ordered retained operations mutate only a private candidate, then complete validation publishes it.
use super::*;

/// Identify drawing damage separately from bounded input/semantic/native-control metadata.
pub(super) fn drawing_id(operation: &CanvasSceneOperation) -> Option<&str> {
    match operation {
        CanvasSceneOperation::Insert { node, .. } | CanvasSceneOperation::Replace { node } => {
            Some(node.id())
        }
        CanvasSceneOperation::Remove { id }
        | CanvasSceneOperation::Move { id, .. }
        | CanvasSceneOperation::Group { id, .. } => Some(id),
        _ => None,
    }
}
/// All lookup, insertion and removal failures leave the caller's accepted document untouched.
pub(super) fn apply(canvas: &mut Canvas, operation: &CanvasSceneOperation) -> Result<(), Failure> {
    let scene = canvas
        .scene
        .as_mut()
        .ok_or_else(|| invalid("Missing retained scene"))?;
    match operation {
        CanvasSceneOperation::Advance => {}
        CanvasSceneOperation::Replace { node } => {
            if !replace(&mut scene.roots, node) {
                return Err(invalid("Unknown scene drawing ID"));
            }
        }
        CanvasSceneOperation::Remove { id } => {
            if !remove(&mut scene.roots, id) {
                return Err(invalid("Unknown scene drawing ID"));
            }
        }
        CanvasSceneOperation::Insert {
            parent,
            index,
            node,
        } => insert(&mut scene.roots, parent.as_deref(), *index, node.clone())?,
        CanvasSceneOperation::Move { id, parent, index } => {
            let node = take(&mut scene.roots, id)
                .ok_or_else(|| invalid("Unknown moving scene drawing ID"))?;
            insert(&mut scene.roots, parent.as_deref(), *index, node)?;
        }
        CanvasSceneOperation::Group {
            id,
            transform,
            clip,
            opacity,
        } => {
            let node =
                find_mut(&mut scene.roots, id).ok_or_else(|| invalid("Unknown scene group"))?;
            let SceneNode::Group {
                transform: old,
                clip: old_clip,
                opacity: old_opacity,
                ..
            } = node
            else {
                return Err(invalid("Scene attribute target is not a group"));
            };
            *old = *transform;
            *old_clip = clip.clone();
            *old_opacity = *opacity;
        }
        CanvasSceneOperation::UpsertTarget { index, target } => {
            upsert(&mut scene.targets, *index, target.clone(), |value| {
                &value.id
            })?
        }
        CanvasSceneOperation::RemoveTarget { id } => {
            remove_item(&mut scene.targets, id, |value| &value.id)?
        }
        CanvasSceneOperation::UpsertSemantic { index, node } => {
            upsert(&mut scene.semantics, *index, node.clone(), |value| {
                &value.id
            })?
        }
        CanvasSceneOperation::RemoveSemantic { id } => {
            remove_item(&mut scene.semantics, id, |value| &value.id)?
        }
        CanvasSceneOperation::UpsertOverlay { index, overlay } => {
            upsert(&mut canvas.overlays, *index, overlay.clone(), |value| {
                &value.id
            })?
        }
        CanvasSceneOperation::RemoveOverlay { id } => {
            remove_item(&mut canvas.overlays, id, |value| &value.id)?
        }
        CanvasSceneOperation::Properties { value } => {
            canvas.font = value.font.clone();
            canvas.scroll = value.scroll.clone();
            canvas.scroll_epoch = value.scroll_epoch;
            canvas.scroll_buffer = value.scroll_buffer.clone();
            canvas.scroll_2d = value.scroll_2d.clone();
            canvas.text_queries = value.text_queries.clone();
            canvas.measure_text = value.measure_text.clone();
            canvas.focusable = value.focusable;
            canvas.grid = value.grid;
            canvas.caret = value.caret;
            canvas.column_resize = value.column_resize.clone();
        }
    }
    Ok(())
}

/// Node lookup returns a mutable candidate reference, never a native object or another owner.
pub(super) fn find_mut<'a>(nodes: &'a mut [SceneNode], id: &str) -> Option<&'a mut SceneNode> {
    for node in nodes {
        if node.id() == id {
            return Some(node);
        }
        if let SceneNode::Group { children, .. } = node
            && let Some(found) = find_mut(children, id)
        {
            return Some(found);
        }
    }
    None
}
/// Detach exactly one subtree so a move cannot duplicate its descendants or create a cycle.
fn take(nodes: &mut Vec<SceneNode>, id: &str) -> Option<SceneNode> {
    if let Some(index) = nodes.iter().position(|node| node.id() == id) {
        return Some(nodes.remove(index));
    }
    for node in nodes {
        if let SceneNode::Group { children, .. } = node
            && let Some(found) = take(children, id)
        {
            return Some(found);
        }
    }
    None
}
/// Explicit parent and index preserve ordinary back-to-front drawing order.
fn insert(
    nodes: &mut Vec<SceneNode>,
    parent: Option<&str>,
    index: usize,
    node: SceneNode,
) -> Result<(), Failure> {
    let siblings = if let Some(parent) = parent {
        let Some(SceneNode::Group { children, .. }) = find_mut(nodes, parent) else {
            return Err(invalid("Unknown insertion parent group"));
        };
        children
    } else {
        nodes
    };
    if index > siblings.len() {
        return Err(invalid("Scene insertion index exceeds siblings"));
    }
    siblings.insert(index, node);
    Ok(())
}
/// Moving an existing metadata object is explicit, so hit/foreground/assistive order stays stable.
fn upsert<T>(
    items: &mut Vec<T>,
    index: usize,
    value: T,
    id: impl Fn(&T) -> &str,
) -> Result<(), Failure> {
    if let Some(position) = items.iter().position(|item| id(item) == id(&value)) {
        items.remove(position);
    }
    if index > items.len() {
        return Err(invalid("Scene metadata index exceeds siblings"));
    }
    items.insert(index, value);
    Ok(())
}
/// Explicit unknown removals reject instead of silently advancing an incorrect base.
fn remove_item<T>(
    items: &mut Vec<T>,
    wanted: &str,
    id: impl Fn(&T) -> &str,
) -> Result<(), Failure> {
    let index = items
        .iter()
        .position(|item| id(item) == wanted)
        .ok_or_else(|| invalid("Unknown scene metadata ID"))?;
    items.remove(index);
    Ok(())
}
/// Construction errors keep the same typed failure across the SDK and native host.
fn invalid(message: &str) -> Failure {
    Failure::new(ErrorCode::InvalidRequest, message)
}
