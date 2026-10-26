//! Native path construction, bitmap guard texels and rectangular clip projection.
use super::*;

/// Native two-stop backgrounds retain explicit alpha; more complex gradients use raster preparation.
pub(super) fn brush(value: &SceneBrush) -> Result<Background, String> {
    match value {
        SceneBrush::Solid { rgba: color } => Ok(rgba(*color).into()),
        SceneBrush::LinearGradient { angle, stops }
            if stops.len() == 2 && stops[0].offset < stops[1].offset =>
        {
            Ok(gpui_kit::linear_gradient(
                angle.rem_euclid(360.),
                gpui_kit::linear_color_stop(rgba(stops[0].rgba), stops[0].offset),
                gpui_kit::linear_color_stop(rgba(stops[1].rgba), stops[1].offset),
            ))
        }
        _ => Err("Canvas gradient requires raster preparation".into()),
    }
}

/// Tessellate in local coordinates first, then transform vertices so nonuniform strokes remain correct.
pub(super) fn build_path(
    geometry: &SceneGeometry,
    stroke: Option<&SceneStroke>,
    transform: SceneTransform,
) -> Result<Path<Pixels>, String> {
    // Keep uninterruptible native tessellation small; larger paths use the bounded raster worker.
    if geometry.segment_count() > 32 {
        return Err("Complex path requires raster preparation".into());
    }
    let mut builder = if let Some(stroke) = stroke {
        if stroke.cap != SceneLineCap::Butt
            || stroke.join != SceneLineJoin::Miter
            || stroke.miter_limit != 4.
        {
            return Err("Canvas stroke requires raster preparation".into());
        }
        let mut builder = PathBuilder::stroke(px(stroke.width));
        if !stroke.dash.is_empty() {
            builder = builder.dash_array(&stroke.dash.iter().copied().map(px).collect::<Vec<_>>());
        }
        builder
    } else {
        let rule = match geometry {
            SceneGeometry::Path {
                rule: SceneFillRule::EvenOdd,
                ..
            } => FillRule::EvenOdd,
            _ => FillRule::NonZero,
        };
        PathBuilder::fill().with_style(PathStyle::Fill(FillOptions::default().with_fill_rule(rule)))
    };
    match geometry {
        SceneGeometry::Rect { rect, radii } => {
            let [a, b, c, d] = corner_radii(*rect, *radii);
            let (x, y, r, bottom) = (rect.x, rect.y, rect.x + rect.w, rect.y + rect.h);
            builder.move_to(point(px(x + a), px(y)));
            builder.line_to(point(px(r - b), px(y)));
            if b > 0. {
                builder.arc_to(
                    point(px(b), px(b)),
                    px(0.),
                    false,
                    true,
                    point(px(r), px(y + b)),
                );
            }
            builder.line_to(point(px(r), px(bottom - c)));
            if c > 0. {
                builder.arc_to(
                    point(px(c), px(c)),
                    px(0.),
                    false,
                    true,
                    point(px(r - c), px(bottom)),
                );
            }
            builder.line_to(point(px(x + d), px(bottom)));
            if d > 0. {
                builder.arc_to(
                    point(px(d), px(d)),
                    px(0.),
                    false,
                    true,
                    point(px(x), px(bottom - d)),
                );
            }
            builder.line_to(point(px(x), px(y + a)));
            if a > 0. {
                builder.arc_to(
                    point(px(a), px(a)),
                    px(0.),
                    false,
                    true,
                    point(px(x + a), px(y)),
                );
            }
            builder.close();
        }
        SceneGeometry::Path { segments, .. } => {
            for segment in segments {
                match *segment {
                    ScenePathSegment::Move { x, y } => builder.move_to(point(px(x), px(y))),
                    ScenePathSegment::Line { x, y } => builder.line_to(point(px(x), px(y))),
                    ScenePathSegment::Quadratic { cx, cy, x, y } => {
                        builder.curve_to(point(px(x), px(y)), point(px(cx), px(cy)))
                    }
                    ScenePathSegment::Cubic {
                        x1,
                        y1,
                        x2,
                        y2,
                        x,
                        y,
                    } => builder.cubic_bezier_to(
                        point(px(x), px(y)),
                        point(px(x1), px(y1)),
                        point(px(x2), px(y2)),
                    ),
                    ScenePathSegment::Close => builder.close(),
                }
            }
        }
    }
    let mut path = builder
        .build()
        .map_err(|error| format!("Canvas path tessellation failed: {error}"))?;
    for vertex in &mut path.vertices {
        let (x, y) = transform.point(vertex.xy_position.x / px(1.), vertex.xy_position.y / px(1.));
        vertex.xy_position = point(px(x), px(y));
    }
    if let Some(first) = path.vertices.first() {
        let mut min = first.xy_position;
        let mut max = min;
        for vertex in &path.vertices {
            min.x = min.x.min(vertex.xy_position.x);
            min.y = min.y.min(vertex.xy_position.y);
            max.x = max.x.max(vertex.xy_position.x);
            max.y = max.y.max(vertex.xy_position.y);
        }
        path.bounds = Bounds::new(min, size(max.x - min.x, max.y - min.y));
    }
    Ok(path)
}

