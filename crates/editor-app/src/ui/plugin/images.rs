//! Rasterize clipped plugin vectors off the UI thread and reuse unchanged scene images.

use gpui_kit::RenderImage;
use plugin_runtime::plugin_protocol::{Paint, Rect, ui::Document};
use resvg::{tiny_skia, usvg};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

#[cfg(test)]
#[path = "images/scroll_tests.rs"]
mod scroll_tests;

/// The image bounds describe the visible raster; the SVG may extend beyond them after zooming.
#[derive(Clone)]
pub(crate) struct VectorImage {
    pub rect: Rect,
    pub image: Arc<RenderImage>,
}

/// Published vectors and images share one immutable owner snapshot, with no native URL/path reads.
#[derive(Clone, Default)]
pub(crate) struct SceneImages {
    vectors: BTreeMap<String, Arc<Vec<Option<VectorImage>>>>,
    /// Structured paths share the worker's publication and keyed retirement with raster resources.
    pub(super) graphics: BTreeMap<String, Arc<super::canvas::scene::PreparedScene>>,
    pub photos: BTreeMap<String, Arc<Photo>>,
}

impl std::ops::Deref for SceneImages {
    type Target = BTreeMap<String, Arc<Vec<Option<VectorImage>>>>;
    fn deref(&self) -> &Self::Target {
        &self.vectors
    }
}

impl SceneImages {
    /// Retiring a worker discards both vector surfaces and asynchronous image results.
    pub fn clear(&mut self) {
        self.vectors.clear();
        self.graphics.clear();
        self.photos.clear();
    }
    /// Resource completion can change layout even when the guest document Arc stays unchanged.
    pub fn changed(&self, other: &Self) -> bool {
        self.graphics.len() != other.graphics.len()
            || self.graphics.iter().any(|(key, value)| {
                other
                    .graphics
                    .get(key)
                    .is_none_or(|old| !Arc::ptr_eq(old, value))
            })
            || self.photos.len() != other.photos.len()
            || self.photos.iter().any(|(key, value)| {
                other
                    .photos
                    .get(key)
                    .is_none_or(|old| !Arc::ptr_eq(value, old))
            })
    }
}

/// Pixel results retain the exact version/URI that authorized them, including localized failures.
pub(crate) struct Photo {
    pub resource: Arc<plugin_runtime::ImageResource>,
    pub decoded:
        Result<Option<super::bitmap::Bitmap>, plugin_runtime::plugin_protocol::api::Failure>,
}

/// Font discovery and parsing are cached while the worker owns the currently published scenes.
#[derive(Default)]
pub(crate) struct VectorRenderer {
    /// Equal structured scenes retain tessellation across unrelated native-tree revisions.
    graphics: BTreeMap<String, Arc<super::canvas::scene::PreparedScene>>,
    /// Derived canvas surfaces retain their identities while the owning native document stays unchanged.
    nested: BTreeMap<String, (Arc<Document>, Arc<Vec<Paint>>)>,
    drawings: BTreeMap<String, (Arc<Vec<Paint>>, Arc<Vec<Option<VectorImage>>>)>,
    trees: BTreeMap<[u8; 32], Arc<usvg::Tree>>,
    fonts: Option<Arc<usvg::fontdb::Database>>,
    /// A replaced document or removed node drops its native raster and encoded bytes together.
    photos: BTreeMap<String, Arc<Photo>>,
    /// A quota error retries only when more resident pixel capacity becomes available.
    attempt_budgets: BTreeMap<String, u64>,
    /// Tests observe decoder work rather than merely stable UI notification identities.
    #[cfg(test)]
    decode_attempts: usize,
}

