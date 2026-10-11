//! Experimental `language.direct 0.1`: readonly references, code actions and signature help.
//!
//! This experimental minor has no stable compatibility promise. Declare `>=0.1,<0.2` in
//! required or optional capabilities and contribute [`Provider`] values through the manifest.
//! Providers require `editor.read`, a trusted workspace and a WASM component. An unavailable optional
//! capability omits this role while leaving other contributions available; denied permissions make
//! the provider unavailable and never grant fallback authority. Providers are selected per
//! language/feature independently of the single native LSP choice. Installing a later candidate
//! never silently replaces an existing effective selection.
//!
//! [`Request`] runs in an isolated pure worker with no host operations, files, processes, private
//! state or mutable text. Return `Output.language_direct` only from `LanguageDirect`; all replies
//! must match request, identity, revision and feature. A source is at most 1 MiB and a proposal at
//! most 512 KiB, with 256 references, 32 actions, 16 signatures and 32 parameters per signature.
//! Dropping/cancelling native work prevents delivery; bounded guest work may finish. Retirement,
//! target changes, close/reopen and provider changes revoke old delivery authority.
//!
//! References use the same native navigation as LSP. Actions use [`crate::workspace::Plan`] and
//! the original provider's ordinary `workspace.files 1.3` authority, user preview and conflict
//! checks; `language.direct` alone grants no writing. Signature labels/documentation are plain
//! text, with explicit active signature and parameter indices, rather than executable markup.
use crate::{
    api::{DocumentVersion, TextPosition},
    language::SourceSnapshot,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Supported readonly language feature; this role never changes the document's selected LSP.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Feature {
    /// Reveal explicit workspace-local targets without modifying source text.
    References,
    /// Propose ordinary previewed workspace plans; write permissions remain separate.
    CodeActions,
    /// Present plain overload and active-parameter data for the current caret.
    SignatureHelp,
}

impl Feature {
    /// Selection uses a deterministic public role key, independent of package or language names.
    pub fn role(self) -> &'static str {
        match self {
            Self::References => "references",
            Self::CodeActions => "code-actions",
            Self::SignatureHelp => "signature-help",
        }
    }
}

/// A package may provide any subset without running an LSP or implementing completion/formatting.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provider {
    /// Package-local opaque identifier, unique among this manifest's direct providers.
    pub id: String,
    /// Declared language identity used by the common provider-selection registry.
    pub language: String,
    /// Nonempty supported subset; missing roles fall back to the effective standard LSP.
    pub features: BTreeSet<Feature>,
}
impl Provider {
    /// Bound opaque identifiers and require at least one implemented feature before admission.
    pub fn valid(&self) -> bool {
        [&self.id, &self.language].into_iter().all(|value| {
            !value.is_empty()
                && value.len() <= 100
                && value.bytes().all(|byte| {
                    byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
                })
        }) && !self.features.is_empty()
    }
}

/// A single readonly caret request uses the exact native document incarnation and UTF-8 source.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    /// Nonzero host correlation value; echo it unchanged in the proposal.
    pub request: u64,
    /// Exact manifest provider identifier selected for this request.
    pub provider: String,
    /// Requested role; a response of another role is invalid even if its data is well formed.
    pub feature: Feature,
    /// Immutable unsaved native text plus its document incarnation and capability revision.
    pub source: SourceSnapshot,
    /// UTF-8 byte offset at a character boundary in `source.text`, including end-of-text.
    pub cursor: usize,
    /// Already resolved settings; configuration does not grant any additional permission.
    pub settings: crate::settings::Effective,
}
impl Request {
    /// Check source/caret authority before any guest instruction runs.
    pub fn valid(&self) -> bool {
        self.request != 0
            && !self.provider.is_empty()
            && self.provider.len() <= 100
            && self.source.valid()
            && self.cursor <= self.source.text.len()
            && self.source.text.is_char_boundary(self.cursor)
    }
}

/// A local reference can be revealed after validating workspace containment and strict UTF-16 positions.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Location {
    /// Workspace-relative Local resource; external or private resources are not accepted.
    pub resource: crate::api::ResourceIdentity,
    /// Strict zero-based UTF-16 line/character; navigation rechecks the live target text.
    pub position: TextPosition,
    /// Plain text label is optional; the native consumer always retains the target path and position.
    #[serde(default)]
    pub label: Option<String>,
}

/// Human-readable parameter names and documentation are language policy, never interpreted by the host.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameter {
    /// Plain display name, limited to 512 UTF-8 bytes.
    pub label: String,
    /// Optional plain documentation, limited to 4 KiB; no markup executes.
    #[serde(default)]
    pub documentation: Option<String>,
}

/// One overload, including optional server/provider-selected active parameter.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Signature {
    /// Complete plain signature label, limited to 4 KiB.
    pub label: String,
    /// Optional plain signature documentation, limited to 8 KiB.
    #[serde(default)]
    pub documentation: Option<String>,
    /// At most 32 parameters, in the same order as the displayed signature.
    #[serde(default)]
    pub parameters: Vec<Parameter>,
    /// Signature-local active index overrides the help-level index for this overload.
    #[serde(default)]
    pub active_parameter: Option<u32>,
}

