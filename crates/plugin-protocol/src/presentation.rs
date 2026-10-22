//! Workspace-owned filename decorations and title entries. Neither grants file or execution access.
//! `explorer.decorations 1.0` publishes semantic filename colors; `ui.workspace 1.0` publishes
//! native title entries and action menus. Both require ui.panels consent and a workspace instance.
//! A single owned collection participates in the ordinary 128-resource quota. At most 8192 paths,
//! eight entries, 2 MiB of path metadata, 2 MiB of entry metadata and 2 MiB of owned artwork are
//! retained. CloseResource, disable, trust loss and cutover revoke clones.
//! Paths are normalized relative identifiers, never new filesystem authority. Only existing native
//! file rows are decorated; directories and absent files retain the Explorer's ordinary behavior.
use crate::{
    api::{ErrorCode, Failure, ResourceHandle},
    tree, ui,
};
use serde::{Deserialize, Serialize};

/// A semantic color and explanation for one normalized workspace-relative file path.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileDecoration {
    /// Forward-slash relative path; absolute, traversal and device paths are rejected.
    pub path: String,
    /// Host theme and user settings resolve this meaning into an actual filename color.
    pub tone: tree::Tone,
    /// Both languages describe the provider's state without replacing ordinary file metadata.
    pub tooltip: ui::LocalizedText,
}

/// A package-local entry mounted beside the project name by the native workspace shell.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    /// Stable local ID; the host namespaces preferences by plugin and entry IDs.
    pub id: String,
    /// Dynamic text such as a selected environment or the actual checked-out branch.
    pub label: ui::LocalizedText,
    /// Accessible explanation shown on hover.
    pub tooltip: ui::LocalizedText,
    /// Package-owned light/dark SVG artwork.
    pub icon: ui::ToolIcon,
    /// Default ascending order; user ordering takes precedence.
    pub order: i32,
    /// An unavailable entry remains visible but cannot open or invoke actions.
    pub disabled: bool,
    /// Busy state is presentation; actual command admission is still checked independently.
    pub busy: bool,
    /// Searchable native rows; actions preserve their captured typed arguments.
    pub menu: Vec<ui::ActionMenuItem>,
    /// Optional bounded refresh command invoked when a user explicitly opens this entry.
    pub on_open: Option<tree::Command>,
}

/// Each publication atomically replaces only its collection, preserving the other presentation.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    /// Allocate the instance's one collection; at least one presentation capability is required.
    Create,
    /// explorer.decorations 1.0: replace all path decorations, including an empty clear.
    Files {
        resource: ResourceHandle,
        files: Vec<FileDecoration>,
    },
    /// ui.workspace 1.0: replace all top entries, including an empty clear.
    Entries {
        resource: ResourceHandle,
        entries: Vec<Entry>,
    },
}

impl FileDecoration {
    /// Validate descriptive paths without opening files or creating missing Explorer nodes.
    pub fn validate(&self) -> Result<(), Failure> {
        if self.path.is_empty()
            || self.path.len() > 4096
            || self.path.contains(['\\', ':'])
            || self.path.chars().any(char::is_control)
            || self.path.split('/').any(|part| {
                part.is_empty()
                    || part == "."
                    || part == ".."
                    || crate::api::is_windows_device_segment(part)
            })
        {
            return Err(Failure::new(
                ErrorCode::InvalidPath,
                "Decoration requires a normalized workspace-relative path",
            ));
        }
        if !ui::valid_text(&self.tooltip) {
            return Err(Failure::new(
                ErrorCode::InvalidRequest,
                "Invalid decoration explanation",
            ));
        }
        Ok(())
    }
}

impl Entry {
    /// Validate presentation budgets; runtime separately checks command schemas and icon ownership.
    pub fn validate(&self) -> Result<(), Failure> {
        if !tree::valid_id(&self.id)
            || !ui::valid_text(&self.label)
            || !ui::valid_text(&self.tooltip)
        {
            return Err(Failure::new(
                ErrorCode::InvalidRequest,
                "Invalid workspace entry metadata",
            ));
        }
        self.icon
            .validate()
            .map_err(|error| Failure::new(ErrorCode::InvalidPath, error))?;
        ui::validate_action_menu(&self.menu)?;
        if let Some(command) = &self.on_open {
            command.validate()?;
        }
        Ok(())
    }
}

#[cfg(feature = "guest")]
/// Allocate the current workspace instance's presentation collection. Returns capability, consent,
/// scope, ownership or quota failures; release it with the ordinary guest resource lifecycle.
pub fn create() -> Result<ResourceHandle, Failure> {
    match crate::api::guest::request(crate::api::Operation::Presentation {
        operation: Operation::Create,
    })? {
        crate::api::Value::Resource(handle) => Ok(handle),
        _ => Err(Failure::new(
            ErrorCode::InvalidRequest,
            "Unexpected presentation allocation result",
        )),
    }
}

#[cfg(feature = "guest")]
/// Atomically replace `resource`'s `files`; requires explorer.decorations and ui.panels consent.
/// Invalid paths, duplicates or budgets leave the last accepted collection unchanged.
pub fn publish_files(resource: ResourceHandle, files: Vec<FileDecoration>) -> Result<(), Failure> {
    crate::api::guest::request(crate::api::Operation::Presentation {
        operation: Operation::Files { resource, files },
    })
    .map(|_| ())
}

#[cfg(feature = "guest")]
/// Atomically replace `resource`'s `entries`; requires ui.workspace, typed commands and owned icons.
/// Old menu callbacks are invalid after a changed publication, close or owner retirement.
pub fn publish_entries(resource: ResourceHandle, entries: Vec<Entry>) -> Result<(), Failure> {
    crate::api::guest::request(crate::api::Operation::Presentation {
        operation: Operation::Entries { resource, entries },
    })
    .map(|_| ())
}
