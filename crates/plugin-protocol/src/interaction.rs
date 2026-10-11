//! Stable native interactions and opaque, instance-bound selected-file authority.
//!
//! # Declaration and completion
//!
//! Use protocol `7`, base API `^1`, capability `ui.interaction ^1` and permission
//! `ui.interaction` for confirmations, quick picks, inputs, notices and progress. Selection
//! instead requires `files.selection ^1`, the `files.select` installation grant and an active
//! trusted workspace. Native selection is implemented on Windows; unsupported platforms
//! return `UnsupportedOperation`, independently of user cancellation.
//!
//! The `start` guest helper returns an accepted editor request, not a user decision. Consume its
//! [`crate::api::Notification::Request`] updates and expect [`crate::api::EditorValue::Interaction`]
//! only on successful completion: a picked ID, bounded input string, confirmation, dismissed
//! notice, explicitly finished progress, or selected resource list. Password input masks
//! presentation; callers must never log the returned secret. Escape, window closure, request
//! cancellation, timeout or retirement produces a `Cancelled` or `Completed { result: Err(..) }`
//! terminal update rather
//! than an empty string or false success. The first terminal result wins; late picks issue no grants.
//!
//! Modals use one FIFO queue per window and restore focus only while owning it. Attributed
//! notices and progress are nonmodal, preserve editor focus and are not persistent history.
//! The `update` helper coalesces changes into the same progress request; percentages are optional and
//! range from 0 to 100. The `finish` helper completes only an owned pending task. Cancellation prevents
//! subsequent updates or finish from reviving it.
//!
//! Titles are 1..=256 bytes and messages at most 4,096 bytes. Quick picks contain 1..=512
//! unique IDs (1..=128 bytes), labels of 1..=256 bytes and descriptions of at most 1,024 bytes.
//! Input budgets are 1..=65,536 bytes, initial values must fit, and placeholders are at most
//! 256 bytes. Each instance has at most 32 pending interactions, with timeouts of
//! 1..=300,000 milliseconds. The Windows picker uses four bounded workers and buffers at
//! most 64 paths per callback. Public SDK admission accepts at most 32 objects per batch;
//! each retained grant consumes the instance's shared 128-resource handle quota. A batch
//! is fully validated before any grant is issued.
//!
//! # Selected resources
//!
//! [`crate::interaction::SelectionMode::File`] grants only the exact selected file, a directory grant confines
//! subsequent relative paths beneath that selected directory, and Save grants only the
//! selected write intent. Selecting a Save target does not itself write or overwrite it.
//! Save is single-selection; `suggested_name` is a basename of at most 255 bytes, cannot be
//! `.` or `..`, and cannot contain a slash, backslash, colon or NUL. A display name or path
//! never substitutes for the opaque [`crate::interaction::SelectedResource::handle`].
//!
//! File reads use the file handle with an empty relative path; directory reads use bounded
//! relative paths. All access rechecks canonical containment, symlinks/junctions, device and
//! alternate-stream rejection, and the live owner. Ordinary replacement invalidates old
//! object handles. Do not persist or transfer them: explicit close, disable, trust loss and
//! instance replacement revoke them. This capability grants neither arbitrary filesystem
//! access nor workspace mutation. A service or cross-plugin command cannot open a picker or
//! inherit a selected object. The first directly invoked host command may request selection
//! under its own approved permissions; a guest-supplied caller identity cannot forge that origin.
use crate::api::{ErrorCode, Failure};
use serde::{Deserialize, Serialize};

/// A native picker grants only the selected object and its declared access intent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionMode {
    File,
    Directory,
    Save,
}

/// files.selection 1.1 adds explicit native write consent; existing File/Directory picks stay readonly.
/// Save always remains an exact write intent and never gains guest read access.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionAccess {
    ReadOnly,
    ReadWrite,
}

/// Borrowed picker metadata lets every native route preserve the same requested access intent.
pub struct SelectionOptions<'a> {
    pub title: &'a str,
    pub mode: SelectionMode,
    pub multiple: bool,
    pub suggested_name: Option<&'a str>,
    pub access: SelectionAccess,
}

/// The display name is informational; authority resides in the selecting instance's opaque slot.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectedResource {
    pub handle: crate::api::ResourceHandle,
    pub name: String,
    pub kind: SelectionMode,
}

/// Severity affects native appearance, never the plugin's installation permissions.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Information,
    Warning,
    Error,
}

/// IDs are the returned values; presentation labels may be translated independently.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PickItem {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// A host interaction always runs under its owning plugin's original request lifetime.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    /// Save selection obtains an exact intent; it does not write or overwrite the chosen file.
    Select {
        title: String,
        mode: SelectionMode,
        multiple: bool,
        suggested_name: Option<String>,
    },
    /// Requires files.selection 1.1. The native picker must display explicit write intent.
    SelectWithAccess {
        title: String,
        mode: SelectionMode,
        multiple: bool,
        suggested_name: Option<String>,
        access: SelectionAccess,
    },
    Confirm {
        title: String,
        message: String,
    },
    /// Nonmodal attributed notice. Its result is delivered when the user dismisses it.
    Notify {
        title: String,
        message: String,
        severity: Severity,
    },
    /// A nonmodal live task retains its pending request until finish, cancel or expiry.
    Progress {
        title: String,
        message: String,
        cancellable: bool,
    },
    /// Updates coalesce on the existing owned task; no second UI request is allocated.
    UpdateProgress {
        request: crate::api::ResourceHandle,
        message: String,
        percent: Option<u8>,
    },
    FinishProgress {
        request: crate::api::ResourceHandle,
    },
    QuickPick {
        title: String,
        items: Vec<PickItem>,
    },
    /// Password presentation never changes the bounded string result; callers must not log it.
    Input {
        title: String,
        value: String,
        placeholder: Option<String>,
        password: bool,
        max_bytes: usize,
    },
}

