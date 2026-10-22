//! Provider-owned navigation and unchanged-region controls for persistent comparisons.
//!
//! `OpenDocumentComparison.navigation` supplies optional previous/next actions. The host draws
//! native arrows and invokes only the publishing instance's declared typed commands, after
//! checking its live authority again. Arguments are immutable metadata, not document access.
//! Providers own item order and source identity; absent actions disable the corresponding arrow.
//! Pinning or revisiting a tab retains its actions. Closing its sources or retiring its owner
//! revokes them. Navigation never implicitly changes another tab or supplies selected-file grants.
use super::{ErrorCode, Failure};
use serde::{Deserialize, Serialize};

/// One typed command in the comparison owner's package; no other plugin can be targeted.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComparisonCommand {
    /// Declared command ID, rechecked with `plugin.commands` and its parameter schema.
    pub command: String,
    /// Frozen provider context, at most 64 KiB; it grants no source or filesystem authority.
    #[serde(default)]
    pub arguments: serde_json::Value,
}

impl ComparisonCommand {
    /// Bound untrusted metadata before allocating UI state; runtime checks declaration and types.
    pub fn validate(&self) -> Result<(), Failure> {
        if self.command.is_empty()
            || self.command.len() > 128
            || self.command.chars().any(char::is_control)
            || serde_json::to_vec(&self.arguments).map_or(true, |bytes| bytes.len() > 65536)
        {
            return Err(Failure::new(
                ErrorCode::InvalidRequest,
                "Invalid comparison navigation command or arguments",
            ));
        }
        Ok(())
    }
}

/// Adjacent items in one provider-defined comparison sequence, independent of current UI selection.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComparisonNavigation {
    /// Omit at the start of the sequence; native navigation does not wrap.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous: Option<ComparisonCommand>,
    /// Omit at the end of the sequence; native navigation does not wrap.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<ComparisonCommand>,
}

/// `editor.diff` 1.4: opt in to synchronized unchanged-region folding with package-owned artwork.
/// The host hides presentation rows only; original text, line numbers and Undo remain native.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComparisonFolding {
    /// Native toolbar artwork for collapsing unchanged regions; requires ui.icons/assets.read.
    pub collapse: crate::ui::ToolIcon,
    /// Native toolbar and individual-region artwork for expanding hidden original rows.
    pub expand: crate::ui::ToolIcon,
    /// Unchanged source lines retained at each edge of a common run; permitted range is 1..=20.
    pub context_lines: u8,
}

impl ComparisonFolding {
    /// Reject invalid context limits and non-owned path syntax before runtime SVG admission.
    pub fn validate(&self) -> Result<(), Failure> {
        if !(1..=20).contains(&self.context_lines) {
            return Err(Failure::new(
                ErrorCode::InvalidRequest,
                "Comparison context must be 1..=20 lines",
            ));
        }
        self.collapse
            .validate()
            .and_then(|_| self.expand.validate())
            .map_err(|message| Failure::new(ErrorCode::InvalidPath, message))
    }
}