/// Explicit overload/parameter indices make direct and LSP signatures share one native card.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignatureHelp {
    /// Nonempty bounded overload list; return `help: None` when there is no signature.
    pub signatures: Vec<Signature>,
    /// Selected overload index; absence selects the first overload.
    #[serde(default)]
    pub active_signature: Option<u32>,
    /// Selected parameter of the selected overload, unless that overload provides an override.
    #[serde(default)]
    pub active_parameter: Option<u32>,
}

/// Direct and LSP results converge on these plain bounded native presentation values.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Result {
    References {
        locations: Vec<Location>,
    },
    CodeActions {
        actions: Vec<crate::diagnostics::Fix>,
    },
    SignatureHelp {
        help: Option<SignatureHelp>,
    },
}
impl Result {
    /// Expose the result role so an unrelated reply cannot satisfy a pending native request.
    pub fn feature(&self) -> Feature {
        match self {
            Self::References { .. } => Feature::References,
            Self::CodeActions { .. } => Feature::CodeActions,
            Self::SignatureHelp { .. } => Feature::SignatureHelp,
        }
    }
    /// Reject entire malformed replies, rather than displaying a partially trusted action list.
    pub fn valid(&self) -> bool {
        let bounded = |text: &str, limit| !text.trim().is_empty() && text.len() <= limit;
        let valid = match self {
            Self::References { locations } => {
                locations.len() <= 256
                    && locations.iter().all(|item| {
                        matches!(item.resource, crate::api::ResourceIdentity::Local { .. })
                            && item.resource.validate().is_ok()
                            && item.label.as_ref().is_none_or(|label| bounded(label, 512))
                    })
            }
            Self::CodeActions { actions } => {
                actions.len() <= 32 && actions.iter().all(crate::diagnostics::Fix::valid)
            }
            Self::SignatureHelp { help: None } => true,
            Self::SignatureHelp { help: Some(help) } => {
                !help.signatures.is_empty()
                    && help.signatures.len() <= 16
                    && help
                        .active_signature
                        .is_none_or(|index| (index as usize) < help.signatures.len())
                    && help.signatures.iter().all(|signature| {
                        bounded(&signature.label, 4096)
                            && signature
                                .documentation
                                .as_ref()
                                .is_none_or(|text| text.len() <= 8192)
                            && signature.parameters.len() <= 32
                            && signature.parameters.iter().all(|parameter| {
                                bounded(&parameter.label, 512)
                                    && parameter
                                        .documentation
                                        .as_ref()
                                        .is_none_or(|text| text.len() <= 4096)
                            })
                            && signature
                                .active_parameter
                                .is_none_or(|index| (index as usize) < signature.parameters.len())
                    })
                    && help.active_parameter.is_none_or(|index| {
                        let signature =
                            &help.signatures[help.active_signature.unwrap_or(0) as usize];
                        (index as usize) < signature.parameters.len()
                    })
            }
        };
        valid && serde_json::to_vec(self).is_ok_and(|bytes| bytes.len() <= 512 * 1024)
    }
}

/// Correlation and source version are checked before converting any language result into native UI.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Proposal {
    /// Echo the request's nonzero correlation value exactly.
    pub request: u64,
    /// Echo the full source incarnation/revision; same path alone never authorizes delivery.
    pub document: DocumentVersion,
    /// Bounded result of the requested feature, consumed by the common native presentation.
    pub result: Result,
}
impl Proposal {
    /// Validate the exact source and feature as well as bounded result data.
    pub fn valid_for(&self, request: &Request) -> bool {
        request.valid()
            && self.request == request.request
            && self.document == request.source.document
            && self.result.feature() == request.feature
            && self.result.valid()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        api::{TextEdit, TextRange},
        diagnostics::Fix,
        workspace::{Change, Plan},
    };

    /// The public proposal boundary must reject the whole list before a control-bearing label
    /// reaches a native button, even when an earlier action and all edit data are well formed.
    #[test]
    fn direct_actions_reject_control_labels_without_accepting_a_valid_prefix() {
        let document = DocumentVersion {
            id: "native-document".into(),
            path: "source.expr".into(),
            revision: 7,
        };
        let request = Request {
            request: 9,
            provider: "expressions".into(),
            feature: Feature::CodeActions,
            source: SourceSnapshot {
                document: document.clone(),
                text: "中文🙂".into(),
            },
            cursor: 6,
            settings: Default::default(),
        };
        let fix = Fix {
            title: "修复表达式".into(),
            plan: Plan {
                title: "Replace expression".into(),
                origin: Some(document.clone()),
                changes: vec![Change::EditDocument {
                    document: document.clone(),
                    edits: vec![TextEdit {
                        range: TextRange { start: 0, end: 6 },
                        text: "新".into(),
                    }],
                    selection: TextRange { start: 3, end: 3 },
                }],
            },
        };
        let mut proposal = Proposal {
            request: 9,
            document,
            result: Result::CodeActions {
                actions: vec![fix.clone()],
            },
        };
        assert!(
            proposal.valid_for(&request),
            "Ordinary Unicode labels remain supported"
        );
        for title in ["修复\n下一行", "修复\0隐藏文本"] {
            let mut malformed = fix.clone();
            malformed.title = title.into();
            proposal.result = Result::CodeActions {
                actions: vec![fix.clone(), malformed],
            };
            assert!(
                !proposal.valid_for(&request),
                "A control-bearing action accepted a partial list"
            );
        }
    }
}
