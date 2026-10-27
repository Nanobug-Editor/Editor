//! Index native glyphs once so long clipped lines and many soft rows cannot rescan entire runs.
use super::*;

/// One immutable physical source line preserves native first-match positions and global bidi levels.
pub(super) struct IndexedLine<'a> {
    glyphs: Vec<(FontId, &'a ShapedGlyph)>,
    positions: Vec<Pixels>,
    bidi: BidiInfo<'a>,
}
impl<'a> IndexedLine<'a> {
    /// Reproduce the locked native x_for_index first-match rule in O(bytes + glyphs) before sorting.
    pub(super) fn new(line: &'a WrappedLine) -> Self {
        let mut glyphs = Vec::new();
        let mut positions = vec![line.width(); line.text.len() + 1];
        let mut filled = 0;
        for run in line.runs() {
            for glyph in &run.glyphs {
                let end = glyph.index.min(line.text.len());
                if end >= filled {
                    positions[filled..=end].fill(glyph.position.x);
                    filled = end + 1;
                }
                glyphs.push((run.font_id, glyph));
            }
        }
        // Stable source ordering preserves native order for multiple glyphs in one shaped cluster.
        glyphs.sort_by_key(|(_, glyph)| glyph.index);
        Self {
            glyphs,
            positions,
            bidi: BidiInfo::new(&line.text, None),
        }
    }
    /// Only this row's native glyphs are visited, even when the rest of the source is clipped.
    pub(super) fn glyphs(&self, range: Range<usize>) -> &[(FontId, &'a ShapedGlyph)] {
        let start = self
            .glyphs
            .partition_point(|(_, glyph)| glyph.index < range.start);
        let end = self
            .glyphs
            .partition_point(|(_, glyph)| glyph.index < range.end);
        &self.glyphs[start..end]
    }
    /// Constant-time native lookup retains the exact original-byte boundary semantics.
    pub(super) fn x(&self, byte: usize) -> Pixels {
        self.positions[byte]
    }
    /// The paragraph base direction controls alignment and the ellipsis side.
    pub(super) fn base_rtl(&self) -> bool {
        self.bidi
            .paragraphs
            .first()
            .is_some_and(|paragraph| paragraph.level.is_rtl())
    }
    /// Apply native UAX #9 L1 to a row-sized copy of already resolved embedding levels.
    /// Running bidi afresh would lose explicit embeddings that began before a soft-wrap boundary.
    pub(super) fn levels(&self, range: Range<usize>) -> Vec<Level> {
        let Some(paragraph) = self.bidi.paragraphs.iter().find(|paragraph| {
            paragraph.range.start <= range.start && paragraph.range.end >= range.end
        }) else {
            return self.bidi.levels[range].to_vec();
        };
        let length = range.end - range.start;
        let row = BidiInfo {
            text: &self.bidi.text[range.clone()],
            original_classes: self.bidi.original_classes[range.clone()].to_vec(),
            levels: self.bidi.levels[range].to_vec(),
            paragraphs: Vec::new(),
        };
        row.reordered_levels(
            &unicode_bidi::ParagraphInfo {
                range: 0..length,
                level: paragraph.level,
            },
            0..length,
        )
    }
}
