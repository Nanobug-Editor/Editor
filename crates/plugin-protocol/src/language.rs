//! Language-service declarations and bounded optional hook data, independent of concrete language names.
//!
//! `language.lsp 1.4` adds standard `textDocument/references`, `textDocument/codeAction` and
//! `textDocument/signatureHelp` to the existing completion, hover, definition and diagnostic transport.
//! Declare this minimum when depending on these native consumers; optional negotiation may omit
//! their dependent behavior. Server initialization must also advertise the corresponding standard
//! capability. Editing and formatting continue to require their independent capabilities.
//! Provider metadata remains available while cold. Only a selected document's role/feature
//! demand starts its controlled service and any necessary guest; enumerating providers never
//! starts every installed language. Preserve the original unsaved snapshot and target while
//! activation waits; delivery still checks document/revision and selected-provider generation.
//!
//! The host synchronizes the unsaved native source and UTF-16 caret, retains its document lease,
//! revision and selected-provider generation, and checks them again on delivery and selection.
//! References are workspace-local; reading another document still requires `workspace.read`.
//! Literal code-action text edits currently target the current source only. Commands, resource
//! operations, other-document edits, annotations and lazy resolution return an explicit unsupported
//! result rather than executing a valid prefix. Accepted actions use the shared native preview,
//! permission, conflict and group-Undo workflow, with readonly `workspace.files 1.3` plan origins.
//! Auxiliary readonly native input conditions require `workspace.files 1.4`; changed inputs reject
//! the complete proposed plan before preview or effects.
//! A declarative language package needs no empty WASM component for this workflow: its controlled
//! service owns an incarnation seal and bounded request/recovery resources under the original
//! `editor.read`, `editor.write`, `editor.edit 1.1` and `workspace.files 1.3` consent/contracts.
//! Its native issuer retains at most 8 batch receipts and 32 pending requests, delivered once
//! through the common manager dispatcher. The service and any optional guest instance must both
//! remain live; retire, scope/trust change, replacement and manager shutdown revoke old proposals
//! and recovery immediately. Native receipts retain the existing group
//! Undo; dropping their last native holder releases the service-owned batch. No file-root authority
//! is implied by a language result or readonly condition. Editing another native workspace
//! document requires its exact version, `workspace.read` and the normal editor write authority.
//!
//! Signature labels and documentation are inert plain text. Closing presentation stops that request's
//! wait, sends `$/cancelRequest` for an outstanding standard request and discards later delivery;
//! it does not retire the shared server or spend its finite transport-retry budget. Request timeout,
//! process supervision and bounded transport remain the existing controlled stdio service behavior.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// language.lsp 1.3 inserts this fixed number of dot directories at the native volume root.
/// Their percent-encoding case carries an immutable wire identity without creating a file.
/// Native plugins with absolute path globs may add a second pattern containing these dot directories.
pub const SNAPSHOT_URI_SEGMENTS: usize = 64;

/// language.editing 1.1 reserves this client/server experimental capability key for semantic pairing.
/// Only the host's negotiated grant can advertise it; provider client_experimental cannot override it.
pub const SEMANTIC_LINKED_EDITING_CAPABILITY: &str = "meEditorSemanticLinkedEditing";

/// This fixed request uses LinkedEditingRangeParams and LinkedEditingRanges over the owned LSP transport.
/// Unlike the standard method, its provider guarantees semantic pairing of initially different names.
pub const SEMANTIC_LINKED_EDITING_METHOD: &str = "meEditor/semanticLinkedEditingRange";

/// Version one permits different initial text/lengths while retaining UTF-16 boundaries and name patterns.
pub const SEMANTIC_LINKED_EDITING_VERSION: u32 = 1;

/// Negotiated language.control support on the shared native transport, injected only by the host.
pub const CONTROL_CAPABILITY: &str = "nanobugControl";
/// Current bounded custom-control wrapper and managed module descriptor protocol.
pub const CONTROL_VERSION: u32 = 1;

/// Server acknowledgement for the host-owned native control extension during LSP initialize.
/// The host checks the exact version and its provider's declared request range before dispatch.
/// This marker carries no permission; undeclared methods and lifecycle notifications stay forbidden.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ControlCapabilities {
    /// Only CONTROL_VERSION is understood by this SDK; future versions must negotiate explicitly.
    pub version: u32,
    /// Unique supported custom requests, bounded by the same 32-method declaration ceiling.
    pub methods: Vec<String>,
}

