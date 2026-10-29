//! Bounded, immutable line correspondence for native comparisons and editor alignment.
//! Histogram matching and indentation cleanup are shared; callers retain their own text/state.

use imara_diff::{Algorithm, Diff, InternedInput, Interner};
use std::ops::Range;
mod cache;

/// Maximum original UTF-8 bytes per source, preserving the existing virtual-document bound.
pub const MAX_SOURCE_BYTES: usize = 1024 * 1024;
/// Bound line metadata independently of text size, including newline-dense untrusted inputs.
pub const MAX_SOURCE_LINES: usize = 100_000;
/// Shared admission allowance in input rows, summed per pair; this is not a runtime deadline.
pub const MAX_COMPARISON_ROWS: usize = 4 * (MAX_SOURCE_LINES + 1);

/// Admission failures occur before token interning; no partial change list is returned.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ComparisonError {
    /// One original source exceeds the byte limit.
    #[error("Comparison source exceeds 1 MiB")]
    SourceBytes,
    /// One source exceeds the metadata line limit.
    #[error("Comparison source exceeds 100,000 lines")]
    SourceLines,
    /// Other live comparisons have reserved the caller's input-row allowance.
    #[error("Comparison exceeds the shared input-row budget")]
    WorkBudget,
}

/// A paired interval is a modification; one empty side is an insertion or deletion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hunk {
    /// Half-open UTF-8 byte range in the unchanged original left source.
    pub left: Range<usize>,
    /// Half-open UTF-8 byte range in the unchanged original right source.
    pub right: Range<usize>,
}

/// Corresponding changed ranges in two borrowed line sequences, before byte projection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineHunk {
    /// Half-open left line indices, including any caller-supplied native EOF row.
    pub left: Range<usize>,
    /// Half-open right line indices, including any caller-supplied native EOF row.
    pub right: Range<usize>,
}

/// Platform EOL differences are ignored, but a missing final newline remains significant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct LineKey<'a> {
    /// Original borrowed content without a terminating LF/CRLF; lone CR stays significant.
    body: &'a str,
    /// True only for an actual terminal LF (possibly preceded by CR).
    terminated: bool,
}

impl AsRef<[u8]> for LineKey<'_> {
    /// Let imara's indentation cleanup inspect content without allocating normalized copies.
    fn as_ref(&self) -> &[u8] {
        self.body.as_bytes()
    }
}

/// Normalize `line` for matching while keeping source bytes untouched for eventual offsets.
fn line_key(line: &str) -> LineKey<'_> {
    match line.strip_suffix('\n') {
        Some(body) => LineKey {
            body: body.strip_suffix('\r').unwrap_or(body),
            terminated: true,
        },
        None => LineKey {
            body: line,
            terminated: false,
        },
    }
}

/// Compare borrowed `left`/`right` lines using Histogram plus indentation cleanup.
/// Every call charges their combined lengths to `budget`, including common edges, since interning
/// reads all rows. Limits fail without spending budget. Alignment callers may supply line bodies
/// and one extra native EOF row; ordinary comparisons retain terminators to distinguish final LF.
/// No text, selection, revision or Undo state is retained. This bound is not a wall-clock timeout.
pub fn line_changes(
    left: &[&str],
    right: &[&str],
    budget: &mut usize,
) -> Result<Vec<LineHunk>, ComparisonError> {
    if left.len() > MAX_SOURCE_LINES + 1 || right.len() > MAX_SOURCE_LINES + 1 {
        return Err(ComparisonError::SourceLines);
    }
    if [left, right].iter().any(|lines| {
        lines
            .iter()
            .fold(0usize, |bytes, line| bytes.saturating_add(line.len()))
            > MAX_SOURCE_BYTES
    }) {
        return Err(ComparisonError::SourceBytes);
    }
    let cost = left.len() + right.len();
    if cost > *budget {
        return Err(ComparisonError::WorkBudget);
    }
    *budget -= cost;
    let mut input = InternedInput {
        before: Vec::with_capacity(left.len()),
        after: Vec::with_capacity(right.len()),
        interner: Interner::new(cost),
    };
    input.update_before(left.iter().copied().map(line_key));
    input.update_after(right.iter().copied().map(line_key));
    let mut diff = Diff::compute(Algorithm::Histogram, &input);
    diff.postprocess_lines(&input);
    Ok(diff
        .hunks()
        .map(|hunk| LineHunk {
            left: hunk.before.start as usize..hunk.before.end as usize,
            right: hunk.after.start as usize..hunk.after.end as usize,
        })
        .collect())
}

/// Compare UTF-8 `left`/`right` and return ordered original-byte intervals, or an admission error.
/// LF and CRLF are equivalent; missing final LF and lone CR are changes. Each source admits up to
/// 1 MiB / 100,000 lines, without allocating an N×M matrix or retaining a second mutable document.
pub fn compare(left: &str, right: &str) -> Result<Vec<Hunk>, ComparisonError> {
    cache::compare(left, right)
}

/// Compute one admitted source pair; the cache retains only content digests and immutable ranges.
fn compare_uncached(left: &str, right: &str) -> Result<Vec<Hunk>, ComparisonError> {
    if left.len() > MAX_SOURCE_BYTES || right.len() > MAX_SOURCE_BYTES {
        return Err(ComparisonError::SourceBytes);
    }
    let lines = [left, right].map(|text| {
        text.split_inclusive('\n')
            .take(MAX_SOURCE_LINES + 1)
            .collect::<Vec<_>>()
    });
    if lines.iter().any(|lines| lines.len() > MAX_SOURCE_LINES) {
        return Err(ComparisonError::SourceLines);
    }
    let mut budget = MAX_COMPARISON_ROWS;
    let hunks = line_changes(&lines[0], &lines[1], &mut budget)?;
    let offsets = lines.map(|lines| {
        let mut offsets = Vec::with_capacity(lines.len() + 1);
        offsets.push(0);
        for line in lines {
            offsets.push(offsets.last().unwrap() + line.len());
        }
        offsets
    });
    Ok(hunks
        .into_iter()
        .map(|hunk| Hunk {
            left: offsets[0][hunk.left.start]..offsets[0][hunk.left.end],
            right: offsets[1][hunk.right.start]..offsets[1][hunk.right.end],
        })
        .collect())
}

#[cfg(test)]
mod tests;
