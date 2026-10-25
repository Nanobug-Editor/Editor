//! Explicit project membership joins native source snapshots without broadcasting unrelated text.
use crate::service::{Method, Schema};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
mod registry;
pub use registry::{Descriptor, Operation, Reference};
#[cfg(feature = "guest")]
pub use registry::{check, control, describe, invalidate, register, register_borrowed};

/// One query page fits the bounded control channel; hosts also enforce its serialized byte budget.
pub const MAX_CANDIDATES: usize = 128;
/// JavaScript-managed engines can represent each admitted native revision exactly.
pub const MAX_REVISION: u64 = 9_007_199_254_740_991;

/// Readonly metadata for an already open native source, with no text or filesystem authority.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Document {
    /// Host-issued document incarnation; a close and reopen must allocate a different identity.
    pub id: String,
    /// Canonical native file URI, matched by the provider's compiler project graph.
    pub uri: String,
    /// EditorState revision captured before membership and source synchronization.
    pub revision: u64,
}

/// Target plus one page of open metadata; providers return only explicit members of this page.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Query {
    pub target: Document,
    pub candidates: Vec<Document>,
}

/// Stable graph identity and the candidate document IDs belonging to this analysis context.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Members {
    /// Bounded opaque generation, independent of paging and candidate ordering.
    pub generation: String,
    /// Unique IDs from the supplied page; omitted candidates must not receive source text.
    pub documents: Vec<String>,
}
impl Members {
    /// Reject foreign IDs, duplicates and unbounded graph data before reading or synchronizing text.
    pub fn valid_for(&self, query: &Query) -> bool {
        !self.generation.is_empty()
            && self.generation.len() <= 128
            && !self.generation.chars().any(char::is_control)
            && self.documents.len() <= MAX_CANDIDATES
            && self.documents.iter().collect::<BTreeSet<_>>().len() == self.documents.len()
            && self
                .documents
                .iter()
                .all(|id| query.candidates.iter().any(|candidate| &candidate.id == id))
    }
}

/// Fixed readonly shape referenced by Provider.related_documents and its declared control method.
/// Hosts send this under `arguments`, with `_nanobug:{native:true}` on their owned LSP connection.
pub fn related_documents_method() -> Method {
    let record = |fields| Schema::Record {
        fields,
        optional: BTreeSet::new(),
    };
    let document = record(BTreeMap::from([
        ("id".into(), Schema::String { max_bytes: 128 }),
        ("uri".into(), Schema::String { max_bytes: 16_384 }),
        (
            "revision".into(),
            Schema::Integer {
                min: 0,
                max: MAX_REVISION as i64,
            },
        ),
    ]));
    Method {
        parameters: record(BTreeMap::from([
            ("target".into(), document.clone()),
            (
                "candidates".into(),
                Schema::Array {
                    items: Box::new(document),
                    max_items: MAX_CANDIDATES,
                },
            ),
        ])),
        result: record(BTreeMap::from([
            ("generation".into(), Schema::String { max_bytes: 128 }),
            (
                "documents".into(),
                Schema::Array {
                    items: Box::new(Schema::String { max_bytes: 128 }),
                    max_items: MAX_CANDIDATES,
                },
            ),
        ])),
        permissions: BTreeSet::from(["editor.read".into(), "workspace.read".into()]),
    }
}
