//! Domain-neutral action menus shared by workspace entries and native tree context menus.
use super::{LocalizedText, ToolIcon};
use crate::{
    api::{ErrorCode, Failure},
    tree::Command,
};
use serde::{Deserialize, Serialize};

/// One stable row in a searchable, collapsible or cascading native action menu.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionMenuItem {
    /// Unique within the entire menu, including collapsed groups and submenus.
    pub id: String,
    /// Both supported languages are supplied by the publishing package.
    pub label: LocalizedText,
    /// Optional contextual suffix; it grants no file or command authority.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<LocalizedText>,
    /// Owned artwork follows the ordinary package icon contract.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<ToolIcon>,
    /// A check marks provider state, independently of keyboard/hover selection.
    #[serde(default)]
    pub checked: bool,
    /// Disabled ancestors also disable every descendant action.
    #[serde(default)]
    pub disabled: bool,
    /// Separate logical groups without introducing selectable separator rows.
    #[serde(default)]
    pub separator_before: bool,
    /// Actions use declared typed commands; submenu preparation never selects a child action.
    pub kind: ActionMenuKind,
}

/// Inline directory groups and action submenus have different click behavior.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ActionMenuKind {
    /// Execute this exact package-local typed command after native admission.
    Action { command: Command },
    /// Toggle an inline directory; the initial expansion yields to user interaction.
    Group {
        expanded: bool,
        items: Vec<ActionMenuItem>,
    },
    /// Open a side submenu; clicking the parent never executes its first action.
    Submenu {
        /// Initial or last accepted child rows; an empty list may be filled after explicit opening.
        items: Vec<ActionMenuItem>,
        /// Optional typed preparation command, invoked only when the user opens this submenu.
        /// Providers can publish the selected branch's actions without duplicating them for every row.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        on_open: Option<Command>,
    },
}

impl ActionMenuItem {
    /// Create an enabled action row from an already described typed `command`.
    pub fn action(id: impl Into<String>, command: Command) -> Self {
        Self {
            id: id.into(),
            label: command.label.clone(),
            detail: None,
            icon: None,
            checked: false,
            disabled: command.disabled,
            separator_before: command.separator_before,
            kind: ActionMenuKind::Action { command },
        }
    }

    /// Return direct children; actions have no descendants.
    pub fn children(&self) -> &[Self] {
        match &self.kind {
            ActionMenuKind::Action { .. } => &[],
            ActionMenuKind::Group { items, .. } | ActionMenuKind::Submenu { items, .. } => items,
        }
    }

    /// Visit every row, including hidden groups; used for admission before native rendering.
    pub fn visit<'a>(&'a self, visitor: &mut impl FnMut(&'a Self)) {
        visitor(self);
        for child in self.children() {
            child.visit(visitor);
        }
    }
}

/// Bound an entire menu to 4096 unique rows, eight levels and 1 MiB before publication.
/// Invalid identities/text, duplicate rows and excessive nesting return ordinary typed failures.
pub fn validate_action_menu(items: &[ActionMenuItem]) -> Result<(), Failure> {
    fn walk(
        items: &[ActionMenuItem],
        depth: usize,
        ids: &mut std::collections::BTreeSet<String>,
    ) -> Result<(), Failure> {
        if depth > 8 && !items.is_empty() {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Action menu nesting exceeds eight levels",
            ));
        }
        for item in items {
            if !crate::tree::valid_id(&item.id) || !ids.insert(item.id.clone()) {
                return Err(Failure::new(
                    ErrorCode::Conflict,
                    "Action menu IDs must be unique and valid",
                ));
            }
            if ids.len() > 4096
                || !valid_text(&item.label)
                || item.detail.as_ref().is_some_and(|text| !valid_text(text))
            {
                return Err(Failure::new(
                    ErrorCode::LimitExceeded,
                    "Action menu metadata exceeds its budget",
                ));
            }
            if let Some(icon) = &item.icon {
                icon.validate()
                    .map_err(|error| Failure::new(ErrorCode::InvalidPath, error))?;
            }
            if let ActionMenuKind::Action { command } = &item.kind {
                command.validate()?;
            }
            if let ActionMenuKind::Submenu {
                on_open: Some(command),
                ..
            } = &item.kind
            {
                command.validate()?;
            }
            walk(item.children(), depth + 1, ids)?;
        }
        Ok(())
    }
    if serde_json::to_vec(items).map_or(true, |bytes| bytes.len() > 1024 * 1024) {
        return Err(Failure::new(
            ErrorCode::LimitExceeded,
            "Action menu exceeds 1 MiB",
        ));
    }
    walk(items, 0, &mut Default::default())
}

/// Bilingual labels and hints are nonempty, bounded, and contain no control characters.
pub(crate) fn valid_text(text: &LocalizedText) -> bool {
    [&text.en, &text.zh_cn].into_iter().all(|value| {
        !value.trim().is_empty() && value.len() <= 4096 && !value.chars().any(char::is_control)
    })
}

/// Resolve an enabled action by stable `id`, rejecting disabled ancestors as well as the leaf.
pub fn action_menu_command(items: &[ActionMenuItem], id: &str) -> Option<Command> {
    for item in items.iter().filter(|item| !item.disabled) {
        if item.id == id {
            return match &item.kind {
                ActionMenuKind::Action { command } if !command.disabled => Some(command.clone()),
                ActionMenuKind::Submenu {
                    on_open: Some(command),
                    ..
                } if !command.disabled => Some(command.clone()),
                _ => None,
            };
        }
        if let Some(command) = action_menu_command(item.children(), id) {
            return Some(command);
        }
    }
    None
}
