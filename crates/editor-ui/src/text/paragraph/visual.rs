//! Normalize native glyph placement without replacing fonts, glyph shaping or GPUI painting.
use super::*;
use gpui_kit::{FontId, GlyphId, ShapedGlyph};
use plugin_runtime::plugin_protocol::ui::{
    ParagraphAlign, ParagraphOverflow, ParagraphStyle, ParagraphVerticalAlign,
};
use std::{collections::BTreeMap, ops::Range};
use unicode_bidi::{BidiInfo, Level};
mod positions;
mod raster;

/// One Windows native paragraph's visual rows, original-byte edges and shaped glyphs.
pub(super) struct VisualParagraph {
    /// Stable row-local candidate edges for native point queries.
    rows: Vec<Row>,
    /// Native glyph IDs retain their original face and offsets; only visual placement changes.
    glyphs: Vec<Glyph>,
    /// A source boundary may have distinct preceding and following visual positions.
    edges: BTreeMap<usize, Edges>,
    /// Visual cluster rectangles also provide disjoint selection fragments.
    clusters: Vec<Cluster>,
    /// Native clip and line height shared by paint and geometry.
    bounds: Bounds<Pixels>,
    line_height: Pixels,
}
/// A visible row's source-boundary positions.
struct Row {
    top: Pixels,
    carets: Vec<(usize, Pixels)>,
    /// Original visible source and its native advance box, excluding synthetic text.
    source: Range<usize>,
    rect: Bounds<Pixels>,
    ellipsis: bool,
}
/// Native glyph paint input, with its actual baseline and a conservative native-font culling box.
struct Glyph {
    font: FontId,
    id: GlyphId,
    size: Pixels,
    origin: gpui_kit::Point<Pixels>,
    top: Pixels,
    emoji: bool,
    /// Resolved source style, retained separately from the font's glyph identity.
    color: Hsla,
}
/// One shaped cluster's source range, visual rectangle and direction-aware endpoints.
struct Cluster {
    source: Range<usize>,
    rect: Bounds<Pixels>,
    start: gpui_kit::Point<Pixels>,
    end: gpui_kit::Point<Pixels>,
}
/// Logical affinity is independent of physical left/right direction.
#[derive(Default)]
struct Edges {
    up: Option<gpui_kit::Point<Pixels>>,
    down: Option<gpui_kit::Point<Pixels>>,
}
/// Temporary logical cluster before Unicode visual ordering.
struct Work {
    source: Range<usize>,
    x: Pixels,
    width: Pixels,
    glyphs: Vec<(FontId, ShapedGlyph)>,
}