impl ControlCapabilities {
    /// Reject unknown versions, duplicate or reserved names, and excessive method ranges.
    /// An otherwise valid marker still requires the host to check every declared provider method.
    pub fn valid(&self) -> bool {
        self.version == CONTROL_VERSION
            && self.methods.len() <= 32
            && self
                .methods
                .iter()
                .all(|method| valid_control_method(method))
            && self
                .methods
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                == self.methods.len()
    }
}

/// Strict initialization marker for the generic semantic linked-editing extension.
/// Both sides must advertise the supported version before the host selects the extension method.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SemanticLinkedEditingCapabilities {
    /// Semantic request version; currently only SEMANTIC_LINKED_EDITING_VERSION is supported.
    pub version: u32,
}

/// Recognition, highlighting, and this LSP provider may be contributed by different packages.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Provider {
    pub id: String,
    pub language: String,
    /// language.content 1.0: retrieve readonly definitions for explicitly declared URI schemes.
    /// The native host owns presentation, source revision checks and revocable virtual documents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<crate::language_content::Provider>,
    /// language.lsp 1.5: additional recognized source languages owned by the same context transport.
    /// Each synchronized document retains its own language ID; the primary language is not repeated.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub languages: Vec<String>,
    pub service: String,
    /// language.lsp 1.6: callable by its owner/delegates, omitted only from user provider choices.
    /// Capabilities, permission consent and native resource accounting are unchanged.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub internal: bool,
    /// language.lsp 1.5: prepare a borrowed transport through this package's declared async service.
    /// A delegated provider has no executable service; only a host-issued one-use binding is accepted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delegate: Option<Delegate>,
    /// language.control 1.0: explicitly declared bounded custom requests over the owned connection.
    /// Standard lifecycle/synchronization methods and write/process permissions remain host-owned.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub control_methods: BTreeMap<String, crate::service::Method>,
    /// language.lsp 1.5: explicitly query related open sources before native analysis. The named
    /// control method must use language_context::related_documents_method's fixed readonly schema.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub related_documents: Option<String>,
    /// Main analysis participates in lsp selection; a formatting-only service sets this to false.
    #[serde(default = "default_primary")]
    pub primary: bool,
    /// language.formatting 1.0: contribute to the independently selected formatter candidates.
    #[serde(default)]
    pub formatting: bool,
    /// language.editing: expose standard prepareRename/rename/linkedEditingRange to native input.
    /// Version 1.1 also allows the separately negotiated generic semantic linked-editing method.
    /// Pairing remains provider policy; this declaration grants neither file IO nor additional processes.
    #[serde(default)]
    pub editing: bool,
    /// language.editing 1.2: native manual refactors may propose edits to related workspace sources.
    /// Every target needs an exact native revision; multiple targets enter normal preview and group Undo.
    /// This declaration grants no filesystem access or permission to bypass ordinary plan admission.
    /// Version 1.3 also admits standard RenameFile proposals when workspace.read/write,
    /// editor.documents 1.1 and the ordinary document/file-plan contracts were granted. File names,
    /// native revisions and disk stamps enter one preview and group recovery; annotations and
    /// unsupported resource operations still fail as a whole. Older negotiated minors stay text-only.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub workspace_edits: bool,
    /// A hook may select another approved startup plan, not invent an executable or argument vector.
    #[serde(default)]
    pub alternatives: Vec<String>,
    /// Only an explicitly saved user/project value overrides native executable resolution.
    #[serde(default)]
    pub executable_setting: Option<String>,
    #[serde(default)]
    pub hook: bool,
    /// dependencies 1.1: first installation may activate resource features after a failed service download.
    /// Updates still require successful preparation; the service remains unavailable until its cache is ready.
    #[serde(default)]
    pub optional_installation: bool,
    /// language.completion: a pure snapshot callback supplements native suggestions without owning text.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub completion_hook: bool,
    /// language.lsp 1.3: use immutable file URI aliases for services publishing unversioned diagnostics.
    /// Each text change closes the prior wire document; logical document identity and disk stay unchanged.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub diagnostic_snapshots: bool,
    #[serde(default)]
    pub initialization_options: Value,
    /// Section names are opaque protocol data; the host never prefixes a concrete server name.
    #[serde(default)]
    pub configuration: BTreeMap<String, Value>,
    /// Experimental protocol flags are plugin data; standard and reserved transport capabilities stay host-owned.
    #[serde(default)]
    pub client_experimental: BTreeMap<String, Value>,
    #[serde(default)]
    pub readiness: Option<Readiness>,
    #[serde(default)]
    pub completion_triggers: Vec<String>,
    #[serde(default)]
    pub completion_after_whitespace: Vec<String>,
}

