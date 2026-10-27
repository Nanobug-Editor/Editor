//! Native Canvas preedit and unacknowledged drafts have no domain undo or editor-document ownership.
use plugin_runtime::plugin_protocol::{
    api::{ErrorCode, Failure},
    ui::{
        CanvasComposition, CanvasCompositionPhase as Phase, CanvasTextEdit, CanvasTextRange,
        TextInputSession, canvas_text_range_from_utf16, grapheme_boundaries,
    },
};
use std::{collections::VecDeque, ops::Range};
mod buffer;
use buffer::Buffer;

/// One exact target epoch's temporary native text, selection and pending acknowledgement snapshots.
pub struct CanvasTextSession {
    /// Last accepted guest declaration; retained to distinguish real selection changes from redraws.
    binding: TextInputSession,
    /// Guest-committed baseline, kept read-only until a valid acknowledgement arrives.
    committed: Buffer,
    /// Necessary native draft; it contains preedit and unacknowledged commits only.
    draft: Buffer,
    /// Original UTF-8 extended-grapheme selection within the current draft.
    selection: CanvasTextRange,
    /// A later native selection must not be overwritten by the preceding Text acknowledgement.
    selection_after_commit: bool,
    /// A cancellation restores the exact pre-composition draft, including earlier pending commits.
    marked: Option<Marked>,
    /// Cumulative acknowledgements cannot overwrite later native snapshots.
    pending: VecDeque<Pending>,
    /// Native Text sequence, distinct from drawing/text revisions and other input kinds.
    last_sequence: u64,
}
/// Native marked source records both its unmodified predecessor and current draft span.
struct Marked {
    original: Buffer,
    range: CanvasTextRange,
    current: CanvasTextRange,
    selection: CanvasTextRange,
}
/// One pending Text sequence's complete result, bounded across the session and process.
struct Pending {
    sequence: u64,
    text: Buffer,
}
impl CanvasTextSession {
    /// Bind native drafts to `binding` and its exact original paragraph `text`.
    /// Invalid original selections and exhausted draft budgets return typed failures before mutation.
    pub fn new(binding: &TextInputSession, text: &str) -> Result<Self, Failure> {
        binding.selection.to_utf16(text).map_err(invalid)?;
        Ok(Self {
            binding: binding.clone(),
            committed: Buffer::new(text)?,
            draft: Buffer::new(text)?,
            selection: binding.selection.clone(),
            selection_after_commit: false,
            marked: None,
            pending: VecDeque::new(),
            last_sequence: binding.acknowledged_sequence,
        })
    }
    /// Read the immutable current declaration; native state never manufactures a document reference.
    pub fn binding(&self) -> &TextInputSession {
        &self.binding
    }
    /// Current native draft for shaping and OS text queries, never a second editor Rope.
    pub fn text(&self) -> &str {
        self.draft.text()
    }
    /// Temporary native text does not reinterpret the guest's committed rich-style byte ranges.
    pub fn is_transient(&self) -> bool {
        self.marked.is_some() || self.draft.text() != self.committed.text()
    }
    /// Current original-byte selection within that same draft.
    pub fn selection(&self) -> CanvasTextRange {
        self.selection.clone()
    }
    /// Current native selection in UTF-16 units.
    pub fn selection_utf16(&self) -> Range<usize> {
        self.selection
            .to_utf16(self.text())
            .expect("native selection is canonical")
    }
    /// Current native marked range in UTF-16 units, absent after commit/cancellation.
    pub fn marked_utf16(&self) -> Option<Range<usize>> {
        self.marked
            .as_ref()
            .and_then(|marked| marked.current.to_utf16(self.text()).ok())
    }
    /// Current original-byte marked range used by native underline/selection painting.
    pub fn marked_range(&self) -> Option<CanvasTextRange> {
        self.marked.as_ref().map(|marked| marked.current.clone())
    }
    /// Last pending predecessor; zero means new edits are based directly on the acknowledged baseline.
    pub fn base_sequence(&self) -> u64 {
        if self.pending.is_empty() {
            0
        } else {
            self.last_sequence
        }
    }
    /// Retain native pending/preedit state across ordinary declarations and cumulative acknowledgements.
    /// Older acknowledgements are ignored; contradictory text/revision or unknown commits return Conflict.
    pub fn reconcile(&mut self, binding: &TextInputSession, text: &str) -> Result<(), Failure> {
        binding.selection.to_utf16(text).map_err(invalid)?;
        if binding.text_revision < self.binding.text_revision
            || binding.acknowledged_sequence < self.binding.acknowledged_sequence
        {
            return Ok(());
        }
        let changed = text != self.committed.text();
        let advance = binding.acknowledged_sequence > self.binding.acknowledged_sequence;
        if (changed || advance) && binding.text_revision <= self.binding.text_revision {
            return Err(conflict("Canvas text revision did not advance"));
        }
        if advance {
            let pending = self
                .pending
                .iter()
                .find(|pending| pending.sequence == binding.acknowledged_sequence)
                .ok_or_else(|| conflict("Canvas acknowledged an unknown native text sequence"))?;
            if pending.text.text() != text {
                return Err(conflict(
                    "Canvas acknowledgement changed the native commit result",
                ));
            }
        } else if changed && (!self.pending.is_empty() || self.marked.is_some()) {
            return Err(conflict(
                "Canvas authoritative text conflicts with pending native input; replace the target epoch",
            ));
        }
        let committed = if changed {
            Some(Buffer::new(text)?)
        } else {
            None
        };
        let next_draft = if changed && self.pending.is_empty() && self.marked.is_none() {
            Some(Buffer::new(text)?)
        } else {
            None
        };
        if advance {
            self.pending
                .retain(|pending| pending.sequence > binding.acknowledged_sequence);
        }
        if let Some(committed) = committed {
            self.committed = committed;
        }
        if let Some(draft) = next_draft {
            self.draft = draft;
        }
        if self.pending.is_empty()
            && self.marked.is_none()
            && (changed || binding.selection != self.binding.selection)
            && !(advance && self.selection_after_commit)
        {
            self.selection = binding.selection.clone();
            self.selection_after_commit = false;
        }
        self.binding = binding.clone();
        Ok(())
    }
    /// Read native UTF-16 text, returning the normalized units alongside the exact original substring.
    pub fn text_for_range(&self, range: Range<usize>) -> (String, Range<usize>) {
        let bytes = canvas_text_range_from_utf16(self.text(), range);
        (
            self.text()[bytes.start..bytes.end].into(),
            bytes
                .to_utf16(self.text())
                .expect("canonical native text range"),
        )
    }
    /// Change native selection without mutating text; the caller emits its separate selection observation.
    pub fn select(&mut self, range: Range<usize>) -> CanvasTextRange {
        self.selection = canvas_text_range_from_utf16(self.text(), range);
        self.selection_after_commit = true;
        self.selection.clone()
    }
    /// Update marked text in native UTF-16 coordinates, preserving an immutable cancellation source.
    /// Empty native preedit cancels; replacements outside an active marked span are rejected atomically.
    pub fn mark(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
    ) -> Result<Option<CanvasComposition>, Failure> {
        if self.binding.read_only {
            return Err(invalid("Canvas text session is readonly"));
        }
        if text.is_empty() {
            return Ok(self.cancel());
        }
        let bytes = self.native_range(range);
        if self.marked.as_ref().is_some_and(|marked| {
            bytes.start < marked.current.start || bytes.end > marked.current.end
        }) {
            return Err(conflict(
                "Native preedit replacement extends outside the marked span",
            ));
        }
        let candidate = self.draft.replaced(bytes.clone(), text)?;
        let relative = canvas_text_range_from_utf16(
            text,
            selected.unwrap_or_else(|| {
                let end = text.encode_utf16().count();
                end..end
            }),
        );
        let selection = CanvasTextRange {
            start: snap(candidate.text(), bytes.start + relative.start),
            end: snap(candidate.text(), bytes.start + relative.end),
        };
        let (phase, marked) = if let Some(marked) = &self.marked {
            (
                Phase::Update,
                Marked {
                    original: marked.original.snapshot()?,
                    range: marked.range.clone(),
                    selection: marked.selection.clone(),
                    current: CanvasTextRange {
                        start: marked.current.start,
                        end: marked.current.end - (bytes.end - bytes.start) + text.len(),
                    },
                },
            )
        } else {
            (
                Phase::Start,
                Marked {
                    original: self.draft.snapshot()?,
                    range: bytes.clone(),
                    selection: self.selection.clone(),
                    current: CanvasTextRange {
                        start: bytes.start,
                        end: bytes.start + text.len(),
                    },
                },
            )
        };
        let observation = CanvasComposition {
            phase,
            text_revision: self.binding.text_revision,
            base_sequence: self.base_sequence(),
            range: marked.range.clone(),
            text: candidate.text()[marked.current.start..marked.current.end].into(),
            selection: CanvasTextRange {
                start: selection.start.saturating_sub(marked.current.start),
                end: selection.end.saturating_sub(marked.current.start),
            },
        };
        // Native marked ranges must remain whole graphemes in both the displayed draft and
        // transported preedit. Joining an adjacent committed cluster needs an explicit guest edit;
        // reject it before mutation rather than handing the OS an invalid marked boundary.
        marked.current.to_utf16(candidate.text()).map_err(invalid)?;
        observation
            .selection
            .to_utf16(&observation.text)
            .map_err(invalid)?;
        self.draft = candidate;
        self.marked = Some(marked);
        self.selection = selection;
        Ok(Some(observation))
    }
    /// Commit independent native text under the caller's next input `sequence`.
    /// Pending snapshots are limited to 32/1 MiB; failure leaves draft, selection and preedit unchanged.
    pub fn commit(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        sequence: u64,
    ) -> Result<(CanvasTextEdit, Option<CanvasComposition>), Failure> {
        if self.binding.read_only {
            return Err(invalid("Canvas text session is readonly"));
        }
        if sequence <= self.last_sequence {
            return Err(conflict("Native text sequence did not advance"));
        }
        if self.pending.len() >= 32 {
            return Err(limit("Canvas native text acknowledgement queue is full"));
        }
        let bytes = self.native_range(range);
        if self.marked.as_ref().is_some_and(|marked| {
            bytes.start < marked.current.start || bytes.end > marked.current.end
        }) {
            return Err(conflict(
                "Native commit replacement extends outside the marked span",
            ));
        }
        let candidate = self.draft.replaced(bytes.clone(), text)?;
        if self
            .pending
            .iter()
            .map(|pending| pending.text.len())
            .sum::<usize>()
            + candidate.len()
            > 1024 * 1024
        {
            return Err(limit("Canvas pending native text exceeds 1 MiB"));
        }
        let snapshot = candidate.snapshot()?;
        let end = snap(candidate.text(), bytes.start + text.len());
        let selection = CanvasTextRange { start: end, end };
        let (range, inserted, ended) = if let Some(marked) = &self.marked {
            let end = marked.current.end - (bytes.end - bytes.start) + text.len();
            (
                marked.range.clone(),
                candidate.text()[marked.current.start..end].to_owned(),
                Some(CanvasComposition {
                    phase: Phase::End,
                    text_revision: self.binding.text_revision,
                    base_sequence: sequence,
                    range: marked.range.clone(),
                    text: String::new(),
                    selection: CanvasTextRange { start: 0, end: 0 },
                }),
            )
        } else {
            (bytes, text.into(), None)
        };
        let edit = CanvasTextEdit {
            text_revision: self.binding.text_revision,
            base_sequence: self.base_sequence(),
            range,
            text: inserted,
            selection: selection.clone(),
        };
        self.draft = candidate;
        self.marked = None;
        self.selection = selection;
        self.last_sequence = sequence;
        self.selection_after_commit = false;
        self.pending.push_back(Pending {
            sequence,
            text: snapshot,
        });
        Ok((edit, ended))
    }
    /// Restore only uncommitted preedit; earlier native text commits and their acknowledgements survive.
    pub fn cancel(&mut self) -> Option<CanvasComposition> {
        let marked = self.marked.take()?;
        let event = CanvasComposition {
            phase: Phase::Cancel,
            text_revision: self.binding.text_revision,
            base_sequence: self.base_sequence(),
            range: marked.range,
            text: String::new(),
            selection: CanvasTextRange { start: 0, end: 0 },
        };
        self.draft = marked.original;
        self.selection = marked.selection;
        Some(event)
    }
    /// Choose the OS range, current mark or current native selection in that order.
    fn native_range(&self, range: Option<Range<usize>>) -> CanvasTextRange {
        range
            .map(|range| canvas_text_range_from_utf16(self.text(), range))
            .unwrap_or_else(|| {
                self.marked
                    .as_ref()
                    .map_or_else(|| self.selection.clone(), |marked| marked.current.clone())
            })
    }
}
/// A resulting caret always sits after the complete extended grapheme containing the insertion edge.
fn snap(text: &str, byte: usize) -> usize {
    grapheme_boundaries(text)
        .find(|boundary| *boundary >= byte)
        .unwrap_or(text.len())
}
/// Invalid native coordinates/policies are recoverable input errors.
fn invalid(message: impl Into<String>) -> Failure {
    Failure::new(ErrorCode::InvalidRequest, message)
}
/// Contradictory source/acknowledgement requires explicit guest reconciliation.
fn conflict(message: impl Into<String>) -> Failure {
    Failure::new(ErrorCode::Conflict, message)
}
/// Resource exhaustion cancels the producer rather than allocating an unbounded native draft.
fn limit(message: impl Into<String>) -> Failure {
    Failure::new(ErrorCode::LimitExceeded, message)
}
