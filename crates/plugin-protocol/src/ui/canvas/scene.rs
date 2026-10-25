//! Structured, instance-local artwork; geometry has no filesystem or native-object authority.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

mod geometry;
pub use geometry::*;
mod style;
pub use style::*;

/// A complete scene under one canvas; replacement identities must not be reused, including ABA.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasScene {
    /// Nonzero semantic identity, retained through drawing-only updates.
    pub identity: u64,
    /// Monotonic drawing version within this identity; zero is not a published version.
    pub revision: u64,
    /// Back-to-front roots with stable scene-local IDs; groups retain their ordered children.
    pub roots: Vec<SceneNode>,
}

/// Groups apply their transform, clip and isolated opacity to all ordered descendants.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SceneNode {
    /// A hierarchical layer; opacity is applied after children are composited together.
    Group {
        /// Unique scene-local identity, at most 128 UTF-8 bytes.
        id: String,
        /// Local-to-parent affine transform, defaulting to identity.
        #[serde(default)]
        transform: SceneTransform,
        /// Optional geometry in this group's transformed coordinates.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        clip: Option<SceneGeometry>,
        /// Isolated group opacity in the inclusive range 0..1.
        #[serde(default = "opaque")]
        opacity: f32,
        /// Back-to-front child layers and primitives.
        children: Vec<SceneNode>,
    },
    /// One path or rectangle with independent optional fill and stroke.
    Shape {
        /// Unique scene-local identity.
        id: String,
        /// Local geometry; no SVG parser or implicit resource reads are involved.
        geometry: SceneGeometry,
        /// Optional fill; omission leaves the interior transparent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fill: Option<SceneBrush>,
        /// Optional independent outline.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stroke: Option<SceneStroke>,
    },
    /// Reuse pixels from a declared Image/FileImage node in the same UI document.
    Image {
        /// Unique scene-local drawing identity.
        id: String,
        /// Existing native image node ID; no new URI or file authority is granted.
        image: String,
        /// Local destination rectangle; the same group transform and clip apply.
        rect: crate::Rect,
    },
}

/// Default opacity for optional group declarations.
fn opaque() -> f32 {
    1.
}

impl SceneNode {
    /// Return this item's stable identity, independent of its position in the tree.
    pub fn id(&self) -> &str {
        match self {
            Self::Group { id, .. } | Self::Shape { id, .. } | Self::Image { id, .. } => id,
        }
    }
    /// Visit this subtree in drawing order, including groups before their children.
    pub fn visit(&self, visitor: &mut impl FnMut(&Self)) {
        visitor(self);
        if let Self::Group { children, .. } = self {
            for child in children {
                child.visit(visitor);
            }
        }
    }
}

impl CanvasScene {
    /// Validate the composed native viewport before allocating paths or accepting inverse input.
    pub(in crate::ui) fn validate_projection(
        &self,
        transform: SceneTransform,
    ) -> Result<(), String> {
        if !transform.valid() {
            return Err("Invalid projected canvas scene transform".into());
        }
        let mut budget = Budget::default();
        for root in &self.roots {
            budget.node(root, transform, 0, 0)?;
        }
        Ok(())
    }
    /// Check IDs, transformed geometry and aggregate budgets before host resource preparation.
    /// Returns an admission error without changing any previously published scene.
    pub fn validate(&self) -> Result<(), String> {
        if self.identity == 0 || self.revision == 0 {
            return Err("Scene identity and revision must be nonzero".into());
        }
        let mut budget = Budget::default();
        for root in &self.roots {
            budget.node(root, SceneTransform::default(), 0, 0)?;
        }
        if serde_json::to_vec(self).map_or(true, |bytes| bytes.len() > 2 * 1024 * 1024) {
            return Err("Canvas scene exceeds its 2 MiB encoded budget".into());
        }
        Ok(())
    }
    /// Return image-node references without following any URI or exposing foreign resources.
    pub fn image_nodes(&self) -> Vec<&str> {
        fn collect<'a>(node: &'a SceneNode, refs: &mut Vec<&'a str>) {
            match node {
                SceneNode::Image { image, .. } => refs.push(image),
                SceneNode::Group { children, .. } => {
                    for child in children {
                        collect(child, refs);
                    }
                }
                _ => {}
            }
        }
        let mut refs = Vec::new();
        for node in &self.roots {
            collect(node, &mut refs);
        }
        refs
    }
}