/// Public service entry that prepares a document's language transport without a second process.
/// The host calls this exact package incarnation, never another provider selected by installation order.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Delegate {
    /// Package-local provided collaboration contract containing the prepare method.
    pub contract: String,
    /// Declared method accepting the fixed document intent and returning a sealed binding token.
    pub method: String,
    /// language.lsp 1.6: direct host preparation may use native choice and own preference CAS.
    /// Nested or cross-plugin calls cannot inherit the private preparation origin.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub interactive: bool,
}
impl Delegate {
    /// Check bounded service identifiers; this declaration grants no service or process authority.
    pub fn valid(&self) -> bool {
        [&self.contract, &self.method].into_iter().all(|name| {
            !name.is_empty()
                && name.len() <= 100
                && name.bytes().all(|byte| {
                    byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
                })
        })
    }

    /// Require the complete closed prepare contract, including its read/module permission ceiling.
    /// Package inspection rejects broader shapes before activation or native dependency preparation.
    pub fn accepts(&self, method: &crate::service::Method) -> bool {
        method == &crate::language_control::binding_prepare_method_for(self.interactive)
    }
}

/// Existing current-protocol declarations retain their ordinary analysis role.
fn default_primary() -> bool {
    true
}

/// Without this optional condition, a successful initialize response plus initialized means ready.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Readiness {
    pub notification: String,
    /// RFC 6901 pointer into notification params.
    pub pointer: String,
    pub expected: Value,
    pub timeout_ms: u32,
}

/// Hooks see resolved settings and fixed candidate declarations within one trusted workspace.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Context {
    pub provider: String,
    pub workspace: String,
    /// language.lsp 1.5: original workspace-relative document and host configuration generation.
    /// Absence retains the existing workspace-wide discovery/install behavior. The path grants no IO.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document: Option<DocumentContext>,
    /// language.lsp 1.6: descriptor resolved by the host from the caller's live registry reference.
    /// This remains a readonly hook; the descriptor grants no new filesystem or process authority.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_context: Option<crate::language_context::Descriptor>,
    /// dependencies 1.2: installation requests dependency plans without starting language analysis.
    /// False denotes ordinary language preparation; discovery without a download plan may be skipped.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub preparing_dependencies: bool,
    /// language.lsp 1.2: canonical assets owned by this package version, for native service configuration.
    /// This string grants no WASI filesystem access; native processes do not inherit the WASM sandbox.
    #[serde(default)]
    pub package_root: String,
    /// language.lsp 1.2: the current isolated private-data directory (including an unpublished candidate).
    /// The host refreshes it after transactional cutover; plugins must not save this location as identity.
    #[serde(default)]
    pub data_root: String,
    pub settings: crate::settings::Effective,
    pub candidates: BTreeMap<String, crate::process::Service>,
}

/// Omitted fields retain declarations. Project roots remain relative to the owning workspace.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Proposal {
    /// language.lsp 1.7: expand whole `${dependency:id}/relative/path` strings in initialization
    /// options and configuration after preparation. Keys and ordinary strings stay literal;
    /// paths must exist inside this plan's leased artifacts. Requires a prepared installation plan.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub expand_dependencies: bool,
    /// Dynamic download plans require the separate dependencies.prepare installation grant.
    pub installation: Option<crate::dependencies::Plan>,
    /// Dynamic executables/arguments require the separate process.exec grant; fixed plans do not.
    pub program: Option<String>,
    pub args: Option<Vec<String>>,
    pub service: Option<String>,
    pub project_root: Option<String>,
    /// language.lsp 1.5: opaque project identity and plugin configuration generation.
    /// The separately validated project_root still defines filesystem containment, never this key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<ProjectContext>,
    pub initialization_options: Option<Value>,
    pub configuration: Option<BTreeMap<String, Value>>,
}

