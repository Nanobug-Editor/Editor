//! Shared scene spaces keep hit, semantic and drawing projections under the same ancestor chain.
use super::{CanvasScene, SceneGeometry, SceneNode, SceneTransform};

/// Borrowed placement contains no copied path data and grants no resource or input authority.
pub struct CanvasPlacement<'a> {
    /// Local space to Canvas content, composed child first.
    pub transform: SceneTransform,
    /// Each ancestor clip and its local-to-content transform, in outer-to-inner order.
    pub clips: Vec<(SceneTransform, &'a SceneGeometry)>,
    /// Fully transparent ancestors expose neither a hit shape nor an accessible object.
    pub visible: bool,
}
impl CanvasScene {
    /// Resolve an optional named group; None means Canvas content. Missing groups return None.
    /// Call after scene validation; returned geometry borrows this immutable scene snapshot.
    pub fn group_placement(&self, id: Option<&str>) -> Option<CanvasPlacement<'_>> {
        fn find<'a>(
            nodes: &'a [SceneNode],
            id: &str,
            parent: SceneTransform,
            clips: &mut Vec<(SceneTransform, &'a SceneGeometry)>,
            visible: bool,
        ) -> Option<CanvasPlacement<'a>> {
            for node in nodes {
                if let SceneNode::Group {
                    id: found,
                    transform,
                    clip,
                    opacity,
                    children,
                } = node
                {
                    let transform = parent.compose(*transform);
                    if let Some(clip) = clip {
                        clips.push((transform, clip));
                    }
                    let result = if found == id {
                        Some(CanvasPlacement {
                            transform,
                            clips: clips.clone(),
                            visible: visible && *opacity > 0.,
                        })
                    } else {
                        find(children, id, transform, clips, visible && *opacity > 0.)
                    };
                    if clip.is_some() {
                        clips.pop();
                    }
                    if result.is_some() {
                        return result;
                    }
                }
            }
            None
        }
        match id {
            None => Some(CanvasPlacement {
                transform: Default::default(),
                clips: Vec::new(),
                visible: true,
            }),
            Some(id) => find(&self.roots, id, Default::default(), &mut Vec::new(), true),
        }
    }
}