impl VisualParagraph {
    /// Preserve native fonts, glyphs and wrapping; normalize visual order and bounded formatting.
    /// Windows' locked backend drops DirectWrite bidi origins, so its clusters use UAX #9 ordering.
    /// Other native backends retain their physical cluster order.
    pub(super) fn new(
        lines: &[WrappedLine],
        bounds: Bounds<Pixels>,
        height: Pixels,
        boundaries: &[usize],
        style: &ParagraphStyle,
        ellipsis: Option<&WrappedLine>,
        color_at: impl Fn(usize) -> Hsla,
    ) -> Self {
        let total = lines
            .iter()
            .map(|line| line.wrap_boundaries.len() + 1)
            .sum::<usize>();
        let capacity = if style.overflow == ParagraphOverflow::Ellipsis {
            (bounds.size.height / height).floor()
        } else {
            (bounds.size.height / height).ceil()
        };
        let capacity = if bounds.size.height > px(0.) {
            (capacity as usize).max(1)
        } else {
            0
        };
        let visible = total.min(capacity);
        let spare = (bounds.size.height - height * total).max(px(0.));
        let shift = match style.vertical_align {
            ParagraphVerticalAlign::Top => px(0.),
            ParagraphVerticalAlign::Center => spare / 2.,
            ParagraphVerticalAlign::Bottom => spare,
        };
        let mut result = Self {
            rows: Vec::new(),
            glyphs: Vec::new(),
            edges: BTreeMap::new(),
            clusters: Vec::new(),
            bounds,
            line_height: height,
        };
        let mut source = 0;
        let mut top = bounds.origin.y + shift;
        let canonical = |byte: usize| boundaries.binary_search(&byte).is_ok();
        let floor = |byte: usize| {
            boundaries[boundaries
                .partition_point(|boundary| *boundary <= byte)
                .saturating_sub(1)]
        };
        let mut row_index = 0;
        'paragraph: for line in lines {
            let indexed = positions::IndexedLine::new(line);
            let base_rtl = indexed.base_rtl();
            let mut start = 0;
            let ends = line
                .wrap_boundaries
                .iter()
                .map(|boundary| line.runs()[boundary.run_ix].glyphs[boundary.glyph_ix].index)
                .chain(std::iter::once(line.text.len()));
            for end in ends {
                if row_index >= visible {
                    break 'paragraph;
                }
                let row_glyphs = indexed.glyphs(start..end);
                let mut starts = row_glyphs
                    .iter()
                    .map(|(_, glyph)| {
                        floor(source + glyph.index)
                            .saturating_sub(source)
                            .max(start)
                    })
                    .collect::<Vec<_>>();
                starts.extend([start, end]);
                starts.sort_unstable();
                starts.dedup();
                let mut work = starts
                    .windows(2)
                    .map(|pair| {
                        let a = indexed.x(pair[0]);
                        let b = indexed.x(pair[1]);
                        Work {
                            source: pair[0]..pair[1],
                            x: a.min(b),
                            width: (b - a).abs(),
                            glyphs: Vec::new(),
                        }
                    })
                    .collect::<Vec<_>>();
                for (font_id, glyph) in row_glyphs {
                    if !work.is_empty() {
                        let index = starts
                            .partition_point(|boundary| *boundary <= glyph.index)
                            .saturating_sub(1)
                            .min(work.len() - 1);
                        work[index].glyphs.push((*font_id, (*glyph).clone()));
                    }
                }
                let mut width = work
                    .iter()
                    .map(|cluster| cluster.width)
                    .fold(px(0.), |a, b| a + b);
                let truncated = style.overflow == ParagraphOverflow::Ellipsis
                    && ((row_index + 1 == visible && total > visible) || width > bounds.size.width);
                let ellipsis_width = if truncated {
                    ellipsis.map_or(px(0.), |line| line.width())
                } else {
                    px(0.)
                };
                if truncated {
                    while width + ellipsis_width > bounds.size.width && !work.is_empty() {
                        width -= work.pop().unwrap().width;
                    }
                }
                let visible_end = work.last().map_or(start, |cluster| cluster.source.end);
                let row_width = width + ellipsis_width;
                let padding = (bounds.size.width - row_width).max(px(0.));
                let left = bounds.origin.x
                    + match style.align {
                        ParagraphAlign::Left => px(0.),
                        ParagraphAlign::Center => padding / 2.,
                        ParagraphAlign::Right => padding,
                    };
                let mut row = Row {
                    top,
                    carets: Vec::new(),
                    source: floor(source + start)..floor(source + visible_end),
                    rect: Bounds::new(
                        gpui_kit::point(left, top),
                        gpui_kit::size(row_width.max(px(1.)), height),
                    ),
                    ellipsis: truncated,
                };
                let levels = indexed.levels(start..end);
                let cluster_levels = work
                    .iter()
                    .map(|cluster| {
                        levels
                            .get(cluster.source.start - start)
                            .copied()
                            .unwrap_or_else(Level::ltr)
                    })
                    .collect::<Vec<_>>();
                let order = if cfg!(target_os = "windows") {
                    BidiInfo::reorder_visual(&cluster_levels)
                } else {
                    let mut order = (0..work.len()).collect::<Vec<_>>();
                    order.sort_by(|a, b| {
                        work[*a]
                            .x
                            .partial_cmp(&work[*b].x)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                    order
                };
                let mut x = left
                    + if truncated && base_rtl {
                        ellipsis_width
                    } else {
                        px(0.)
                    };
                let baseline = top + (height - line.ascent() - line.descent()) / 2. + line.ascent();
                for index in order {
                    let cluster = &work[index];
                    let rtl = cluster_levels[index].is_rtl();
                    let a = gpui_kit::point(if rtl { x + cluster.width } else { x }, top);
                    let b = gpui_kit::point(if rtl { x } else { x + cluster.width }, top);
                    let original = source + cluster.source.start..source + cluster.source.end;
                    if canonical(original.start) {
                        result.edges.entry(original.start).or_default().down = Some(a);
                        row.carets.push((original.start, a.x));
                    }
                    if canonical(original.end) {
                        result.edges.entry(original.end).or_default().up = Some(b);
                        row.carets.push((original.end, b.x));
                    }
                    for (font, glyph) in &cluster.glyphs {
                        result.glyphs.push(Glyph {
                            font: *font,
                            id: glyph.id,
                            size: line.font_size(),
                            origin: gpui_kit::point(
                                x + glyph.position.x - cluster.x,
                                baseline + glyph.position.y,
                            ),
                            top,
                            emoji: glyph.is_emoji,
                            color: color_at(source + glyph.index),
                        });
                    }
                    result.clusters.push(Cluster {
                        source: original,
                        rect: Bounds::new(
                            gpui_kit::point(x, top),
                            gpui_kit::size(cluster.width, height),
                        ),
                        start: a,
                        end: b,
                    });
                    x += cluster.width;
                }
                if row.carets.is_empty() && canonical(row.source.start) {
                    let position = gpui_kit::point(left, top);
                    let edge = result.edges.entry(row.source.start).or_default();
                    edge.up = Some(position);
                    edge.down = Some(position);
                    row.carets.push((row.source.start, left));
                }
                if truncated {
                    if let Some(ellipsis) = ellipsis {
                        let x = if base_rtl { left } else { left + width };
                        let baseline = top
                            + (height - ellipsis.ascent() - ellipsis.descent()) / 2.
                            + ellipsis.ascent();
                        for run in ellipsis.runs() {
                            for glyph in &run.glyphs {
                                result.glyphs.push(Glyph {
                                    font: run.font_id,
                                    id: glyph.id,
                                    size: ellipsis.font_size(),
                                    origin: gpui_kit::point(
                                        x + glyph.position.x,
                                        baseline + glyph.position.y,
                                    ),
                                    top,
                                    emoji: glyph.is_emoji,
                                    color: color_at(row.source.end.saturating_sub(1)),
                                });
                            }
                        }
                    }
                }
                // A selected explicit CRLF/LF receives a minimal cue without inventing a caret inside it.
                if !truncated
                    && end == line.text.len()
                    && source + end < *boundaries.last().unwrap()
                {
                    let a = gpui_kit::point(if base_rtl { left } else { left + width }, top);
                    result.clusters.push(Cluster {
                        source: floor(source + end)..source + end + 1,
                        rect: Bounds::new(a, gpui_kit::size(px(1.), height)),
                        start: a,
                        end: a,
                    });
                }
                result.rows.push(row);
                row_index += 1;
                top += height;
                start = end;
            }
            source += line.text.len() + 1;
        }
        result
    }
    /// Original ranges and the same native advance boxes used for alignment and clipping.
    pub(super) fn lines(&self) -> impl Iterator<Item = (Range<usize>, Bounds<Pixels>, bool)> + '_ {
        self.rows
            .iter()
            .map(|row| (row.source.clone(), row.rect, row.ellipsis))
    }
    /// Resolve logical affinity; a glyph-cluster interior uses that cluster's actual native edge.
    pub(super) fn position(
        &self,
        byte: usize,
        downstream: bool,
    ) -> Option<gpui_kit::Point<Pixels>> {
        if let Some(edge) = self.edges.get(&byte) {
            return if downstream {
                edge.down.or(edge.up)
            } else {
                edge.up.or(edge.down)
            };
        }
        self.clusters
            .iter()
            .find(|cluster| cluster.source.start < byte && byte < cluster.source.end)
            .map(|cluster| {
                if downstream {
                    cluster.end
                } else {
                    cluster.start
                }
            })
    }
    /// Select the physical row first, then its nearest original-byte native edge.
    pub(super) fn hit(&self, point: gpui_kit::Point<Pixels>) -> Option<usize> {
        let row = self
            .rows
            .iter()
            .find(|row| point.y < row.top + self.line_height)
            .or_else(|| self.rows.last())?;
        row.carets
            .iter()
            .min_by(|a, b| {
                (a.1 - point.x)
                    .abs()
                    .partial_cmp(&(b.1 - point.x).abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(a.0.cmp(&b.0))
            })
            .map(|entry| entry.0)
    }
    /// Use the same native glyph and emoji APIs as GPUI's standard text painter.
    pub(super) fn paint(&self, window: &mut Window) -> Result<(), String> {
        for glyph in &self.glyphs {
            let extent = window
                .text_system()
                .bounding_box(glyph.font, glyph.size)
                .size;
            if !Bounds::new(gpui_kit::point(glyph.origin.x, glyph.top), extent)
                .intersects(&window.content_mask().bounds)
            {
                continue;
            }
            let result = if glyph.emoji {
                window.paint_emoji(glyph.origin, glyph.font, glyph.id, glyph.size)
            } else {
                window.paint_glyph(glyph.origin, glyph.font, glyph.id, glyph.size, glyph.color)
            };
            result.map_err(|error| format!("Native bidi glyph paint failed: {error}"))?;
        }
        Ok(())
    }
    /// Preserve disjoint visual runs instead of spanning unselected glyphs in mixed-direction text.
    pub(super) fn selection(&self, range: Range<usize>) -> Result<Vec<Bounds<Pixels>>, String> {
        let mut output: Vec<Bounds<Pixels>> = Vec::new();
        for cluster in &self.clusters {
            if cluster.source.start >= range.end || range.start >= cluster.source.end {
                continue;
            }
            let mut rect = cluster.rect;
            rect.size.width = rect.size.width.max(px(1.));
            let origin = gpui_kit::point(
                rect.origin.x.max(self.bounds.origin.x),
                rect.origin.y.max(self.bounds.origin.y),
            );
            let right = (rect.origin.x + rect.size.width)
                .min(self.bounds.origin.x + self.bounds.size.width);
            let bottom = (rect.origin.y + rect.size.height)
                .min(self.bounds.origin.y + self.bounds.size.height);
            if right <= origin.x || bottom <= origin.y {
                continue;
            }
            rect = Bounds::new(origin, gpui_kit::size(right - origin.x, bottom - origin.y));
            if let Some(previous) = output.last_mut() {
                if previous.origin.y == rect.origin.y
                    && rect.origin.x <= previous.origin.x + previous.size.width + px(0.01)
                {
                    previous.size.width = (previous.origin.x + previous.size.width)
                        .max(rect.origin.x + rect.size.width)
                        - previous.origin.x;
                    continue;
                }
            }
            if output.len() >= 256 {
                return Err("Native bidi selection exceeds its fragment budget".into());
            }
            output.push(rect);
        }
        Ok(output)
    }
}