/// One host-issued discovery target. This identifies configuration intent rather than source text;
/// ordinary feature requests must additionally retain their document incarnation and revision.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct DocumentContext {
    /// Normalized workspace-relative logical path, including unsaved files with an existing parent.
    pub path: String,
    /// Monotonic host configuration intent for this document; old intents cannot replace newer ones.
    pub generation: u64,
}
impl DocumentContext {
    /// Validate shape only; native admission must also resolve links and check physical containment.
    pub fn valid(&self) -> bool {
        crate::api::ResourceIdentity::Local {
            path: self.path.clone(),
        }
        .validate()
        .is_ok()
    }
}

/// Plugin-owned project identity. It is scoped to the selected provider, canonical root and package;
/// matching keys do not authorize cross-plugin resources or substitute for an instance lease.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProjectContext {
    /// Opaque stable identifier for this plugin's project discovery result, at most 256 UTF-8 bytes.
    pub key: String,
    /// Monotonic plugin facts/configuration generation; changed plans retire older generations.
    pub generation: u64,
}
impl ProjectContext {
    /// Reject oversized/control-bearing identities before they enter bounded native caches.
    pub fn valid(&self) -> bool {
        !self.key.is_empty() && self.key.len() <= 256 && !self.key.chars().any(char::is_control)
    }
}

/// A readonly snapshot is tied to the editor incarnation and revision; it is never a document handle.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceSnapshot {
    pub document: crate::api::DocumentVersion,
    /// At most 1 MiB of UTF-8; the native editor remains the sole mutable text owner.
    pub text: String,
}

impl SourceSnapshot {
    /// Bounded metadata and text are checked before guest execution or accepting an asynchronous result.
    pub fn valid(&self) -> bool {
        self.text.len() <= 1024 * 1024
            && crate::api::FileVersion {
                id: self.document.id.clone(),
                path: self.document.path.clone(),
                revision: self.document.revision,
            }
            .validate()
            .is_ok()
    }
}

/// Explicit opt-in supplements run in a separate pure guest, without initialization side effects or IO.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompletionRequest {
    pub request: u64,
    pub provider: String,
    pub source: SourceSnapshot,
    /// UTF-8 byte caret position in exactly `source.text`.
    pub cursor: usize,
    /// The logical document URI, before native transport aliases; grants no file or external URL access.
    pub uri: String,
    /// Standard native diagnostics proven to belong to this exact source; None means not known yet.
    /// Language policy may use the code/message to distinguish unavailable rule sources from content errors.
    pub diagnostics: Option<Vec<CompletionDiagnostic>>,
    pub settings: crate::settings::Effective,
}

/// A bounded standard diagnostic summary, with no executable action or server-specific host interpretation.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompletionDiagnostic {
    /// Standard LSP codes are strings or integers; the guest owns their domain meaning.
    pub code: Option<Value>,
    pub message: String,
}

impl CompletionRequest {
    /// Source authority, cursor boundaries and native summaries are checked before invoking the pure guest.
    pub fn valid(&self) -> bool {
        self.source.valid()
            && self.cursor <= self.source.text.len()
            && self.source.text.is_char_boundary(self.cursor)
            && !self.uri.is_empty()
            && self.uri.len() <= 8192
            && !self.uri.chars().any(char::is_control)
            && self.diagnostics.as_ref().is_none_or(|items| {
                items.len() <= 128
                    && items.iter().all(|item| {
                        item.message.len() <= 1024
                            && item.code.as_ref().is_none_or(|code| match code {
                                Value::String(value) => value.len() <= 128,
                                Value::Number(value) => value.is_i64() || value.is_u64(),
                                _ => false,
                            })
                    })
            })
    }
}

/// Plain text replacement; there are no commands, extra edits or mutable guest-owned selections.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompletionItem {
    pub label: String,
    pub replace: crate::api::TextRange,
    pub new_text: String,
}

/// Native items keep precedence; supplements are bounded and tied to their exact source/request.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompletionProposal {
    pub request: u64,
    pub document: crate::api::DocumentVersion,
    pub items: Vec<CompletionItem>,
}

