//! Bounded point queries retain original UTF-8 positions and the actual native font environment.
use super::*;
use std::collections::BTreeSet;
use unicode_segmentation::UnicodeSegmentation as _;

/// Return whether `byte` is an extended grapheme boundary in original UTF-8 `text`.
/// End-of-text is valid; out-of-range, split scalar, combining and joined-emoji positions are false.
pub fn is_grapheme_boundary(text: &str, byte: usize) -> bool {
    byte <= text.len()
        && text.is_char_boundary(byte)
        && (byte == text.len() || text.grapheme_indices(true).any(|(index, _)| index == byte))
}

/// Iterate original UTF-8 extended grapheme boundaries in `text`, including end-of-text.
/// The iterator borrows the string and exposes no host state or native layout objects.
pub fn grapheme_boundaries(text: &str) -> impl Iterator<Item = usize> + '_ {
    text.grapheme_indices(true)
        .map(|(byte, _)| byte)
        .chain(std::iter::once(text.len()))
}

/// One point in Canvas content coordinates, before the visual viewport and OS DPI.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasTextPoint {
    /// Horizontal logical coordinate.
    pub x: f32,
    /// Vertical logical coordinate.
    pub y: f32,
}
/// A shared byte at a soft wrap or bidi edge can have two native visual caret positions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasTextAffinity {
    /// Prefer the preceding visual line/run.
    Upstream,
    /// Prefer the following visual line/run.
    #[default]
    Downstream,
}
/// One original UTF-8 caret position, required to be a whole grapheme boundary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasTextPosition {
    /// Original byte index.
    pub byte: usize,
    /// Visual side at a shared boundary.
    #[serde(default)]
    pub affinity: CanvasTextAffinity,
}
/// Half-open original byte selection; endpoints are extended grapheme boundaries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasTextRange {
    /// Inclusive source boundary.
    pub start: usize,
    /// Exclusive source boundary, at least start.
    pub end: usize,
}
/// Native caret rectangle in Canvas content coordinates; a clipped caret has no rectangle.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasCaretRect {
    /// Addressed source byte.
    pub byte: usize,
    /// Addressed visual affinity.
    pub affinity: CanvasTextAffinity,
    /// Visible clipped logical rectangle, absent outside the paragraph/Canvas viewport.
    pub rect: Option<crate::Rect>,
}
/// Disjoint visible fragments for one source selection, in Canvas content coordinates.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasSelectionRects {
    /// Exact requested original source range.
    pub range: CanvasTextRange,
    /// Native line fragments; at most 256 per selection.
    pub rects: Vec<crate::Rect>,
}
/// Ask for nearest legal original byte positions after the named paragraph reaches native paint.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasTextQuery {
    /// Unique nonempty query ID in this Canvas, at most 128 UTF-8 bytes.
    pub id: String,
    /// Stable drawing ID of the paragraph in this scene.
    pub paragraph: String,
    /// The paragraph's exact layout identity; stale requests are rejected at publication.
    pub layout_key: String,
    /// Canvas content points; points, positions and selections share a 256-item Canvas budget.
    #[serde(default)]
    pub points: Vec<CanvasTextPoint>,
    /// Requested original byte carets, at most 64 per query.
    #[serde(default)]
    pub positions: Vec<CanvasTextPosition>,
    /// Requested original byte selections, at most 64 per query.
    #[serde(default)]
    pub selections: Vec<CanvasTextRange>,
}
/// One addressed native layout result; no guessed character widths cross the public boundary.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasTextResult {
    /// Visible native rows in source order, at most 1024; ranges always address original text.
    #[serde(default)]
    pub lines: Vec<CanvasTextLine>,
    /// Ordered requested caret geometry, including source and affinity echoes.
    #[serde(default)]
    pub positions: Vec<CanvasCaretRect>,
    /// Ordered requested selection fragments.
    #[serde(default)]
    pub selections: Vec<CanvasSelectionRects>,
    /// Exact requested points prevent a queued report being reused for a changed declaration.
    pub points: Vec<CanvasTextPoint>,
    /// Original request ID, paragraph ID and layout identity.
    pub id: String,
    /// Drawing ID actually shaped.
    pub paragraph: String,
    /// Exact layout identity actually shaped.
    pub layout_key: String,
    /// Nonzero opaque binding to native font IDs, descriptor, size and DPI; zero only on error.
    pub font_environment: u64,
    /// Ordered original UTF-8 byte positions, one per requested point on success.
    pub hits: Vec<usize>,
    /// A bounded native-layout failure; failed results contain no hit positions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
