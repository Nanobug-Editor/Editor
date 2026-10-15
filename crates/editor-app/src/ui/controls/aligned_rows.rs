//! Immutable logical-line alignment and measured pixel projection for two or three native editors.
//!
//! This module owns no document text, editor state, selection or undo history. Callers discard a
//! plan when its source revisions change and remeasure after native width/font/wrapping changes.

use std::{collections::BTreeMap, ops::Range};

/// Maximum UTF-8 bytes inspected from each borrowed native source.
pub(crate) const MAX_SOURCE_BYTES: usize = 1024 * 1024;
/// Includes the empty EOF logical row after a trailing LF, even in an otherwise empty source.
pub(crate) const MAX_SOURCE_ROWS: usize = 2001;
/// Both pivot comparisons share this work budget; common prefix/suffix rows do not consume it.
pub(crate) const MAX_LCS_CELLS: usize = 4_000_000;

/// Planning/layout failures request an explicit independent-layout fallback from the UI owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AlignError {
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
pub(crate) struct AlignedRows {
    /// Ranges include their original LF/CRLF bytes and an empty EOF row after a trailing LF.
    pub(crate) source_rows: Vec<Vec<Range<usize>>>,
    /// One equally sized vector per pane: each entry names a native logical row, or a blank slot.
    pub(crate) slots: Vec<Vec<Option<usize>>>,
}

/// Blank pixels inserted at a native source coordinate; no text offset or source line is created.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Gap {
    pub(crate) source_y: f32,
    pub(crate) height: f32,
}

/// An untranslated native content interval and its equally tall display interval. Adjacent rows
/// without an intervening gap are coalesced, so a painter can clip/translate each slice once.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SourceSlice {
    pub(crate) source: Range<f32>,
    pub(crate) display: Range<f32>,
}

/// Pixel projection for one native editor; all coordinates are unscrolled content coordinates.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PaneLayout {
    pub(crate) gaps: Vec<Gap>,
    pub(crate) slices: Vec<SourceSlice>,
    pub(crate) source_height: f32,
    pub(crate) display_height: f32,
}

/// All panes share the sum of their visual slot heights, including native soft-wrapped rows.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AlignedLayout {
    pub(crate) panes: Vec<PaneLayout>,
    pub(crate) display_height: f32,
}

impl AlignedRows {
    /// Align two or three borrowed UTF-8 sources around pane zero. The returned value retains only
    /// byte ranges/row indices; byte, row and shared LCS limits fail before unbounded work is done.
    pub(crate) fn new(sources: &[&str]) -> Result<Self, AlignError> {
        let mut budget = MAX_LCS_CELLS;
        Self::new_with_budget(sources, &mut budget)
    }