/// Successful native results never encode cancellation as an empty string or false selection.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Value {
    Selected(Vec<SelectedResource>),
    Confirmed,
    Dismissed,
    Finished,
    Picked(String),
    Input(String),
}

impl Operation {
    /// Old directory/file selection never inherits a newer capability's write authority.
    pub fn selection(&self) -> Option<SelectionOptions<'_>> {
        let (title, mode, multiple, suggested_name, access) = match self {
            Self::Select {
                title,
                mode,
                multiple,
                suggested_name,
            } => (
                title,
                mode,
                multiple,
                suggested_name,
                if *mode == SelectionMode::Save {
                    SelectionAccess::ReadWrite
                } else {
                    SelectionAccess::ReadOnly
                },
            ),
            Self::SelectWithAccess {
                title,
                mode,
                multiple,
                suggested_name,
                access,
            } => (title, mode, multiple, suggested_name, *access),
            _ => return None,
        };
        Some(SelectionOptions {
            title,
            mode: *mode,
            multiple: *multiple,
            suggested_name: suggested_name.as_deref(),
            access,
        })
    }
    /// Validate transport bounds and unique identities before allocating a native request.
    pub fn validate(&self) -> Result<(), Failure> {
        match self {
            Self::Select {
                title,
                mode,
                multiple,
                suggested_name,
            }
            | Self::SelectWithAccess {
                title,
                mode,
                multiple,
                suggested_name,
                ..
            } => {
                if title.is_empty()
                    || title.len() > 256
                    || (*mode == SelectionMode::Save && *multiple)
                    || suggested_name.as_ref().is_some_and(|name| {
                        name.is_empty()
                            || name.len() > 255
                            || name.contains(['/', '\\', ':', '\0'])
                            || name == "."
                            || name == ".."
                    })
                {
                    return Err(Failure::new(
                        ErrorCode::InvalidRequest,
                        "Invalid native selection title, mode or basename",
                    ));
                }
                return Ok(());
            }
            Self::Confirm { title, message }
            | Self::Notify { title, message, .. }
            | Self::Progress { title, message, .. } => {
                if title.is_empty() || title.len() > 256 || message.len() > 4096 {
                    return Err(Failure::new(
                        ErrorCode::InvalidRequest,
                        "Invalid interaction title or message",
                    ));
                }
                return Ok(());
            }
            Self::UpdateProgress {
                message, percent, ..
            } => {
                if message.len() > 4096 || percent.is_some_and(|value| value > 100) {
                    return Err(Failure::new(
                        ErrorCode::InvalidRequest,
                        "Invalid progress message or percentage",
                    ));
                }
                return Ok(());
            }
            Self::FinishProgress { .. } => return Ok(()),
            _ => {}
        }
        if let Self::Input {
            title,
            value,
            placeholder,
            max_bytes,
            ..
        } = self
        {
            if title.is_empty()
                || title.len() > 256
                || *max_bytes == 0
                || *max_bytes > 65536
                || value.len() > *max_bytes
                || placeholder.as_ref().is_some_and(|text| text.len() > 256)
            {
                return Err(Failure::new(
                    ErrorCode::InvalidRequest,
                    "Invalid input title or byte budget",
                ));
            }
            return Ok(());
        }
        let Self::QuickPick { title, items } = self else {
            unreachable!()
        };
        let mut ids = std::collections::BTreeSet::new();
        if title.is_empty()
            || title.len() > 256
            || items.is_empty()
            || items.len() > 512
            || items.iter().any(|item| {
                item.id.is_empty()
                    || item.id.len() > 128
                    || !ids.insert(&item.id)
                    || item.label.is_empty()
                    || item.label.len() > 256
                    || item
                        .description
                        .as_ref()
                        .is_some_and(|text| text.len() > 1024)
            })
        {
            return Err(Failure::new(
                ErrorCode::InvalidRequest,
                "Invalid quick-pick title, items or duplicate identity",
            ));
        }
        Ok(())
    }
}

#[cfg(feature = "guest")]
/// Start a native interaction; confirmation and cancellation arrive through Notification::Request.
pub fn start(operation: Operation, timeout_ms: u32) -> Result<crate::api::ResourceHandle, Failure> {
    crate::api::guest::editor(
        crate::api::EditorOperation::Interaction { operation },
        timeout_ms,
    )
}

#[cfg(feature = "guest")]
/// Replace the visible progress under an owned pending task; updates coalesce without new UI slots.
pub fn update(
    request: crate::api::ResourceHandle,
    message: impl Into<String>,
    percent: Option<u8>,
) -> Result<(), Failure> {
    crate::api::guest::request(crate::api::Operation::Editor {
        operation: crate::api::EditorOperation::Interaction {
            operation: Operation::UpdateProgress {
                request,
                message: message.into(),
                percent,
            },
        },
        timeout_ms: 30000,
    })
    .map(|_| ())
}

#[cfg(feature = "guest")]
/// Complete a live progress task. A user cancellation wins over this late finish.
pub fn finish(request: crate::api::ResourceHandle) -> Result<(), Failure> {
    crate::api::guest::request(crate::api::Operation::Editor {
        operation: crate::api::EditorOperation::Interaction {
            operation: Operation::FinishProgress { request },
        },
        timeout_ms: 30000,
    })
    .map(|_| ())
}