impl VectorRenderer {
    /// Repaint only changed scene vectors, retaining native image IDs across idle worker polls.
    pub fn prepare(&mut self, documents: &BTreeMap<String, Arc<Document>>) -> SceneImages {
        let mut surfaces = BTreeMap::new();
        let mut nested_keys = BTreeSet::new();
        // Derived scene textures and paths are bounded across plugins, in addition to source decoding.
        let mut scene_pixels = 64 * 1024 * 1024_u64;
        let mut scene_vertices = 1_000_000_usize;
        for (key, document) in documents {
            let mut visit = |node: &plugin_runtime::plugin_protocol::ui::Node| {
                if let plugin_runtime::plugin_protocol::ui::Kind::Canvas(drawing) = &node.kind {
                    let image_prefix = format!("{key}/image/");
                    let key = format!("{key}/canvas/{}", node.id);
                    nested_keys.insert(key.clone());
                    if let Some(scene) = &drawing.scene {
                        let photos=scene.image_nodes().into_iter().filter_map(|id|{
                            self.photos.get(&format!("{image_prefix}{id}"))
                                .filter(|photo|document.source.as_ref().is_some_and(|source|photo.resource.source==*source)||document.file.as_ref().is_some_and(|file|photo.resource.source==plugin_runtime::plugin_protocol::api::ContentVersion::File(file.clone())))
                                .map(|photo|(id.to_owned(),photo.clone()))
                        }).collect::<BTreeMap<_,_>>();
                        let limits = (scene_pixels, scene_vertices);
                        if self
                            .graphics
                            .get(&key)
                            .is_none_or(|old| !old.current(scene, document, &photos, limits))
                        {
                            let prepared = super::canvas::scene::PreparedScene::new(
                                scene.clone(),
                                document,
                                photos,
                                limits,
                                |source, bounds, images| {
                                    self.rasterize_with_budget(
                                        source,
                                        bounds,
                                        bounds,
                                        limits.0,
                                        Some(images),
                                    )
                                },
                            );
                            self.graphics.insert(key.clone(), Arc::new(prepared));
                        }
                        if let Some(Ok(drawing)) =
                            self.graphics.get(&key).map(|prepared| &prepared.drawing)
                        {
                            scene_pixels = scene_pixels.saturating_sub(drawing.resident_bytes());
                            scene_vertices = scene_vertices.saturating_sub(drawing.vertex_count());
                        }
                    } else {
                        self.graphics.remove(&key);
                    }
                    if self
                        .nested
                        .get(&key)
                        .is_none_or(|(owner, _)| !Arc::ptr_eq(owner, document))
                    {
                        self.nested.insert(
                            key.clone(),
                            (document.clone(), Arc::new(drawing.paint.clone())),
                        );
                    }
                    surfaces.insert(key.clone(), self.nested[&key].1.clone());
                }
            };
            document.root.visit(&mut visit);
            if let Some(toolbar) = &document.editor_toolbar {
                toolbar.visit(&mut visit);
            }
            if let Some(dialog) = &document.dialog {
                dialog.content.visit(&mut visit);
            }
        }
        self.nested.retain(|key, _| nested_keys.contains(key));
        self.graphics.retain(|key, _| nested_keys.contains(key));
        let scenes = &surfaces;
        self.drawings.retain(|key, _| scenes.contains_key(key));
        let mut used = BTreeSet::new();
        for (key, scene) in scenes {
            if !scene
                .iter()
                .any(|operation| matches!(operation, Paint::Svg { .. }))
            {
                // Ordinary text/terminal scenes need no parallel array of empty image slots.
                self.drawings.remove(key);
                continue;
            }
            for operation in scene.iter() {
                if let Paint::Svg { source, .. } = operation {
                    used.insert(<[u8; 32]>::from(Sha256::digest(source.as_bytes())));
                }
            }
            if self
                .drawings
                .get(key)
                .is_some_and(|(old, _)| Arc::ptr_eq(old, scene))
            {
                continue;
            }
            let mut images = Vec::with_capacity(scene.len());
            for operation in scene.iter() {
                let image = if let Paint::Svg { source, rect, clip } = operation {
                    // Scrolling a stable tile changes only its placement. Reuse its pixels when
                    // size and source-relative clipping are identical, even if a sibling fill/text
                    // changed its operation index. Scale or clipping changes still rasterize anew.
                    let cached = self.drawings.get(key).and_then(|(old, images)| {
                        old.iter()
                            .zip(images.iter())
                            .find_map(|(operation, image)| {
                                let Paint::Svg {
                                    source: old_source,
                                    rect: old_rect,
                                    clip: old_clip,
                                } = operation
                                else {
                                    return None;
                                };
                                (source == old_source
                                    && rect.w == old_rect.w
                                    && rect.h == old_rect.h
                                    && clip.w == old_clip.w
                                    && clip.h == old_clip.h
                                    && clip.x - rect.x == old_clip.x - old_rect.x
                                    && clip.y - rect.y == old_clip.y - old_rect.y)
                                    .then(|| image.as_ref())
                                    .flatten()
                                    .and_then(|image| {
                                        Some(VectorImage {
                                            rect: intersection(*rect, *clip)?,
                                            image: image.image.clone(),
                                        })
                                    })
                            })
                    });
                    cached.or_else(|| self.rasterize(source, *rect, *clip))
                } else {
                    None
                };
                images.push(image);
            }
            self.drawings
                .insert(key.clone(), (scene.clone(), Arc::new(images)));
        }
        // Removed documents and plugin packages must not retain their decoded resources forever.
        self.trees.retain(|digest, _| used.contains(digest));
        let vectors = self
            .drawings
            .iter()
            .map(|(key, (_, images))| (key.clone(), images.clone()))
            .collect();
        SceneImages {
            vectors,
            graphics: self.graphics.clone(),
            photos: BTreeMap::new(),
        }
    }