/// Duplicate boundary texels under the derived-image quota so atlas filtering cannot bleed in.
pub(super) fn guarded_image(
    bitmap: &super::super::super::bitmap::Bitmap,
    remaining: u64,
) -> Result<std::sync::Arc<gpui_kit::RenderImage>, String> {
    let width = bitmap.width + 2;
    let height = bitmap.height + 2;
    if u64::from(width) * u64::from(height) * 4 > remaining {
        return Err("Canvas image exceeds its derived pixel budget".into());
    }
    let bytes = bitmap
        .image
        .as_bytes(0)
        .ok_or("Canvas image frame is unavailable")?;
    let mut output = image::RgbaImage::new(width, height);
    for (x, y, pixel) in output.enumerate_pixels_mut() {
        let (sx, sy) = (
            x.saturating_sub(1).min(bitmap.width - 1),
            y.saturating_sub(1).min(bitmap.height - 1),
        );
        let offset = ((sy * bitmap.width + sx) * 4) as usize;
        *pixel = image::Rgba(
            bytes[offset..offset + 4]
                .try_into()
                .map_err(|_| "Invalid canvas image frame")?,
        );
    }
    Ok(std::sync::Arc::new(gpui_kit::RenderImage::new([
        image::Frame::new(output),
    ])))
}

/// Axis-aligned rectangular groups use GPUI's native intersecting mask stack, including reflection.
pub(super) fn rectangular_clip(
    geometry: &SceneGeometry,
    transform: SceneTransform,
    parent: Option<protocol::Rect>,
) -> Result<protocol::Rect, String> {
    let SceneGeometry::Rect { rect, radii } = geometry else {
        return Err("Path clipping requires raster preparation".into());
    };
    let [_, b, c, _, _, _] = transform.0;
    if b != 0. || c != 0. || radii.iter().any(|radius| *radius != 0.) {
        return Err("Transformed clipping requires raster preparation".into());
    }
    let (x0, y0) = transform.point(rect.x, rect.y);
    let (x1, y1) = transform.point(rect.x + rect.w, rect.y + rect.h);
    let mut clip = protocol::Rect {
        x: x0.min(x1),
        y: y0.min(y1),
        w: (x1 - x0).abs(),
        h: (y1 - y0).abs(),
    };
    if let Some(parent) = parent {
        let right = (clip.x + clip.w).min(parent.x + parent.w);
        let bottom = (clip.y + clip.h).min(parent.y + parent.h);
        clip.x = clip.x.max(parent.x);
        clip.y = clip.y.max(parent.y);
        clip.w = (right - clip.x).max(0.);
        clip.h = (bottom - clip.y).max(0.);
    }
    Ok(clip)
}
