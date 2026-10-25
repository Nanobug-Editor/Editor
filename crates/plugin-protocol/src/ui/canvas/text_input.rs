//! Target text sessions own only native drafts; committed domain text and undo remain in the guest.
use super::*;

/// Native input binding for one stable target epoch, using a declared paragraph's original text.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextInputSession {
    /// Scene drawing ID of the authoritative committed paragraph.
    pub paragraph: String,
    /// Exact declared layout identity, updated with text/typography changes.
    pub layout_key: String,
    /// Nonzero monotonic committed text revision within this target epoch.
    pub text_revision: u64,
    /// Guest-owned original UTF-8 selection; unchanged declarations do not reset native preedit.
    pub selection: CanvasTextRange,
    /// Last cumulatively accepted native Text sequence, initially zero.
    /// Acknowledged text must equal that commit's complete draft; later pending input stays visible.
    #[serde(default)]
    pub acknowledged_sequence: u64,
    /// Keep focus and selection while rejecting edits/composition on a readonly text object.
    #[serde(default)]
    pub read_only: bool,
    /// Opt into native whole-grapheme deletion, selection/navigation and line breaks.
    /// Raw key transitions remain observable. Domain undo and business shortcuts remain guest-owned.
    #[serde(default)]
    pub editing_keys: bool,
}
/// One native commit over the draft produced by the previous accepted native text sequence.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasTextEdit {
    /// Committed baseline known to the native session when this edit was created.
    pub text_revision: u64,
    /// Previous native Text sequence; zero means no unacknowledged predecessor.
    pub base_sequence: u64,
    /// Whole original UTF-8 grapheme range in that predecessor's draft, before replacement.
    pub range: CanvasTextRange,
    /// Independent committed text, at most 64 KiB; raw key envelopes carry no committed string.
    /// Explicit native `editing_keys` may produce a separate deletion or line-break commit.
    pub text: String,
    /// Whole-grapheme selection in the resulting draft.
    pub selection: CanvasTextRange,
}
/// Native composition lifecycle is separate from committed text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasCompositionPhase {
    /// Native marked text started.
    Start,
    /// Native marked text changed, without a committed domain edit.
    Update,
    /// Native composition committed; the independent Text event contains the actual replacement.
    End,
    /// Native composition was cancelled without a domain edit.
    Cancel,
}
/// Bounded native preedit observation; it grants no editor document or domain undo ownership.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasComposition {
    /// Native lifecycle transition.
    pub phase: CanvasCompositionPhase,
    /// Native session's committed baseline.
    pub text_revision: u64,
    /// Last unacknowledged native Text sequence, or zero.
    pub base_sequence: u64,
    /// Original range replaced by the complete preedit, before any preedit was inserted.
    pub range: CanvasTextRange,
    /// Complete preedit on Start/Update; empty on End/Cancel.
    pub text: String,
    /// Whole-grapheme range relative to preedit text; empty on End/Cancel.
    pub selection: CanvasTextRange,
}
impl CanvasScene {
    /// Resolve session sources before native allocation; native editor documents use their existing node.
    pub(super) fn validate_text_inputs(&self) -> Result<(), String> {
        let mut count = 0;
        for target in &self.targets {
            if let Some(input) = &target.text_input {
                count += 1;
                let text = self
                    .paragraph_text(&input.paragraph, &input.layout_key)
                    .ok_or("Native text session requires an exact declared paragraph")?;
                if count > 32
                    || !target.focusable
                    || input.text_revision == 0
                    || input.selection.to_utf16(text).is_err()
                {
                    return Err("Invalid native text session source, selection or quota".into());
                }
            }
        }
        Ok(())
    }
}
impl CanvasTextEdit {
    /// Native draft context validates graphemes; this first gate bounds every transported allocation.
    pub(super) fn valid(&self, sequence: u64) -> bool {
        self.text_revision != 0
            && self.base_sequence < sequence
            && self.text.len() <= 65536
            && bounded_range(&self.range)
            && bounded_range(&self.selection)
    }
}
impl CanvasComposition {
    /// Preedit selections refer to their own original string; empty terminal phases cannot commit text.
    pub(super) fn valid(&self, sequence: u64) -> bool {
        self.text_revision != 0
            && self.base_sequence < sequence
            && self.text.len() <= 65536
            && bounded_range(&self.range)
            && self.selection.to_utf16(&self.text).is_ok()
            && (!matches!(
                self.phase,
                CanvasCompositionPhase::End | CanvasCompositionPhase::Cancel
            ) || (self.text.is_empty() && self.selection.start == 0 && self.selection.end == 0))
    }
}
/// Session drafts use the same 64 KiB original-byte limit as paragraph text.
fn bounded_range(range: &CanvasTextRange) -> bool {
    range.start <= range.end && range.end <= 65536
}
