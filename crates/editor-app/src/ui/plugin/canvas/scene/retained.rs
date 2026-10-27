//! Stable drawing items share immutable prepared payloads while the current publication owns residency.
use super::*;
use std::sync::Arc;
pub(super) mod layers;

/// A layer receives only its currently remaining shared pixel allowance before native allocation.
pub(super) type Rasterizer<'a> = dyn FnMut(
        &str,
        protocol::Rect,
        &BTreeMap<String, Arc<Vec<u8>>>,
        u64,
    ) -> Option<super::super::super::images::VectorImage>
    + 'a;

/// Production counters distinguish actual worker work, native layout work and current residency.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct SceneStatistics {
    /// Native items or layers compiled: cumulative on SceneImages, per-preparation on PreparedScene.
    pub prepared_items: u64,
    /// Item/layer reuse: cumulative on SceneImages, per-preparation on PreparedScene.
    pub reused_items: u64,
    /// Hit/semantic compilations: cumulative on SceneImages, per-preparation on PreparedScene.
    pub target_preparations: u64,
    /// Exact target-geometry reuse: cumulative on SceneImages, per-preparation on PreparedScene.
    pub target_reuses: u64,
    /// Completed layer rasters: cumulative on SceneImages, per-preparation on PreparedScene.
    pub rasterizations: u64,
    /// Actual immutable paragraph layouts produced by the retained native painter.
    pub paragraph_layouts: u64,
    /// Live prepared leaf entries, excluding old input/presentation snapshots.
    pub cached_items: usize,
    /// Unique resident derived RGBA bytes in the current publication.
    pub resident_bytes: u64,
    /// Derived image pixels still owned by current or older live published snapshots; atlas memory is separate.
    pub live_resident_bytes: u64,
    /// Live derived image owners, counted without extending their lifetime.
    pub live_textures: usize,
    /// Current prepared native path and input geometry vertices.
    pub vertices: usize,
}
impl SceneStatistics {
    /// Accumulate work once per newly prepared scene; residency is recomputed from current owners.
    pub(crate) fn add_work(&mut self, other: Self) {
        self.prepared_items = self.prepared_items.saturating_add(other.prepared_items);
        self.reused_items = self.reused_items.saturating_add(other.reused_items);
        self.target_preparations = self
            .target_preparations
            .saturating_add(other.target_preparations);
        self.target_reuses = self.target_reuses.saturating_add(other.target_reuses);
        self.rasterizations = self.rasterizations.saturating_add(other.rasterizations);
    }
}

/// Drawing revisions do not change input geometry; only its declared owners and full placement matter.
pub(super) fn same_targets(before: &CanvasScene, after: &CanvasScene) -> bool {
    if before.targets != after.targets || before.semantics != after.semantics {
        return false;
    }
    let spaces = after
        .targets
        .iter()
        .map(|target| target.space.as_deref())
        .chain(
            after
                .semantics
                .iter()
                .map(|semantic| semantic.space.as_deref()),
        )
        .collect::<std::collections::BTreeSet<_>>();
    spaces.into_iter().all(
        |id| match (before.group_placement(id), after.group_placement(id)) {
            (Some(a), Some(b)) => {
                a.transform == b.transform && a.visible == b.visible && a.clips == b.clips
            }
            _ => false,
        },
    )
}

/// Complete per-leaf inputs and their immutable result; no plugin/native mutable domain state is stored.
pub(super) struct Item {
    layer: bool,
    layer_photos: BTreeMap<String, Arc<super::super::super::images::Photo>>,
    declaration: SceneNode,
    transform: SceneTransform,
    clip: Option<protocol::Rect>,
    typography: Option<Typography>,
    photo: Option<Arc<super::super::super::images::Photo>>,
    paths: Vec<Arc<ScenePrimitive>>,
    bitmaps: BTreeMap<String, Arc<gpui_kit::RenderImage>>,
    vertices: usize,
}