    /// Charge comparison work to a caller-owned scene budget. Independent groups share this
    /// counter, while cached plans require no further work until their source content changes.
    pub(crate) fn new_with_budget(
        sources: &[&str],
        budget: &mut usize,
    ) -> Result<Self, AlignError> {
        if !(2..=3).contains(&sources.len()) {
            return Err(AlignError::PaneCount);
        }
        let source_rows = sources
            .iter()
            .enumerate()
            .map(|(pane, source)| native_rows(source, pane))
            .collect::<Result<Vec<_>, _>>()?;
        // Intern borrowed line bodies once. LCS then compares bounded integer symbols instead of
        // repeatedly scanning long, nearly identical lines inside its quadratic work budget.
        let mut symbols = BTreeMap::<&str, u16>::new();
        let rows = sources
            .iter()
            .zip(&source_rows)
            .map(|(source, ranges)| {
                ranges
                    .iter()
                    .map(|range| {
                        let original = &source[range.clone()];
                        let content = original
                            .strip_suffix('\n')
                            .map_or(original, |line| line.strip_suffix('\r').unwrap_or(line));
                        let next = symbols.len() as u16;
                        *symbols.entry(content).or_insert(next)
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let pairs = rows[1..]
            .iter()
            .map(|other| PairRows::new(&rows[0], other, budget))
            .collect::<Result<Vec<_>, _>>()?;
        let mut slots = vec![Vec::new(); sources.len()];
        // Keys are (next pivot row, insertion ordinal), then (pivot row, usize::MAX). Equal
        // insertion ordinals in different peers share a slot; the longest bucket defines its size.
        for pivot in 0..=rows[0].len() {
            let inserted = pairs
                .iter()
                .map(|pair| pair.before[pivot].len())
                .max()
                .unwrap_or(0);
            for ordinal in 0..inserted {
                slots[0].push(None);
                for (pane, pair) in pairs.iter().enumerate() {
                    slots[pane + 1].push(pair.before[pivot].get(ordinal).copied());
                }
            }
            if pivot < rows[0].len() {
                slots[0].push(Some(pivot));
                for (pane, pair) in pairs.iter().enumerate() {
                    slots[pane + 1].push(pair.at[pivot]);
                }
            }
        }
        Ok(Self { source_rows, slots })
    }

    /// Project native logical-row y ranges onto the aligned slots. Each pane must provide one
    /// finite, nonnegative range per source row, starting at zero with exactly continuous edges.
    /// Invalid measurements or arithmetic overflow return an error without a partial layout.
    pub(crate) fn layout(
        &self,
        measurements: &[Vec<Range<f32>>],
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
        let display_height = heights.iter().try_fold(0_f32, |sum, height| {
            let next = sum + height;
            next.is_finite()
                .then_some(next)
                .ok_or(AlignError::GeometryOverflow)
        })?;
        let panes = measurements
            .iter()
            .enumerate()
            .map(|(pane, rows)| layout_pane(&self.slots[pane], rows, &heights, display_height))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(AlignedLayout {
            panes,
            display_height,
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
    pub(crate) fn source_to_display(&self, source_y: f32) -> Option<f32> {
        if !source_y.is_finite() {
            return None;
        }
        let source_y = source_y.clamp(0., self.source_height);
        let index = self
            .slices
            .partition_point(|slice| slice.source.end <= source_y);
        Some(self.slices.get(index).map_or(self.display_height, |slice| {
            (slice.display.start + (source_y - slice.source.start))
                .clamp(slice.display.start, slice.display.end)
        }))
    }

    /// Map display pixels to native y without assigning bytes to a gap. Inside a gap choose the
    /// closest real slice edge; the upper edge stays at most half a pixel inside the preceding
    /// native row, surviving addition to large window origins. Ties prefer the preceding slice.
    pub(crate) fn display_to_source(&self, display_y: f32) -> Option<f32> {
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
    fn new(pivot: &[u16], other: &[u16], budget: &mut usize) -> Result<Self, AlignError> {
        let mut prefix = 0;
        while prefix < pivot.len().min(other.len()) && pivot[prefix] == other[prefix] {
            prefix += 1;
        }
        let mut suffix = 0;
        while suffix < (pivot.len() - prefix).min(other.len() - prefix)
            && pivot[pivot.len() - suffix - 1] == other[other.len() - suffix - 1]
        {
            suffix += 1;
        }
        let a = &pivot[prefix..pivot.len() - suffix];
        let b = &other[prefix..other.len() - suffix];
        let cells = a.len() * b.len();
        if cells > *budget {
            return Err(AlignError::ComparisonBudget);
        }
        *budget -= cells;
        let mut anchors = (0..prefix).map(|row| (row, row)).collect::<Vec<_>>();
        if !a.is_empty() && !b.is_empty() {
            let stride = b.len() + 1;
            let mut lengths = vec![0_u16; (a.len() + 1) * stride];
            for i in (0..a.len()).rev() {
                for j in (0..b.len()).rev() {
                    lengths[i * stride + j] = if a[i] == b[j] {
                        lengths[(i + 1) * stride + j + 1] + 1
                    } else {
                        lengths[(i + 1) * stride + j].max(lengths[i * stride + j + 1])
                    };
                }
            }
            let (mut i, mut j) = (0, 0);
            while i < a.len() && j < b.len() {
                if a[i] == b[j] {
                    anchors.push((prefix + i, prefix + j));
                    i += 1;
                    j += 1;
                } else if lengths[(i + 1) * stride + j] >= lengths[i * stride + j + 1] {
                    i += 1;
                } else {
                    j += 1;
                }
            }
        }
        anchors.extend(
            (0..suffix).map(|row| (pivot.len() - suffix + row, other.len() - suffix + row)),
        );
        // A sentinel flushes trailing insertions into the bucket after the final pivot row.
        anchors.push((pivot.len(), other.len()));
        let mut pair = Self {
            at: vec![None; pivot.len()],
            before: vec![Vec::new(); pivot.len() + 1],
        };
        let (mut a_start, mut b_start) = (0, 0);
        for (a_end, b_end) in anchors {
            let paired = (a_end - a_start).min(b_end - b_start);
            for row in 0..paired {
                pair.at[a_start + row] = Some(b_start + row);
            }
            pair.before[a_end].extend(b_start + paired..b_end);
            if a_end < pivot.len() {
                pair.at[a_end] = Some(b_end);
            }
            a_start = a_end + 1;
            b_start = b_end + 1;
        }
        Ok(pair)
    }
}

fn layout_pane(
    slots: &[Option<usize>],
    rows: &[Range<f32>],
    heights: &[f32],
    display_height: f32,
) -> Result<PaneLayout, AlignError> {
    let mut layout = PaneLayout {
        gaps: Vec::new(),
        slices: Vec::new(),
        source_height: rows.last().map_or(0., |row| row.end),
        display_height,
    };
    let (mut source_y, mut display_y) = (0., 0.);
    for (slot, height) in slots.iter().zip(heights) {
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
#[path = "aligned_rows/tests.rs"]
mod tests;
