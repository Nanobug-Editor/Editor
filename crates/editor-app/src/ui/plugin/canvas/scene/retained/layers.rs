//! Independent root layers preserve compositing while retaining unchanged native pixel resources.
use super::*;

/// Prepare roots in exact back-to-front order; only an unsupported isolated root becomes a bitmap.
pub(in crate::ui::plugin::canvas::scene) fn prepare_root(
    drawing: &mut SceneDrawing,
    node: &SceneNode,
    photos: &BTreeMap<String, Arc<super::super::super::super::images::Photo>>,
    limits: (u64, usize),
    typography: &Typography,
    previous: Option<&SceneDrawing>,
    statistics: &mut SceneStatistics,
    rasterize: &mut Rasterizer<'_>,
) -> Result<(), String> {
    let dependencies = || {
        let mut dependencies = BTreeMap::new();
        node.visit(&mut |node| {
            if let SceneNode::Image { image, .. } = node
                && let Some(photo) = photos.get(image)
            {
                dependencies.insert(image.clone(), photo.clone());
            }
        });
        dependencies
    };
    if let Some(cached) = previous
        .and_then(|old| old.retained.get(node.id()))
        .filter(|item| {
            item.layer && item.declaration == *node && item.typography.as_ref() == Some(typography)
        })
    {
        let current = dependencies();
        if cached.layer_photos.len() == current.len()
            && cached.layer_photos.iter().all(|(id, photo)| {
                current
                    .get(id)
                    .is_some_and(|current| Arc::ptr_eq(current, photo))
            })
        {
            drawing.paths.extend(cached.paths.iter().cloned());
            drawing.bitmaps.extend(
                cached
                    .bitmaps
                    .iter()
                    .map(|(id, image)| (id.clone(), image.clone())),
            );
            if drawing.resident_bytes() > limits.0 {
                return Err("Canvas retained layers exceed the shared pixel budget".into());
            }
            drawing.retained.insert(node.id().into(), cached.clone());
            statistics.reused_items += 1;
            return Ok(());
        }
    }
    let start = drawing.paths.len();
    let vertices = drawing.vertices;
    if prepare(
        drawing,
        node,
        SceneTransform::default(),
        photos,
        limits,
        None,
        typography,
        previous,
        statistics,
    )
    .is_ok()
    {
        return Ok(());
    }
    // A failed isolated attempt cannot retain partial native primitives or their temporary textures.
    drawing.paths.truncate(start);
    drawing.vertices = vertices;
    let mut failed = std::collections::BTreeSet::new();
    node.visit(&mut |node| {
        failed.insert(node.id().to_owned());
    });
    drawing.retained.retain(|id, _| !failed.contains(id));
    let used = drawing
        .retained
        .values()
        .flat_map(|item| item.bitmaps.keys().cloned())
        .collect::<std::collections::BTreeSet<_>>();
    drawing.bitmaps.retain(|id, _| used.contains(id));
    let budget = limits.0.saturating_sub(drawing.resident_bytes());
    let fragment = CanvasScene {
        identity: 1,
        revision: 1,
        roots: vec![node.clone()],
        targets: Vec::new(),
        semantics: Vec::new(),
    };
    let (svg, bounds, images) = svg::encode(&fragment, photos, typography, budget, None)?;
    let image = rasterize(&svg, bounds, &images, budget)
        .ok_or("Canvas layer raster preparation failed or exhausted its budget")?;
    let size = image.image.size(0);
    let width = u64::try_from(size.width.0).map_err(|_| "Invalid native layer width")?;
    let height = u64::try_from(size.height.0).map_err(|_| "Invalid native layer height")?;
    if width.saturating_mul(height).saturating_mul(4) > budget {
        return Err("Canvas layer raster result exceeded its reserved pixel budget".into());
    }
    // Guest image IDs cannot contain NUL, so derived layer slots cannot collide with admitted images.
    let bitmaps = BTreeMap::from([(format!("\0layer:{}", node.id()), image.image.clone())]);
    let paths = vec![Arc::new(ScenePrimitive::Raster { image, clip: None })];
    drawing.paths.extend(paths.iter().cloned());
    drawing.bitmaps.extend(
        bitmaps
            .iter()
            .map(|(id, image)| (id.clone(), image.clone())),
    );
    drawing.retained.insert(
        node.id().into(),
        Arc::new(Item {
            layer: true,
            layer_photos: dependencies(),
            declaration: node.clone(),
            transform: SceneTransform::default(),
            clip: None,
            typography: Some(typography.clone()),
            photo: None,
            paths,
            bitmaps,
            vertices: 0,
        }),
    );
    statistics.prepared_items += 1;
    statistics.rasterizations += 1;
    Ok(())
}
