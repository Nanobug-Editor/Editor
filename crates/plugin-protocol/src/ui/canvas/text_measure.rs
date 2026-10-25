//! Bounded native sample measurement supports content-sized layouts without font-width guesses.
use super::*;

/// A generic group of single-line strings measured with the same native face as canvas text.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextMeasure {
    /// Nonempty scene-local identity, at most 128 UTF-8 bytes; unique within the canvas.
    pub id: String,
    /// Samples whose maximum unellipsized advance is returned, including an empty string if needed.
    pub texts: Vec<String>,
    /// Explicit logical font size from 1 through 128 pixels, before a visual viewport transform.
    pub size: f32,
    /// Select the bold face before shaping every sample.
    pub bold: bool,
    /// Optional family override; absent inherits the canvas's resolved UI family.
    #[serde(default)]
    pub font: Option<String>,
}

/// The maximum native advance of one requested group, without clipping or guessed character widths.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextWidth {
    /// Echo the exact request identity, retaining the enclosing scene's ordinary authority.
    pub id: String,
    /// Nonnegative finite maximum advance, in untransformed logical pixels, at most one million.
    pub width: f32,
}

impl Canvas {
    /// Bound all groups together: at most 16 groups, 256 samples and 64 KiB of sample text.
    /// Each single-line sample admits at most 4096 bytes; font and ID limits match ordinary canvas text.
    pub(super) fn validate_measurements(&self) -> Result<(), String> {
        let mut ids = std::collections::BTreeSet::new();
        let mut count = 0;
        let mut bytes = 0;
        if self.measure_text.len() > 16 {
            return Err("Canvas text measurement quota exceeded".into());
        }
        for request in &self.measure_text {
            count += request.texts.len();
            let valid = !request.id.is_empty()
                && request.id.len() <= 128
                && ids.insert(&request.id)
                && !request.texts.is_empty()
                && request.size.is_finite()
                && (1. ..=128.).contains(&request.size)
                && request
                    .font
                    .as_ref()
                    .is_none_or(|font| !font.is_empty() && font.len() <= 256)
                && request.texts.iter().all(|text| {
                    bytes += text.len();
                    text.len() <= 4096 && !text.contains(['\r', '\n'])
                });
            if !valid || count > 256 || bytes > 65536 {
                return Err("Invalid canvas text measurement or quota".into());
            }
        }
        Ok(())
    }
}
