//! Immutable logical-line alignment and measured pixel projection for two or three native editors.
//!
//! This module owns no document text, editor state, selection or undo history. Callers discard a
//! plan when its source revisions change and remeasure after native width/font/wrapping changes.

use std::ops::Range;

/// Maximum UTF-8 bytes inspected from each borrowed native source.
pub(crate) const MAX_SOURCE_BYTES: usize = editor_core::comparison::MAX_SOURCE_BYTES;
/// Includes the empty EOF logical row after a trailing LF, even in an otherwise empty source.
pub(crate) const MAX_SOURCE_ROWS: usize = editor_core::comparison::MAX_SOURCE_LINES + 1;
/// Pivot and peer insertion comparisons share one linear input-row allowance.
pub(crate) const MAX_COMPARISON_ROWS: usize = editor_core::comparison::MAX_COMPARISON_ROWS;

/// Planning/layout failures request an explicit independent-layout fallback from the UI owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AlignError {
    PaneCount,
    SourceBytes { pane: usize },
    SourceRows { pane: usize },
    ComparisonBudget,
    MeasurementPaneCount,
    MeasurementRowCount { pane: usize },
    InvalidGeometry { pane: usize, row: usize },
    GeometryOverflow,
}

/// Native UTF-8 line ranges and ordered visual slots. Each source row occurs exactly once in its
/// pane's slots; `None` reserves visual space without adding bytes. Treat this metadata as immutable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlignedRows {
    /// Ranges include their original LF/CRLF bytes and an empty EOF row after a trailing LF.
    pub(crate) source_rows: Vec<Vec<Range<usize>>>,
    /// One equally sized vector per pane: each entry names a native logical row, or a blank slot.
    pub(crate) slots: Vec<Vec<Option<usize>>>,
    /// Exact original bytes agree in every pane; line-ending-only changes are never hidden.
    unchanged: Vec<bool>,
}

/// Blank pixels inserted at a native source coordinate; no text offset or source line is created.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gap {
    pub(crate) source_y: f32,
    pub(crate) height: f32,
}

/// An untranslated native content interval and its equally tall display interval. Adjacent rows
/// without an intervening gap are coalesced, so a painter can clip/translate each slice once.
#[derive(Clone, Debug, PartialEq)]
pub struct SourceSlice {
    pub(crate) source: Range<f32>,
    pub(crate) display: Range<f32>,
}

/// Pixel projection for one native editor; all coordinates are unscrolled content coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct PaneLayout {
    pub(crate) gaps: Vec<Gap>,
    pub(crate) slices: Vec<SourceSlice>,
    pub(crate) source_height: f32,
    pub(crate) display_height: f32,
}

/// All panes share the sum of their visual slot heights, including native soft-wrapped rows.
#[derive(Clone, Debug, PartialEq)]
pub struct AlignedLayout {
    pub(crate) panes: Vec<PaneLayout>,
    pub(crate) display_height: f32,
    /// Collapsed slot ranges and their shared zero-byte disclosure-row geometry.
    pub(crate) folds: Vec<(Range<usize>, Range<f32>)>,
}

impl AlignedRows {
    /// Align two or three borrowed UTF-8 sources around pane zero. The returned value retains only
    /// byte ranges/row indices; byte, row and shared input-row limits fail before unbounded work is done.
    pub fn new(sources: &[&str]) -> Result<Self, AlignError> {
        let mut budget = MAX_COMPARISON_ROWS;
        Self::new_with_budget(sources, &mut budget)
    }