/// One visible native soft row, including an explicit indication of visual truncation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasTextLine {
    /// Visible original grapheme range, excluding an explicit line separator and synthetic ellipsis.
    pub range: CanvasTextRange,
    /// Clipped native row rectangle in Canvas content logical coordinates.
    pub rect: crate::Rect,
    /// Native ellipsis replaces content after this row's source end.
    pub ellipsis: bool,
}
impl CanvasScene {
    /// Borrow the original text only when both drawing and layout identities still match.
    pub fn paragraph_text(&self, id: &str, key: &str) -> Option<&str> {
        fn find<'a>(node: &'a SceneNode, id: &str, key: &str) -> Option<&'a str> {
            match node {
                SceneNode::Paragraph {
                    id: found,
                    layout_key,
                    text,
                    ..
                } if found == id && layout_key == key => Some(text),
                SceneNode::Group { children, .. } => {
                    children.iter().find_map(|node| find(node, id, key))
                }
                _ => None,
            }
        }
        self.roots.iter().find_map(|node| find(node, id, key))
    }
}
impl Canvas {
    /// Admit only current paragraph references and finite bounded work before native shaping.
    pub(super) fn validate_text_queries(&self) -> Result<(), String> {
        let mut ids = BTreeSet::new();
        let mut points = 0;
        if self.text_queries.len() > 16 {
            return Err("Too many native text queries".into());
        }
        for query in &self.text_queries {
            points += query.points.len() + query.positions.len() + query.selections.len();
            if !scene::valid_id(&query.id)
                || !ids.insert(&query.id)
                || (query.points.is_empty()
                    && query.positions.is_empty()
                    && query.selections.is_empty())
                || query.points.len() > 64
                || query.positions.len() > 64
                || query.selections.len() > 64
                || points > 256
                || query.points.iter().any(|point| {
                    ![point.x, point.y]
                        .into_iter()
                        .all(|value| value.is_finite() && value.abs() <= 1_000_000.)
                })
                || self
                    .scene
                    .as_ref()
                    .and_then(|scene| scene.paragraph_text(&query.paragraph, &query.layout_key))
                    .is_none()
            {
                return Err("Invalid native text query or stale paragraph identity".into());
            }
            let text = self
                .scene
                .as_ref()
                .unwrap()
                .paragraph_text(&query.paragraph, &query.layout_key)
                .unwrap();
            if query
                .positions
                .iter()
                .any(|position| !is_grapheme_boundary(text, position.byte))
                || query.selections.iter().any(|range| {
                    range.start > range.end
                        || !is_grapheme_boundary(text, range.start)
                        || !is_grapheme_boundary(text, range.end)
                })
            {
                return Err("Native text geometry requires original grapheme boundaries".into());
            }
        }
        Ok(())
    }
    /// Verify report addressing, count and UTF-8 ranges before forwarding a native observation.
    pub(in crate::ui) fn accepts_text_layout(
        &self,
        identity: u64,
        revision: u64,
        results: &[CanvasTextResult],
    ) -> bool {
        if results.len() > 16
            || results
                .iter()
                .flat_map(|result| &result.selections)
                .map(|selection| selection.rects.len())
                .sum::<usize>()
                + results
                    .iter()
                    .map(|result| result.lines.len())
                    .sum::<usize>()
                > 8192
        {
            return false;
        }
        self.scene.as_ref().is_some_and(|scene| {
            scene.identity == identity
                && scene.revision == revision
                && !self.text_queries.is_empty()
                && results.len() == self.text_queries.len()
                && results
                    .iter()
                    .zip(&self.text_queries)
                    .all(|(result, query)| {
                        if result.points != query.points
                            || result.id != query.id
                            || result.paragraph != query.paragraph
                            || result.layout_key != query.layout_key
                        {
                            return false;
                        }
                        if let Some(error) = &result.error {
                            return !error.is_empty()
                                && error.len() <= 512
                                && result.hits.is_empty()
                                && result.positions.is_empty()
                                && result.selections.is_empty()
                                && result.lines.is_empty();
                        }
                        result.font_environment != 0
                            && result.positions.len() == query.positions.len()
                            && result.positions.iter().zip(&query.positions).all(
                                |(result, request)| {
                                    result.byte == request.byte
                                        && result.affinity == request.affinity
                                        && result.rect.is_none_or(valid_rect)
                                },
                            )
                            && result.selections.len() == query.selections.len()
                            && result.selections.iter().zip(&query.selections).all(
                                |(result, request)| {
                                    result.range == *request
                                        && result.rects.len() <= 256
                                        && result.rects.iter().all(|rect| valid_rect(*rect))
                                },
                            )
                            && result.hits.len() == query.points.len()
                            && scene
                                .paragraph_text(&query.paragraph, &query.layout_key)
                                .is_some_and(|text| {
                                    result.lines.len() <= 1024
                                        && result.lines.iter().all(|line| {
                                            line.range.start <= line.range.end
                                                && is_grapheme_boundary(text, line.range.start)
                                                && is_grapheme_boundary(text, line.range.end)
                                                && valid_rect(line.rect)
                                        })
                                        && result
                                            .lines
                                            .windows(2)
                                            .all(|pair| pair[0].range.end <= pair[1].range.start)
                                        && result
                                            .hits
                                            .iter()
                                            .all(|byte| is_grapheme_boundary(text, *byte))
                                })
                    })
        })
    }
}

/// Geometry observations remain finite and within the public Canvas coordinate extent.
fn valid_rect(rect: crate::Rect) -> bool {
    [
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        rect.x + rect.w,
        rect.y + rect.h,
    ]
    .into_iter()
    .all(|value| value.is_finite() && value.abs() <= 1_000_000.)
        && rect.w >= 0.
        && rect.h >= 0.
}