/// Prepare groups structurally, then compile/reuse leaves with the same inherited native dependencies.
pub(super) fn prepare(
    drawing: &mut SceneDrawing,
    node: &SceneNode,
    transform: SceneTransform,
    photos: &BTreeMap<String, Arc<super::super::super::images::Photo>>,
    limits: (u64, usize),
    clip: Option<protocol::Rect>,
    typography: &Typography,
    previous: Option<&SceneDrawing>,
    statistics: &mut SceneStatistics,
) -> Result<(), String> {
    if clip.is_some_and(|clip| clip.w <= 0. || clip.h <= 0.) {
        return Ok(());
    }
    if let SceneNode::Group {
        transform: child,
        clip: local,
        opacity,
        children,
        ..
    } = node
    {
        if *opacity == 0. {
            return Ok(());
        }
        if *opacity != 1. {
            return Err("Canvas isolated group requires raster preparation".into());
        }
        let transform = transform.compose(*child);
        let clip = match local {
            Some(geometry) => Some(rectangular_clip(geometry, transform, clip)?),
            None => clip,
        };
        for child in children {
            prepare(
                drawing, child, transform, photos, limits, clip, typography, previous, statistics,
            )?;
        }
        return Ok(());
    }
    let photo = match node {
        SceneNode::Image { image, .. } => photos.get(image).cloned(),
        _ => None,
    };
    let native_typography =
        matches!(node, SceneNode::Paragraph { .. }).then_some(typography.clone());
    if let Some(cached) = previous
        .and_then(|old| old.retained.get(node.id()))
        .filter(|cached| {
            cached.declaration == *node
                && cached.transform == transform
                && cached.clip == clip
                && cached.typography == native_typography
                && same_photo(cached.photo.as_ref(), photo.as_ref())
        })
    {
        drawing.vertices = drawing.vertices.saturating_add(cached.vertices);
        if drawing.vertices > limits.1 {
            return Err("Canvas tessellation exceeds its shared vertex budget".into());
        }
        drawing.paths.extend(cached.paths.iter().cloned());
        drawing.bitmaps.extend(
            cached
                .bitmaps
                .iter()
                .map(|(id, image)| (id.clone(), image.clone())),
        );
        if drawing.resident_bytes() > limits.0 {
            return Err("Canvas retained pixels exceed the shared budget".into());
        }
        drawing.retained.insert(node.id().into(), cached.clone());
        statistics.reused_items += 1;
        return Ok(());
    }
    let start = drawing.paths.len();
    let vertices = drawing.vertices;
    drawing.node(node, transform, photos, limits, clip)?;
    if let Some(old) = previous.and_then(|old| old.retained.get(node.id())) {
        let before = old.paths.iter().find_map(|item| match item.as_ref() {
            ScenePrimitive::Paragraph { drawing, .. } => Some(drawing),
            _ => None,
        });
        if let Some(before) = before {
            for item in &mut drawing.paths[start..] {
                if let Some(ScenePrimitive::Paragraph { drawing, .. }) = Arc::get_mut(item) {
                    drawing.reuse_layout(before);
                }
            }
        }
    }
    let bitmaps = match node {
        SceneNode::Image { image, .. } => drawing
            .bitmaps
            .get(image)
            .map(|pixels| BTreeMap::from([(image.clone(), pixels.clone())]))
            .unwrap_or_default(),
        _ => BTreeMap::new(),
    };
    drawing.retained.insert(
        node.id().into(),
        Arc::new(Item {
            layer: false,
            layer_photos: BTreeMap::new(),
            declaration: node.clone(),
            transform,
            clip,
            typography: native_typography,
            photo,
            paths: drawing.paths[start..].to_vec(),
            bitmaps,
            vertices: drawing.vertices.saturating_sub(vertices),
        }),
    );
    statistics.prepared_items += 1;
    Ok(())
}

/// Identical metadata is insufficient after decode/resource completion; compare actual immutable owners.
fn same_photo(
    a: Option<&Arc<super::super::super::images::Photo>>,
    b: Option<&Arc<super::super::super::images::Photo>>,
) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}