    /// Decode only completed authorized resources, under a shared 64 MiB native raster budget.
    pub fn prepare_resources(
        &mut self,
        documents: &BTreeMap<String, Arc<Document>>,
        resources: &BTreeMap<String, Arc<plugin_runtime::ImageResource>>,
    ) -> SceneImages {
        self.photos.retain(|key, photo| {
            resources
                .get(key)
                .is_some_and(|resource| Arc::ptr_eq(&photo.resource, resource))
        });
        self.attempt_budgets
            .retain(|key, _| self.photos.contains_key(key));
        let mut allocated = self
            .photos
            .values()
            .filter_map(|photo| photo.decoded.as_ref().ok().and_then(Option::as_ref))
            .map(|bitmap| u64::from(bitmap.width) * u64::from(bitmap.height) * 4)
            .sum::<u64>();
        for (key, resource) in resources {
            let remaining = (64 * 1024 * 1024_u64).saturating_sub(allocated);
            // Retry derived pixel quotas after retirement, while static limits and IO errors stay cached.
            let retry = self.photos.get(key).is_none_or(|photo| matches!((&photo.decoded, &resource.state),
                (Err(error), plugin_runtime::ImageState::Ready(_)) if error.code == plugin_runtime::plugin_protocol::api::ErrorCode::LimitExceeded
                    && self.attempt_budgets.get(key).is_some_and(|budget| remaining > *budget)));
            if retry {
                let decoded = match &resource.state {
                    plugin_runtime::ImageState::Loading => Ok(None),
                    plugin_runtime::ImageState::Failed(error) => Err(error.clone()),
                    plugin_runtime::ImageState::Ready(bytes) => {
                        #[cfg(test)]
                        {
                            self.decode_attempts += 1;
                        }
                        self.attempt_budgets.insert(key.clone(), remaining);
                        super::bitmap::decode_with_budget(bytes, remaining).map(Some)
                    }
                };
                if let Ok(Some(bitmap)) = &decoded {
                    allocated += u64::from(bitmap.width) * u64::from(bitmap.height) * 4;
                }
                let unchanged = self.photos.get(key).filter(|old| Arc::ptr_eq(&old.resource, resource) && matches!((&old.decoded, &decoded), (Err(a), Err(b)) if a.code == b.code && a.message == b.message));
                let photo = unchanged.cloned().unwrap_or_else(|| {
                    Arc::new(Photo {
                        resource: resource.clone(),
                        decoded,
                    })
                });
                self.photos.insert(key.clone(), photo);
            }
        }
        let mut output = self.prepare(documents);
        output.photos = self.photos.clone();
        output
    }

    /// Preserve SVG alpha, block ambient file reads, and allocate only the visible clipped area.
    fn rasterize(&mut self, source: &str, rect: Rect, clip: Rect) -> Option<VectorImage> {
        self.rasterize_with_budget(source, rect, clip, 64 * 1024 * 1024, None)
    }