/// One traversal charges primitives, IDs and path segments across every nested layer.
#[derive(Default)]
struct Budget {
    ids: BTreeSet<String>,
    segments: usize,
}
impl Budget {
    /// Admit one node under `parent`, rejecting excessive depth before recursing further.
    fn node(
        &mut self,
        node: &SceneNode,
        parent: SceneTransform,
        depth: usize,
        clips: usize,
    ) -> Result<(), String> {
        if depth > 32
            || !valid_id(node.id())
            || !self.ids.insert(node.id().into())
            || self.ids.len() > 32_000
        {
            return Err("Invalid/duplicate scene identity or layer budget exceeded".into());
        }
        match node {
            SceneNode::Group {
                transform,
                clip,
                opacity,
                children,
                ..
            } => {
                let transform = parent.compose(*transform);
                if !transform.valid() || !opacity.is_finite() || !(0. ..=1.).contains(opacity) {
                    return Err("Invalid canvas transform or opacity".into());
                }
                let clips = clips + usize::from(clip.is_some());
                if clips > 16 {
                    return Err("Canvas clip nesting exceeds sixteen".into());
                }
                if let Some(clip) = clip {
                    self.geometry(clip, transform)?;
                }
                for child in children {
                    self.node(child, transform, depth + 1, clips)?;
                }
            }
            SceneNode::Shape {
                geometry,
                fill,
                stroke,
                ..
            } => {
                self.geometry(geometry, parent)?;
                if fill.is_none() && stroke.is_none() {
                    return Err("A scene shape needs fill or stroke".into());
                }
                if let Some(fill) = fill {
                    fill.validate()?;
                }
                if let Some(stroke) = stroke {
                    stroke.validate()?;
                    if !stroke.dash.is_empty() {
                        // Charge expansion before GPUI/resvg can loop or allocate. f64 accounting
                        // also catches subnormal periods that cannot advance a large f32 cursor.
                        let period = stroke
                            .dash
                            .iter()
                            .map(|value| f64::from(*value))
                            .sum::<f64>();
                        let work = (geometry.length_bound() / period).ceil()
                            * stroke.dash.len() as f64
                            + stroke.dash.len() as f64;
                        if !work.is_finite() || work > 128_000. {
                            return Err("Canvas dash expansion exceeds its work budget".into());
                        }
                        self.segments += work as usize;
                        if self.segments > 128_000 {
                            return Err(
                                "Canvas dash expansion exceeds its shared segment budget".into()
                            );
                        }
                    }
                    let [a, b, c, d, _, _] = parent.0;
                    let margin = (a.abs() + c.abs()).max(b.abs() + d.abs())
                        * stroke.width
                        * 0.5
                        * stroke.miter_limit;
                    if !geometry.valid_with_margin(parent, margin) {
                        return Err("Projected canvas stroke exceeds the drawing extent".into());
                    }
                }
            }
            SceneNode::Image { image, rect, .. } => {
                if !valid_id(image) {
                    return Err("Invalid canvas image reference".into());
                }
                self.geometry(
                    &SceneGeometry::Rect {
                        rect: *rect,
                        radii: [0.; 4],
                    },
                    parent,
                )?;
            }
        }
        Ok(())
    }
    /// Charge and validate geometry after the complete ancestor transform.
    fn geometry(
        &mut self,
        geometry: &SceneGeometry,
        transform: SceneTransform,
    ) -> Result<(), String> {
        self.segments += geometry.segment_count();
        if self.segments > 128_000 || !geometry.valid(transform) {
            return Err("Invalid canvas geometry or path budget exceeded".into());
        }
        Ok(())
    }
}

/// Stable scene identifiers exclude control characters and empty strings.
pub(super) fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 128 && !id.chars().any(char::is_control)
}
