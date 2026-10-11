//! Stable `editor.diagnostics 1.0`: owned collections feed the native problems model without LSP.
//!
//! Require this capability plus `editor.read` and `editor.diagnostics` installation permissions.
//! Create a collection with [`Operation::Create`], then publish complete per-document sets through
//! [`crate::api::EditorOperation::Diagnostics`]. Publications carry the exact open identity and
//! revision; stale or closed documents fail rather than following another active editor. Empty
//! publications clear that document only. `CloseResource` releases the collection, and retirement
//! revokes its diagnostics and fixes. Handles cannot be transferred to another instance or scope.
//!
//! At most 16 collections per instance, 256 diagnostics per publication, eight related locations
//! and eight fixes per diagnostic are accepted. One publication is at most 512 KiB, and the native
//! document retains at most 1,024 direct diagnostics across all sources. Ranges use UTF-8 bytes;
//! locations use strict zero-based UTF-16 positions. All positions are checked against live text
//! before presentation or navigation. A diagnostic is data, never permission to read or write.
//!
//! Fixes declare ordinary [`crate::workspace::Plan`] values. The native user action re-enters the
//! original instance's `workspace.files 1.3` authority and preview/confirmation workflow. That
//! instance still needs `editor.write`/`workspace.write` for each requested edit. Cancellation stops
//! delivery or a pending plan; it does not promise rollback. Clear or release stale collections
//! after observing document change/close events. Unsupported required capability rejects admission;
//! optional capability absence should omit the checker UI and calls.

use crate::{
    api::{DocumentVersion, ErrorCode, Failure, ResourceHandle, TextRange},
    workspace::Plan,
};
use serde::{Deserialize, Serialize};

/// Collection creation is synchronous; native publication uses the ordinary editor completion gate.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    /// Create a labeled collection owned by this instance; the label is visible source attribution.
    Create { label: String },
}

/// Publishing replaces one collection's complete set at one exact document version.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Publication {
    /// Replace this collection's set for this exact open document; an empty set clears only it.
    Publish {
        /// Original owned handle returned by `Create`, never a package-name substitute.
        collection: ResourceHandle,
        /// Full native document incarnation/revision captured from the document capability.
        document: DocumentVersion,
        /// At most 256 unique IDs; the publication and every proposed fix are bounded.
        diagnostics: Vec<Diagnostic>,
    },
    /// A versioned clear cannot erase a newer publication or another collection.
    Clear {
        collection: ResourceHandle,
        document: DocumentVersion,
    },
}

impl Publication {
    /// Borrow the owned handle without interpreting its namespace as authority.
    pub fn collection(&self) -> &ResourceHandle {
        match self {
            Self::Publish { collection, .. } | Self::Clear { collection, .. } => collection,
        }
    }
    /// Borrow the explicit target used by admission and the native revision check.
    pub fn document(&self) -> &DocumentVersion {
        match self {
            Self::Publish { document, .. } | Self::Clear { document, .. } => document,
        }
    }
    /// Validate bounded transport data; the native editor additionally checks actual UTF-8 ranges.
    pub fn validate(&self) -> Result<(), Failure> {
        crate::api::ResourceIdentity::Local {
            path: self.document().path.clone(),
        }
        .validate()?;
        if self.document().id.is_empty() || self.document().id.len() > 128 {
            return Err(Failure::new(
                ErrorCode::InvalidRequest,
                "Invalid diagnostic document identity",
            ));
        }
        if let Self::Publish { diagnostics, .. } = self {
            let mut ids = std::collections::BTreeSet::new();
            if diagnostics.len() > 256
                || diagnostics
                    .iter()
                    .any(|item| !item.valid() || !ids.insert(&item.id))
            {
                return Err(Failure::new(
                    ErrorCode::InvalidRequest,
                    "Invalid diagnostic IDs, fields or quotas",
                ));
            }
        }
        if serde_json::to_vec(self).map_or(true, |bytes| bytes.len() > 512 * 1024) {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Diagnostic publication exceeds 512 KiB",
            ));
        }
        Ok(())
    }
}

/// Severity maps to the same native underline, hover and problem row used by standard LSP results.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
    Information,
    Hint,
}

/// A workspace-relative related location never grants access outside the originating instance.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Related {
    /// Plain explanation shown next to the related target, limited to 1 KiB.
    pub message: String,
    /// Local workspace target; following another document additionally requires workspace.read.
    pub resource: crate::api::ResourceIdentity,
    /// Strict zero-based UTF-16 location, checked against actual text before navigation.
    pub position: crate::api::TextPosition,
}

/// An explicit business proposal is applied only through the existing native plan preview.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fix {
    /// Plain native action label, limited to 256 bytes and without control characters.
    pub title: String,
    /// Ordinary bounded plan; owner grants and current source/target revisions are rechecked.
    pub plan: Plan,
}

/// The collection provides owner/source identity; plugins cannot forge another source in this payload.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Diagnostic {
    /// Stable collection-local identity, unique within this publication and at most 128 bytes.
    pub id: String,
    /// Shared native severity for underline, hover and Problems presentation.
    pub severity: Severity,
    /// UTF-8 byte range in the exact published native source, including empty insertion ranges.
    pub range: TextRange,
    /// Nonempty plain description, limited to 4 KiB.
    pub message: String,
    /// Optional short checker-specific code; it never identifies or authorizes another owner.
    #[serde(default)]
    pub code: Option<String>,
    /// At most eight related locations; owner permission still controls navigation.
    #[serde(default)]
    pub related: Vec<Related>,
    /// At most eight optional plans; selecting one starts the ordinary preview workflow.
    #[serde(default)]
    pub fixes: Vec<Fix>,
}

impl Diagnostic {
    /// Bound textual metadata and proposed edits independently of the source-text validation.
    pub fn valid(&self) -> bool {
        !self.id.is_empty()
            && self.id.len() <= 128
            && !self.id.chars().any(char::is_control)
            && self.range.start <= self.range.end
            && !self.message.trim().is_empty()
            && self.message.len() <= 4096
            && self.code.as_ref().is_none_or(|code| code.len() <= 128)
            && self.related.len() <= 8
            && self.fixes.len() <= 8
            && self.related.iter().all(|item| {
                !item.message.trim().is_empty()
                    && item.message.len() <= 1024
                    && matches!(item.resource, crate::api::ResourceIdentity::Local { .. })
                    && item.resource.validate().is_ok()
            })
            && self.fixes.iter().all(|fix| {
                !fix.title.is_empty()
                    && fix.title.len() <= 256
                    && !fix.title.chars().any(char::is_control)
                    && fix.plan.validate().is_ok()
            })
    }
}

#[cfg(feature = "guest")]
/// Create a collection; release its resource with `CloseResource` when the checker is no longer needed.
pub fn create(label: String) -> Result<ResourceHandle, Failure> {
    match crate::api::guest::request(crate::api::Operation::Diagnostics {
        operation: Operation::Create { label },
    })? {
        crate::api::Value::Resource(handle) => Ok(handle),
        _ => Err(Failure::new(
            ErrorCode::InvalidState,
            "Diagnostic collection handle expected",
        )),
    }
}

#[cfg(feature = "guest")]
/// Start revision-bound native publication; consume its EditorRequest notification before assuming success.
pub fn publish(publication: Publication) -> Result<crate::api::guest::EditorTask, Failure> {
    publication.validate()?;
    crate::api::guest::EditorTask::start(
        crate::api::EditorOperation::Diagnostics { publication },
        30_000,
    )
}