    /// Scene fallback checks its shared remaining output capacity before native allocation.
    fn rasterize_with_budget(
        &mut self,
        source: &str,
        rect: Rect,
        clip: Rect,
        remaining: u64,
        scene_images: Option<&BTreeMap<String, Arc<Vec<u8>>>>,
    ) -> Option<VectorImage> {
        let target = intersection(rect, clip)?;
        let digest = <[u8; 32]>::from(Sha256::digest(source.as_bytes()));
        let tree = if scene_images.is_none()
            && let Some(tree) = self.trees.get(&digest)
        {
            tree.clone()
        } else {
            let fonts = self
                .fonts
                .get_or_insert_with(|| super::svg::fonts())
                .clone();
            let options = usvg::Options {
                fontdb: fonts,
                image_href_resolver: if let Some(images) = scene_images {
                    usvg::ImageHrefResolver {
                        resolve_data: Box::new(|_, _, _| None),
                        resolve_string: Box::new(move |uri, _| {
                            images.get(uri).cloned().map(usvg::ImageKind::PNG)
                        }),
                    }
                } else {
                    usvg::ImageHrefResolver {
                        // Embedded data images remain supported; arbitrary filesystem/URL reads do not.
                        resolve_string: Box::new(|_, _| None),
                        ..Default::default()
                    }
                },
                ..Default::default()
            };
            // Canvas XML is generated only from validated numeric geometry and bounded PNG
            // pixels. Guest SVG retains its independent XML/reuse guards; do not put generated
            // trees into that cache, where a matching guest string could bypass those guards.
            let tree = Arc::new(if scene_images.is_some() {
                if source.len() > 4 * 1024 * 1024 {
                    return None;
                }
                usvg::Tree::from_str(source, &options).ok()?
            } else {
                super::svg::parse(source, &options).ok()?
            });
            if scene_images.is_none() {
                self.trees.insert(digest, tree.clone());
            }
            tree
        };
        // Two pixels per logical pixel keep normal high-DPI previews sharp; cap one raster at 16 MiB.
        let density = 2_f32.min(2048. / target.w).min(2048. / target.h);
        let width = (target.w * density).ceil().clamp(1., 2048.) as u32;
        let height = (target.h * density).ceil().clamp(1., 2048.) as u32;
        if u64::from(width) * u64::from(height) * 4 > remaining {
            return None;
        }
        let transform = tiny_skia::Transform::from_row(
            rect.w / tree.size().width() * density,
            0.,
            0.,
            rect.h / tree.size().height() * density,
            (rect.x - target.x) * density,
            (rect.y - target.y) * density,
        );
        super::svg::render_budget(&tree, transform, width, height, 64 * 1024 * 1024).ok()?;
        let mut pixmap = tiny_skia::Pixmap::new(width, height)?;
        resvg::render(&tree, transform, &mut pixmap.as_mut());
        let mut pixels = pixmap.take();
        for pixel in pixels.chunks_exact_mut(4) {
            // GPUI expects straight BGRA; tiny-skia emits premultiplied RGBA.
            let alpha = u32::from(pixel[3]);
            if alpha > 0 {
                for channel in &mut pixel[..3] {
                    *channel = ((u32::from(*channel) * 255 + alpha / 2) / alpha).min(255) as u8;
                }
            }
            pixel.swap(0, 2);
        }
        let buffer = image::RgbaImage::from_raw(width, height, pixels)?;
        Some(VectorImage {
            rect: target,
            image: Arc::new(RenderImage::new([image::Frame::new(buffer)])),
        })
    }
}

/// Empty intersections need neither a raster allocation nor a native image draw.
fn intersection(rect: Rect, clip: Rect) -> Option<Rect> {
    let x = rect.x.max(clip.x);
    let y = rect.y.max(clip.y);
    let w = (rect.x + rect.w).min(clip.x + clip.w) - x;
    let h = (rect.y + rect.h).min(clip.y + clip.h) - y;
    (w > 0. && h > 0.).then_some(Rect { x, y, w, h })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Quota failures retry after retirement; unchanged errors retain their Arc instead of redrawing forever.
    #[test]
    fn resident_native_images_are_bounded_and_recover_after_node_retirement() {
        use plugin_runtime::{ImageResource, ImageState, plugin_protocol::api};
        let image = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            2048,
            4096,
            image::Rgba([0, 0, 0, 255]),
        ));
        let mut encoded = std::io::Cursor::new(Vec::new());
        image
            .write_to(&mut encoded, image::ImageFormat::Png)
            .unwrap();
        let bytes = Arc::new(encoded.into_inner());
        let source = api::DocumentVersion {
            id: "document".into(),
            path: "notes.md".into(),
            revision: 1,
        };
        let mut resources = (0..3)
            .map(|index| {
                (
                    format!("test/preview/image/{index}"),
                    Arc::new(ImageResource {
                        source: source.clone().into(),
                        uri: format!("{index}.png"),
                        state: ImageState::Ready(bytes.clone()),
                    }),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut renderer = VectorRenderer::default();
        let first = renderer.prepare_resources(&BTreeMap::new(), &resources);
        assert_eq!(
            first
                .photos
                .values()
                .filter(|photo| matches!(photo.decoded, Ok(Some(_))))
                .count(),
            2
        );
        assert!(
            matches!(&first.photos["test/preview/image/2"].decoded, Err(error) if error.code == api::ErrorCode::LimitExceeded)
        );
        let second = renderer.prepare_resources(&BTreeMap::new(), &resources);
        assert!(!first.changed(&second));
        assert_eq!(
            renderer.decode_attempts, 3,
            "an unchanged quota failure must not decode again on every worker poll"
        );
        resources.remove("test/preview/image/0");
        let recovered = renderer.prepare_resources(&BTreeMap::new(), &resources);
        assert!(matches!(
            recovered.photos["test/preview/image/2"].decoded,
            Ok(Some(_))
        ));
        let removed = renderer.prepare_resources(&BTreeMap::new(), &BTreeMap::new());
        assert!(removed.photos.is_empty());
    }
}