impl CompletionProposal {
    /// Reject the whole reply on stale metadata, invalid UTF-8 ranges or excessive response work.
    pub fn valid_for(&self, request: &CompletionRequest) -> bool {
        self.request == request.request
            && self.document == request.source.document
            && self.items.len() <= 256
            && self.items.iter().all(|item| {
                !item.label.is_empty()
                    && item.label.len() <= 256
                    && item.new_text.len() <= 4096
                    && item.replace.start <= request.cursor
                    && request.cursor <= item.replace.end
                    && item.replace.end <= request.source.text.len()
                    && request.source.text.is_char_boundary(item.replace.start)
                    && request.source.text.is_char_boundary(item.replace.end)
            })
            && serde_json::to_vec(self).is_ok_and(|bytes| bytes.len() <= 256 * 1024)
    }
}

impl Provider {
    /// Package inspection rejects unbounded protocol data before invoking a guest or starting native code.
    pub fn valid(&self) -> bool {
        let id = |text: &str| {
            !text.is_empty()
                && text.len() <= 100
                && text.bytes().all(|byte| {
                    byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
                })
        };
        id(&self.id)
            && id(&self.language)
            && (!self.workspace_edits || self.editing)
            && self.content.as_ref().is_none_or(|content| content.valid())
            && self.languages.len() <= 32
            && self
                .languages
                .iter()
                .all(|language| id(language) && language != &self.language)
            && self
                .languages
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                == self.languages.len()
            && self.control_methods.len() <= 32
            && self.control_methods.iter().all(|(name, method)| {
                valid_control_method(name)
                    && method.parameters.validate(0).is_ok()
                    && method.result.validate(0).is_ok()
                    && method.permissions.iter().all(|permission| {
                        matches!(
                            permission.as_str(),
                            "editor.read" | "workspace.read" | "native.modules"
                        )
                    })
            })
            && self.related_documents.as_ref().is_none_or(|name| {
                valid_control_method(name)
                    && self.control_methods.get(name)
                        == Some(&crate::language_context::related_documents_method())
            })
            && match &self.delegate {
                Some(delegate) => {
                    delegate.valid()
                        && self.service.is_empty()
                        && self.alternatives.is_empty()
                        && !self.hook
                        && self.executable_setting.is_none()
                        && self.control_methods.is_empty()
                }
                None => id(&self.service),
            }
            && self.alternatives.len() <= 32
            && self.alternatives.iter().all(|value| id(value))
            && self.configuration.len() <= 64
            && self.configuration.keys().all(|key| key.len() <= 256)
            && self.client_experimental.len() <= 64
            && self
                .client_experimental
                .iter()
                .all(|(key, value)| !key.is_empty() && key.len() <= 256 && bounded_json(value, 0))
            && self.readiness.as_ref().is_none_or(|ready| {
                !ready.notification.is_empty()
                    && ready.notification.len() <= 256
                    && (ready.pointer.is_empty() || ready.pointer.starts_with('/'))
                    && ready.pointer.len() <= 256
                    && (1..=300_000).contains(&ready.timeout_ms)
            })
            && self.completion_triggers.len() <= 64
            && self.completion_after_whitespace.len() <= 64
            && self
                .completion_triggers
                .iter()
                .chain(&self.completion_after_whitespace)
                .all(|value| value.len() <= 256)
            && serde_json::to_vec(self).is_ok_and(|bytes| bytes.len() <= 256 * 1024)
    }
}

/// Custom control names remain plugin data, but cannot impersonate host-owned LSP lifecycle,
/// synchronization or native editing channels. This shape check grants no request or process authority.
pub fn valid_control_method(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"/._-".contains(&byte))
        && !matches!(name, "initialize" | "initialized" | "shutdown" | "exit")
        // Only the native owner can retire consumer references. Allowing a guest declaration
        // here would turn a lifecycle notification into an ordinary callable custom request.
        && name != crate::language_control::CONSUMERS_RETIRED_METHOD
        && ![
            "textDocument/",
            "workspace/",
            "$/",
            "notebookDocument/",
            "window/",
            "client/",
        ]
        .iter()
        .any(|prefix| name.starts_with(prefix))
}

/// The overall encoded budget limits breadth; a separate depth budget prevents pathological nested flags.
fn bounded_json(value: &Value, depth: usize) -> bool {
    depth <= 16
        && match value {
            Value::Array(values) => values.iter().all(|value| bounded_json(value, depth + 1)),
            Value::Object(values) => values.values().all(|value| bounded_json(value, depth + 1)),
            _ => true,
        }
}
