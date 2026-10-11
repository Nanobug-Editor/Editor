//! Owned document markers without editable text or an independent Undo history.
//! Ranges address UTF-8 bytes of the exact admitted revision; points may address byte zero or EOF.
//!
//! Stable core `editor.decorations 1.0`, protocol 7/base 1. The minimum host must advertise this
//! capability; document reads/events additionally negotiate `editor.documents 1.1`. Every call
//! requires `editor.read`; owned SVG icons also require `ui.icons` and `assets.read`.
//! Collections hold at most 1024 markers/512 KiB metadata and 2 MiB total owned artwork.
//! One instance holds at most sixteen collections under the common 128-resource quota.
//! Native documents additionally admit at most 4096 markers across all sources. A collection
//! belongs to one exact open document incarnation; `Replace` may never retarget another document.
//! A point at an insertion edge obeys `Affinity`; a replacement containing the point collapses it
//! to the chosen edge. Range endpoints exclude insertions exactly at either outside edge, expand
//! for interior insertion, and become invalid (`Position.range = None`) when wholly replaced.
//! Ordinary native Undo emits ordinary inverse edits; it does not resurrect deleted ranges.
//! Recompute business ranges from fresh immutable document snapshots after formal document events
//! when restoration is required. The host owns current coordinates only, with no second Undo stack.
//! Requests are accepted asynchronous `EditorTask`s. Cancel/timeout before native publication leaves
//! presentation unchanged; cancellation never undoes an already published collection. Release with
//! `api::guest::close_resource`; document close, owner retirement and trust loss revoke only this
//! collection. Concurrent providers cannot clear each other's ranges, icons or hover descriptions.
//! Wrong/closed owner is `InvalidHandle`, changed document `StaleRevision`, wrong document
//! `Conflict`, split UTF-8/out-of-bounds coordinates `InvalidRequest`, quotas `LimitExceeded`, and
//! authority/negotiation failures `PermissionDenied`/`CapabilityUnavailable`.
use crate::api::{DocumentVersion, ErrorCode, Failure, ResourceHandle, TextRange};
use serde::{Deserialize, Serialize};

/// Insertions at an anchor choose its left or right side. Deletions collapse points to that edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Affinity {
    Left,
    Right,
}
/// Current text coordinates only. Fully replaced/deleted ranges become invalid and never auto-resurrect.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Anchor {
    Point { offset: usize, affinity: Affinity },
    Range { range: TextRange },
}
/// A shared geometry-only package icon and a bounded explanation are optional presentation metadata.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Marker {
    pub id: String,
    pub anchor: Anchor,
    pub label: crate::ui::LocalizedText,
    /// Gutter and tree/tool icons follow one package-owned SVG shape and theme selection contract.
    pub icon: Option<crate::ui::ToolIcon>,
    /// Semantic host colors stay legible in light and dark themes; no fixed background is required.
    pub style: Style,
}
/// Native semantic colors do not permit arbitrary host theme or syntax changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Style {
    Accent,
    Success,
    Warning,
    Error,
}
/// Invalid markers remain bounded metadata so the owner can explicitly replace them after document events.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Position {
    pub id: String,
    pub range: Option<TextRange>,
}
/// Each collection belongs to one exact open document incarnation and one provider instance.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    Create {
        document: DocumentVersion,
        markers: Vec<Marker>,
    },
    /// Replaces only this source's collection; stale versions fail before changing any marker.
    Replace {
        handle: ResourceHandle,
        document: DocumentVersion,
        markers: Vec<Marker>,
    },
    /// Returns current positions and revision. It never follows the active editor or recreates deleted text.
    Read { handle: ResourceHandle },
}
impl Operation {
    /// The runtime checks bounded metadata; only the native text owner can validate UTF-8 boundaries.
    pub fn validate(&self) -> Result<(), Failure> {
        let markers = match self {
            Self::Create { markers, .. } | Self::Replace { markers, .. } => markers,
            Self::Read { .. } => return Ok(()),
        };
        if markers.len() > 1024
            || serde_json::to_vec(markers).map_or(true, |bytes| bytes.len() > 512 * 1024)
        {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Decoration collection exceeds its marker or metadata budget",
            ));
        }
        let mut ids = std::collections::BTreeSet::new();
        for marker in markers {
            if !crate::tree::valid_id(&marker.id)
                || !ids.insert(&marker.id)
                || marker.label.en.is_empty()
                || marker.label.zh_cn.is_empty()
                || marker.label.en.len() > 4096
                || marker.label.zh_cn.len() > 4096
                || matches!(&marker.anchor, Anchor::Range { range } if range.start >= range.end)
            {
                return Err(Failure::new(
                    ErrorCode::InvalidRequest,
                    "Invalid decoration identity, explanation or range",
                ));
            }
            if let Some(icon) = &marker.icon {
                icon.validate()
                    .map_err(|error| Failure::new(ErrorCode::InvalidPath, error))?;
            }
        }
        Ok(())
    }
}
#[cfg(feature = "guest")]
/// Start one owned collection; consume `EditorValue::Decorations` through the ordinary request task.
pub fn create(
    document: DocumentVersion,
    markers: Vec<Marker>,
    timeout_ms: u32,
) -> Result<crate::api::guest::EditorTask, Failure> {
    crate::api::guest::EditorTask::start(
        crate::api::EditorOperation::Decorations {
            operation: Operation::Create { document, markers },
        },
        timeout_ms,
    )
}
#[cfg(feature = "guest")]
/// Recompute business markers from an immutable document snapshot after the formal document event.
/// This is the restoration seam for a deleted range after Undo; the host stores no marker undo stack.
pub fn replace(
    handle: ResourceHandle,
    document: DocumentVersion,
    markers: Vec<Marker>,
    timeout_ms: u32,
) -> Result<crate::api::guest::EditorTask, Failure> {
    crate::api::guest::EditorTask::start(
        crate::api::EditorOperation::Decorations {
            operation: Operation::Replace {
                handle,
                document,
                markers,
            },
        },
        timeout_ms,
    )
}
#[cfg(feature = "guest")]
/// Observe transformed positions, including invalid ranges and collapsed points, at the native current revision.
pub fn read(
    handle: ResourceHandle,
    timeout_ms: u32,
) -> Result<crate::api::guest::EditorTask, Failure> {
    crate::api::guest::EditorTask::start(
        crate::api::EditorOperation::Decorations {
            operation: Operation::Read { handle },
        },
        timeout_ms,
    )
}