    /// Charge comparison work to a caller-owned scene budget. Independent groups share this
    /// counter, while cached plans require no further work until their source content changes.
    pub fn new_with_budget(sources: &[&str], budget: &mut usize) -> Result<Self, AlignError> {
        if !(2..=3).contains(&sources.len()) {
            return Err(AlignError::PaneCount);
        }
        let source_rows = sources
            .iter()
            .enumerate()
            .map(|(pane, source)| native_rows(source, pane))
            .collect::<Result<Vec<_>, _>>()?;
        // Borrow bodies for three-pane insertion buckets; pivot pairs reuse exact byte hunks.
        // EOF remains a real caret row even when the byte comparison has no corresponding line.
        let rows = sources
            .iter()
            .zip(&source_rows)
            .map(|(source, ranges)| {
                ranges
                    .iter()
                    .map(|range| {
                        let original = &source[range.clone()];
                        original
                            .strip_suffix('\n')
                            .map_or(original, |line| line.strip_suffix('\r').unwrap_or(line))
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let pairs = (1..sources.len())
            .map(|pane| {
                PairRows::sources(
                    sources[0],
                    sources[pane],
                    &source_rows[0],
                    &source_rows[pane],
                    budget,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut slots = vec![Vec::new(); sources.len()];
        // Pivot anchors bound each insertion bucket. In a three-pane group the peers can share
        // new lines absent from the pivot, so align that bucket before returning to its anchor.
        for pivot in 0..=rows[0].len() {
            let left = &pairs[0].before[pivot];
            if let Some(right) = pairs
                .get(1)
                .map(|pair| &pair.before[pivot])
                .filter(|right| !left.is_empty() || !right.is_empty())
            {
                let left_symbols = left.iter().map(|row| rows[1][*row]).collect::<Vec<_>>();
                let right_symbols = right.iter().map(|row| rows[2][*row]).collect::<Vec<_>>();
                // Reuse the same bounded matcher and scene ledger; a failed peer comparison
                // requests the existing independent-layout fallback without a partial plan.
                let inserted = PairRows::new(&left_symbols, &right_symbols, budget)?;
                for row in 0..=left.len() {
                    for other in &inserted.before[row] {
                        slots[0].push(None);
                        slots[1].push(None);
                        slots[2].push(Some(right[*other]));
                    }
                    if row < left.len() {
                        slots[0].push(None);
                        slots[1].push(Some(left[row]));
                        slots[2].push(inserted.at[row].map(|other| right[other]));
                    }
                }
            } else {
                // A lone peer needs no second diff, and empty three-pane buckets need no slots.
                for row in left {
                    slots[0].push(None);
                    slots[1].push(Some(*row));
                }
            }
            if pivot < rows[0].len() {
                slots[0].push(Some(pivot));
                for (pane, pair) in pairs.iter().enumerate() {
                    slots[pane + 1].push(pair.at[pivot]);
                }
            }
        }
        let unchanged = (0..slots[0].len())
            .map(|slot| {
                let Some(first) = slots[0][slot] else {
                    return false;
                };
                let original = &sources[0][source_rows[0][first].clone()];
                (1..sources.len()).all(|pane| {
                    slots[pane][slot].is_some_and(|row| {
                        &sources[pane][source_rows[pane][row].clone()] == original
                    })
                })
            })
            .collect();
        Ok(Self {
            source_rows,
            slots,
            unchanged,
        })
    }

    /// Keep `context` original lines at both ends of each exact common run, including EOF runs.
    /// A disclosure replaces at least two lines, so collapsing never makes the display taller.
    pub fn unchanged_regions(&self, context: usize) -> Vec<Range<usize>> {
        let mut result = Vec::new();
        let mut start = 0;
        while start < self.unchanged.len() {
            if !self.unchanged[start] {
                start += 1;
                continue;
            }
            let end = start
                + self.unchanged[start..]
                    .iter()
                    .take_while(|same| **same)
                    .count();
            if end - start > context.saturating_mul(2).saturating_add(1) {
                result.push(start + context..end - context);
            }
            start = end;
        }
        result
    }

    /// Project native logical-row y ranges onto the aligned slots. Each pane must provide one
    /// finite, nonnegative range per source row, starting at zero with exactly continuous edges.
    /// Invalid measurements or arithmetic overflow return an error without a partial layout.
    pub fn layout(&self, measurements: &[Vec<Range<f32>>]) -> Result<AlignedLayout, AlignError> {
        self.layout_with_folds(measurements, &[], 0.)
    }

    /// Omit only common source slots while retaining their original native byte/line identities.
    /// `folds` must be ordered, disjoint common regions; each becomes one zero-byte control row.
    pub fn layout_with_folds(
        &self,
        measurements: &[Vec<Range<f32>>],
        folds: &[Range<usize>],
        disclosure_height: f32,
    ) -> Result<AlignedLayout, AlignError> {
        self.validate_measurements(measurements)?;
        let mut heights = vec![0_f32; self.slots[0].len()];
        for (pane, rows) in measurements.iter().enumerate() {
            for (slot, row) in self.slots[pane].iter().enumerate() {
                if let Some(row) = row {
                    heights[slot] = heights[slot].max(rows[*row].end - rows[*row].start);
                }
            }
        }
        let mut hidden = vec![false; heights.len()];
        for fold in folds {
            if !disclosure_height.is_finite()
                || disclosure_height <= 0.
                || fold.is_empty()
                || fold.end > heights.len()
                || fold
                    .clone()
                    .any(|slot| hidden[slot] || !self.unchanged[slot])
            {
                return Err(AlignError::GeometryOverflow);
            }
            for slot in fold.clone() {
                hidden[slot] = true;
                heights[slot] = 0.;
            }
            heights[fold.start] = disclosure_height;
        }
        let mut display_y = 0.;
        let mut markers = Vec::new();
        for (slot, height) in heights.iter().enumerate() {
            if let Some(fold) = folds.iter().find(|fold| fold.start == slot) {
                markers.push((fold.clone(), display_y..display_y + height));
            }
            display_y += height;
        }
        let display_height = heights.iter().try_fold(0_f32, |sum, height| {
            let next = sum + height;
            next.is_finite()
                .then_some(next)
                .ok_or(AlignError::GeometryOverflow)
        })?;
        let panes = measurements
            .iter()
            .enumerate()
            .map(|(pane, rows)| {
                layout_pane(&self.slots[pane], rows, &heights, &hidden, display_height)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(AlignedLayout {
            panes,
            display_height,
            folds: markers,
        })
    }

    fn validate_measurements(&self, measurements: &[Vec<Range<f32>>]) -> Result<(), AlignError> {
        if measurements.len() != self.source_rows.len() {
            return Err(AlignError::MeasurementPaneCount);
        }
        for (pane, rows) in measurements.iter().enumerate() {
            if rows.len() != self.source_rows[pane].len() {
                return Err(AlignError::MeasurementRowCount { pane });
            }
            let mut previous = 0_f32;
            for (row, range) in rows.iter().enumerate() {
                if !range.start.is_finite()
                    || !range.end.is_finite()
                    || range.start != previous
                    || range.end < range.start
                {
                    return Err(AlignError::InvalidGeometry { pane, row });
                }
                previous = range.end;
            }
        }
        Ok(())
    }
}

impl PaneLayout {
    /// Map a native y coordinate to display pixels. Finite out-of-bounds inputs clamp to the
    /// content edges; a boundary with a gap belongs to the following real row (EOF to display end).
    pub fn source_to_display(&self, source_y: f32) -> Option<f32> {
        if !source_y.is_finite() {
            return None;
        }
        let source_y = source_y.clamp(0., self.source_height);
        let index = self
            .slices
            .partition_point(|slice| slice.source.end <= source_y);
        if self
            .slices
            .get(index)
            .is_some_and(|slice| source_y < slice.source.start)
        {
            return None;
        }
        Some(self.slices.get(index).map_or(self.display_height, |slice| {
            (slice.display.start + (source_y - slice.source.start))
                .clamp(slice.display.start, slice.display.end)
        }))
    }

    /// Map display pixels to native y without assigning bytes to a gap. Inside a gap choose the
    /// closest real slice edge; the upper edge stays at most half a pixel inside the preceding
    /// native row, surviving addition to large window origins. Ties prefer the preceding slice.
    pub fn display_to_source(&self, display_y: f32) -> Option<f32> {
        if !display_y.is_finite() {
            return None;
        }
        let display_y = display_y.clamp(0., self.display_height);
        if display_y == self.display_height {
            return Some(self.source_height);
        }
        let index = self
            .slices
            .partition_point(|slice| slice.display.end <= display_y);
        let after = self.slices.get(index);
        if let Some(slice) = after.filter(|slice| slice.display.start <= display_y) {
            return Some(
                (slice.source.start + (display_y - slice.display.start))
                    .clamp(slice.source.start, slice.source.end),
            );
        }
        let before = index
            .checked_sub(1)
            .and_then(|index| self.slices.get(index));
        match (before, after) {
            (Some(before), Some(after))
                if display_y - before.display.end > after.display.start - display_y =>
            {
                Some(after.source.start)
            }
            (Some(before), _) => {
                let inset = ((before.source.end - before.source.start) * 0.5).min(0.5);
                Some(
                    (before.source.end - inset)
                        .min(before.source.end.next_down())
                        .max(before.source.start),
                )
            }
            (_, Some(after)) => Some(after.source.start),
            _ => Some(0.),
        }
    }
}

fn native_rows(source: &str, pane: usize) -> Result<Vec<Range<usize>>, AlignError> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(AlignError::SourceBytes { pane });
    }
    let mut start = 0;
    let mut rows = Vec::new();
    for (offset, byte) in source.bytes().enumerate() {
        if byte == b'\n' {
            rows.push(start..offset + 1);
            start = offset + 1;
            if rows.len() >= MAX_SOURCE_ROWS {
                return Err(AlignError::SourceRows { pane });
            }
        }
    }
    rows.push(start..source.len());
    Ok(rows)
}

struct PairRows {
    at: Vec<Option<usize>>,
    before: Vec<Vec<usize>>,
}
impl PairRows {
    /// Reuse the host's exact byte comparison, then project ranges onto native rows including EOF.
    /// Cache hits still reserve the same scene rows; cached work never bypasses admission quotas.
    fn sources(
        left: &str,
        right: &str,
        left_rows: &[Range<usize>],
        right_rows: &[Range<usize>],
        budget: &mut usize,
    ) -> Result<Self, AlignError> {
        let cost = left_rows.len() + right_rows.len();
        if cost > *budget {
            return Err(AlignError::ComparisonBudget);
        }
        let hunks = editor_core::comparison::compare(left, right)
            .map_err(|_| AlignError::ComparisonBudget)?;
        *budget -= cost;
        let changes = hunks
            .into_iter()
            .map(|hunk| editor_core::comparison::LineHunk {
                left: left_rows.partition_point(|row| row.start < hunk.left.start)
                    ..left_rows.partition_point(|row| row.start < hunk.left.end),
                right: right_rows.partition_point(|row| row.start < hunk.right.start)
                    ..right_rows.partition_point(|row| row.start < hunk.right.end),
            })
            .collect::<Vec<_>>();
        Ok(Self::from_changes(
            left_rows.len(),
            right_rows.len(),
            changes,
        ))
    }

    /// Turn Histogram unchanged intervals in borrowed row bodies into monotonic native anchors.
    /// Every pivot/peer row charges the shared input-row budget, including insertion buckets.
    fn new(pivot: &[&str], other: &[&str], budget: &mut usize) -> Result<Self, AlignError> {
        let hunks = editor_core::comparison::line_changes(pivot, other, budget)
            .map_err(|_| AlignError::ComparisonBudget)?;
        Ok(Self::from_changes(pivot.len(), other.len(), hunks))
    }

    /// Native EOF is a caret row; match only available tails when just one source ends with LF.
    fn from_changes(
        left_count: usize,
        right_count: usize,
        hunks: Vec<editor_core::comparison::LineHunk>,
    ) -> Self {
        let mut anchors = Vec::new();
        let (mut left, mut right) = (0, 0);
        for hunk in hunks {
            anchors.extend((0..hunk.left.start - left).map(|row| (left + row, right + row)));
            left = hunk.left.end;
            right = hunk.right.end;
        }
        anchors.extend(
            (0..(left_count - left).min(right_count - right)).map(|row| (left + row, right + row)),
        );
        // A sentinel flushes trailing insertions into the bucket after the final pivot row.
        anchors.push((left_count, right_count));
        let mut pair = Self {
            at: vec![None; left_count],
            before: vec![Vec::new(); left_count + 1],
        };
        let (mut a_start, mut b_start) = (0, 0);
        for (a_end, b_end) in anchors {
            let paired = (a_end - a_start).min(b_end - b_start);
            for row in 0..paired {
                pair.at[a_start + row] = Some(b_start + row);
            }
            pair.before[a_end].extend(b_start + paired..b_end);
            if a_end < left_count {
                pair.at[a_end] = Some(b_end);
            }
            a_start = a_end + 1;
            b_start = b_end + 1;
        }
        pair
    }
}

fn layout_pane(
    slots: &[Option<usize>],
    rows: &[Range<f32>],
    heights: &[f32],
    hidden: &[bool],
    display_height: f32,
) -> Result<PaneLayout, AlignError> {
    let mut layout = PaneLayout {
        gaps: Vec::new(),
        slices: Vec::new(),
        source_height: rows.last().map_or(0., |row| row.end),
        display_height,
    };
    let (mut source_y, mut display_y) = (0., 0.);
    for ((slot, height), hidden) in slots.iter().zip(heights).zip(hidden) {
        if *hidden {
            if let Some(row) = slot {
                source_y = rows[*row].end;
            }
            display_y += height;
            continue;
        }
        let native_height = if let Some(row) = slot {
            let source = rows[*row].clone();
            let native_height = source.end - source.start;
            if native_height > 0. {
                let display = display_y..display_y + native_height;
                if let Some(last) = layout.slices.last_mut().filter(|last| {
                    last.source.end == source.start && last.display.end == display.start
                }) {
                    last.source.end = source.end;
                    last.display.end = display.end;
                } else {
                    layout.slices.push(SourceSlice { source, display });
                }
            }
            source_y = rows[*row].end;
            native_height
        } else {
            0.
        };
        let extra = height - native_height;
        if extra > 0. {
            if let Some(last) = layout
                .gaps
                .last_mut()
                .filter(|gap| gap.source_y == source_y)
            {
                last.height += extra;
                if !last.height.is_finite() {
                    return Err(AlignError::GeometryOverflow);
                }
            } else {
                layout.gaps.push(Gap {
                    source_y,
                    height: extra,
                });
            }
        }
        display_y += height;
    }
    Ok(layout)
}

#[cfg(test)]
mod tests;
