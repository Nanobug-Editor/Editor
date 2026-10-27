//! Visible Canvas semantics are inert data; native actions retain the ordinary target authority.
use super::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Portable native role, independent of AccessKit's enum representation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasSemanticRole {
    /// Named container.
    Group,
    /// Read-only label.
    Label,
    /// Activatable command.
    Button,
    /// Editable plugin-domain text.
    TextInput,
    /// Bounded grid with logical row/column totals.
    Grid,
    /// One logical row.
    Row,
    /// One logical grid cell.
    GridCell,
    /// Named column header.
    ColumnHeader,
    /// Named row header.
    RowHeader,
    /// Non-text artwork.
    Image,
    /// Bounded list.
    List,
    /// One logical list entry.
    ListItem,
    /// Numeric range.
    Slider,
    /// Boolean state.
    CheckBox,
    /// Hierarchical entry.
    TreeItem,
}
/// Declared assistive actions never synthesize editor document access or pointer authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasSemanticAction {
    /// Acquire the existing native target focus.
    Focus,
    /// Invoke the guest's domain command.
    Activate,
    /// Submit an inert bounded value to the guest.
    SetValue,
    /// Ask the guest to reveal logical content.
    ScrollIntoView,
    /// Increase a guest-owned value.
    Increment,
    /// Decrease a guest-owned value.
    Decrement,
}
/// One scene-local accessible object. Names cover both supported UI locales.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasSemanticNode {
    /// Stable identity independent of array position and visible text.
    pub id: String,
    /// Optional parent semantic ID; the validated forest has maximum depth 32.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// Native role.
    pub role: CanvasSemanticRole,
    /// Both Chinese and English accessible names are required.
    pub name: crate::ui::LocalizedText,
    /// Supplementary localized instructions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<crate::ui::LocalizedText>,
    /// Guest-owned inert current value; native preedit is not a domain edit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// One hit target supplies shared geometry/focus/epoch. Native overlays declare their own semantics.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// Noninteractive semantics may supply a content/group-local rectangle instead of a target.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rect: Option<crate::Rect>,
    /// Optional named group space for a noninteractive rectangle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub space: Option<String>,
    /// Disabled objects expose state and reject actions.
    #[serde(default)]
    pub disabled: bool,
    /// A readonly object cannot expose SetValue.
    #[serde(default)]
    pub read_only: bool,
    /// Logical selection state, separate from native keyboard focus.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected: Option<bool>,
    /// Logical tree expansion state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expanded: Option<bool>,
    /// Logical checkbox state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked: Option<bool>,
    /// One-based logical row; totals do not allocate invisible semantic children.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row: Option<u32>,
    /// One-based logical column.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
    /// Logical row total, at most one million.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row_count: Option<u32>,
    /// Logical column total, at most one million.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column_count: Option<u32>,
    /// Finite native actions; at most one declaration of each action.
    #[serde(default)]
    pub actions: Vec<CanvasSemanticAction>,
}
impl CanvasScene {
    /// Disabled semantic containers deny descendant assistive actions, including unvalidated cycles.
    pub(super) fn semantic_enabled(&self, node: &CanvasSemanticNode) -> bool {
        let mut current = Some(node);
        for _ in 0..32 {
            let Some(node) = current else {
                return true;
            };
            if node.disabled {
                return false;
            }
            current = match node.parent.as_deref() {
                None => None,
                Some(id) => match self.semantics.iter().find(|parent| parent.id == id) {
                    Some(parent) => Some(parent),
                    None => return false,
                },
            };
        }
        current.is_none()
    }
    /// Validate the bounded semantic forest and exact target/group references before publication.
    pub(super) fn validate_semantics(&self) -> Result<(), String> {
        if self.semantics.len() > 4096 {
            return Err("Canvas semantic node budget exceeded".into());
        }
        let nodes: BTreeMap<_, _> = self
            .semantics
            .iter()
            .map(|node| (node.id.as_str(), node))
            .collect();
        if nodes.len() != self.semantics.len() {
            return Err("Duplicate Canvas semantic identity".into());
        }
        let mut targets = BTreeSet::new();
        let mut text = 0usize;
        for node in &self.semantics {
            let label = |label: &crate::ui::LocalizedText| {
                !label.en.is_empty()
                    && !label.zh_cn.is_empty()
                    && label.en.len() <= 4096
                    && label.zh_cn.len() <= 4096
            };
            text += node.name.en.len()
                + node.name.zh_cn.len()
                + node.value.as_ref().map_or(0, String::len)
                + node
                    .description
                    .as_ref()
                    .map_or(0, |text| text.en.len() + text.zh_cn.len());
            if node.id.is_empty()
                || node.id.len() > 128
                || node.id.chars().any(char::is_control)
                || !label(&node.name)
                || node.description.as_ref().is_some_and(|text| !label(text))
                || text > 65536
                || node.actions.len() > 6
                || node.actions.iter().collect::<BTreeSet<_>>().len() != node.actions.len()
                || [node.row, node.column, node.row_count, node.column_count]
                    .into_iter()
                    .flatten()
                    .any(|v| v == 0 || v > 1_000_000)
                || node.row.zip(node.row_count).is_some_and(|(i, n)| i > n)
                || node
                    .column
                    .zip(node.column_count)
                    .is_some_and(|(i, n)| i > n)
                || (node.read_only && node.actions.contains(&CanvasSemanticAction::SetValue))
            {
                return Err("Invalid Canvas semantic name, state or budget".into());
            }
            if let Some(target) = &node.target {
                let target = self
                    .targets
                    .iter()
                    .find(|item| item.id == *target)
                    .ok_or("Missing Canvas semantic target")?;
                if !targets.insert(&target.id)
                    || node.rect.is_some()
                    || node.space.is_some()
                    || (node.actions.contains(&CanvasSemanticAction::Focus) && !target.focusable)
                {
                    return Err("Duplicate or contradictory Canvas semantic target".into());
                }
            } else {
                let rect = node
                    .rect
                    .ok_or("Noninteractive Canvas semantics require a rectangle")?;
                if !node.actions.is_empty()
                    || !rect.x.is_finite()
                    || !rect.y.is_finite()
                    || !rect.w.is_finite()
                    || !rect.h.is_finite()
                    || rect.w < 0.
                    || rect.h < 0.
                    || [rect.x, rect.y, rect.w, rect.h]
                        .into_iter()
                        .any(|v| v.abs() > 1_000_000.)
                    || self.group_placement(node.space.as_deref()).is_none()
                {
                    return Err("Invalid Canvas semantic geometry or action owner".into());
                }
            }
            let mut parent = node.parent.as_deref();
            let mut visited = BTreeSet::from([node.id.as_str()]);
            while let Some(id) = parent {
                if !visited.insert(id) || visited.len() > 32 {
                    return Err("Cyclic or too deep Canvas semantics".into());
                }
                parent = nodes
                    .get(id)
                    .ok_or("Missing Canvas semantic parent")?
                    .parent
                    .as_deref();
            }
        }
        Ok(())
    }
}
